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
#![cfg(target_env = "ohos")]

mod event_loop;
mod keycodes;

use winit_core::application::ApplicationHandler;
use winit_core::event_loop::ActiveEventLoop as CoreActiveEventLoop;
use winit_core::window::Window as CoreWindow;

use self::ability::{Configuration, OpenHarmonyApp, Rect};
pub use crate::event_loop::{
    ActiveEventLoop, EventLoop, EventLoopProxy, PlatformSpecificEventLoopAttributes,
    PlatformSpecificWindowAttributes, Window,
};

/// Additional methods on [`EventLoop`] that are specific to OpenHarmony.
pub trait EventLoopExtOpenHarmony {
    /// Initializes the winit event loop.
    /// Unlike [`run_app()`] this method return immediately.
    /// [^1]: `run_app()` is _not_ available on OpnHarmony
    fn spawn_app<A: ApplicationHandler + 'static>(self, app: A);

    /// Get the [`OpenHarmonyApp`] which was used to create this event loop.
    fn openharmony_app(&self) -> &OpenHarmonyApp;
}

/// Additional methods on [`ActiveEventLoop`] that are specific to OpenHarmony.
pub trait ActiveEventLoopExtOpenHarmony {
    /// Get the [`OpenHarmonyApp`] which was used to create this event loop.
    fn openharmony_app(&self) -> &OpenHarmonyApp;
}

impl ActiveEventLoopExtOpenHarmony for dyn CoreActiveEventLoop + '_ {
    fn openharmony_app(&self) -> &OpenHarmonyApp {
        let event_loop = self.cast_ref::<ActiveEventLoop>().unwrap();
        &event_loop.app
    }
}

/// Additional methods on [`Window`] that are specific to OpenHarmony.
pub trait WindowExtOpenHarmony {
    fn content_rect(&self) -> Rect;

    fn config(&self) -> Configuration;
}

impl WindowExtOpenHarmony for dyn CoreWindow + '_ {
    fn content_rect(&self) -> Rect {
        let window = self.cast_ref::<Window>().unwrap();
        window.content_rect()
    }

    fn config(&self) -> Configuration {
        let window = self.cast_ref::<Window>().unwrap();
        window.config()
    }
}

pub trait EventLoopBuilderExtOpenHarmony {
    /// Associates the [`OpenHarmonyApp`] that was passed to `openharmony-ability::ability` with the event loop
    ///
    /// This must be called on OpenHarmony since the [`OpenHarmonyApp`] is not global state.
    fn with_openharmony_app(&mut self, app: OpenHarmonyApp) -> &mut Self;
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
    pub use openharmony_ability::*;

    #[doc(no_inline)]
    pub use openharmony_ability_derive::*;
}
