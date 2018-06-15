
//! DPI is important, so read the docs for this module if you don't want to be confused.
//!
//! Originally, `winit` dealt entirely in physical pixels (excluding unintentional inconsistencies), but now all
//! window-related functions both produce and consume logical pixels. Monitor-related functions still use physical
//! pixels, as do any context-related functions in `glutin`.
//!
//! If you've never heard of these terms before, then you're not alone, and this documentation will explain the
//! concepts.
//!
//! Modern screens have a defined physical resolution, most commonly 1920x1080. Indepedent of that is the amount of
//! space the screen occupies, which is to say, the height and width in millimeters. The relationship between these two
//! measurements is the *pixel density*. Mobile screens require a high pixel density, as they're held close to the
//! eyes. Larger displays also require a higher pixel density, hence the growing presence of 1440p and 4K displays.
//!
//! So, this presents a problem. Let's say we want to render a square 100px button. It will occupy 100x100 of the
//! screen's pixels, which in many cases, seems perfectly fine. However, because this size doesn't account for the
//! screen's dimensions or pixel density, the button's size can vary quite a bit. On a 4K display, it would be unusably
//! small.
//!
//! That's a description of what happens when the button is 100x100 *physical* pixels. Instead, let's try using 100x100
//! *logical* pixels. To map logical pixels to physical pixels, we simply multiply by the DPI factor. On a "typical"
//! desktop display, the DPI factor will be 1.0, so 100x100 logical pixels equates to 100x100 physical pixels. However,
//! a 1440p display may have a DPI factor of 1.25, so the button is rendered as 125x125 physical pixels. Ideally, the
//! button now has approximately the same perceived size across varying displays.
//!
//! Failure to account for the DPI factor can create a badly degraded user experience. Most notably, it can make users
//! feel like they have bad eyesight, which will potentially cause them to think about growing elderly, resulting in
//! them entering an existential panic. Once users enter that state, they will no longer be focused on your application.
//!
//! There are two ways to get the DPI factor: either by calling
//! [`MonitorId::get_hidpi_factor`](../struct.MonitorId.html#method.get_hidpi_factor), or
//! [`Window::get_hidpi_factor`](../struct.Window.html#method.get_hidpi_factor). You'll almost always use the latter,
//! which is basically equivalent to `window.get_current_monitor().get_hidpi_factor()` anyway.
//!
//! Here's an overview of what sort of DPI factors you can expect, and where they come from:
//! - **Windows:** On Windows 8 and 10, per-monitor scaling is readily configured by users from the display settings.
//! While users are free to select any option they want, they're only given a selection of "nice" DPI factors, i.e.
//! 1.0, 1.25, 1.5... on Windows 7, the DPI factor is global and changing it requires logging out.
//! - **macOS:** The buzzword is "retina displays", which have a DPI factor of 2.0. Otherwise, the DPI factor is 1.0.
//! Intermediate DPI factors are never used, thus 1440p displays/etc. aren't properly supported. It's possible for any
//! display to use that 2.0 DPI factor, given the use of the command line.
//! - **X11:** On X11, we calcuate the DPI factor based on the millimeter dimensions provided by XRandR. This can
//! result in a wide range of possible values, including some interesting ones like 1.0833333333333333. This can be
//! overridden using the `WINIT_HIDPI_FACTOR` environment variable, though that's not recommended.
//! - **Wayland:** On Wayland, DPI factors are very much at the discretion of the user.
//! - **iOS:** DPI factors are both constant and device-specific on iOS.
//! - **Android:** This feature isn't yet implemented on Android, so the DPI factor will always be returned as 1.0.
//!
//! The window's logical size is conserved across DPI changes, resulting in the physical size changing instead. This
//! may be surprising on X11, but is quite standard elsewhere. Physical size changes produce a
//! [`Resized`](../enum.WindowEvent.html#variant.Resized) event, even on platforms where no resize actually occurs,
//! such as macOS and Wayland. As a result, it's not necessary to separately handle
//! [`HiDpiFactorChanged`](../enum.WindowEvent.html#variant.HiDpiFactorChanged) if you're only listening for size.
//!
//! Your GPU has no awareness of the concept of logical pixels, and unless you like wasting pixel density, your
//! framebuffer's size should be in physical pixels.

