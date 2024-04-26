//! # DPI
//!
//! ## Why should I care about UI scaling?
//!
//! Modern computer screens don't have a consistent relationship between resolution and size.
//! 1920x1080 is a common resolution for both desktop and mobile screens, despite mobile screens
//! typically being less than a quarter the size of their desktop counterparts. Moreover, neither
//! desktop nor mobile screens have consistent resolutions within their own size classes - common
//! mobile screens range from below 720p to above 1440p, and desktop screens range from 720p to 5K
//! and beyond.
//!
//! Given that, it's a mistake to assume that 2D content will only be displayed on screens with
//! a consistent pixel density. If you were to render a 96-pixel-square image on a 1080p screen and
//! then render the same image on a similarly-sized 4K screen, the 4K rendition would only take up
//! about a quarter of the physical space as it did on the 1080p screen. That issue is especially
//! problematic with text rendering, where quarter-sized text becomes a significant legibility
//! problem.
//!
//! Failure to account for the scale factor can create a significantly degraded user experience.
//! Most notably, it can make users feel like they have bad eyesight, which will potentially cause
//! them to think about growing elderly, resulting in them having an existential crisis. Once users
//! enter that state, they will no longer be focused on your application.
//!
//! ## How should I handle it?
//!
//! The solution to this problem is to account for the device's *scale factor*. The scale factor is
//! the factor UI elements should be scaled by to be consistent with the rest of the user's system -
//! for example, a button that's usually 50 pixels across would be 100 pixels across on a device
//! with a scale factor of `2.0`, or 75 pixels across with a scale factor of `1.5`.
//!
//! Many UI systems, such as CSS, expose DPI-dependent units like [points] or [picas]. That's
//! usually a mistake since there's no consistent mapping between the scale factor and the screen's
//! actual DPI. Unless printing to a physical medium, you should work in scaled pixels rather
//! than any DPI-dependent units.
//!
//! ### Position and Size types
//!
//! The [`PhysicalPosition`] / [`PhysicalSize`] / [`PhysicalUnit`] types correspond with the actual
//! pixels on the device, and the [`LogicalPosition`] / [`LogicalSize`] / [`LogicalUnit`] types
//! correspond to the physical pixels divided by the scale factor.
//!
//! The position and size types are generic over their exact pixel type, `P`, to allow the
//! API to have integer precision where appropriate (e.g. most window manipulation functions) and
//! floating precision when necessary (e.g. logical sizes for fractional scale factors and touch
//! input). If `P` is a floating-point type, please do not cast the values with `as {int}`. Doing so
//! will truncate the fractional part of the float rather than properly round to the nearest
//! integer. Use the provided `cast` function or [`From`]/[`Into`] conversions, which handle the
//! rounding properly. Note that precision loss will still occur when rounding from a float to an
//! int, although rounding lessens the problem.
//!
//! ## Cargo Features
//!
//! This crate provides the following Cargo features:
//!
//! * `serde`: Enables serialization/deserialization of certain types with [Serde](https://crates.io/crates/serde).
//! * `mint`: Enables mint (math interoperability standard types) conversions.
//!
//!
//! [points]: https://en.wikipedia.org/wiki/Point_(typography)
//! [picas]: https://en.wikipedia.org/wiki/Pica_(typography)

#![cfg_attr(docsrs, feature(doc_auto_cfg, doc_cfg_hide), doc(cfg_hide(doc, docsrs)))]
#![forbid(unsafe_code)]

#[cfg(feature = "serde")]
use serde::{Deserialize, Serialize};

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

/// Checks that the scale factor is a normal positive `f64`.
///
/// All functions that take a scale factor assert that this will return `true`. If you're sourcing
/// scale factors from anywhere other than winit, it's recommended to validate them using this
/// function before passing them to winit; otherwise, you risk panics.
#[inline]
pub fn validate_scale_factor(scale_factor: f64) -> bool {
    scale_factor.is_sign_positive() && scale_factor.is_normal()
}

/// A logical pixel unit.
#[derive(Debug, Copy, Clone, Eq, PartialEq, Ord, PartialOrd, Default, Hash)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct LogicalUnit<P>(pub P);

impl<P> LogicalUnit<P> {
    /// Represents a maximum logical unit that is equal to [`f64::MAX`].
    pub const MAX: LogicalUnit<f64> = LogicalUnit::new(f64::MAX);
    /// Represents a minimum logical unit of [`f64::MAX`].
    pub const MIN: LogicalUnit<f64> = LogicalUnit::new(f64::MIN);
    /// Represents a logical unit of `0_f64`.
    pub const ZERO: LogicalUnit<f64> = LogicalUnit::new(0.0);

    #[inline]
    pub const fn new(v: P) -> Self {
        LogicalUnit(v)
    }
}

