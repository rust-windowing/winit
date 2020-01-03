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
//! *logical* pixels. To map logical pixels to physical pixels, we simply multiply by the DPI (dots per inch) factor.
//! On a "typical" desktop display, the DPI factor will be 1.0, so 100x100 logical pixels equates to 100x100 physical
//! pixels. However, a 1440p display may have a DPI factor of 1.25, so the button is rendered as 125x125 physical pixels.
//! Ideally, the button now has approximately the same perceived size across varying displays.
//!
//! Failure to account for the DPI factor can create a badly degraded user experience. Most notably, it can make users
//! feel like they have bad eyesight, which will potentially cause them to think about growing elderly, resulting in
//! them entering an existential panic. Once users enter that state, they will no longer be focused on your application.
//!
//! There are two ways to get the DPI factor:
//! - You can track the [`HiDpiFactorChanged`](crate::event::WindowEvent::HiDpiFactorChanged) event of your
//!   windows. This event is sent any time the DPI factor changes, either because the window moved to another monitor,
//!   or because the user changed the configuration of their screen.
//! - You can also retrieve the DPI factor of a monitor by calling
//!   [`MonitorHandle::hidpi_factor`](crate::monitor::MonitorHandle::hidpi_factor), or the
//!   current DPI factor applied to a window by calling
//!   [`Window::hidpi_factor`](crate::window::Window::hidpi_factor), which is roughly equivalent
//!   to `window.current_monitor().hidpi_factor()`.
//!
//! Depending on the platform, the window's actual DPI factor may only be known after
//! the event loop has started and your window has been drawn once. To properly handle these cases,
//! the most robust way is to monitor the [`HiDpiFactorChanged`](crate::event::WindowEvent::HiDpiFactorChanged)
//! event and dynamically adapt your drawing logic to follow the DPI factor.
//!
//! Here's an overview of what sort of DPI factors you can expect, and where they come from:
//! - **Windows:** On Windows 8 and 10, per-monitor scaling is readily configured by users from the display settings.
//! While users are free to select any option they want, they're only given a selection of "nice" DPI factors, i.e.
//! 1.0, 1.25, 1.5... on Windows 7, the DPI factor is global and changing it requires logging out.
//! - **macOS:** The buzzword is "retina displays", which have a DPI factor of 2.0. Otherwise, the DPI factor is 1.0.
//! Intermediate DPI factors are never used, thus 1440p displays/etc. aren't properly supported. It's possible for any
//! display to use that 2.0 DPI factor, given the use of the command line.
//! - **X11:** On X11, we calculate the DPI factor based on the millimeter dimensions provided by XRandR. This can
//! result in a wide range of possible values, including some interesting ones like 1.0833333333333333. This can be
//! overridden using the `WINIT_HIDPI_FACTOR` environment variable, though that's not recommended.
//! - **Wayland:** On Wayland, DPI factors are set per-screen by the server, and are always integers (most often 1 or 2).
//! - **iOS:** DPI factors are both constant and device-specific on iOS.
//! - **Android:** This feature isn't yet implemented on Android, so the DPI factor will always be returned as 1.0.
//! - **Web:** DPI factors are handled by the browser and will always be 1.0 for your application.
//!
//! The window's logical size is conserved across DPI changes, resulting in the physical size changing instead. This
//! may be surprising on X11, but is quite standard elsewhere. Physical size changes always produce a
//! [`Resized`](crate::event::WindowEvent::Resized) event, even on platforms where no resize actually occurs,
//! such as macOS and Wayland. As a result, it's not necessary to separately handle
//! [`HiDpiFactorChanged`](crate::event::WindowEvent::HiDpiFactorChanged) if you're only listening for size.
//!
//! Your GPU has no awareness of the concept of logical pixels, and unless you like wasting pixel density, your
//! framebuffer's size should be in physical pixels.
//!
//! `winit` will send [`Resized`](crate::event::WindowEvent::Resized) events whenever a window's logical size
//! changes, and [`HiDpiFactorChanged`](crate::event::WindowEvent::HiDpiFactorChanged) events
//! whenever the DPI factor changes. Receiving either of these events means that the physical size of your window has
//! changed, and you should recompute it using the latest values you received for each. If the logical size and the
//! DPI factor change simultaneously, `winit` will send both events together; thus, it's recommended to buffer
//! these events and process them at the end of the queue.
//!
//! If you never received any [`HiDpiFactorChanged`](crate::event::WindowEvent::HiDpiFactorChanged) events,
//! then your window's DPI factor is 1.

