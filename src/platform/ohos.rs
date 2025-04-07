//! # OpenHarmony
//!
//! The OpenHarmony backend builds on (and exposes types from) the [`ohos-rs`](https://docs.rs/ohos-rs/) crate.
//!
//! Native OpenHarmony applications need some form of "glue" crate that is responsible
//! for defining the main entry point for your Rust application as well as tracking
//! various life-cycle events and synchronizing with the main thread.
//!
//! Winit uses the [openharmony-ability](https://docs.rs/openharmony-ability/) as a
//! glue crate.
//!

use self::ability::{Configuration, OpenHarmonyApp, Rect};
use crate::application::ApplicationHandler;
use crate::event_loop::{self, ActiveEventLoop, EventLoop, EventLoopBuilder};
use crate::window::{Window, WindowAttributes};

/// Additional methods on [`EventLoop`] that are specific to OpenHarmony.
pub trait EventLoopExtOpenHarmony {
    /// A type provided by the user that can be passed through `Event::UserEvent`.
    type UserEvent: 'static;

    /// Initializes the winit event loop.
    /// Unlike [`run_app()`] this method return immediately.
    /// [^1]: `run_app()` is _not_ available on OpnHarmony
    fn spawn_app<A: ApplicationHandler<Self::UserEvent> + 'static>(self, app: A);

    /// Get the [`OpenHarmonyApp`] which was used to create this event loop.
    fn openharmony_app(&self) -> &OpenHarmonyApp;
}

impl<T> EventLoopExtOpenHarmony for EventLoop<T> {
    type UserEvent = T;

    fn spawn_app<A: ApplicationHandler<Self::UserEvent> + 'static>(self, app: A) {
        let app = Box::leak(Box::new(app));
        let event_looper = Box::leak(Box::new(self));

        let _ = event_looper.event_loop.run_on_demand(|event, event_loop| {
            event_loop::dispatch_event_for_app(app, event_loop, event)
        });
    }

    fn openharmony_app(&self) -> &OpenHarmonyApp {
        &self.event_loop.openharmony_app
    }
}

/// Additional methods on [`ActiveEventLoop`] that are specific to OpenHarmony.
pub trait ActiveEventLoopExtOpenHarmony {
    /// Get the [`OpenHarmonyApp`] which was used to create this event loop.
    fn openharmony_app(&self) -> &OpenHarmonyApp;
}

/// Additional methods on [`Window`] that are specific to OpenHarmony.
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

/// Additional methods on [`WindowAttributes`] that are specific to OpenHarmony.
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
///
/// For compatibility applications should then import the [`OpenHarmonyApp`] type for
/// their `init(app: OpenHarmonyApp)` function and use `openharmony-ability-derive` to
/// implement entry like:
/// ```rust
/// #[cfg(target_env = "ohos")]
/// use winit::platform::ohos::ability::OpenHarmonyApp;
/// use openharmony_ability_derive::ability;
///
/// #[ability]
/// fn init(app: OpenHarmonyApp) {
///     // ...
/// }
/// ```
pub mod ability {
    #[doc(no_inline)]
    #[cfg(ohos_platform)]
    pub use openharmony_ability::*;

    #[doc(no_inline)]
    #[cfg(ohos_platform)]
    pub use openharmony_ability_derive::*;

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