impl<P: Pixel> LogicalUnit<P> {
    #[inline]
    pub fn from_physical<T: Into<PhysicalUnit<X>>, X: Pixel>(
        physical: T,
        scale_factor: f64,
    ) -> Self {
        physical.into().to_logical(scale_factor)
    }

    #[inline]
    pub fn to_physical<X: Pixel>(&self, scale_factor: f64) -> PhysicalUnit<X> {
        assert!(validate_scale_factor(scale_factor));
        PhysicalUnit::new(self.0.into() * scale_factor).cast()
    }

    #[inline]
    pub fn cast<X: Pixel>(&self) -> LogicalUnit<X> {
        LogicalUnit(self.0.cast())
    }
}

impl<P: Pixel, X: Pixel> From<X> for LogicalUnit<P> {
    fn from(v: X) -> LogicalUnit<P> {
        LogicalUnit::new(v.cast())
    }
}

impl<P: Pixel> From<LogicalUnit<P>> for u8 {
    fn from(v: LogicalUnit<P>) -> u8 {
        v.0.cast()
    }
}

impl<P: Pixel> From<LogicalUnit<P>> for u16 {
    fn from(v: LogicalUnit<P>) -> u16 {
        v.0.cast()
    }
}

impl<P: Pixel> From<LogicalUnit<P>> for u32 {
    fn from(v: LogicalUnit<P>) -> u32 {
        v.0.cast()
    }
}

impl<P: Pixel> From<LogicalUnit<P>> for i8 {
    fn from(v: LogicalUnit<P>) -> i8 {
        v.0.cast()
    }
}

impl<P: Pixel> From<LogicalUnit<P>> for i16 {
    fn from(v: LogicalUnit<P>) -> i16 {
        v.0.cast()
    }
}

impl<P: Pixel> From<LogicalUnit<P>> for i32 {
    fn from(v: LogicalUnit<P>) -> i32 {
        v.0.cast()
    }
}

impl<P: Pixel> From<LogicalUnit<P>> for f32 {
    fn from(v: LogicalUnit<P>) -> f32 {
        v.0.cast()
    }
}

impl<P: Pixel> From<LogicalUnit<P>> for f64 {
    fn from(v: LogicalUnit<P>) -> f64 {
        v.0.cast()
    }
}

/// A physical pixel unit.
#[derive(Debug, Copy, Clone, Eq, PartialEq, Ord, PartialOrd, Default, Hash)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct PhysicalUnit<P>(pub P);

impl<P> PhysicalUnit<P> {
    /// Represents a maximum physical unit that is equal to [`f64::MAX`].
    pub const MAX: LogicalUnit<f64> = LogicalUnit::new(f64::MAX);
    /// Represents a minimum physical unit of [`f64::MAX`].
    pub const MIN: LogicalUnit<f64> = LogicalUnit::new(f64::MIN);
    /// Represents a physical unit of `0_f64`.
    pub const ZERO: LogicalUnit<f64> = LogicalUnit::new(0.0);

    #[inline]
    pub const fn new(v: P) -> Self {
        PhysicalUnit(v)
    }
}

impl<P: Pixel> PhysicalUnit<P> {
    #[inline]
    pub fn from_logical<T: Into<LogicalUnit<X>>, X: Pixel>(logical: T, scale_factor: f64) -> Self {
        logical.into().to_physical(scale_factor)
    }

    #[inline]
    pub fn to_logical<X: Pixel>(&self, scale_factor: f64) -> LogicalUnit<X> {
        assert!(validate_scale_factor(scale_factor));
        LogicalUnit::new(self.0.into() / scale_factor).cast()
    }

    #[inline]
    pub fn cast<X: Pixel>(&self) -> PhysicalUnit<X> {
        PhysicalUnit(self.0.cast())
    }
}

impl<P: Pixel, X: Pixel> From<X> for PhysicalUnit<P> {
    fn from(v: X) -> PhysicalUnit<P> {
        PhysicalUnit::new(v.cast())
    }
}

impl<P: Pixel> From<PhysicalUnit<P>> for u8 {
    fn from(v: PhysicalUnit<P>) -> u8 {
        v.0.cast()
    }
}

impl<P: Pixel> From<PhysicalUnit<P>> for u16 {
    fn from(v: PhysicalUnit<P>) -> u16 {
        v.0.cast()
    }
}

impl<P: Pixel> From<PhysicalUnit<P>> for u32 {
    fn from(v: PhysicalUnit<P>) -> u32 {
        v.0.cast()
    }
}

impl<P: Pixel> From<PhysicalUnit<P>> for i8 {
    fn from(v: PhysicalUnit<P>) -> i8 {
        v.0.cast()
    }
}

impl<P: Pixel> From<PhysicalUnit<P>> for i16 {
    fn from(v: PhysicalUnit<P>) -> i16 {
        v.0.cast()
    }
}

