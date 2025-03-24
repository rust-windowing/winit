//! # Wayland
//!
//! **Note:** Windows don't appear on Wayland until you draw/present to them.
//!
//! By default, Winit loads system libraries using `dlopen`. This can be
//! disabled by disabling the `"wayland-dlopen"` cargo feature.
//!
//! ## Client-side decorations
//!
//! Winit provides client-side decorations by default, but the behaviour can
//! be controlled with the following feature flags:
//!
//! * `wayland-csd-adwaita` (default).
//! * `wayland-csd-adwaita-crossfont`.
//! * `wayland-csd-adwaita-notitle`.
#[cfg(wayland_platform)]
pub use sctk::{
    compositor::Region,
    shell::wlr_layer::{Anchor, KeyboardInteractivity, Layer},
};

use crate::dpi::{LogicalPosition, LogicalSize};
use crate::error::RequestError;
use crate::event_loop::{ActiveEventLoop, EventLoop, EventLoopBuilder};
pub use crate::window::Theme;
use crate::window::{Window as CoreWindow, WindowAttributes};

/// Additional methods on [`ActiveEventLoop`] that are specific to Wayland.
pub trait ActiveEventLoopExtWayland {
    /// True if the [`ActiveEventLoop`] uses Wayland.
    fn is_wayland(&self) -> bool;
}

impl ActiveEventLoopExtWayland for dyn ActiveEventLoop + '_ {
    #[inline]
    fn is_wayland(&self) -> bool {
        self.cast_ref::<crate::platform_impl::wayland::ActiveEventLoop>().is_some()
    }
}

/// Additional methods on [`EventLoop`] that are specific to Wayland.
pub trait EventLoopExtWayland {
    /// True if the [`EventLoop`] uses Wayland.
    fn is_wayland(&self) -> bool;
}

impl EventLoopExtWayland for EventLoop {
    #[inline]
    fn is_wayland(&self) -> bool {
        self.event_loop.is_wayland()
    }
}

/// Additional methods on [`EventLoopBuilder`] that are specific to Wayland.
pub trait EventLoopBuilderExtWayland {
    /// Force using Wayland.
    fn with_wayland(&mut self) -> &mut Self;

    /// Whether to allow the event loop to be created off of the main thread.
    ///
    /// By default, the window is only allowed to be created on the main
    /// thread, to make platform compatibility easier.
    fn with_any_thread(&mut self, any_thread: bool) -> &mut Self;
}

impl EventLoopBuilderExtWayland for EventLoopBuilder {
    #[inline]
    fn with_wayland(&mut self) -> &mut Self {
        self.platform_specific.forced_backend = Some(crate::platform_impl::Backend::Wayland);
        self
    }

    #[inline]
    fn with_any_thread(&mut self, any_thread: bool) -> &mut Self {
        self.platform_specific.any_thread = any_thread;
        self
    }
}

/// Additional methods on [`Window`] that are specific to Wayland.
///
/// [`Window`]: crate::window::Window
pub trait WindowExtWayland {
    fn create_region(
        &self,
        position: LogicalPosition<i32>,
        size: LogicalSize<i32>,
    ) -> Result<Region, RequestError>;
    fn set_region(&self, region: Option<&Region>);
}

impl WindowExtWayland for dyn CoreWindow + '_ {
    fn create_region(
        &self,
        pos: LogicalPosition<i32>,
        size: LogicalSize<i32>,
    ) -> Result<Region, RequestError> {
        let window = self.cast_ref::<crate::platform_impl::wayland::Window>().unwrap();
        let region = window.create_region()?;
        region.add(pos.x, pos.y, size.width, size.height);
        Ok(region)
    }

    fn set_region(&self, region: Option<&Region>) {
        let window = self.cast_ref::<crate::platform_impl::wayland::Window>().unwrap();
        window.set_region(region);
    }
}

/// Additional methods on [`WindowAttributes`] that are specific to Wayland.
pub trait WindowAttributesExtWayland {
    /// Build window with the given name.
    ///
    /// The `general` name sets an application ID, which should match the `.desktop`
    /// file distributed with your program. The `instance` is a `no-op`.
    ///
    /// For details about application ID conventions, see the
    /// [Desktop Entry Spec](https://specifications.freedesktop.org/desktop-entry-spec/desktop-entry-spec-latest.html#desktop-file-id)
    fn with_name(self, general: impl Into<String>, instance: impl Into<String>) -> Self;

    #[cfg(wayland_platform)]
    fn with_anchor(self, anchor: Anchor) -> Self;

    #[cfg(wayland_platform)]
    fn with_exclusive_zone(self, exclusive_zone: i32) -> Self;

    #[cfg(wayland_platform)]
    fn with_margin(self, top: i32, right: i32, bottom: i32, left: i32) -> Self;

    #[cfg(wayland_platform)]
    fn with_keyboard_interactivity(self, keyboard_interactivity: KeyboardInteractivity) -> Self;

    #[cfg(wayland_platform)]
    fn with_layer(self, layer: Layer) -> Self;

    #[cfg(wayland_platform)]
    fn with_region(self, position: LogicalPosition<i32>, size: LogicalSize<i32>) -> Self;

    #[cfg(wayland_platform)]
    fn with_output(self, output: u64) -> Self;
}

impl WindowAttributesExtWayland for WindowAttributes {
    #[inline]
    fn with_name(mut self, general: impl Into<String>, instance: impl Into<String>) -> Self {
        self.platform_specific.name =
            Some(crate::platform_impl::ApplicationName::new(general.into(), instance.into()));
        self
    }

    #[inline]
    #[cfg(wayland_platform)]
    fn with_anchor(mut self, anchor: Anchor) -> Self {
        self.platform_specific.wayland.anchor = Some(anchor);
        self
    }

    #[inline]
    #[cfg(wayland_platform)]
    fn with_exclusive_zone(mut self, exclusive_zone: i32) -> Self {
        self.platform_specific.wayland.exclusive_zone = Some(exclusive_zone);
        self
    }

    #[inline]
    #[cfg(wayland_platform)]
    fn with_margin(mut self, top: i32, right: i32, bottom: i32, left: i32) -> Self {
        self.platform_specific.wayland.margin = Some((top, right, bottom, left));
        self
    }

    #[inline]
    #[cfg(wayland_platform)]
    fn with_keyboard_interactivity(
        mut self,
        keyboard_interactivity: KeyboardInteractivity,
    ) -> Self {
        self.platform_specific.wayland.keyboard_interactivity = Some(keyboard_interactivity);
        self
    }

    #[inline]
    #[cfg(wayland_platform)]
    fn with_layer(mut self, layer: Layer) -> Self {
        self.platform_specific.wayland.layer = Some(layer);
        self
    }

    #[inline]
    #[cfg(wayland_platform)]
    fn with_region(mut self, position: LogicalPosition<i32>, size: LogicalSize<i32>) -> Self {
        self.platform_specific.wayland.region = Some((position, size));
        self
    }

    #[inline]
    #[cfg(wayland_platform)]
    fn with_output(mut self, output: u64) -> Self {
        self.platform_specific.wayland.output = Some(output);
        self
    }
}
