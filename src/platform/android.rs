use crate::{
    event_loop::{EventLoop, EventLoopBuilder, EventLoopWindowTarget},
    window::{Window, WindowBuilder},
};

use android_activity::{AndroidApp, ConfigurationRef, Rect};

/// Additional methods on [`EventLoop`] that are specific to Android.
pub trait EventLoopExtAndroid {}

impl<T> EventLoopExtAndroid for EventLoop<T> {}

/// Additional methods on [`EventLoopWindowTarget`] that are specific to Android.
pub trait EventLoopWindowTargetExtAndroid {}

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

impl<T> EventLoopWindowTargetExtAndroid for EventLoopWindowTarget<T> {}

/// Additional methods on [`WindowBuilder`] that are specific to Android.
pub trait WindowBuilderExtAndroid {}

impl WindowBuilderExtAndroid for WindowBuilder {}

pub trait EventLoopBuilderExtAndroid {
    /// Associates the `AndroidApp` that was passed to `android_main()` with the event loop
    ///
    /// This must be called on Android since the `AndroidApp` is not global state.
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
/// For compatibility applications should then import the `AndroidApp` type for
/// their `android_main(app: AndroidApp)` function like:
/// ```rust
/// #[cfg(target_os="android")]
/// use winit::platform::android::activity::AndroidApp;
/// ```
pub mod activity {
    // We enable the `"native-activity"` feature just so that we can build the
    // docs, but it'll be very confusing for users to see the docs with that
    // feature enabled, so we avoid inlining it so that they're forced to view
    // it on the crate's own docs.rs page.
    #[doc(no_inline)]
    pub use android_activity::*;
}
