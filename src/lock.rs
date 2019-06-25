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
}

pub struct Locked {
    // This is stuff is public due to our 'no encapsulation' policy.
    pub obj: UnsafeCell<Box<dyn Obj>>,
    pub state: LockState,
}
impl fmt::Debug for Locked {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{:?} {:?}", self.state, self.obj.get())
    }
}
impl Locked {
    pub fn new(obj: Box<dyn Obj>) -> Box<Self> {
        Box::new(Locked {
            obj: UnsafeCell::new(obj),
            state: LockState::Open,
        })
    }
    // Rust does a fantastic job here.
    pub fn can(&self, access: Access) -> bool {
        match (self.state, access) {
            (LockState::Open, _) => true,
            (LockState::Read(_), Access::Read) => true,
            (LockState::Read(_), Access::Write) => false,
            (LockState::Write(orig), _) if orig == thread_id() => panic!("thread deadlock"),
            (LockState::Write(_), _) => false,
        }
    }
    pub fn acquire(&mut self, access: Access) {
        //println!("acquire {:?} on {:?}", access, self);
        self.state = match (self.state, access) {
            (LockState::Write(_), Access::Read) => panic!("kernel multi-locked object via 'WR'"),
            (LockState::Write(_), Access::Write) => panic!("kernel multi-locked object via 'WW'"),
            (LockState::Read(_), Access::Write) => panic!("kernel multi-locked object via 'RW'"),
            (LockState::Read(n), Access::Read) => LockState::Read(n + 1), // checked_add? nah
            (LockState::Open, Access::Read) => LockState::Read(0),
            (LockState::Open, Access::Write) => LockState::Write(thread_id()),
        }
    }
    pub fn release(&mut self, access: Access) {
        //println!("release {:?} on {:?}", access, self);
        self.state = match (self.state, access) {
            (LockState::Open, access) => {
                panic!("tried to release({:?}) a lock that is already open", access)
            }
            (LockState::Write(_), Access::Write) => LockState::Open,
            (LockState::Read(0), Access::Read) => LockState::Open,
            (LockState::Read(n), Access::Read) => LockState::Read(n - 1),
            (state, access) => panic!("Mismatched release({:?}) to {:?}", access, state),
        }
    }
    #[allow(clippy::borrowed_box)]
    pub unsafe fn contents(&mut self) -> *mut dyn Obj {
        let obj: *mut Box<dyn Obj> = self.obj.get();
        let obj: &mut Box<dyn Obj> = &mut *obj;
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
    pub fn into_inner(mut self) -> Box<dyn Obj> {
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
        if self.state != LockState::Open {
            // FIXME: Would abort be more appropriate? Ugh!
            panic!("Locked object dropped: {:?}", self.state);
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
    type Target = dyn Obj;
    #[allow(clippy::borrowed_box)]
    fn deref(&self) -> &dyn Obj {
        unsafe {
            let lock: &Locked = &*self.lock;
            let obj: *mut Box<dyn Obj> = lock.obj.get();
            let obj: &Box<dyn Obj> = &*obj;
            obj.deref()
        }
    }
}
impl Deref for GuardMut {
    type Target = dyn Obj;
    #[allow(clippy::borrowed_box)]
    fn deref(&self) -> &dyn Obj {
        unsafe {
            let lock: &Locked = &*self.lock;
            let obj: *mut Box<dyn Obj> = lock.obj.get();
            let obj: &Box<dyn Obj> = &*obj;
            obj.deref()
        }
    }
}
impl DerefMut for GuardMut {
    fn deref_mut(&mut self) -> &mut dyn Obj {
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