/// Checks that the DPI factor is a normal positive `f64`.
///
/// All functions that take a DPI factor assert that this will return `true`. If you're sourcing DPI factors from
/// anywhere other than winit, it's recommended to validate them using this function before passing them to winit;
/// otherwise, you risk panics.
#[inline]
pub fn validate_hidpi_factor(dpi_factor: f64) -> bool {
    dpi_factor.is_sign_positive() && dpi_factor.is_normal()
}

/// A position represented in logical pixels.
///
/// The position is stored as floats, so please be careful. Casting floats to integers truncates the fractional part,
/// which can cause noticable issues. To help with that, an `Into<(i32, i32)>` implementation is provided which
/// does the rounding for you.
#[derive(Debug, Copy, Clone, PartialEq)]
pub struct LogicalPosition {
    pub x: f64,
    pub y: f64,
}

impl LogicalPosition {
    #[inline]
    pub fn new(x: f64, y: f64) -> Self {
        LogicalPosition { x, y }
    }

    #[inline]
    pub fn from_physical<T: Into<PhysicalPosition>>(physical: T, dpi_factor: f64) -> Self {
        physical.into().to_logical(dpi_factor)
    }

    #[inline]
    pub fn to_physical(&self, dpi_factor: f64) -> PhysicalPosition {
        assert!(validate_hidpi_factor(dpi_factor));
        let x = self.x * dpi_factor;
        let y = self.y * dpi_factor;
        PhysicalPosition::new(x, y)
    }
}

impl From<(f64, f64)> for LogicalPosition {
    #[inline]
    fn from((x, y): (f64, f64)) -> Self {
        Self::new(x, y)
    }
}

impl From<(i32, i32)> for LogicalPosition {
    #[inline]
    fn from((x, y): (i32, i32)) -> Self {
        Self::new(x as f64, y as f64)
    }
}

impl Into<(f64, f64)> for LogicalPosition {
    #[inline]
    fn into(self) -> (f64, f64) {
        (self.x, self.y)
    }
}

impl Into<(i32, i32)> for LogicalPosition {
    /// Note that this rounds instead of truncating.
    #[inline]
    fn into(self) -> (i32, i32) {
        (self.x.round() as _, self.y.round() as _)
    }
}

/// A position represented in physical pixels.
///
/// The position is stored as floats, so please be careful. Casting floats to integers truncates the fractional part,
/// which can cause noticable issues. To help with that, an `Into<(i32, i32)>` implementation is provided which
/// does the rounding for you.
#[derive(Debug, Copy, Clone, PartialEq)]
pub struct PhysicalPosition {
    pub x: f64,
    pub y: f64,
}

impl PhysicalPosition {
    #[inline]
    pub fn new(x: f64, y: f64) -> Self {
        PhysicalPosition { x, y }
    }

    #[inline]
    pub fn from_logical<T: Into<LogicalPosition>>(logical: T, dpi_factor: f64) -> Self {
        logical.into().to_physical(dpi_factor)
    }

    #[inline]
    pub fn to_logical(&self, dpi_factor: f64) -> LogicalPosition {
        assert!(validate_hidpi_factor(dpi_factor));
        let x = self.x / dpi_factor;
        let y = self.y / dpi_factor;
        LogicalPosition::new(x, y)
    }
}

impl From<(f64, f64)> for PhysicalPosition {
    #[inline]
    fn from((x, y): (f64, f64)) -> Self {
        Self::new(x, y)
    }
}

impl From<(i32, i32)> for PhysicalPosition {
    #[inline]
    fn from((x, y): (i32, i32)) -> Self {
        Self::new(x as f64, y as f64)
    }
}

