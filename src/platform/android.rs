//! # Android
//!
//! The Android backend builds on (and exposes types from) the [`ndk`](https://docs.rs/ndk/) crate.
//!
//! Native Android applications need some form of "glue" crate that is responsible
//! for defining the main entry point for your Rust application as well as tracking
//! various life-cycle events and synchronizing with the main JVM thread.
//!
//! Winit uses the [android-activity](https://docs.rs/android-activity/) as a
//! glue crate (prior to `0.28` it used
//! [ndk-glue](https://github.com/rust-windowing/android-ndk-rs/tree/master/ndk-glue)).
//!
//! The version of the glue crate that your application depends on _must_ match the
//! version that Winit depends on because the glue crate is responsible for your
//! application's main entry point. If Cargo resolves multiple versions, they will
//! clash.
//!
//! `winit` glue compatibility table:
//!
//! | winit |       ndk-glue               |
//! | :---: | :--------------------------: |
//! | 0.30  | `android-activity = "0.6"`   |
//! | 0.29  | `android-activity = "0.5"`   |
//! | 0.28  | `android-activity = "0.4"`   |
//! | 0.27  | `ndk-glue = "0.7"`           |
//! | 0.26  | `ndk-glue = "0.5"`           |
//! | 0.25  | `ndk-glue = "0.3"`           |
//! | 0.24  | `ndk-glue = "0.2"`           |
//!
//! The recommended way to avoid a conflict with the glue version is to avoid explicitly
//! depending on the `android-activity` crate, and instead consume the API that
//! is re-exported by Winit under `winit::platform::android::activity::*`
//!
//! Running on an Android device needs a dynamic system library. Add this to Cargo.toml:
//!
//! ```toml
//! [lib]
//! name = "main"
//! crate-type = ["cdylib"]
//! ```
//!
//! All Android applications are based on an `Activity` subclass, and the
//! `android-activity` crate is designed to support different choices for this base
//! class. Your application _must_ specify the base class it needs via a feature flag:
//!
//! | Base Class       | Feature Flag      |  Notes  |
//! | :--------------: | :---------------: | :-----: |
//! | `NativeActivity` | `android-native-activity` | Built-in to Android - it is possible to use without compiling any Java or Kotlin code. Java or Kotlin code may be needed to subclass `NativeActivity` to access some platform features. It does not derive from the [`AndroidAppCompat`] base class.|
//! | [`GameActivity`] | `android-game-activity`   | Derives from [`AndroidAppCompat`], a defacto standard `Activity` base class that helps support a wider range of Android versions. Requires a build system that can compile Java or Kotlin and fetch Android dependencies from a [Maven repository][agdk_jetpack] (or link with an embedded [release][agdk_releases] of [`GameActivity`]) |
//!
//! [`GameActivity`]: https://developer.android.com/games/agdk/game-activity
//! [`GameTextInput`]: https://developer.android.com/games/agdk/add-support-for-text-input
//! [`AndroidAppCompat`]: https://developer.android.com/reference/androidx/appcompat/app/AppCompatActivity
//! [agdk_jetpack]: https://developer.android.com/jetpack/androidx/releases/games
//! [agdk_releases]: https://developer.android.com/games/agdk/download#agdk-libraries
//! [Gradle]: https://developer.android.com/studio/build
//!
//! For more details, refer to these `android-activity` [example applications](https://github.com/rust-mobile/android-activity/tree/main/examples).
//!
//! ## Converting from `ndk-glue` to `android-activity`
//!
//! If your application is currently based on `NativeActivity` via the `ndk-glue` crate and building
//! with `cargo apk`, then the minimal changes would be:
//! 1. Remove `ndk-glue` from your `Cargo.toml`
//! 2. Enable the `"android-native-activity"` feature for Winit: `winit = { version = "0.30.8",
//!    features = [ "android-native-activity" ] }`
//! 3. Add an `android_main` entrypoint (as above), instead of using the '`[ndk_glue::main]` proc
//!    macro from `ndk-macros` (optionally add a dependency on `android_logger` and initialize
//!    logging as above).
//! 4. Pass a clone of the `AndroidApp` that your application receives to Winit when building your
//!    event loop (as shown above).

