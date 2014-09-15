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

## Platform-specific notes

### Android

 - To compile the examples for android, initialize the submodules, go to `deps/apk-builder/apk-builder` and run `cargo build`, then go back to `gl-init` and call `ANDROID_HOME=/path/to/sdk NDK_HOME=/path/to/ndk NDK_STANDALONE=/path/to/standalone cargo test --no-run --target=arm-linux-androideabi`
 - Events are not implemented

### Emscripten

 - Work will start when Emscripten gets updated to LLVM 3.5 (which should happen soon)

### OS/X

 - This library compiles for OS/X but calling any function will fail
 - Some low-level issues related to Objective C bindings make the implementation difficult to write
 - Looking for contributors

### Win32

 - Pixel formats are not implemented
 - If you don't have MinGW installed, you will need to provide `libgdi32.a` and `libopengl32.a` ; you can put them in `C:\Users\you\.rust`

### X11

 - Some input events are not implemented
 - Pixel formats not implemented
 - The implementation probably needs a cleanup