impl Into<(f64, f64)> for PhysicalPosition {
    #[inline]
    fn into(self) -> (f64, f64) {
        (self.x, self.y)
    }
}

impl Into<(i32, i32)> for PhysicalPosition {
    /// Note that this rounds instead of truncating.
    #[inline]
    fn into(self) -> (i32, i32) {
        (self.x.round() as _, self.y.round() as _)
    }
}

/// A size represented in logical pixels.
///
/// The size is stored as floats, so please be careful. Casting floats to integers truncates the fractional part,
/// which can cause noticable issues. To help with that, an `Into<(u32, u32)>` implementation is provided which
/// does the rounding for you.
#[derive(Debug, Copy, Clone, PartialEq)]
pub struct LogicalSize {
    pub width: f64,
    pub height: f64,
}

impl LogicalSize {
    #[inline]
    pub fn new(width: f64, height: f64) -> Self {
        LogicalSize { width, height }
    }

    #[inline]
    pub fn from_physical<T: Into<PhysicalSize>>(physical: T, dpi_factor: f64) -> Self {
        physical.into().to_logical(dpi_factor)
    }

    #[inline]
    pub fn to_physical(&self, dpi_factor: f64) -> PhysicalSize {
        assert!(validate_hidpi_factor(dpi_factor));
        let width = self.width * dpi_factor;
        let height = self.height * dpi_factor;
        PhysicalSize::new(width, height)
    }
}

impl From<(f64, f64)> for LogicalSize {
    #[inline]
    fn from((width, height): (f64, f64)) -> Self {
        Self::new(width, height)
    }
}

impl From<(u32, u32)> for LogicalSize {
    #[inline]
    fn from((width, height): (u32, u32)) -> Self {
        Self::new(width as f64, height as f64)
    }
}

impl Into<(f64, f64)> for LogicalSize {
    #[inline]
    fn into(self) -> (f64, f64) {
        (self.width, self.height)
    }
}

impl Into<(u32, u32)> for LogicalSize {
    /// Note that this rounds instead of truncating.
    #[inline]
    fn into(self) -> (u32, u32) {
        (self.width.round() as _, self.height.round() as _)
    }
}

/// A size represented in physical pixels.
///
/// The size is stored as floats, so please be careful. Casting floats to integers truncates the fractional part,
/// which can cause noticable issues. To help with that, an `Into<(u32, u32)>` implementation is provided which
/// does the rounding for you.
#[derive(Debug, Copy, Clone, PartialEq)]
pub struct PhysicalSize {
    pub width: f64,
    pub height: f64,
}

impl PhysicalSize {
    #[inline]
    pub fn new(width: f64, height: f64) -> Self {
        PhysicalSize { width, height }
    }

    #[inline]
    pub fn from_logical<T: Into<LogicalSize>>(logical: T, dpi_factor: f64) -> Self {
        logical.into().to_physical(dpi_factor)
    }

    #[inline]
    pub fn to_logical(&self, dpi_factor: f64) -> LogicalSize {
        assert!(validate_hidpi_factor(dpi_factor));
        let width = self.width / dpi_factor;
        let height = self.height / dpi_factor;
        LogicalSize::new(width, height)
    }
}

impl From<(f64, f64)> for PhysicalSize {
    #[inline]
    fn from((width, height): (f64, f64)) -> Self {
        Self::new(width, height)
    }
}

impl From<(u32, u32)> for PhysicalSize {
    #[inline]
    fn from((width, height): (u32, u32)) -> Self {
        Self::new(width as f64, height as f64)
    }
}

impl Into<(f64, f64)> for PhysicalSize {
    #[inline]
    fn into(self) -> (f64, f64) {
        (self.width, self.height)
    }
}

impl Into<(u32, u32)> for PhysicalSize {
    /// Note that this rounds instead of truncating.
    #[inline]
    fn into(self) -> (u32, u32) {
        (self.width.round() as _, self.height.round() as _)
    }
}
