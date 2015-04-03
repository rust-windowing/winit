# glutin -  OpenGL, UTilities and INput
[![Gitter](https://badges.gitter.im/Join Chat.svg)](https://gitter.im/tomaka/glutin?utm_source=badge&utm_medium=badge&utm_campaign=pr-badge&utm_content=badge)

Alternative to GLFW in pure Rust.

[![Build Status](https://travis-ci.org/tomaka/glutin.png?branch=master)](https://travis-ci.org/tomaka/glutin)
[![Build status](https://ci.appveyor.com/api/projects/status/cv5xewg3uchb3854/branch/master?svg=true)](https://ci.appveyor.com/project/tomaka/glutin/branch/master)

```toml
[dependencies]
glutin = "*"
```

Note that the crates.io version won't compile on OS/X and Android because the required dependencies haven't been uploaded yet. Instead you can use the git version which works everywhere:

```toml
[dependencies.glutin]
git = "https://github.com/tomaka/glutin"
```

## [Documentation](http://tomaka.github.io/glutin/)

## Try it!

```bash
git clone https://github.com/tomaka/glutin
cd glutin
cargo run --example window
```

## Usage

Glutin is an OpenGL context creation library and doesn't directly provide OpenGL bindings for you.

```toml
[dependencies]
gl = "*"
libc = "*"
```

```rust
extern crate glutin;
extern crate libc;
extern crate gl;

fn main() {
    let window = glutin::Window::new().unwrap();

    unsafe { window.make_current() };

    unsafe {
        gl::load_with(|symbol| window.get_proc_address(symbol));

        gl::ClearColor(0.0, 1.0, 0.0, 1.0);
    }

    while !window.is_closed() {
        window.wait_events();

        unsafe { gl::Clear(gl::COLOR_BUFFER_BIT) };

        window.swap_buffers();
    }
}
```

## Platform-specific notes

### Android

 - To compile the examples for android, initialize the submodules, go to `deps/apk-builder/apk-builder` and run `cargo build`, then go back to `glutin` and call `ANDROID_HOME=/path/to/sdk NDK_HOME=/path/to/ndk NDK_STANDALONE=/path/to/standalone cargo test --no-run --target=arm-linux-androideabi`
 - Events and vsync are not implemented
 - Headless rendering doesn't work

### Emscripten

 - Work will start when Emscripten gets updated to LLVM 3.5 (which should happen soon)

### OS/X

 - Some events are not implemented
 - Implementation is still work-in-progress
 - Vsync not implemented

### Win32

 - You must call `glFlush` before `swap_buffers`, or else on Windows 8 nothing will be visible on the window
 - Changing the cursor (set_cursor) is not implemented

### X11

 - Some input events are not implemented
 - Pixel formats not implemented
 - Vsync not implemented
 - Not all mouse cursors are implemented (ContextMenu, ...)
