//#![no_std]

extern crate alloc;

use alloc::boxed::Box;
use core::{
    alloc::Layout,
    sync::atomic::{AtomicUsize, Ordering},
};
use std::{arch::x86_64::_mm_addsub_pd, cell::UnsafeCell, ffi::c_ulonglong, mem::MaybeUninit};

use libc::{
    c_int, c_uint, c_void, clone_args, pid_t, SYS_clone3, CLONE_CHILD_CLEARTID, CLONE_FILES,
    CLONE_FS, CLONE_IO, CLONE_PARENT_SETTID, CLONE_SIGHAND, CLONE_SYSVSEM, CLONE_THREAD, CLONE_VM,
    MAP_ANONYMOUS, MAP_PRIVATE, MAP_STACK, PROT_READ, PROT_WRITE, STDOUT_FILENO,
};

fn main() {
    let buf = b"Hello, World!\n";
    unsafe { libc::write(STDOUT_FILENO, buf.as_ptr().cast(), buf.len()) };

    let pid = spawn_thread3(move || {
        let buf = b"Hello World\n";
        unsafe { libc::write(STDOUT_FILENO, buf.as_ptr().cast(), buf.len()) };
    });

    let buf = b"Wait!\n";
    unsafe { libc::write(STDOUT_FILENO, buf.as_ptr().cast(), buf.len()) };

    while THREADS.load(Ordering::SeqCst) > 0 {}
}

// fn spawn_thread<F>(f: F) -> pid_t
// where
//     F: FnOnce() + Send + 'static,
// {
//     let f = Box::into_raw(Box::new(f)) as *mut c_void;

//     let stack = unsafe { alloc::alloc::alloc(Layout::array::<u8>(4096).unwrap()).add(4096) };

//     THREADS.fetch_add(1, Ordering::Relaxed);
//     let pid = unsafe {
//         libc::clone(
//             __thread_trampoline::<F>,
//             stack as *mut c_void,
//             CLONE_VM
//                 | CLONE_THREAD
//                 | CLONE_SYSVSEM
//                 | CLONE_SIGHAND
//                 | CLONE_IO
//                 | CLONE_FS
//                 | CLONE_FILES,
//             f,
//         )
//     };

//     pid
// }

static THREADS: AtomicUsize = AtomicUsize::new(0);

// extern "C" fn __thread_trampoline<F>(f: *mut c_void) -> c_int
// where
//     F: FnOnce(),
// {
//     let f = unsafe { Box::from_raw(f as *mut F) };
//     f();
//     THREADS.fetch_sub(1, Ordering::SeqCst);
//     0
// }