pub trait Pixel: Copy + Into<f64> {
    fn from_f64(f: f64) -> Self;
    fn cast<P: Pixel>(self) -> P {
        P::from_f64(self.into())
    }
}

impl Pixel for u8 {
    fn from_f64(f: f64) -> Self {
        f.round() as u8
    }
}
impl Pixel for u16 {
    fn from_f64(f: f64) -> Self {
        f.round() as u16
    }
}
impl Pixel for u32 {
    fn from_f64(f: f64) -> Self {
        f.round() as u32
    }
}
impl Pixel for i8 {
    fn from_f64(f: f64) -> Self {
        f.round() as i8
    }
}
impl Pixel for i16 {
    fn from_f64(f: f64) -> Self {
        f.round() as i16
    }
}
impl Pixel for i32 {
    fn from_f64(f: f64) -> Self {
        f.round() as i32
    }
}
impl Pixel for f32 {
    fn from_f64(f: f64) -> Self {
        f as f32
    }
}
impl Pixel for f64 {
    fn from_f64(f: f64) -> Self {
        f
    }
}

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
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct LogicalPosition<P> {
    pub x: P,
    pub y: P,
}

impl<P> LogicalPosition<P> {
    #[inline]
    pub const fn new(x: P, y: P) -> Self {
        LogicalPosition { x, y }
    }
}

impl<P: Pixel> LogicalPosition<P> {
    #[inline]
    pub fn from_physical<T: Into<PhysicalPosition<X>>, X: Pixel>(
        physical: T,
        dpi_factor: f64,
    ) -> Self {
        physical.into().to_logical(dpi_factor)
    }

    #[inline]
    pub fn to_physical<X: Pixel>(&self, dpi_factor: f64) -> PhysicalPosition<X> {
        assert!(validate_hidpi_factor(dpi_factor));
        let x = self.x.into() * dpi_factor;
        let y = self.y.into() * dpi_factor;
        PhysicalPosition::new(x, y).cast()
    }

    #[inline]
    pub fn cast<X: Pixel>(&self) -> LogicalPosition<X> {
        LogicalPosition {
            x: self.x.cast(),
            y: self.y.cast(),
        }
    }
}

impl<P: Pixel, X: Pixel> From<(X, X)> for LogicalPosition<P> {
    fn from((x, y): (X, X)) -> LogicalPosition<P> {
        LogicalPosition::new(x.cast(), y.cast())
    }
}

impl<P: Pixel, X: Pixel> Into<(X, X)> for LogicalPosition<P> {
    fn into(self: Self) -> (X, X) {
        (self.x.cast(), self.y.cast())
    }
}

impl<P: Pixel, X: Pixel> From<[X; 2]> for LogicalPosition<P> {
    fn from([x, y]: [X; 2]) -> LogicalPosition<P> {
        LogicalPosition::new(x.cast(), y.cast())
    }
}

impl<P: Pixel, X: Pixel> Into<[X; 2]> for LogicalPosition<P> {
    fn into(self: Self) -> [X; 2] {
        [self.x.cast(), self.y.cast()]
    }
}

/// A position represented in physical pixels.
///
/// The position is stored as floats, so please be careful. Casting floats to integers truncates the fractional part,
/// which can cause noticable issues. To help with that, an `Into<(i32, i32)>` implementation is provided which
/// does the rounding for you.
#[derive(Debug, Copy, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct PhysicalPosition<P> {
    pub x: P,
    pub y: P,
}

impl<P> PhysicalPosition<P> {
    #[inline]
    pub const fn new(x: P, y: P) -> Self {
        PhysicalPosition { x, y }
    }
}

