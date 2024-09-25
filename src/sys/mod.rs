mod linux;

pub(crate) use linux_raw_sys::general::clone_args;

#[cfg(target_os = "linux")]
pub(crate) use linux::{create_stack, exit_thread, fork_thread, wait, wake};

#[derive(Copy, Clone, Debug)]
pub(crate) struct Stack {
    pub(crate) ptr: *mut u8,
    pub(crate) len: usize,
}
