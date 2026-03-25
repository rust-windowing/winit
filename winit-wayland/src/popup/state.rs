use sctk::shell::xdg::popup::PopupHandler;
use wayland_protocols::wp::fractional_scale::v1::client::wp_fractional_scale_v1::WpFractionalScaleV1;

#[derive(Default, Debug)]
pub struct State {
    /// The current window title.
    title: String,

    scale_factor: f64,
    fractional_scale: Option<WpFractionalScaleV1>,
}

impl State {
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
    pub fn scale_factor(&self) -> f64 {
        self.scale_factor
    }
}