impl<P: Pixel> From<PhysicalUnit<P>> for i32 {
    fn from(v: PhysicalUnit<P>) -> i32 {
        v.0.cast()
    }
}

impl<P: Pixel> From<PhysicalUnit<P>> for f32 {
    fn from(v: PhysicalUnit<P>) -> f32 {
        v.0.cast()
    }
}

impl<P: Pixel> From<PhysicalUnit<P>> for f64 {
    fn from(v: PhysicalUnit<P>) -> f64 {
        v.0.cast()
    }
}

/// A pixel unit that's either physical or logical.
#[derive(Debug, Copy, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub enum PixelUnit {
    Physical(PhysicalUnit<i32>),
    Logical(LogicalUnit<f64>),
}

impl PixelUnit {
    /// Represents a maximum logical unit that is equal to [`f64::MAX`].
    pub const MAX: PixelUnit = PixelUnit::Logical(LogicalUnit::new(f64::MAX));
    /// Represents a minimum logical unit of [`f64::MAX`].
    pub const MIN: PixelUnit = PixelUnit::Logical(LogicalUnit::new(f64::MIN));
    /// Represents a logical unit of `0_f64`.
    pub const ZERO: PixelUnit = PixelUnit::Logical(LogicalUnit::new(0.0));

    pub fn new<S: Into<PixelUnit>>(unit: S) -> PixelUnit {
        unit.into()
    }

    pub fn to_logical<P: Pixel>(&self, scale_factor: f64) -> LogicalUnit<P> {
        match *self {
            PixelUnit::Physical(unit) => unit.to_logical(scale_factor),
            PixelUnit::Logical(unit) => unit.cast(),
        }
    }

    pub fn to_physical<P: Pixel>(&self, scale_factor: f64) -> PhysicalUnit<P> {
        match *self {
            PixelUnit::Physical(unit) => unit.cast(),
            PixelUnit::Logical(unit) => unit.to_physical(scale_factor),
        }
    }
}

impl<P: Pixel> From<PhysicalUnit<P>> for PixelUnit {
    #[inline]
    fn from(unit: PhysicalUnit<P>) -> PixelUnit {
        PixelUnit::Physical(unit.cast())
    }
}

impl<P: Pixel> From<LogicalUnit<P>> for PixelUnit {
    #[inline]
    fn from(unit: LogicalUnit<P>) -> PixelUnit {
        PixelUnit::Logical(unit.cast())
    }
}

/// A position represented in logical pixels.
///
/// The position is stored as floats, so please be careful. Casting floats to integers truncates the
/// fractional part, which can cause noticeable issues. To help with that, an `Into<(i32, i32)>`
/// implementation is provided which does the rounding for you.
#[derive(Debug, Copy, Clone, Eq, PartialEq, Ord, PartialOrd, Default, Hash)]
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
        scale_factor: f64,
    ) -> Self {
        physical.into().to_logical(scale_factor)
    }

    #[inline]
    pub fn to_physical<X: Pixel>(&self, scale_factor: f64) -> PhysicalPosition<X> {
        assert!(validate_scale_factor(scale_factor));
        let x = self.x.into() * scale_factor;
        let y = self.y.into() * scale_factor;
        PhysicalPosition::new(x, y).cast()
    }

    #[inline]
    pub fn cast<X: Pixel>(&self) -> LogicalPosition<X> {
        LogicalPosition { x: self.x.cast(), y: self.y.cast() }
    }
}

impl<P: Pixel, X: Pixel> From<(X, X)> for LogicalPosition<P> {
    fn from((x, y): (X, X)) -> LogicalPosition<P> {
        LogicalPosition::new(x.cast(), y.cast())
    }
}

impl<P: Pixel, X: Pixel> From<LogicalPosition<P>> for (X, X) {
    fn from(p: LogicalPosition<P>) -> (X, X) {
        (p.x.cast(), p.y.cast())
    }
}

impl<P: Pixel, X: Pixel> From<[X; 2]> for LogicalPosition<P> {
    fn from([x, y]: [X; 2]) -> LogicalPosition<P> {
        LogicalPosition::new(x.cast(), y.cast())
    }
}

impl<P: Pixel, X: Pixel> From<LogicalPosition<P>> for [X; 2] {
    fn from(p: LogicalPosition<P>) -> [X; 2] {
        [p.x.cast(), p.y.cast()]
    }
}

#[cfg(feature = "mint")]
impl<P: Pixel> From<mint::Point2<P>> for LogicalPosition<P> {
    fn from(p: mint::Point2<P>) -> Self {
        Self::new(p.x, p.y)
    }
}

#[cfg(feature = "mint")]
impl<P: Pixel> From<LogicalPosition<P>> for mint::Point2<P> {
    fn from(p: LogicalPosition<P>) -> Self {
        mint::Point2 { x: p.x, y: p.y }
    }
}

