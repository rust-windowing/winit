use core::fmt;
use std::any::Any;
use std::error::Error;
use std::hash::Hash;
use std::ops::Deref;
use std::sync::Arc;
use std::time::Duration;

#[doc(inline)]
pub use cursor_icon::CursorIcon;

/// The maximum width and height for a cursor image representation when using
/// [`CustomCursorSource::from_rgba`] or [`CustomCursorSource::from_rgba_representations`].
pub const MAX_CURSOR_SIZE: u16 = 2048;

const PIXEL_SIZE: usize = 4;

/// See [`Window::set_cursor()`][crate::window::Window::set_cursor] for more details.
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
#[allow(clippy::exhaustive_enums)]
pub enum Cursor {
    Icon(CursorIcon),
    Custom(CustomCursor),
}

impl Default for Cursor {
    fn default() -> Self {
        Self::Icon(CursorIcon::default())
    }
}

impl From<CursorIcon> for Cursor {
    fn from(icon: CursorIcon) -> Self {
        Self::Icon(icon)
    }
}

impl From<CustomCursor> for Cursor {
    fn from(custom: CustomCursor) -> Self {
        Self::Custom(custom)
    }
}

/// Use a custom image as a cursor (mouse pointer).
///
/// Is guaranteed to be cheap to clone.
///
/// ## Platform-specific
///
/// **Web**: Some browsers have limits on cursor sizes usually at 128x128.
///
/// # Example
///
/// ```no_run
/// # use winit_core::event_loop::ActiveEventLoop;
/// # use winit_core::window::Window;
/// # fn scope(event_loop: &dyn ActiveEventLoop, window: &dyn Window) {
/// use winit_core::cursor::CustomCursorSource;
///
/// let w = 10;
/// let h = 10;
/// let rgba = vec![255; (w * h * 4) as usize];
///
/// #[cfg(not(target_family = "wasm"))]
/// let source = CustomCursorSource::from_rgba(rgba, w, h, w / 2, h / 2).unwrap();
///
/// #[cfg(target_family = "wasm")]
/// let source = CustomCursorSource::Url {
///     url: String::from("http://localhost:3000/cursor.png"),
///     hotspot_x: 0,
///     hotspot_y: 0,
/// };
///
/// if let Ok(custom_cursor) = event_loop.create_custom_cursor(source) {
///     window.set_cursor(custom_cursor.clone().into());
/// }
/// # }
/// ```
#[derive(Clone, Debug)]
pub struct CustomCursor(pub Arc<dyn CustomCursorProvider>);

pub trait CustomCursorProvider: Any + fmt::Debug + Send + Sync {
    /// Whether a cursor was backed by animation.
    fn is_animated(&self) -> bool;
}

impl PartialEq for CustomCursor {
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.0, &other.0)
    }
}

impl Eq for CustomCursor {}

impl Hash for CustomCursor {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        Arc::as_ptr(&self.0).hash(state);
    }
}

impl Deref for CustomCursor {
    type Target = dyn CustomCursorProvider;

    fn deref(&self) -> &Self::Target {
        self.0.deref()
    }
}

impl_dyn_casting!(CustomCursorProvider);

/// Source for [`CustomCursor`].
///
/// See [`CustomCursor`] for more details.
#[derive(Debug, Clone, Eq, Hash, PartialEq)]
#[non_exhaustive]
pub enum CustomCursorSource {
    /// Cursor that is backed by RGBA image.
    ///
    /// See [CustomCursorSource::from_rgba] for more.
    ///
    /// ## Platform-specific
    ///
    /// - **iOS / Android / Orbital:** Unsupported
    Image(CursorImage),
    /// Animated cursor.
    ///
    /// See [CustomCursorSource::from_animation] for more.
    ///
    /// ## Platform-specific
    ///
    /// - **iOS / Android / Wayland / Windows / X11 / macOS / Orbital:** Unsupported
    Animation(CursorAnimation),
    /// Creates a new cursor from a URL pointing to an image.
    /// It uses the [url css function](https://developer.mozilla.org/en-US/docs/Web/CSS/url),
    /// but browser support for image formats is inconsistent. Using [PNG] is recommended.
    ///
    /// [PNG]: https://en.wikipedia.org/wiki/PNG
    ///
    /// ## Platform-specific
    ///
    /// - **iOS / Android / Wayland / Windows / X11 / macOS / Orbital:** Unsupported
    Url { hotspot_x: u16, hotspot_y: u16, url: String },
}