impl<P: Pixel> PhysicalPosition<P> {
    #[inline]
    pub fn from_logical<T: Into<LogicalPosition<X>>, X: Pixel>(
        logical: T,
        dpi_factor: f64,
    ) -> Self {
        logical.into().to_physical(dpi_factor)
    }

    #[inline]
    pub fn to_logical<X: Pixel>(&self, dpi_factor: f64) -> LogicalPosition<X> {
        assert!(validate_hidpi_factor(dpi_factor));
        let x = self.x.into() / dpi_factor;
        let y = self.y.into() / dpi_factor;
        LogicalPosition::new(x, y).cast()
    }

    #[inline]
    pub fn cast<X: Pixel>(&self) -> PhysicalPosition<X> {
        PhysicalPosition {
            x: self.x.cast(),
            y: self.y.cast(),
        }
    }
}

impl<P: Pixel, X: Pixel> From<(X, X)> for PhysicalPosition<P> {
    fn from((x, y): (X, X)) -> PhysicalPosition<P> {
        PhysicalPosition::new(x.cast(), y.cast())
    }
}

impl<P: Pixel, X: Pixel> Into<(X, X)> for PhysicalPosition<P> {
    fn into(self: Self) -> (X, X) {
        (self.x.cast(), self.y.cast())
    }
}

impl<P: Pixel, X: Pixel> From<[X; 2]> for PhysicalPosition<P> {
    fn from([x, y]: [X; 2]) -> PhysicalPosition<P> {
        PhysicalPosition::new(x.cast(), y.cast())
    }
}

impl<P: Pixel, X: Pixel> Into<[X; 2]> for PhysicalPosition<P> {
    fn into(self: Self) -> [X; 2] {
        [self.x.cast(), self.y.cast()]
    }
}

/// A size represented in logical pixels.
///
/// The size is stored as floats, so please be careful. Casting floats to integers truncates the fractional part,
/// which can cause noticable issues. To help with that, an `Into<(u32, u32)>` implementation is provided which
/// does the rounding for you.
#[derive(Debug, Copy, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct LogicalSize<P> {
    pub width: P,
    pub height: P,
}

impl<P> LogicalSize<P> {
    #[inline]
    pub const fn new(width: P, height: P) -> Self {
        LogicalSize { width, height }
    }
}

impl<P: Pixel> LogicalSize<P> {
    #[inline]
    pub fn from_physical<T: Into<PhysicalSize<X>>, X: Pixel>(physical: T, dpi_factor: f64) -> Self {
        physical.into().to_logical(dpi_factor)
    }

    #[inline]
    pub fn to_physical<X: Pixel>(&self, dpi_factor: f64) -> PhysicalSize<X> {
        assert!(validate_hidpi_factor(dpi_factor));
        let width = self.width.into() * dpi_factor;
        let height = self.height.into() * dpi_factor;
        PhysicalSize::new(width, height).cast()
    }

    #[inline]
    pub fn cast<X: Pixel>(&self) -> LogicalSize<X> {
        LogicalSize {
            width: self.width.cast(),
            height: self.height.cast(),
        }
    }
}

impl<P: Pixel, X: Pixel> From<(X, X)> for LogicalSize<P> {
    fn from((x, y): (X, X)) -> LogicalSize<P> {
        LogicalSize::new(x.cast(), y.cast())
    }
}

impl<P: Pixel, X: Pixel> Into<(X, X)> for LogicalSize<P> {
    fn into(self: LogicalSize<P>) -> (X, X) {
        (self.width.cast(), self.height.cast())
    }
}

impl<P: Pixel, X: Pixel> From<[X; 2]> for LogicalSize<P> {
    fn from([x, y]: [X; 2]) -> LogicalSize<P> {
        LogicalSize::new(x.cast(), y.cast())
    }
}

impl<P: Pixel, X: Pixel> Into<[X; 2]> for LogicalSize<P> {
    fn into(self: Self) -> [X; 2] {
        [self.width.cast(), self.height.cast()]
    }
}

/// A size represented in physical pixels.
#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct PhysicalSize<P> {
    pub width: P,
    pub height: P,
}

impl<P> PhysicalSize<P> {
    #[inline]
    pub const fn new(width: P, height: P) -> Self {
        PhysicalSize { width, height }
    }
}