fn spawn_thread2<F>(f: F) -> pid_t
where
    F: FnOnce() + Send + 'static,
{
    let stack_size = 4096 * 512;

    // let layout = Layout::array::<u8>(stack_size)
    //     .unwrap()
    //     .align_to(16)
    //     .unwrap();
    // dbg!(&layout);
    // let stack = unsafe { alloc::alloc::alloc(layout) };
    // if stack.is_null() {
    //     panic!();
    // }

    let stack = unsafe {
        libc::mmap(
            core::ptr::null_mut(),
            stack_size,
            PROT_READ | PROT_WRITE,
            MAP_PRIVATE | MAP_ANONYMOUS | MAP_STACK,
            -1,
            0,
        )
    };

    unsafe {
        dbg!(libc::getpid());
    }

    let f = Box::new(f);

    let stack_start = unsafe { stack.add(stack_size) };
    // let stack_start = stack;

    //let mut child_tid: UnsafeCell<MaybeUninit<u64>> = UnsafeCell::new(MaybeUninit::uninit());
    //let parent_tid: MaybeUninit<u64> = MaybeUninit::uninit();

    let child_tid: *mut MaybeUninit<u64> = Box::into_raw(Box::new(MaybeUninit::uninit()));
    let parent_tid: *mut MaybeUninit<u64> = Box::into_raw(Box::new(MaybeUninit::uninit()));

    let mut args: libc::clone_args = unsafe { core::mem::zeroed() };

    let flags = CLONE_VM as c_uint as c_ulonglong
        | CLONE_THREAD as c_uint as c_ulonglong
        | CLONE_SYSVSEM as c_uint as c_ulonglong
        | CLONE_SIGHAND as c_uint as c_ulonglong
        // | CLONE_IO as c_uint as c_ulonglong
        | CLONE_FS as c_uint as c_ulonglong
        | CLONE_FILES as c_uint as c_ulonglong
        | CLONE_CHILD_CLEARTID as c_uint as c_ulonglong
        | CLONE_PARENT_SETTID as c_uint as c_ulonglong;

    args.flags = flags;
    // args.stack = stack_start as u64 + 0x1000;
    args.stack = stack_start as u64 - 0x1000;
    args.stack_size = stack_size as u64;
    args.child_tid = unsafe { (&*child_tid).as_ptr() as u64 };
    args.parent_tid = unsafe { (&*parent_tid).as_ptr() as u64 };

    THREADS.fetch_add(1, Ordering::Relaxed);

    let args = Box::leak(Box::new(args));

    let res = clone3(&args);
    // dbg!(&res);

    // if res.0 == 0 {
    //     panic!("");
    // }

    if res.0 == 0 {
        loop {}
        // f();
    }

    // dbg!(&res);

    // dbg!(unsafe { child_tid.get_mut().assume_init_read() });
    // dbg!(unsafe { parent_tid.assume_init_read() });

    // if unsafe { (&*child_tid.get()).assume_init_read() } == 0 {
    //     f();
    // }

    //unsafe { parent_tid.assume_init() }
    res.0
}

fn clone3(args: &clone_args) -> errno::Errno {
    let rax: u64 = 0x1b3;
    let ptr = args as *const libc::clone_args;
    let len = core::mem::size_of::<clone_args>();

    let ret = unsafe { syscall2(SYS_clone3 as usize, ptr as usize, len as usize) };

    // unsafe {
    //     core::arch::asm!(
    //         "mov rax, {number}",
    //         "mov rdi, {ptr}",
    //         "mov rsi, {len}",
    //         "syscall",
    //         number = inout(reg) rax,
    //         ptr = in(reg) ptr,
    //         len = in(reg) len,
    //         options(nostack),
    //     );
    // }

    errno::Errno(ret as i32)
}

unsafe fn syscall2(num: usize, arg0: usize, arg1: usize) -> usize {
    let mut ret: usize;

    unsafe {
        core::arch::asm!(
            "syscall",
            inlateout("rax") num => ret,
            in("rdi") arg0,
            in("rsi") arg1,
            out("rcx") _,
            out("r11") _,
            //options(nostack, preserves_flags),
        );
    }

    ret
}

