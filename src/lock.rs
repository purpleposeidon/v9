//! Low-level locking.
use crate::prelude_lib::*;
use std::cell::UnsafeCell;
use std::thread::ThreadId;
fn thread_id() -> ThreadId {
    ::std::thread::current().id()
}

#[derive(Debug, Copy, Clone, PartialEq)]
pub enum LockState {
    Open,
    Write(ThreadId),
    Read(u64),
    Poison,
}

pub struct Locked {
    // This is stuff is public due to our 'no encapsulation' policy.
    pub obj: UnsafeCell<Box<dyn AnyDebug>>,
    pub state: LockState,
    pub name: Name,
}
impl fmt::Debug for Locked {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "Locked({})::{:?}", self.name, self.state)
    }
}
impl Locked {
    pub fn new(obj: Box<dyn AnyDebug>, name: Name) -> Box<Self> {
        Box::new(Locked {
            obj: UnsafeCell::new(obj),
            state: LockState::Open,
            name,
        })
    }
    pub fn is_poisoned(&self) -> bool {
        self.state == LockState::Poison
    }
    // Rust does a fantastic job here.
    pub fn can(&self, access: Access) -> bool {
        match (self.state, access) {
            (LockState::Open, _) => true,
            (LockState::Read(_), Access::Read) => true,
            (LockState::Read(_), Access::Write) => false,
            (LockState::Write(orig), _) if orig == thread_id() => {
                panic!("thread deadlock")
            },
            (LockState::Write(_), _) => false,
            (LockState::Poison, _) => false,
        }
    }
    pub fn acquire(&mut self, access: Access) {
        //println!("acquire {:?} on {:?}", access, self);
        self.state = match (self.state, access) {
            (LockState::Write(_), Access::Read) => {
                panic!("kernel multi-locked object via 'WR'")
            },
            (LockState::Write(_), Access::Write) => {
                panic!("kernel multi-locked object via 'WW'")
            },
            (LockState::Read(_), Access::Write) => {
                panic!("kernel multi-locked object via 'RW'")
            },
            (LockState::Read(n), Access::Read) => LockState::Read(n + 1), // checked_add? nah
            (LockState::Open, Access::Read) => LockState::Read(0),
            (LockState::Open, Access::Write) => LockState::Write(thread_id()),
            (LockState::Poison, _) => {
                panic!("acquired poisoned lock object");
            },
        }
    }
    pub fn release(&mut self, access: Access) {
        //println!("release {:?} on {:?}", access, self);
        self.state = match (self.state, access) {
            (LockState::Poison, _) => self.state,
            (LockState::Open, access) => {
                panic!("tried to release({:?}) a lock that is already open", access)
            }
            (LockState::Write(_), Access::Write) => LockState::Open,
            (LockState::Read(0), Access::Read) => LockState::Open,
            (LockState::Read(n), Access::Read) => LockState::Read(n - 1),
            (state, access) => {
                panic!("Mismatched release({:?}) to {:?}", access, state)
            },
        }
    }
    #[allow(clippy::borrowed_box)]
    pub unsafe fn contents(&mut self) -> *mut dyn AnyDebug {
        let obj: *mut Box<dyn AnyDebug> = self.obj.get();
        let obj: &mut Box<dyn AnyDebug> = &mut *obj;
        obj.deref_mut()
    }
    pub unsafe fn read(&mut self) -> GuardRef {
        self.acquire(Access::Read);
        GuardRef { lock: self }
    }
    pub unsafe fn write(&mut self) -> GuardMut {
        self.acquire(Access::Write);
        GuardMut { lock: self }
    }
    pub fn into_inner(mut self) -> Box<dyn AnyDebug> {
        unsafe {
            self.acquire(Access::Write);
            let stuff = self.contents();
            ::std::mem::forget(self);
            Box::from_raw(stuff)
        }
    }
    // FIXME: How safe & sound are the guards?
}
impl Drop for Locked {
    fn drop(&mut self) {
        if let LockState::Write(_) = self.state {
            if std::thread::panicking() {
                self.state = LockState::Poison;
            } else if let LockState::Poison = self.state {
                // This is fine.
            } else {
                panic!("Locked object dropped without release(): {:?}", self);
            }
        }
    }
}
pub struct GuardRef {
    lock: *const Locked,
}
pub struct GuardMut {
    lock: *mut Locked,
}
impl Deref for GuardRef {
    type Target = dyn AnyDebug;
    #[allow(clippy::borrowed_box)]
    fn deref(&self) -> &dyn AnyDebug {
        unsafe {
            let lock: &Locked = &*self.lock;
            let obj: *mut Box<dyn AnyDebug> = lock.obj.get();
            let obj: &Box<dyn AnyDebug> = &*obj;
            obj.deref()
        }
    }
}
impl Deref for GuardMut {
    type Target = dyn AnyDebug;
    #[allow(clippy::borrowed_box)]
    fn deref(&self) -> &dyn AnyDebug {
        unsafe {
            let lock: &Locked = &*self.lock;
            let obj: *mut Box<dyn AnyDebug> = lock.obj.get();
            let obj: &Box<dyn AnyDebug> = &*obj;
            obj.deref()
        }
    }
}
impl DerefMut for GuardMut {
    fn deref_mut(&mut self) -> &mut dyn AnyDebug {
        unsafe {
            let lock: &mut Locked = &mut *self.lock;
            &mut *lock.contents()
        }
    }
}
impl Drop for GuardRef {
    fn drop(&mut self) {
        unsafe {
            let lock: &mut Locked = &mut *(self.lock as *mut Locked);
            lock.release(Access::Read);
        }
    }
}
impl Drop for GuardMut {
    fn drop(&mut self) {
        unsafe {
            let lock: &mut Locked = &mut *self.lock;
            lock.release(Access::Write);
        }
    }
}
