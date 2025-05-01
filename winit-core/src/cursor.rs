use core::fmt;
use std::error::Error;
use std::hash::Hash;
use std::ops::Deref;
use std::sync::Arc;
use std::time::Duration;

use cursor_icon::CursorIcon;

use crate::as_any::{impl_dyn_casting, AsAny};

/// The maximum width and height for a cursor when using [`CustomCursorSource::from_rgba`].
pub const MAX_CURSOR_SIZE: u16 = 2048;

const PIXEL_SIZE: usize = 4;

/// See [`Window::set_cursor()`][crate::window::Window::set_cursor] for more details.
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
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
/// # use winit::event_loop::ActiveEventLoop;
/// # use winit::window::Window;
/// # fn scope(event_loop: &dyn ActiveEventLoop, window: &dyn Window) {
/// use winit::window::CustomCursorSource;
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

pub trait CustomCursorProvider: AsAny + fmt::Debug + Send + Sync {
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
pub enum BadImage {
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
}

impl fmt::Display for BadImage {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
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
        }
    }
}

impl Error for BadImage {}

/// An error produced when using [`CustomCursorSource::from_animation`] with invalid arguments.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
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
pub struct CursorImage {
    pub(crate) rgba: Vec<u8>,
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

        if hotspot_x >= width || hotspot_y >= height {
            return Err(BadImage::HotspotOutOfBounds { width, height, hotspot_x, hotspot_y });
        }

        Ok(CursorImage { rgba, width, height, hotspot_x, hotspot_y })
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

    pub fn hotspot_x(&self) -> u16 {
        self.hotspot_x
    }

    pub fn hotspot_y(&self) -> u16 {
        self.hotspot_y
    }
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