/// A position represented in physical pixels.
#[derive(Debug, Copy, Clone, Eq, PartialEq, Ord, PartialOrd, Default, Hash)]
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
        scale_factor: f64,
    ) -> Self {
        logical.into().to_physical(scale_factor)
    }

    #[inline]
    pub fn to_logical<X: Pixel>(&self, scale_factor: f64) -> LogicalPosition<X> {
        assert!(validate_scale_factor(scale_factor));
        let x = self.x.into() / scale_factor;
        let y = self.y.into() / scale_factor;
        LogicalPosition::new(x, y).cast()
    }

    #[inline]
    pub fn cast<X: Pixel>(&self) -> PhysicalPosition<X> {
        PhysicalPosition { x: self.x.cast(), y: self.y.cast() }
    }
}

impl<P: Pixel, X: Pixel> From<(X, X)> for PhysicalPosition<P> {
    fn from((x, y): (X, X)) -> PhysicalPosition<P> {
        PhysicalPosition::new(x.cast(), y.cast())
    }
}

impl<P: Pixel, X: Pixel> From<PhysicalPosition<P>> for (X, X) {
    fn from(p: PhysicalPosition<P>) -> (X, X) {
        (p.x.cast(), p.y.cast())
    }
}

impl<P: Pixel, X: Pixel> From<[X; 2]> for PhysicalPosition<P> {
    fn from([x, y]: [X; 2]) -> PhysicalPosition<P> {
        PhysicalPosition::new(x.cast(), y.cast())
    }
}

impl<P: Pixel, X: Pixel> From<PhysicalPosition<P>> for [X; 2] {
    fn from(p: PhysicalPosition<P>) -> [X; 2] {
        [p.x.cast(), p.y.cast()]
    }
}

#[cfg(feature = "mint")]
impl<P: Pixel> From<mint::Point2<P>> for PhysicalPosition<P> {
    fn from(p: mint::Point2<P>) -> Self {
        Self::new(p.x, p.y)
    }
}

#[cfg(feature = "mint")]
impl<P: Pixel> From<PhysicalPosition<P>> for mint::Point2<P> {
    fn from(p: PhysicalPosition<P>) -> Self {
        mint::Point2 { x: p.x, y: p.y }
    }
}

/// A size represented in logical pixels.
#[derive(Debug, Copy, Clone, Eq, PartialEq, Ord, PartialOrd, Default, Hash)]
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
    pub fn from_physical<T: Into<PhysicalSize<X>>, X: Pixel>(
        physical: T,
        scale_factor: f64,
    ) -> Self {
        physical.into().to_logical(scale_factor)
    }

    #[inline]
    pub fn to_physical<X: Pixel>(&self, scale_factor: f64) -> PhysicalSize<X> {
        assert!(validate_scale_factor(scale_factor));
        let width = self.width.into() * scale_factor;
        let height = self.height.into() * scale_factor;
        PhysicalSize::new(width, height).cast()
    }

    #[inline]
    pub fn cast<X: Pixel>(&self) -> LogicalSize<X> {
        LogicalSize { width: self.width.cast(), height: self.height.cast() }
    }
}

impl<P: Pixel, X: Pixel> From<(X, X)> for LogicalSize<P> {
    fn from((x, y): (X, X)) -> LogicalSize<P> {
        LogicalSize::new(x.cast(), y.cast())
    }
}

impl<P: Pixel, X: Pixel> From<LogicalSize<P>> for (X, X) {
    fn from(s: LogicalSize<P>) -> (X, X) {
        (s.width.cast(), s.height.cast())
    }
}

impl<P: Pixel, X: Pixel> From<[X; 2]> for LogicalSize<P> {
    fn from([x, y]: [X; 2]) -> LogicalSize<P> {
        LogicalSize::new(x.cast(), y.cast())
    }
}

impl<P: Pixel, X: Pixel> From<LogicalSize<P>> for [X; 2] {
    fn from(s: LogicalSize<P>) -> [X; 2] {
        [s.width.cast(), s.height.cast()]
    }
}

#[cfg(feature = "mint")]
impl<P: Pixel> From<mint::Vector2<P>> for LogicalSize<P> {
    fn from(v: mint::Vector2<P>) -> Self {
        Self::new(v.x, v.y)
    }
}

#[cfg(feature = "mint")]
impl<P: Pixel> From<LogicalSize<P>> for mint::Vector2<P> {
    fn from(s: LogicalSize<P>) -> Self {
        mint::Vector2 { x: s.width, y: s.height }
    }
}

