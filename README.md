# winit - Cross-platform window creation and management in Rust

[![Crates.io](https://img.shields.io/crates/v/winit.svg)](https://crates.io/crates/winit)
[![Docs.rs](https://docs.rs/winit/badge.svg)](https://docs.rs/winit)
[![CI Status](https://github.com/rust-windowing/winit/workflows/CI/badge.svg)](https://github.com/rust-windowing/winit/actions)

```toml
[dependencies]
winit = "0.28.7"
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

The Android backend builds on (and exposes types from) the [`ndk`](https://docs.rs/ndk/0.7.0/ndk/) crate.

Native Android applications need some form of "glue" crate that is responsible
for defining the main entry point for your Rust application as well as tracking
various life-cycle events and synchronizing with the main JVM thread.

Winit uses the [android-activity](https://github.com/rib/android-activity) as a
glue crate (prior to `0.28` it used
[ndk-glue](https://github.com/rust-windowing/android-ndk-rs/tree/master/ndk-glue)).

The version of the glue crate that your application depends on _must_ match the
version that Winit depends on because the glue crate is responsible for your
application's main entrypoint. If Cargo resolves multiple versions they will
clash.

`winit` glue compatibility table:

| winit |       ndk-glue               |
| :---: | :--------------------------: |
| 0.28  | `android-activity = "0.4"`   |
| 0.27  | `ndk-glue = "0.7"`           |
| 0.26  | `ndk-glue = "0.5"`           |
| 0.25  | `ndk-glue = "0.3"`           |
| 0.24  | `ndk-glue = "0.2"`           |

The recommended way to avoid a conflict with the glue version is to avoid explicitly
depending on the `android-activity` crate, and instead consume the API that
is re-exported by Winit under `winit::platform::android::activity::*`

Running on an Android device needs a dynamic system library, add this to Cargo.toml:

```toml
[lib]
name = "main"
crate-type = ["cdylib"]
```

All Android applications are based on an `Activity` subclass and the
`android-activity` crate is designed to support different choices for this base
class. Your application _must_ specify the base class it needs via a feature flag:

| Base Class       | Feature Flag      |  Notes  |
| :--------------: | :---------------: | :-----: |
| `NativeActivity` | `android-native-activity` | Built-in to Android - it is possible to use without compiling any Java or Kotlin code. Java or Kotlin code may be needed to subclass `NativeActivity` to access some platform features. It does not derive from the [`AndroidAppCompat`] base class.|
| [`GameActivity`] | `android-game-activity`   | Derives from [`AndroidAppCompat`] which is a defacto standard `Activity` base class that helps support a wider range of Android versions. Requires a build system that can compile Java or Kotlin and fetch Android dependencies from a [Maven repository][agdk_jetpack] (or link with an embedded [release][agdk_releases] of [`GameActivity`]) |

[`GameActivity`]: https://developer.android.com/games/agdk/game-activity
[`GameTextInput`]: https://developer.android.com/games/agdk/add-support-for-text-input
[`AndroidAppCompat`]: https://developer.android.com/reference/androidx/appcompat/app/AppCompatActivity
[agdk_jetpack]: https://developer.android.com/jetpack/androidx/releases/games
[agdk_releases]: https://developer.android.com/games/agdk/download#agdk-libraries
[Gradle]: https://developer.android.com/studio/build

For example, add this to Cargo.toml:
```toml
winit = { version = "0.28", features = [ "android-native-activity" ] }

[target.'cfg(target_os = "android")'.dependencies]
android_logger = "0.11.0"
```

And, for example, define an entry point for your library like this:
```rust
#[cfg(target_os = "android")]
use winit::platform::android::activity::AndroidApp;

#[cfg(target_os = "android")]
#[no_mangle]
fn android_main(app: AndroidApp) {
    use winit::platform::android::EventLoopBuilderExtAndroid;

    android_logger::init_once(android_logger::Config::default().with_min_level(log::Level::Trace));

    let event_loop = EventLoopBuilder::with_user_event()
        .with_android_app(app)
        .build();
    _main(event_loop);
}
```

For more details, refer to these `android-activity` [example applications](https://github.com/rib/android-activity/tree/main/examples).

##### Converting from `ndk-glue` to `android-activity`

If your application is currently based on `NativeActivity` via the `ndk-glue` crate and building with `cargo apk` then the minimal changes would be:
1. Remove `ndk-glue` from your `Cargo.toml`
2. Enable the `"android-native-activity"` feature for Winit: `winit = { version = "0.28", features = [ "android-native-activity" ] }`
3. Add an `android_main` entrypoint (as above), instead of using the '`[ndk_glue::main]` proc macro from `ndk-macros` (optionally add a dependency on `android_logger` and initialize logging as above).
4. Pass a clone of the `AndroidApp` that your application receives to Winit when building your event loop (as shown above).

#### MacOS

A lot of functionality expects the application to be ready before you start
doing anything; this includes creating windows, fetching monitors, drawing,
and so on, see issues [#2238], [#2051] and [#2087].

If you encounter problems, you should try doing your initialization inside
`Event::NewEvents(StartCause::Init)`.

#### iOS

Similar to macOS, iOS's main `UIApplicationMain` does some init work that's required
by all UI related code, see issue [#1705]. You should consider creating your windows
inside `Event::NewEvents(StartCause::Init)`.


[#2238]: https://github.com/rust-windowing/winit/issues/2238
[#2051]: https://github.com/rust-windowing/winit/issues/2051
[#2087]: https://github.com/rust-windowing/winit/issues/2087
[#1705]: https://github.com/rust-windowing/winit/issues/1705

#### Redox OS

Redox OS has some functionality not present yet, that will be implemented when
its orbital display server provides it.
