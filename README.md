# winit - Cross-platform window creation and management in Rust

[![Crates.io](https://img.shields.io/crates/v/winit.svg)](https://crates.io/crates/winit)
[![Docs.rs](https://docs.rs/winit/badge.svg)](https://docs.rs/winit)
[![CI Status](https://github.com/rust-windowing/winit/workflows/CI/badge.svg)](https://github.com/rust-windowing/winit/actions)

```toml
[dependencies]
winit = "0.27.2"
```

## [Documentation](https://docs.rs/winit)

For features _within_ the scope of winit, see [FEATURES.md](FEATURES.md).

For features _outside_ the scope of winit, see [Missing features provided by other crates](https://github.com/rust-windowing/winit/wiki/Missing-features-provided-by-other-crates) in the wiki.

## Contact Us

Join us in any of these:

[![Matrix](https://img.shields.io/badge/Matrix-%23rust--windowing%3Amatrix.org-blueviolet.svg)](https://matrix.to/#/#rust-windowing:matrix.org)
[![Libera.Chat](https://img.shields.io/badge/libera.chat-%23winit-red.svg)](https://web.libera.chat/#winit)

## Usage

Winit is a window creation and management library. It can create windows and lets you handle
events (for example: the window being resized, a key being pressed, a mouse movement, etc.)
produced by window.

Winit is designed to be a low-level brick in a hierarchy of libraries. Consequently, in order to
show something on the window you need to use the platform-specific getters provided by winit, or
another library.

```rust
use winit::{
    event::{Event, WindowEvent},
    event_loop::{ControlFlow, EventLoop},
    window::WindowBuilder,
};

fn main() {
    let event_loop = EventLoop::new();
    let window = WindowBuilder::new().build(&event_loop).unwrap();

    event_loop.run(move |event, _, control_flow| {
        *control_flow = ControlFlow::Wait;

        match event {
            Event::WindowEvent {
                event: WindowEvent::CloseRequested,
                window_id,
            } if window_id == window.id() => *control_flow = ControlFlow::Exit,
            _ => (),
        }
    });
}
```

Winit is only officially supported on the latest stable version of the Rust compiler.

### Cargo Features

Winit provides the following features, which can be enabled in your `Cargo.toml` file:
* `serde`: Enables serialization/deserialization of certain types with [Serde](https://crates.io/crates/serde).
* `x11` (enabled by default): On Unix platform, compiles with the X11 backend
* `wayland` (enabled by default): On Unix platform, compiles with the Wayland backend
* `mint`: Enables mint (math interoperability standard types) conversions.

### Platform-specific usage

#### Wayland

Note that windows don't appear on Wayland until you draw/present to them.

`winit` doesn't do drawing, try the examples in [`glutin`] instead.

[`glutin`]: https://github.com/rust-windowing/glutin

#### WebAssembly

To run the web example: `cargo run-wasm --example web`

Winit supports compiling to the `wasm32-unknown-unknown` target with `web-sys`.

On the web platform, a Winit window is backed by a `<canvas>` element. You can
either [provide Winit with a `<canvas>` element][web with_canvas], or [let Winit
create a `<canvas>` element which you can then retrieve][web canvas getter] and
insert it into the DOM yourself.

For example code using Winit with WebAssembly, check out the [web example]. For
information on using Rust on WebAssembly, check out the [Rust and WebAssembly
book].

[web with_canvas]: https://docs.rs/winit/latest/wasm32-unknown-unknown/winit/platform/web/trait.WindowBuilderExtWebSys.html#tymethod.with_canvas
[web canvas getter]: https://docs.rs/winit/latest/wasm32-unknown-unknown/winit/platform/web/trait.WindowExtWebSys.html#tymethod.canvas
[web example]: ./examples/web.rs
[Rust and WebAssembly book]: https://rustwasm.github.io/book/

#### Android

This library makes use of the [ndk-rs](https://github.com/rust-windowing/android-ndk-rs) crates, refer to that repo for more documentation.

The `ndk-glue` version needs to match the version used by `winit`. Otherwise, the application will not start correctly as `ndk-glue`'s internal `NativeActivity` static is not the same due to version mismatch.

`winit` compatibility table with `ndk-glue`:

| winit |       ndk-glue       |
| :---: | :------------------: |
| 0.24  | `ndk-glue = "0.2.0"` |
| 0.25  | `ndk-glue = "0.3.0"` |
| 0.26  | `ndk-glue = "0.5.0"` |
| 0.27  | `ndk-glue = "0.7.0"` |

Running on an Android device needs a dynamic system library, add this to Cargo.toml:

```toml
[[example]]
name = "request_redraw_threaded"
crate-type = ["cdylib"]
```

And add this to the example file to add the native activity glue:
```rust
#[cfg_attr(target_os = "android", ndk_glue::main(backtrace = "on"))]
fn main() {
    ...
}
```

And run the application with `cargo apk run --example request_redraw_threaded`

#### MacOS

A lot of functionality expects the application to be ready before you start
doing anything; this includes creating windows, fetching monitors, drawing,
and so on, see issues [#2238], [#2051] and [#2087].

If you encounter problems, you should try doing your initialization inside
`Event::NewEvents(StartCause::Init)`.

[#2238]: https://github.com/rust-windowing/winit/issues/2238
[#2051]: https://github.com/rust-windowing/winit/issues/2051
[#2087]: https://github.com/rust-windowing/winit/issues/2087
