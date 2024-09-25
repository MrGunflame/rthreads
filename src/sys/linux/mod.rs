mod x86_64;

#[cfg(target_arch = "x86_64")]
pub(crate) use x86_64::{create_stack, exit_thread, fork_thread};
