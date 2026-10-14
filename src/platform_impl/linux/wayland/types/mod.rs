//! Wayland protocol implementation boilerplate.

pub mod cursor;
pub mod kwin_blur;
pub mod wp_fractional_scaling;
pub mod wp_viewporter;
pub mod xdg_activation;

#[cfg(target_os = "haiku")]
pub mod haiku_decoration;
