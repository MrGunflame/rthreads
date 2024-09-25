#![no_std]

extern crate alloc;

mod sys;

use alloc::boxed::Box;
use core::cell::UnsafeCell;
use core::ffi::c_ulonglong;
use core::mem::ManuallyDrop;
use core::mem::MaybeUninit;
use core::sync::atomic::{AtomicU32, Ordering};

use alloc::sync::Arc;

use libc::{c_uint, CLONE_FILES, CLONE_FS, CLONE_SIGHAND, CLONE_SYSVSEM, CLONE_THREAD, CLONE_VM};

use sys::{clone_args, create_stack, exit_thread, fork_thread, wait, wake, Stack};

const FUTEX_INIT: u32 = 0;
const FUTEX_DONE: u32 = 1;

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
        loop {
            if self.inner.futex.load(Ordering::Acquire) == FUTEX_DONE {
                return unsafe { (*self.inner.value.get()).assume_init_read() };
            }

            wait(&self.inner.futex, FUTEX_DONE);
        }
    }
}

struct JoinState<T> {
    value: UnsafeCell<MaybeUninit<T>>,
    futex: AtomicU32,
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
        value: UnsafeCell::new(MaybeUninit::uninit()),
        futex: AtomicU32::new(FUTEX_INIT),
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

        (*state.join_state.value.get()).write(value);
        state.join_state.futex.store(FUTEX_DONE, Ordering::Release);
        wake(&state.join_state.futex);

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