impl CustomCursorSource {
    /// Creates a new cursor from an rgba buffer.
    ///
    /// The width, height, and hotspot are specified in physical pixels.
    ///
    /// The alpha channel is assumed to be **not** premultiplied.
    pub fn from_rgba(
        rgba: Vec<u8>,
        width: u16,
        height: u16,
        hotspot_x: u16,
        hotspot_y: u16,
    ) -> Result<Self, BadImage> {
        CursorImage::from_rgba(rgba, width, height, hotspot_x, hotspot_y).map(Self::Image)
    }

    /// Creates a new cursor from one or more RGBA image representations.
    ///
    /// The hotspot is specified in logical pixels. Each representation declares both its physical
    /// pixel size and the logical cursor size it represents. All representations must have the same
    /// logical size.
    ///
    /// For example, a 32x32 cursor can be supplied as 32x32, 40x40, 48x48, and 64x64 physical
    /// representations that all declare a 32x32 logical size.
    ///
    /// ## Platform-specific
    ///
    /// - **macOS:** All representations are passed to AppKit, allowing the system to choose the
    ///   best representation for the display.
    /// - **Windows:** The representation closest to the window's current DPI is used.
    /// - **Other platforms:** Only one representation is used. A representation whose physical size
    ///   matches the logical size is preferred when available, otherwise the first representation
    ///   is used.
    ///
    /// The alpha channel is assumed to be **not** premultiplied.
    pub fn from_rgba_representations(
        representations: Vec<CursorImageRepresentation>,
        hotspot_x: u16,
        hotspot_y: u16,
    ) -> Result<Self, BadImage> {
        CursorImage::from_rgba_representations(representations, hotspot_x, hotspot_y)
            .map(Self::Image)
    }

    /// Crates a new animated cursor from multiple [`CustomCursor`]s
    /// Supplied `cursors` can't be empty or other animations.
    pub fn from_animation(
        duration: Duration,
        cursors: Vec<CustomCursor>,
    ) -> Result<Self, BadAnimation> {
        CursorAnimation::new(duration, cursors).map(Self::Animation)
    }
}

/// An error produced when using [`CustomCursorSource::from_rgba`] with invalid arguments.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[non_exhaustive]
pub enum BadImage {
    /// Produced when no image representations were supplied.
    EmptyRepresentations,
    /// Produced when the image dimensions are larger than [`MAX_CURSOR_SIZE`]. This doesn't
    /// guarantee that the cursor will work, but should avoid many platform and device specific
    /// limits.
    TooLarge { width: u16, height: u16 },
    /// Produced when the length of the `rgba` argument isn't divisible by 4, thus `rgba` can't be
    /// safely interpreted as 32bpp RGBA pixels.
    ByteCountNotDivisibleBy4 { byte_count: usize },
    /// Produced when the number of pixels (`rgba.len() / 4`) isn't equal to `width * height`.
    /// At least one of your arguments is incorrect.
    DimensionsVsPixelCount { width: u16, height: u16, width_x_height: u64, pixel_count: u64 },
    /// Produced when the hotspot is outside the image bounds
    HotspotOutOfBounds { width: u16, height: u16, hotspot_x: u16, hotspot_y: u16 },
    /// Produced when a cursor representation has a zero logical width or height.
    ZeroLogicalSize { width: u16, height: u16 },
    /// Produced when cursor representations do not resolve to the same logical size.
    InconsistentLogicalSize { expected_width: u16, expected_height: u16, width: u16, height: u16 },
}