use crate::event_loop::{ActiveEventLoop, EventLoop, EventLoopBuilder};
use crate::window::{Window, WindowAttributes};

use self::activity::{AndroidApp, ConfigurationRef, Rect};

/// Additional methods on [`EventLoop`] that are specific to Android.
pub trait EventLoopExtAndroid {
    /// Get the [`AndroidApp`] which was used to create this event loop.
    fn android_app(&self) -> &AndroidApp;
}

impl<T> EventLoopExtAndroid for EventLoop<T> {
    fn android_app(&self) -> &AndroidApp {
        &self.event_loop.android_app
    }
}

/// Additional methods on [`ActiveEventLoop`] that are specific to Android.
pub trait ActiveEventLoopExtAndroid {
    /// Get the [`AndroidApp`] which was used to create this event loop.
    fn android_app(&self) -> &AndroidApp;
}

/// Additional methods on [`Window`] that are specific to Android.
pub trait WindowExtAndroid {
    fn content_rect(&self) -> Rect;

    fn config(&self) -> ConfigurationRef;
}

impl WindowExtAndroid for Window {
    fn content_rect(&self) -> Rect {
        self.window.content_rect()
    }

    fn config(&self) -> ConfigurationRef {
        self.window.config()
    }
}

impl ActiveEventLoopExtAndroid for ActiveEventLoop {
    fn android_app(&self) -> &AndroidApp {
        &self.p.app
    }
}

/// Additional methods on [`WindowAttributes`] that are specific to Android.
pub trait WindowAttributesExtAndroid {}

impl WindowAttributesExtAndroid for WindowAttributes {}

pub trait EventLoopBuilderExtAndroid {
    /// Associates the [`AndroidApp`] that was passed to `android_main()` with the event loop
    ///
    /// This must be called on Android since the [`AndroidApp`] is not global state.
    fn with_android_app(&mut self, app: AndroidApp) -> &mut Self;

    /// Calling this will mark the volume keys to be manually handled by the application
    ///
    /// Default is to let the operating system handle the volume keys
    fn handle_volume_keys(&mut self) -> &mut Self;
}

impl<T> EventLoopBuilderExtAndroid for EventLoopBuilder<T> {
    fn with_android_app(&mut self, app: AndroidApp) -> &mut Self {
        self.platform_specific.android_app = Some(app);
        self
    }

    fn handle_volume_keys(&mut self) -> &mut Self {
        self.platform_specific.ignore_volume_keys = false;
        self
    }
}

/// Re-export of the `android_activity` API
///
/// Winit re-exports the `android_activity` API for convenience so that most
/// applications can rely on the Winit crate to resolve the required version of
/// `android_activity` and avoid any chance of a conflict between Winit and the
/// application crate.
///
/// Unlike most libraries there can only be a single implementation
/// of the `android_activity` glue crate linked with an application because
/// it is responsible for the application's `android_main()` entry point.
///
/// Since Winit depends on a specific version of `android_activity` the simplest
/// way to avoid creating a conflict is for applications to avoid explicitly
/// depending on the `android_activity` crate, and instead consume the API that
/// is re-exported by Winit.
///
/// For compatibility applications should then import the [`AndroidApp`] type for
/// their `android_main(app: AndroidApp)` function like:
/// ```rust
/// #[cfg(target_os = "android")]
/// use winit::platform::android::activity::AndroidApp;
/// ```
pub mod activity {
    // We enable the `"native-activity"` feature just so that we can build the
    // docs, but it'll be very confusing for users to see the docs with that
    // feature enabled, so we avoid inlining it so that they're forced to view
    // it on the crate's own docs.rs page.
    #[doc(no_inline)]
    #[cfg(android_platform)]
    pub use android_activity::*;

    #[cfg(not(android_platform))]
    #[doc(hidden)]
    pub struct Rect;
    #[cfg(not(android_platform))]
    #[doc(hidden)]
    pub struct ConfigurationRef;
    #[cfg(not(android_platform))]
    #[doc(hidden)]
    pub struct AndroidApp;
}
