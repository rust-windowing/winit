//! # OpenHarmony
//!
//! The OpenHarmony backend builds on (and exposes types from) the [`ndk`](https://docs.rs/ndk/) crate.
//!
//! Native OpenHarmony applications need some form of "glue" crate that is responsible
//! for defining the main entry point for your Rust application as well as tracking
//! various life-cycle events and synchronizing with the main JVM thread.
//!
//! Winit uses the [openharmony-ability](https://docs.rs/openharmony-ability/) as a
//! glue crate (prior to `0.28` it used
//! [ndk-glue](https://github.com/rust-windowing/android-ndk-rs/tree/master/ndk-glue)).
//!
//! The version of the glue crate that your application depends on _must_ match the
//! version that Winit depends on because the glue crate is responsible for your
//! application's main entry point. If Cargo resolves multiple versions, they will
//! clash.
//!

use self::ability::{Configuration, OpenHarmonyApp, Rect};
use crate::event_loop::{ActiveEventLoop, EventLoop, EventLoopBuilder};
use crate::window::{Window, WindowAttributes};

/// Additional methods on [`EventLoop`] that are specific to OpenHarmony.
pub trait EventLoopExtOpenHarmony {
    /// Get the [`OpenHarmonyApp`] which was used to create this event loop.
    fn openharmony_app(&self) -> &OpenHarmonyApp;
}

impl<T> EventLoopExtOpenHarmony for EventLoop<T> {
    fn openharmony_app(&self) -> &OpenHarmonyApp {
        &self.event_loop.openharmony_app
    }
}

/// Additional methods on [`ActiveEventLoop`] that are specific to Android.
pub trait ActiveEventLoopExtOpenHarmony {
    /// Get the [`AndroidApp`] which was used to create this event loop.
    fn openharmony_app(&self) -> &OpenHarmonyApp;
}

/// Additional methods on [`Window`] that are specific to Android.
pub trait WindowExtOpenHarmony {
    fn content_rect(&self) -> Rect;

    fn config(&self) -> Configuration;
}

impl WindowExtOpenHarmony for Window {
    fn content_rect(&self) -> Rect {
        self.window.content_rect()
    }

    fn config(&self) -> Configuration {
        self.window.config()
    }
}

impl ActiveEventLoopExtOpenHarmony for ActiveEventLoop {
    fn openharmony_app(&self) -> &OpenHarmonyApp {
        &self.p.app
    }
}

/// Additional methods on [`WindowAttributes`] that are specific to Android.
pub trait WindowAttributesExtOpenHarmony {}

impl WindowAttributesExtOpenHarmony for WindowAttributes {}

pub trait EventLoopBuilderExtOpenHarmony {
    /// Associates the [`OpenHarmonyApp`] that was passed to `openharmony-ability::ability` with the event loop
    ///
    /// This must be called on OpenHarmony since the [`OpenHarmonyApp`] is not global state.
    fn with_openharmony_app(&mut self, app: OpenHarmonyApp) -> &mut Self;

}

impl<T> EventLoopBuilderExtOpenHarmony for EventLoopBuilder<T> {
    fn with_openharmony_app(&mut self, app: OpenHarmonyApp) -> &mut Self {
        self.platform_specific.openharmony_app = Some(app);
        self
    }
}

/// Re-export of the `openharmony-ability` API
///
/// Winit re-exports the `openharmony-ability` API for convenience so that most
/// applications can rely on the Winit crate to resolve the required version of
/// `openharmony-ability` and avoid any chance of a conflict between Winit and the
/// application crate.
///
/// Unlike most libraries there can only be a single implementation
/// of the `openharmony-ability` glue crate linked with an application because
/// it is responsible for the application's `android_main()` entry point.
///
/// Since Winit depends on a specific version of `android_activity` the simplest
/// way to avoid creating a conflict is for applications to avoid explicitly
/// depending on the `android_activity` crate, and instead consume the API that
/// is re-exported by Winit.
///
/// For compatibility applications should then import the [`OpenHarmonyApp`] type for
/// their `init(app: OpenHarmonyApp)` function like:
/// ```rust
/// #[cfg(target_env = "ohos")]
/// use winit::platform::ohos::ability::OpenHarmonyApp;
/// ```
pub mod ability {
    // We enable the `"native-activity"` feature just so that we can build the
    // docs, but it'll be very confusing for users to see the docs with that
    // feature enabled, so we avoid inlining it so that they're forced to view
    // it on the crate's own docs.rs page.
    #[doc(no_inline)]
    #[cfg(ohos_platform)]
    pub use openharmony_ability::*;

    #[cfg(not(ohos_platform))]
    #[doc(hidden)]
    pub struct Rect;
    #[cfg(not(ohos_platform))]
    #[doc(hidden)]
    pub struct ConfigurationRef;
    #[cfg(not(ohos_platform))]
    #[doc(hidden)]
    pub struct OpenHarmonyApp;
}