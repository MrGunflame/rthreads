//#![no_std]

use fork::spawn;

use libc::STDOUT_FILENO;

fn main() {
    // let buf = b"Hello, World!\n";
    // unsafe { libc::write(STDOUT_FILENO, buf.as_ptr().cast(), buf.len()) };
    println!("Hello World!");

    let handle = spawn(move || {
        // let buf = b"Hello World\n";
        // unsafe { libc::write(STDOUT_FILENO, buf.as_ptr().cast(), buf.len()) };
        println!("Hello World");
    });

    handle.join();

    // let buf = b"Wait!\n";
    // unsafe { libc::write(STDOUT_FILENO, buf.as_ptr().cast(), buf.len()) };
    println!("Wait!");
}