/// A size represented in physical pixels.
#[derive(Debug, Copy, Clone, Eq, PartialEq, Ord, PartialOrd, Default, Hash)]
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
    pub fn from_logical<T: Into<LogicalSize<X>>, X: Pixel>(logical: T, scale_factor: f64) -> Self {
        logical.into().to_physical(scale_factor)
    }

    #[inline]
    pub fn to_logical<X: Pixel>(&self, scale_factor: f64) -> LogicalSize<X> {
        assert!(validate_scale_factor(scale_factor));
        let width = self.width.into() / scale_factor;
        let height = self.height.into() / scale_factor;
        LogicalSize::new(width, height).cast()
    }

    #[inline]
    pub fn cast<X: Pixel>(&self) -> PhysicalSize<X> {
        PhysicalSize { width: self.width.cast(), height: self.height.cast() }
    }
}

impl<P: Pixel, X: Pixel> From<(X, X)> for PhysicalSize<P> {
    fn from((x, y): (X, X)) -> PhysicalSize<P> {
        PhysicalSize::new(x.cast(), y.cast())
    }
}

impl<P: Pixel, X: Pixel> From<PhysicalSize<P>> for (X, X) {
    fn from(s: PhysicalSize<P>) -> (X, X) {
        (s.width.cast(), s.height.cast())
    }
}

impl<P: Pixel, X: Pixel> From<[X; 2]> for PhysicalSize<P> {
    fn from([x, y]: [X; 2]) -> PhysicalSize<P> {
        PhysicalSize::new(x.cast(), y.cast())
    }
}

impl<P: Pixel, X: Pixel> From<PhysicalSize<P>> for [X; 2] {
    fn from(s: PhysicalSize<P>) -> [X; 2] {
        [s.width.cast(), s.height.cast()]
    }
}

#[cfg(feature = "mint")]
impl<P: Pixel> From<mint::Vector2<P>> for PhysicalSize<P> {
    fn from(v: mint::Vector2<P>) -> Self {
        Self::new(v.x, v.y)
    }
}

#[cfg(feature = "mint")]
impl<P: Pixel> From<PhysicalSize<P>> for mint::Vector2<P> {
    fn from(s: PhysicalSize<P>) -> Self {
        mint::Vector2 { x: s.width, y: s.height }
    }
}