impl<P: Pixel> PhysicalSize<P> {
    #[inline]
    pub fn from_logical<T: Into<LogicalSize<X>>, X: Pixel>(logical: T, dpi_factor: f64) -> Self {
        logical.into().to_physical(dpi_factor)
    }

    #[inline]
    pub fn to_logical<X: Pixel>(&self, dpi_factor: f64) -> LogicalSize<X> {
        assert!(validate_hidpi_factor(dpi_factor));
        let width = self.width.into() / dpi_factor;
        let height = self.height.into() / dpi_factor;
        LogicalSize::new(width, height).cast()
    }

    #[inline]
    pub fn cast<X: Pixel>(&self) -> PhysicalSize<X> {
        PhysicalSize {
            width: self.width.cast(),
            height: self.height.cast(),
        }
    }
}

impl<P: Pixel, X: Pixel> From<(X, X)> for PhysicalSize<P> {
    fn from((x, y): (X, X)) -> PhysicalSize<P> {
        PhysicalSize::new(x.cast(), y.cast())
    }
}

impl<P: Pixel, X: Pixel> Into<(X, X)> for PhysicalSize<P> {
    fn into(self: Self) -> (X, X) {
        (self.width.cast(), self.height.cast())
    }
}

impl<P: Pixel, X: Pixel> From<[X; 2]> for PhysicalSize<P> {
    fn from([x, y]: [X; 2]) -> PhysicalSize<P> {
        PhysicalSize::new(x.cast(), y.cast())
    }
}

impl<P: Pixel, X: Pixel> Into<[X; 2]> for PhysicalSize<P> {
    fn into(self: Self) -> [X; 2] {
        [self.width.cast(), self.height.cast()]
    }
}

#[derive(Debug, Copy, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub enum Size {
    Physical(PhysicalSize<u32>),
    Logical(LogicalSize<f64>),
}

impl Size {
    pub fn new<S: Into<Size>>(size: S) -> Size {
        size.into()
    }

    pub fn to_logical<P: Pixel>(&self, dpi_factor: f64) -> LogicalSize<P> {
        match *self {
            Size::Physical(size) => size.to_logical(dpi_factor),
            Size::Logical(size) => size.cast(),
        }
    }

    pub fn to_physical<P: Pixel>(&self, dpi_factor: f64) -> PhysicalSize<P> {
        match *self {
            Size::Physical(size) => size.cast(),
            Size::Logical(size) => size.to_physical(dpi_factor),
        }
    }
}

impl<P: Pixel> From<PhysicalSize<P>> for Size {
    #[inline]
    fn from(size: PhysicalSize<P>) -> Size {
        Size::Physical(size.cast())
    }
}

impl<P: Pixel> From<LogicalSize<P>> for Size {
    #[inline]
    fn from(size: LogicalSize<P>) -> Size {
        Size::Logical(size.cast())
    }
}

#[derive(Debug, Copy, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub enum Position {
    Physical(PhysicalPosition<i32>),
    Logical(LogicalPosition<f64>),
}

impl Position {
    pub fn new<S: Into<Position>>(position: S) -> Position {
        position.into()
    }

    pub fn to_logical<P: Pixel>(&self, dpi_factor: f64) -> LogicalPosition<P> {
        match *self {
            Position::Physical(position) => position.to_logical(dpi_factor),
            Position::Logical(position) => position.cast(),
        }
    }

    pub fn to_physical<P: Pixel>(&self, dpi_factor: f64) -> PhysicalPosition<P> {
        match *self {
            Position::Physical(position) => position.cast(),
            Position::Logical(position) => position.to_physical(dpi_factor),
        }
    }
}

impl<P: Pixel> From<PhysicalPosition<P>> for Position {
    #[inline]
    fn from(position: PhysicalPosition<P>) -> Position {
        Position::Physical(position.cast())
    }
}

impl<P: Pixel> From<LogicalPosition<P>> for Position {
    #[inline]
    fn from(position: LogicalPosition<P>) -> Position {
        Position::Logical(position.cast())
    }
}
