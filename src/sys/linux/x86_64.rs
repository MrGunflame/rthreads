use core::arch::asm;
use std::hint::unreachable_unchecked;

use linux_raw_sys::general::{
    __NR_clone3, __NR_exit, __NR_mmap, __NR_munmap, MAP_ANONYMOUS, MAP_PRIVATE, MAP_STACK,
    PROT_READ, PROT_WRITE,
};

use crate::sys::{clone_args, Stack};

pub(crate) unsafe fn fork_thread(
    args: &clone_args,
    fn_ptr: *mut unsafe extern "C" fn(*mut ()) -> !,
    fn_arg: *mut (),
) -> usize {
    let number = __NR_clone3 as usize;
    let args_ptr = args;
    let args_len = size_of::<clone_args>();

    let mut pid: usize;
    asm!(
        "syscall",
        // The process is forked at this point.
        // If rax == 0 we are in the forked process (thread).
        // If rax != 0 we are in the main thread and rax contains
        // the PID of the thread.
        "test rax, rax",
        "jnz 2f",
        // Call `fn_ptr` with `fn_arg` with the SysV ABI.
        // The caller guarantees that the function behind `fn_ptr` never
        // returns, so we don't have to handle that case.
        "mov rdi, r9",
        "call r8",
        "2:",
        inlateout("rax") number => pid,
        in("rdi") args_ptr,
        in("rsi") args_len,
        in("r8") fn_ptr,
        in("r9") fn_arg,
    );

    pid
}

pub(crate) fn create_stack(len: usize) -> *mut u8 {
    let mut ptr: usize;
    unsafe {
        asm!(
            "syscall",
            inlateout("rax") __NR_mmap as usize => ptr,
            // addr
            in("rdi") 0,
            // len
            in("rsi") len,
            // prot
            in("rdx") PROT_READ | PROT_WRITE,
            // flags
            in("r10") MAP_PRIVATE | MAP_ANONYMOUS | MAP_STACK,
            // fd
            in("r8") -1,
            // offset
            in("r9") 0,
        );
    }

    ptr as *mut u8
}

pub(crate) unsafe fn exit_thread(code: i32, stack: Stack) -> ! {
    // We are deallocating the stack frame under our own feet.
    // We must be careful to not use the stack anymore after
    // calling munmap.
    unsafe {
        asm!(
            "syscall",
            "mov rax, r8",
            "mov rdi, r9",
            "syscall",
            in("rax") __NR_munmap,
            in("rdi") stack.ptr,
            in("rsi") stack.len,
            in("r8") __NR_exit,
            in("r9") code,
        );

        unreachable_unchecked()
    }
}
