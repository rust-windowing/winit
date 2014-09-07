# gl-init-rs

Alternative to GLFW in pure Rust.

## Try it!

```bash
git clone https://github.com/tomaka/gl-init-rs
cd gl-init-rs
cargo test
./target/test/window    # or target\test\window.exe
```

## Usage

```rust
extern crate init = "gl-init-rs";
extern crate libc;
extern crate gl;

fn main() {
    let window = init::Window::new().unwrap();

    unsafe { window.make_current() };

    gl::load_with(|symbol| window.get_proc_address(symbol));

    gl::ClearColor(0.0, 1.0, 0.0, 1.0);

    while !window.is_closed() {
        window.wait_events();

        gl::Clear(gl::COLOR_BUFFER_BIT);

        window.swap_buffers();
    }
}
```