fn spawn_thread3<F>(f: F) -> pid_t
where
    F: FnOnce() + Send + 'static,
{
    let stack_size = 1024 * 512;
    let stack = unsafe {
        libc::mmap(
            core::ptr::null_mut(),
            stack_size,
            PROT_READ | PROT_WRITE,
            MAP_PRIVATE | MAP_ANONYMOUS | MAP_STACK,
            -1,
            0,
        )
    };

    let f = Box::into_raw(Box::new(f));

    unsafe extern "C" fn run_thread<F>(f: *mut c_void)
    where
        F: FnOnce(),
    {
        let f = unsafe { Box::from_raw(f as *mut F) };
        f();
        THREADS.fetch_sub(1, Ordering::SeqCst);
    }

    let flags = CLONE_VM as c_uint as c_ulonglong
        | CLONE_THREAD as c_uint as c_ulonglong
        | CLONE_SYSVSEM as c_uint as c_ulonglong
        | CLONE_SIGHAND as c_uint as c_ulonglong
        // | CLONE_IO as c_uint as c_ulonglong
        | CLONE_FS as c_uint as c_ulonglong
        | CLONE_FILES as c_uint as c_ulonglong;
    // | CLONE_CHILD_CLEARTID as c_uint as c_ulonglong
    // | CLONE_PARENT_SETTID as c_uint as c_ulonglong;

    let mut args: libc::clone_args = unsafe { core::mem::zeroed() };
    args.flags = flags;
    // args.stack = stack_start as u64 + 0x1000;
    // args.stack = stack_start as u64 - 0x1000;
    args.stack = stack as u64 + 0x1000;
    args.stack_size = stack_size as u64;
    // args.child_tid = unsafe { (&*child_tid).as_ptr() as u64 };
    // args.parent_tid = unsafe { (&*parent_tid).as_ptr() as u64 };

    THREADS.fetch_add(1, Ordering::Relaxed);

    let ptr = (&args as *const libc::clone_args) as u64;
    let len = core::mem::size_of::<libc::clone_args>();

    let fn_ptr = run_thread::<F> as *mut unsafe extern "C" fn(*mut c_void);
    let fn_arg = f as *mut c_void;

    dbg!(
        ptr as *const (),
        len as *const (),
        fn_ptr,
        fn_arg,
        args.stack as *const (),
    );

    let number = SYS_clone3;
    // unsafe {
    //     #![allow(named_asm_labels)]
    //     core::arch::asm!(
    //         "mov rax, {number}",
    //         "mov rdi, {ptr}",
    //         "mov rsi, {len}",
    //         "mov r12, {fn_ptr}",
    //         "mov r13, {fn_arg}",
    //         "syscall",
    //         "test rax, rax",
    //         "jnz end",
    //         "xor ebp, ebp",
    //         "mov rdx, r12",
    //         "mov rdi, r13",
    //         "call rdx",
    //         "l1:",
    //         "jmp l1",
    //         "end:",
    //         number = in(reg) number,
    //         ptr = in(reg) ptr,
    //         len = in(reg) len,
    //         fn_ptr = in(reg) fn_ptr,
    //         fn_arg = in(reg) fn_arg,
    //         //clobber_abi("C"),
    //     );
    // }
    // let res = clone3(&args);
    // if res.0 == 0 {
    //     std::process::exit(1);
    // }

    unsafe {
        clone_thread(&args, fn_ptr as usize, fn_arg as usize);
    }

    loop {}

    0
}

#[inline(never)]
#[allow(named_asm_labels)]
unsafe extern "C" fn clone_thread(
    args: &clone_args,
    //fn_ptr: *mut unsafe extern "C" fn(*mut c_void),
    fn_ptr: usize,
    //fn_arg: *const c_void,
    fn_arg: usize,
) {
    let number = SYS_clone3;
    let args_ptr = args;
    let args_len = size_of::<clone_args>();

    //let fn_ptr = 0x00005c94918dd1a0 as *mut unsafe extern "C" fn(*mut c_void);

    dbg!(fn_ptr);
    //let fn_arg = fn_ptr;

    core::arch::asm!(
        "mov r8, {fn_ptr}",
        "mov r9, {fn_arg}",
        "mov rax, {number}",
        "mov rdi, {args_ptr}",
        "mov rsi, {args_len}",
        //"push r9",
        //"push r8",
        // "mov rdi, r8",
        // "mov rax, 60",
        //"syscall",
        //"push {fn_ptr}",
        //"push {fn_arg}",
        "syscall",
        // "pop rdi",
        // "pop rsi",
        "test rax, rax",
        "jnz main_thread",
        // Call `fn_ptr(fn_arg)`.
        "mov rdi, r9",
        "call r8",
        // "mov rax, 60",
        // "mov rdi, 123",
        // "syscall",
        // "e:",
        // "jmp e",
        "main_thread:",
        number = in(reg) number,
        args_ptr = in(reg) args_ptr,
        args_len = in(reg) args_len,
        fn_ptr = in(reg) fn_ptr,
        fn_arg = in(reg) fn_arg,
    );
}
