extern crate alloc;

mod sys;

use alloc::boxed::Box;
use core::ffi::c_ulonglong;
use core::mem::ManuallyDrop;
use std::sync::{Arc, Condvar, Mutex};

use libc::{c_uint, CLONE_FILES, CLONE_FS, CLONE_SIGHAND, CLONE_SYSVSEM, CLONE_THREAD, CLONE_VM};

use sys::{clone_args, create_stack, exit_thread, fork_thread, Stack};

pub struct Builder {}

impl Builder {
    pub fn new() -> Self {
        Self {}
    }

    pub unsafe fn spawn_unchecked<'a, F, T>(self, f: F) -> JoinHandle<T>
    where
        F: FnOnce() -> T + Send + 'a,
        T: Send + 'a,
    {
        spawn_unchecked(f)
    }

    pub fn spawn<F, T>(self, f: F) -> JoinHandle<T>
    where
        F: FnOnce() -> T + Send + 'static,
        T: Send + 'static,
    {
        unsafe { self.spawn_unchecked(f) }
    }
}

pub struct JoinHandle<T> {
    inner: Arc<JoinState<T>>,
    pid: Pid,
}

impl<T> JoinHandle<T> {
    pub fn join(self) -> T {
        let mut guard = self.inner.mutex.lock().unwrap();
        loop {
            if let Some(value) = guard.take() {
                return value;
            }

            guard = self.inner.cvar.wait(guard).unwrap();
        }
    }
}

struct JoinState<T> {
    mutex: Mutex<Option<T>>,
    cvar: Condvar,
}

struct ThreadState<T, F> {
    join_state: Arc<JoinState<T>>,
    f: ManuallyDrop<F>,
    stack: Stack,
}

pub fn spawn<T, F>(f: F) -> JoinHandle<T>
where
    F: FnOnce() -> T + Send + 'static,
    T: Send + 'static,
{
    Builder::new().spawn(f)
}

unsafe fn spawn_unchecked<'a, T, F>(f: F) -> JoinHandle<T>
where
    F: FnOnce() -> T + Send + 'a,
    T: Send,
{
    let stack_size = 1024 * 1024;
    let stack = create_stack(stack_size);
    if stack.is_null() {
        panic!("failed to create stack for new thread");
    }

    let join_state = Arc::new(JoinState {
        mutex: Mutex::new(None),
        cvar: Condvar::new(),
    });
    let f = Box::into_raw(Box::new(ThreadState {
        join_state: join_state.clone(),
        f: ManuallyDrop::new(f),
        stack: Stack {
            ptr: stack as *mut u8,
            len: stack_size,
        },
    }));

    const CLONE_FLAGS: c_ulonglong = CLONE_VM as c_uint as c_ulonglong
        | CLONE_THREAD as c_uint as c_ulonglong
        | CLONE_SYSVSEM as c_uint as c_ulonglong
        | CLONE_FS as c_uint as c_ulonglong
        | CLONE_FILES as c_uint as c_ulonglong
        //| CLONE_IO as c_uint as c_ulonglong
        | CLONE_SIGHAND as c_uint as c_ulonglong;

    let mut args: clone_args = unsafe { core::mem::zeroed() };
    args.flags = CLONE_FLAGS;
    args.stack = stack as u64 + 0x1000;
    args.stack_size = stack_size as u64;

    unsafe extern "C" fn run_thread<T, F>(f: *mut ()) -> !
    where
        F: FnOnce() -> T + Send,
        T: Send,
    {
        let mut state = unsafe { Box::from_raw(f as *mut ThreadState<T, F>) };
        let f = unsafe { ManuallyDrop::take(&mut state.f) };
        let value = f();

        *state.join_state.mutex.lock().unwrap() = Some(value);
        state.join_state.cvar.notify_one();

        let stack = state.stack;
        drop(state);
        exit_thread(0, stack);
    }

    let fn_ptr = run_thread::<T, F> as *mut unsafe extern "C" fn(*mut ()) -> !;
    let fn_arg = f as *mut ();

    let pid = unsafe { fork_thread(&args, fn_ptr, fn_arg) };

    JoinHandle {
        inner: join_state,
        pid: Pid(pid),
    }
}

#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
struct Pid(usize);
