# winit - Cross-platform window creation and management in Rust

[![Crates.io](https://img.shields.io/crates/v/winit.svg)](https://crates.io/crates/winit)
[![Docs.rs](https://docs.rs/winit/badge.svg)](https://docs.rs/winit)
[![CI Status](https://github.com/rust-windowing/winit/workflows/CI/badge.svg)](https://github.com/rust-windowing/winit/actions)

```toml
[dependencies]
winit = "0.29.15"
```

## [Documentation](https://docs.rs/winit)

For features _within_ the scope of winit, see [FEATURES.md](FEATURES.md).

For features _outside_ the scope of winit, see [Are we GUI Yet?](https://areweguiyet.com/) and [Are we game yet?](https://arewegameyet.rs/), depending on what kind of project you're looking to do.

## Contact Us

Join us in any of these:

[![Matrix](https://img.shields.io/badge/Matrix-%23rust--windowing%3Amatrix.org-blueviolet.svg)](https://matrix.to/#/#rust-windowing:matrix.org)
[![Libera.Chat](https://img.shields.io/badge/libera.chat-%23winit-red.svg)](https://web.libera.chat/#winit)

## Usage

Winit is a window creation and management library. It can create windows and lets you handle
events (for example: the window being resized, a key being pressed, a mouse movement, etc.)
produced by the window.

Winit is designed to be a low-level brick in a hierarchy of libraries. Consequently, in order to
show something on the window you need to use the platform-specific getters provided by winit, or
another library.

### Cargo Features

Winit provides the following features, which can be enabled in your `Cargo.toml` file:
* `serde`: Enables serialization/deserialization of certain types with [Serde](https://crates.io/crates/serde).
* `x11` (enabled by default): On Unix platform, compiles with the X11 backend
* `wayland` (enabled by default): On Unix platform, compiles with the Wayland backend
* `mint`: Enables mint (math interoperability standard types) conversions.

## MSRV Policy

This crate's Minimum Supported Rust Version (MSRV) is **1.65**. Changes to
the MSRV will be accompanied by a minor version bump.

As a **tentative** policy, the upper bound of the MSRV is given by the following
formula:

```
min(sid, stable - 3)
```

Where `sid` is the current version of `rustc` provided by [Debian Sid], and
`stable` is the latest stable version of Rust. This bound may be broken in case of a major ecosystem shift or a security vulnerability.

[Debian Sid]: https://packages.debian.org/sid/rustc

The exception is for the Android platform, where a higher Rust version
must be used for certain Android features. In this case, the MSRV will be
capped at the latest stable version of Rust minus three. This inconsistency is
not reflected in Cargo metadata, as it is not powerful enough to expose this
restriction.

All crates in the [`rust-windowing`] organizations have the
same MSRV policy.

[`rust-windowing`]: https://github.com/rust-windowing

### Platform-specific usage

#### Wayland

Note that windows don't appear on Wayland until you draw/present to them.

#### WebAssembly

To run the web example: `cargo run-wasm --example web`

Winit supports compiling to the `wasm32-unknown-unknown` target with `web-sys`.

On the web platform, a Winit window is backed by a `<canvas>` element. You can
either [provide Winit with a `<canvas>` element][web with_canvas], or [let Winit
create a `<canvas>` element which you can then retrieve][web canvas getter] and
insert it into the DOM yourself.

For the example code using Winit with WebAssembly, check out the [web example]. For
information on using Rust on WebAssembly, check out the [Rust and WebAssembly
book].

[web with_canvas]: https://docs.rs/winit/latest/wasm32-unknown-unknown/winit/platform/web/trait.WindowBuilderExtWebSys.html#tymethod.with_canvas
[web canvas getter]: https://docs.rs/winit/latest/wasm32-unknown-unknown/winit/platform/web/trait.WindowExtWebSys.html#tymethod.canvas
[web example]: ./examples/web.rs
[Rust and WebAssembly book]: https://rustwasm.github.io/book/

#### Android

The Android backend builds on (and exposes types from) the [`ndk`](https://docs.rs/ndk/latest/ndk/) crate.

Native Android applications need some form of "glue" crate that is responsible
for defining the main entry point for your Rust application as well as tracking
various life-cycle events and synchronizing with the main JVM thread.

Winit uses the [android-activity](https://github.com/rib/android-activity) as a
glue crate (prior to `0.28` it used
[ndk-glue](https://github.com/rust-windowing/android-ndk-rs/tree/master/ndk-glue)).

The version of the glue crate that your application depends on _must_ match the
version that Winit depends on because the glue crate is responsible for your
application's main entry point. If Cargo resolves multiple versions, they will
clash.

`winit` glue compatibility table:

| winit |       ndk-glue               |
| :---: | :--------------------------: |
| 0.29  | `android-activity = "0.5"`   |
| 0.28  | `android-activity = "0.4"`   |
| 0.27  | `ndk-glue = "0.7"`           |
| 0.26  | `ndk-glue = "0.5"`           |
| 0.25  | `ndk-glue = "0.3"`           |
| 0.24  | `ndk-glue = "0.2"`           |

The recommended way to avoid a conflict with the glue version is to avoid explicitly
depending on the `android-activity` crate, and instead consume the API that
is re-exported by Winit under `winit::platform::android::activity::*`

Running on an Android device needs a dynamic system library. Add this to Cargo.toml:

```toml
[lib]
name = "main"
crate-type = ["cdylib"]
```

All Android applications are based on an `Activity` subclass, and the
`android-activity` crate is designed to support different choices for this base
class. Your application _must_ specify the base class it needs via a feature flag:

| Base Class       | Feature Flag      |  Notes  |
| :--------------: | :---------------: | :-----: |
| `NativeActivity` | `android-native-activity` | Built-in to Android - it is possible to use without compiling any Java or Kotlin code. Java or Kotlin code may be needed to subclass `NativeActivity` to access some platform features. It does not derive from the [`AndroidAppCompat`] base class.|
| [`GameActivity`] | `android-game-activity`   | Derives from [`AndroidAppCompat`], a defacto standard `Activity` base class that helps support a wider range of Android versions. Requires a build system that can compile Java or Kotlin and fetch Android dependencies from a [Maven repository][agdk_jetpack] (or link with an embedded [release][agdk_releases] of [`GameActivity`]) |

[`GameActivity`]: https://developer.android.com/games/agdk/game-activity
[`GameTextInput`]: https://developer.android.com/games/agdk/add-support-for-text-input
[`AndroidAppCompat`]: https://developer.android.com/reference/androidx/appcompat/app/AppCompatActivity
[agdk_jetpack]: https://developer.android.com/jetpack/androidx/releases/games
[agdk_releases]: https://developer.android.com/games/agdk/download#agdk-libraries
[Gradle]: https://developer.android.com/studio/build

For more details, refer to these `android-activity` [example applications](https://github.com/rust-mobile/android-activity/tree/main/examples).

##### Converting from `ndk-glue` to `android-activity`

If your application is currently based on `NativeActivity` via the `ndk-glue` crate and building with `cargo apk`, then the minimal changes would be:
1. Remove `ndk-glue` from your `Cargo.toml`
2. Enable the `"android-native-activity"` feature for Winit: `winit = { version = "0.29.15", features = [ "android-native-activity" ] }`
3. Add an `android_main` entrypoint (as above), instead of using the '`[ndk_glue::main]` proc macro from `ndk-macros` (optionally add a dependency on `android_logger` and initialize logging as above).
4. Pass a clone of the `AndroidApp` that your application receives to Winit when building your event loop (as shown above).

#### MacOS

A lot of functionality expects the application to be ready before you start
doing anything; this includes creating windows, fetching monitors, drawing,
and so on, see issues [#2238], [#2051] and [#2087].

If you encounter problems, you should try doing your initialization inside
`Event::Resumed`.

#### iOS

Similar to macOS, iOS's main `UIApplicationMain` does some init work that's required
by all UI-related code (see issue [#1705]). It would be best to consider creating your windows
inside `Event::Resumed`.


[#2238]: https://github.com/rust-windowing/winit/issues/2238
[#2051]: https://github.com/rust-windowing/winit/issues/2051
[#2087]: https://github.com/rust-windowing/winit/issues/2087
[#1705]: https://github.com/rust-windowing/winit/issues/1705

#### Redox OS

Redox OS has some functionality not yet present that will be implemented when
its orbital display server provides it.
