# rthreads

Thread spawning on linux with pure Rust. This is implemented using the `clone3` syscall.

```rust
fn main() {
    let handle = rthreads::spawn(move || {
        println!("Hello World!");
    });

    handle.join();
}
```