impl fmt::Display for BadImage {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            BadImage::EmptyRepresentations => {
                write!(f, "At least one image representation must be supplied.")
            },
            BadImage::TooLarge { width, height } => write!(
                f,
                "The specified dimensions ({width:?}x{height:?}) are too large. The maximum is \
                 {MAX_CURSOR_SIZE:?}x{MAX_CURSOR_SIZE:?}.",
            ),
            BadImage::ByteCountNotDivisibleBy4 { byte_count } => write!(
                f,
                "The length of the `rgba` argument ({byte_count:?}) isn't divisible by 4, making \
                 it impossible to interpret as 32bpp RGBA pixels.",
            ),
            BadImage::DimensionsVsPixelCount { width, height, width_x_height, pixel_count } => {
                write!(
                    f,
                    "The specified dimensions ({width:?}x{height:?}) don't match the number of \
                     pixels supplied by the `rgba` argument ({pixel_count:?}). For those \
                     dimensions, the expected pixel count is {width_x_height:?}.",
                )
            },
            BadImage::HotspotOutOfBounds { width, height, hotspot_x, hotspot_y } => write!(
                f,
                "The specified hotspot ({hotspot_x:?}, {hotspot_y:?}) is outside the image bounds \
                 ({width:?}x{height:?}).",
            ),
            BadImage::ZeroLogicalSize { width, height } => write!(
                f,
                "The specified cursor image representation logical dimensions \
                 ({width:?}x{height:?}) must be greater than zero.",
            ),
            BadImage::InconsistentLogicalSize {
                expected_width,
                expected_height,
                width,
                height,
            } => write!(
                f,
                "The specified cursor image representation has logical dimensions \
                 ({width:?}x{height:?}), but previous representations have logical dimensions \
                 ({expected_width:?}x{expected_height:?}).",
            ),
        }
    }
}

impl Error for BadImage {}

/// An error produced when using [`CustomCursorSource::from_animation`] with invalid arguments.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[non_exhaustive]
pub enum BadAnimation {
    /// Produced when no cursors were supplied.
    Empty,
    /// Produced when a supplied cursor is an animation.
    Animation,
}

impl fmt::Display for BadAnimation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Empty => write!(f, "No cursors supplied"),
            Self::Animation => write!(f, "A supplied cursor is an animation"),
        }
    }
}

impl Error for BadAnimation {}

#[derive(Debug, Clone, Eq, Hash, PartialEq)]
pub struct CursorImageRepresentation {
    pub(crate) rgba: Vec<u8>,
    pub(crate) width: u16,
    pub(crate) height: u16,
    pub(crate) logical_width: u16,
    pub(crate) logical_height: u16,
}

impl CursorImageRepresentation {
    /// Creates a new cursor image representation from an RGBA buffer.
    ///
    /// The width and height are specified in physical pixels. The logical width and height specify
    /// the cursor size represented by those physical pixels.
    ///
    /// For example, a 40x40 representation with a 32x32 logical size contributes a 125% cursor
    /// image.
    pub fn from_rgba(
        rgba: Vec<u8>,
        width: u16,
        height: u16,
        logical_width: u16,
        logical_height: u16,
    ) -> Result<Self, BadImage> {
        validate_rgba(&rgba, width, height)?;

        if logical_width == 0 || logical_height == 0 {
            return Err(BadImage::ZeroLogicalSize { width: logical_width, height: logical_height });
        }

        Ok(Self { rgba, width, height, logical_width, logical_height })
    }

    pub fn buffer(&self) -> &[u8] {
        self.rgba.as_slice()
    }

    pub fn buffer_mut(&mut self) -> &mut [u8] {
        self.rgba.as_mut_slice()
    }

    pub fn width(&self) -> u16 {
        self.width
    }

    pub fn height(&self) -> u16 {
        self.height
    }

    pub fn logical_width(&self) -> u16 {
        self.logical_width
    }

    pub fn logical_height(&self) -> u16 {
        self.logical_height
    }
}

#[derive(Debug, Clone, Eq, Hash, PartialEq)]
pub struct CursorImage {
    pub(crate) representations: Vec<CursorImageRepresentation>,
    pub(crate) width: u16,
    pub(crate) height: u16,
    pub(crate) hotspot_x: u16,
    pub(crate) hotspot_y: u16,
}

impl CursorImage {
    pub(crate) fn from_rgba(
        rgba: Vec<u8>,
        width: u16,
        height: u16,
        hotspot_x: u16,
        hotspot_y: u16,
    ) -> Result<Self, BadImage> {
        let representation =
            CursorImageRepresentation::from_rgba(rgba, width, height, width, height)?;
        Self::from_rgba_representations(vec![representation], hotspot_x, hotspot_y)
    }

    pub(crate) fn from_rgba_representations(
        representations: Vec<CursorImageRepresentation>,
        hotspot_x: u16,
        hotspot_y: u16,
    ) -> Result<Self, BadImage> {
        let first = representations.first().ok_or(BadImage::EmptyRepresentations)?;
        let width = first.logical_width();
        let height = first.logical_height();

        for representation in &representations {
            let representation_width = representation.logical_width();
            let representation_height = representation.logical_height();
            if representation_width != width || representation_height != height {
                return Err(BadImage::InconsistentLogicalSize {
                    expected_width: width,
                    expected_height: height,
                    width: representation_width,
                    height: representation_height,
                });
            }
        }

        if hotspot_x >= width || hotspot_y >= height {
            return Err(BadImage::HotspotOutOfBounds { width, height, hotspot_x, hotspot_y });
        }

        Ok(CursorImage { representations, width, height, hotspot_x, hotspot_y })
    }