/// A size that's either physical or logical.
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

    pub fn to_logical<P: Pixel>(&self, scale_factor: f64) -> LogicalSize<P> {
        match *self {
            Size::Physical(size) => size.to_logical(scale_factor),
            Size::Logical(size) => size.cast(),
        }
    }

    pub fn to_physical<P: Pixel>(&self, scale_factor: f64) -> PhysicalSize<P> {
        match *self {
            Size::Physical(size) => size.cast(),
            Size::Logical(size) => size.to_physical(scale_factor),
        }
    }

    pub fn clamp<S: Into<Size>>(input: S, min: S, max: S, scale_factor: f64) -> Size {
        let (input, min, max) = (
            input.into().to_physical::<f64>(scale_factor),
            min.into().to_physical::<f64>(scale_factor),
            max.into().to_physical::<f64>(scale_factor),
        );

        let width = input.width.clamp(min.width, max.width);
        let height = input.height.clamp(min.height, max.height);

        PhysicalSize::new(width, height).into()
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

/// A position that's either physical or logical.
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

    pub fn to_logical<P: Pixel>(&self, scale_factor: f64) -> LogicalPosition<P> {
        match *self {
            Position::Physical(position) => position.to_logical(scale_factor),
            Position::Logical(position) => position.cast(),
        }
    }

    pub fn to_physical<P: Pixel>(&self, scale_factor: f64) -> PhysicalPosition<P> {
        match *self {
            Position::Physical(position) => position.cast(),
            Position::Logical(position) => position.to_physical(scale_factor),
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    macro_rules! test_pixel_int_impl {
        ($($name:ident => $ty:ty),*) => {$(
            #[test]
            fn $name() {
                assert_eq!(
                    <$ty as Pixel>::from_f64(37.0),
                    37,
                );
                assert_eq!(
                    <$ty as Pixel>::from_f64(37.4),
                    37,
                );
                assert_eq!(
                    <$ty as Pixel>::from_f64(37.5),
                    38,
                );
                assert_eq!(
                    <$ty as Pixel>::from_f64(37.9),
                    38,
                );

                assert_eq!(
                    <$ty as Pixel>::cast::<u8>(37),
                    37,
                );
                assert_eq!(
                    <$ty as Pixel>::cast::<u16>(37),
                    37,
                );
                assert_eq!(
                    <$ty as Pixel>::cast::<u32>(37),
                    37,
                );
                assert_eq!(
                    <$ty as Pixel>::cast::<i8>(37),
                    37,
                );
                assert_eq!(
                    <$ty as Pixel>::cast::<i16>(37),
                    37,
                );
                assert_eq!(
                    <$ty as Pixel>::cast::<i32>(37),
                    37,
                );
            }
        )*};
    }

    test_pixel_int_impl! {
        test_pixel_int_u8 => u8,
        test_pixel_int_u16 => u16,
        test_pixel_int_u32 => u32,
        test_pixel_int_i8 => i8,
        test_pixel_int_i16 => i16
    }

    macro_rules! assert_approx_eq {
        ($a:expr, $b:expr $(,)?) => {
            assert!(($a - $b).abs() < 0.001, "{} is not approximately equal to {}", $a, $b);
        };
    }

    macro_rules! test_pixel_float_impl {
    ($($name:ident => $ty:ty),*) => {$(
        #[test]
        fn $name() {
            assert_approx_eq!(
                <$ty as Pixel>::from_f64(37.0),
                37.0,
            );
            assert_approx_eq!(
                <$ty as Pixel>::from_f64(37.4),
                37.4,
            );
            assert_approx_eq!(
                <$ty as Pixel>::from_f64(37.5),
                37.5,
            );
            assert_approx_eq!(
                <$ty as Pixel>::from_f64(37.9),
                37.9,
            );

            assert_eq!(
                <$ty as Pixel>::cast::<u8>(37.0),
                37,
            );
            assert_eq!(
                <$ty as Pixel>::cast::<u8>(37.4),
                37,
            );
            assert_eq!(
                <$ty as Pixel>::cast::<u8>(37.5),
                38,
            );

            assert_eq!(
                <$ty as Pixel>::cast::<u16>(37.0),
                37,
            );
            assert_eq!(
                <$ty as Pixel>::cast::<u16>(37.4),
                37,
            );
            assert_eq!(
                <$ty as Pixel>::cast::<u16>(37.5),
                38,
            );

            assert_eq!(
                <$ty as Pixel>::cast::<u32>(37.0),
                37,
            );
            assert_eq!(
                <$ty as Pixel>::cast::<u32>(37.4),
                37,
            );
            assert_eq!(
                <$ty as Pixel>::cast::<u32>(37.5),
                38,
            );

            assert_eq!(
                <$ty as Pixel>::cast::<i8>(37.0),
                37,
            );
            assert_eq!(
                <$ty as Pixel>::cast::<i8>(37.4),
                37,
            );
            assert_eq!(
                <$ty as Pixel>::cast::<i8>(37.5),
                38,
            );

            assert_eq!(
                <$ty as Pixel>::cast::<i16>(37.0),
                37,
            );
            assert_eq!(
                <$ty as Pixel>::cast::<i16>(37.4),
                37,
            );
            assert_eq!(
                <$ty as Pixel>::cast::<i16>(37.5),
                38,
            );
        }
    )*};
}

    test_pixel_float_impl! {
        test_pixel_float_f32 => f32,
        test_pixel_float_f64 => f64
    }

    #[test]
    fn test_validate_scale_factor() {
        assert!(validate_scale_factor(1.0));
        assert!(validate_scale_factor(2.0));
        assert!(validate_scale_factor(3.0));
        assert!(validate_scale_factor(1.5));
        assert!(validate_scale_factor(0.5));

        assert!(!validate_scale_factor(0.0));
        assert!(!validate_scale_factor(-1.0));
        assert!(!validate_scale_factor(f64::INFINITY));
        assert!(!validate_scale_factor(f64::NAN));
        assert!(!validate_scale_factor(f64::NEG_INFINITY));
    }

    #[test]
    fn test_logical_unity() {
        let log_unit = LogicalUnit::new(1.0);
        assert_eq!(log_unit.to_physical::<u32>(1.0), PhysicalUnit::new(1));
        assert_eq!(log_unit.to_physical::<u32>(2.0), PhysicalUnit::new(2));
        assert_eq!(log_unit.cast::<u32>(), LogicalUnit::new(1));
        assert_eq!(log_unit, LogicalUnit::from_physical(PhysicalUnit::new(1.0), 1.0));
        assert_eq!(log_unit, LogicalUnit::from_physical(PhysicalUnit::new(2.0), 2.0));
        assert_eq!(LogicalUnit::from(2.0), LogicalUnit::new(2.0));

        let x: f64 = log_unit.into();
        assert_eq!(x, 1.0);
    }

    #[test]
    fn test_physical_unit() {
        assert_eq!(PhysicalUnit::from_logical(LogicalUnit::new(1.0), 1.0), PhysicalUnit::new(1));
        assert_eq!(PhysicalUnit::from_logical(LogicalUnit::new(2.0), 0.5), PhysicalUnit::new(1));
        assert_eq!(PhysicalUnit::from(2.0), PhysicalUnit::new(2.0,));
        assert_eq!(PhysicalUnit::from(2.0), PhysicalUnit::new(2.0));

        let x: f64 = PhysicalUnit::new(1).into();
        assert_eq!(x, 1.0);
    }

    #[test]
    fn test_logical_position() {
        let log_pos = LogicalPosition::new(1.0, 2.0);
        assert_eq!(log_pos.to_physical::<u32>(1.0), PhysicalPosition::new(1, 2));
        assert_eq!(log_pos.to_physical::<u32>(2.0), PhysicalPosition::new(2, 4));
        assert_eq!(log_pos.cast::<u32>(), LogicalPosition::new(1, 2));
        assert_eq!(log_pos, LogicalPosition::from_physical(PhysicalPosition::new(1.0, 2.0), 1.0));
        assert_eq!(log_pos, LogicalPosition::from_physical(PhysicalPosition::new(2.0, 4.0), 2.0));
        assert_eq!(LogicalPosition::from((2.0, 2.0)), LogicalPosition::new(2.0, 2.0));
        assert_eq!(LogicalPosition::from([2.0, 3.0]), LogicalPosition::new(2.0, 3.0));

        let x: (f64, f64) = log_pos.into();
        assert_eq!(x, (1.0, 2.0));
        let x: [f64; 2] = log_pos.into();
        assert_eq!(x, [1.0, 2.0]);
    }

    #[test]
    fn test_physical_position() {
        assert_eq!(
            PhysicalPosition::from_logical(LogicalPosition::new(1.0, 2.0), 1.0),
            PhysicalPosition::new(1, 2)
        );
        assert_eq!(
            PhysicalPosition::from_logical(LogicalPosition::new(2.0, 4.0), 0.5),
            PhysicalPosition::new(1, 2)
        );
        assert_eq!(PhysicalPosition::from((2.0, 2.0)), PhysicalPosition::new(2.0, 2.0));
        assert_eq!(PhysicalPosition::from([2.0, 3.0]), PhysicalPosition::new(2.0, 3.0));

        let x: (f64, f64) = PhysicalPosition::new(1, 2).into();
        assert_eq!(x, (1.0, 2.0));
        let x: [f64; 2] = PhysicalPosition::new(1, 2).into();
        assert_eq!(x, [1.0, 2.0]);
    }

    #[test]
    fn test_logical_size() {
        let log_size = LogicalSize::new(1.0, 2.0);
        assert_eq!(log_size.to_physical::<u32>(1.0), PhysicalSize::new(1, 2));
        assert_eq!(log_size.to_physical::<u32>(2.0), PhysicalSize::new(2, 4));
        assert_eq!(log_size.cast::<u32>(), LogicalSize::new(1, 2));
        assert_eq!(log_size, LogicalSize::from_physical(PhysicalSize::new(1.0, 2.0), 1.0));
        assert_eq!(log_size, LogicalSize::from_physical(PhysicalSize::new(2.0, 4.0), 2.0));
        assert_eq!(LogicalSize::from((2.0, 2.0)), LogicalSize::new(2.0, 2.0));
        assert_eq!(LogicalSize::from([2.0, 3.0]), LogicalSize::new(2.0, 3.0));

        let x: (f64, f64) = log_size.into();
        assert_eq!(x, (1.0, 2.0));
        let x: [f64; 2] = log_size.into();
        assert_eq!(x, [1.0, 2.0]);
    }

    #[test]
    fn test_physical_size() {
        assert_eq!(
            PhysicalSize::from_logical(LogicalSize::new(1.0, 2.0), 1.0),
            PhysicalSize::new(1, 2)
        );
        assert_eq!(
            PhysicalSize::from_logical(LogicalSize::new(2.0, 4.0), 0.5),
            PhysicalSize::new(1, 2)
        );
        assert_eq!(PhysicalSize::from((2.0, 2.0)), PhysicalSize::new(2.0, 2.0));
        assert_eq!(PhysicalSize::from([2.0, 3.0]), PhysicalSize::new(2.0, 3.0));

        let x: (f64, f64) = PhysicalSize::new(1, 2).into();
        assert_eq!(x, (1.0, 2.0));
        let x: [f64; 2] = PhysicalSize::new(1, 2).into();
        assert_eq!(x, [1.0, 2.0]);
    }

    #[test]
    fn test_size() {
        assert_eq!(Size::new(PhysicalSize::new(1, 2)), Size::Physical(PhysicalSize::new(1, 2)));
        assert_eq!(
            Size::new(LogicalSize::new(1.0, 2.0)),
            Size::Logical(LogicalSize::new(1.0, 2.0))
        );

        assert_eq!(
            Size::new(PhysicalSize::new(1, 2)).to_logical::<f64>(1.0),
            LogicalSize::new(1.0, 2.0)
        );
        assert_eq!(
            Size::new(PhysicalSize::new(1, 2)).to_logical::<f64>(2.0),
            LogicalSize::new(0.5, 1.0)
        );
        assert_eq!(
            Size::new(LogicalSize::new(1.0, 2.0)).to_logical::<f64>(1.0),
            LogicalSize::new(1.0, 2.0)
        );

        assert_eq!(
            Size::new(PhysicalSize::new(1, 2)).to_physical::<u32>(1.0),
            PhysicalSize::new(1, 2)
        );
        assert_eq!(
            Size::new(PhysicalSize::new(1, 2)).to_physical::<u32>(2.0),
            PhysicalSize::new(1, 2)
        );
        assert_eq!(
            Size::new(LogicalSize::new(1.0, 2.0)).to_physical::<u32>(1.0),
            PhysicalSize::new(1, 2)
        );
        assert_eq!(
            Size::new(LogicalSize::new(1.0, 2.0)).to_physical::<u32>(2.0),
            PhysicalSize::new(2, 4)
        );

        let small = Size::Physical((1, 2).into());
        let medium = Size::Logical((3, 4).into());
        let medium_physical = Size::new(medium.to_physical::<u32>(1.0));
        let large = Size::Physical((5, 6).into());
        assert_eq!(Size::clamp(medium, small, large, 1.0), medium_physical);
        assert_eq!(Size::clamp(small, medium, large, 1.0), medium_physical);
        assert_eq!(Size::clamp(large, small, medium, 1.0), medium_physical);
    }

    #[test]
    fn test_position() {
        assert_eq!(
            Position::new(PhysicalPosition::new(1, 2)),
            Position::Physical(PhysicalPosition::new(1, 2))
        );
        assert_eq!(
            Position::new(LogicalPosition::new(1.0, 2.0)),
            Position::Logical(LogicalPosition::new(1.0, 2.0))
        );

        assert_eq!(
            Position::new(PhysicalPosition::new(1, 2)).to_logical::<f64>(1.0),
            LogicalPosition::new(1.0, 2.0)
        );
        assert_eq!(
            Position::new(PhysicalPosition::new(1, 2)).to_logical::<f64>(2.0),
            LogicalPosition::new(0.5, 1.0)
        );
        assert_eq!(
            Position::new(LogicalPosition::new(1.0, 2.0)).to_logical::<f64>(1.0),
            LogicalPosition::new(1.0, 2.0)
        );

        assert_eq!(
            Position::new(PhysicalPosition::new(1, 2)).to_physical::<u32>(1.0),
            PhysicalPosition::new(1, 2)
        );
        assert_eq!(
            Position::new(PhysicalPosition::new(1, 2)).to_physical::<u32>(2.0),
            PhysicalPosition::new(1, 2)
        );
        assert_eq!(
            Position::new(LogicalPosition::new(1.0, 2.0)).to_physical::<u32>(1.0),
            PhysicalPosition::new(1, 2)
        );
        assert_eq!(
            Position::new(LogicalPosition::new(1.0, 2.0)).to_physical::<u32>(2.0),
            PhysicalPosition::new(2, 4)
        );
    }

    // Eat coverage for the Debug impls et al
    #[test]
    fn ensure_attrs_do_not_panic() {
        let _ = format!("{:?}", LogicalPosition::<u32>::default().clone());
        HashSet::new().insert(LogicalPosition::<u32>::default());

        let _ = format!("{:?}", PhysicalPosition::<u32>::default().clone());
        HashSet::new().insert(PhysicalPosition::<u32>::default());

        let _ = format!("{:?}", LogicalSize::<u32>::default().clone());
        HashSet::new().insert(LogicalSize::<u32>::default());

        let _ = format!("{:?}", PhysicalSize::<u32>::default().clone());
        HashSet::new().insert(PhysicalSize::<u32>::default());

        let _ = format!("{:?}", Size::Physical((1, 2).into()).clone());
        let _ = format!("{:?}", Position::Physical((1, 2).into()).clone());
    }

    #[test]
    fn ensure_copy_trait() {
        fn is_copy<T: Copy>() {}

        is_copy::<LogicalUnit<i32>>();
        is_copy::<PhysicalUnit<f64>>();
        is_copy::<PixelUnit>();

        is_copy::<LogicalSize<i32>>();
        is_copy::<PhysicalSize<f64>>();
        is_copy::<Size>();

        is_copy::<LogicalPosition<i32>>();
        is_copy::<PhysicalPosition<f64>>();
        is_copy::<Position>();
    }

    #[test]
    fn ensure_partial_eq_trait() {
        fn is_partial_eq<T: PartialEq>() {}

        is_partial_eq::<LogicalUnit<i32>>();
        is_partial_eq::<PhysicalUnit<f64>>();
        is_partial_eq::<PixelUnit>();

        is_partial_eq::<LogicalSize<i32>>();
        is_partial_eq::<PhysicalSize<f64>>();
        is_partial_eq::<Size>();

        is_partial_eq::<LogicalPosition<i32>>();
        is_partial_eq::<PhysicalPosition<f64>>();
        is_partial_eq::<Position>();
    }
}
