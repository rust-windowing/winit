use std::sync::Arc;

use dpi::{LogicalSize, Size};
use sctk::shell::xdg::XdgPositioner;
use sctk::shell::xdg::popup::{PopupConfigure, PopupHandler};
use sctk::subcompositor::{self, SubcompositorState};
use wayland_protocols::wp::fractional_scale::v1::client::wp_fractional_scale_v1::WpFractionalScaleV1;

#[derive(Debug)]
pub struct State {
    positioner: XdgPositioner,

    /// The current window title.
    title: String,

    scale_factor: f64,
    fractional_scale: Option<WpFractionalScaleV1>,

    pub last_configure: Option<PopupConfigure>,

    /// The size of the popup
    size: LogicalSize<u32>,

    /// True if the compositor constrained the size of the popup
    constrained: bool,

    /// TODO: maybe this can be removed, because we can use also last_configure to determine
    /// If called the first time or not!
    /// Initial window size provided by the user. Removed on the first
    /// configure.
    /// Required because we don't know the scaling yet
    /// when constructing the state
    initial_size: Option<Size>,
}

impl State {
    pub fn new(positioner: XdgPositioner, initial_size: Size) -> Self {
        Self {
            positioner,
            initial_size: Some(initial_size),
            constrained: false,
            size: initial_size.to_logical(1.),
            scale_factor: 1.,
            last_configure: None,
            title: String::default(),
            fractional_scale: None,
        }
    }

    pub fn configure(&mut self, configure: PopupConfigure) // -> bool
    {
        // NOTE: when using fractional scaling or wl_compositor@v6 the scaling
        // should be delivered before the first configure, thus apply it to
        // properly scale the physical sizes provided by the users.
        if let Some(initial_size) = self.initial_size.take() {
            self.size = initial_size.to_logical(self.scale_factor());
        }

        // The popup was constrained to a different size by the compositor
        assert!(configure.width >= 0);
        assert!(configure.height >= 0);
        self.constrained = self.size.width != configure.width as u32
            || self.size.height != configure.height as u32;

        self.size.width = (configure.width as u32).into();
        self.size.height = (configure.height as u32).into();

        // false
    }

    #[inline]
    pub fn set_title(&mut self, title: &str) {
        self.title = title.to_owned();
    }

    #[inline]
    pub fn title(&self) -> &str {
        &self.title
    }

    /// Set the scale factor for the given window.
    #[inline]
    pub fn set_scale_factor(&mut self, scale_factor: f64) {
        self.scale_factor = scale_factor;

        // NOTE: When fractional scaling is not used update the buffer scale.
        // if self.fractional_scale.is_none() {
        //     let _ = self.window.set_buffer_scale(self.scale_factor as _);
        // }

        // if let Some(frame) = self.frame.as_mut() {
        //     frame.set_scaling_factor(scale_factor);
        // }
    }

    #[inline]
    pub fn is_configured(&self) -> bool {
        self.last_configure.is_some()
    }

    #[inline]
    pub fn scale_factor(&self) -> f64 {
        self.scale_factor
    }

    #[inline]
    pub fn surface_size(&self) -> LogicalSize<u32> {
        self.size
    }
}