    pub fn buffer(&self) -> &[u8] {
        self.primary_representation().buffer()
    }

    pub fn buffer_mut(&mut self) -> &mut [u8] {
        self.primary_representation_mut().buffer_mut()
    }

    pub fn width(&self) -> u16 {
        self.primary_representation().width()
    }

    pub fn height(&self) -> u16 {
        self.primary_representation().height()
    }

    pub fn hotspot_x(&self) -> u16 {
        self.physical_hotspot_x(self.primary_representation())
    }

    pub fn hotspot_y(&self) -> u16 {
        self.physical_hotspot_y(self.primary_representation())
    }

    pub fn logical_width(&self) -> u16 {
        self.width
    }

    pub fn logical_height(&self) -> u16 {
        self.height
    }

    pub fn logical_hotspot_x(&self) -> u16 {
        self.hotspot_x
    }

    pub fn logical_hotspot_y(&self) -> u16 {
        self.hotspot_y
    }

    pub fn representations(&self) -> &[CursorImageRepresentation] {
        self.representations.as_slice()
    }

    pub fn physical_hotspot_x(&self, representation: &CursorImageRepresentation) -> u16 {
        scale_logical_coordinate(self.hotspot_x, representation.width(), self.width)
    }

    pub fn physical_hotspot_y(&self, representation: &CursorImageRepresentation) -> u16 {
        scale_logical_coordinate(self.hotspot_y, representation.height(), self.height)
    }

    fn primary_representation(&self) -> &CursorImageRepresentation {
        self.representations
            .iter()
            .find(|representation| {
                representation.width() == representation.logical_width()
                    && representation.height() == representation.logical_height()
            })
            .unwrap_or_else(|| &self.representations[0])
    }

    fn primary_representation_mut(&mut self) -> &mut CursorImageRepresentation {
        let index = self
            .representations
            .iter()
            .position(|representation| {
                representation.width() == representation.logical_width()
                    && representation.height() == representation.logical_height()
            })
            .unwrap_or(0);
        &mut self.representations[index]
    }
}

fn scale_logical_coordinate(coordinate: u16, physical_size: u16, logical_size: u16) -> u16 {
    let coordinate = coordinate as u32;
    let physical_size = physical_size as u32;
    let logical_size = logical_size as u32;
    ((coordinate * physical_size + logical_size / 2) / logical_size) as u16
}

fn validate_rgba(rgba: &[u8], width: u16, height: u16) -> Result<(), BadImage> {
    if width > MAX_CURSOR_SIZE || height > MAX_CURSOR_SIZE {
        return Err(BadImage::TooLarge { width, height });
    }

    if rgba.len() % PIXEL_SIZE != 0 {
        return Err(BadImage::ByteCountNotDivisibleBy4 { byte_count: rgba.len() });
    }

    let pixel_count = (rgba.len() / PIXEL_SIZE) as u64;
    let width_x_height = width as u64 * height as u64;
    if pixel_count != width_x_height {
        return Err(BadImage::DimensionsVsPixelCount {
            width,
            height,
            width_x_height,
            pixel_count,
        });
    }

    Ok(())
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct CursorAnimation {
    pub(crate) duration: Duration,
    pub(crate) cursors: Vec<CustomCursor>,
}

impl CursorAnimation {
    pub fn new(duration: Duration, cursors: Vec<CustomCursor>) -> Result<Self, BadAnimation> {
        if cursors.is_empty() {
            return Err(BadAnimation::Empty);
        }

        if cursors.iter().any(|cursor| cursor.is_animated()) {
            return Err(BadAnimation::Animation);
        }

        Ok(Self { duration, cursors })
    }

    pub fn duration(&self) -> Duration {
        self.duration
    }

    pub fn cursors(&self) -> &[CustomCursor] {
        self.cursors.as_slice()
    }

    pub fn into_raw(self) -> (Duration, Vec<CustomCursor>) {
        (self.duration, self.cursors)
    }
}
