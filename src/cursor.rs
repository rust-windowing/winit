use core::fmt;
use std::{error::Error, sync::Arc};

use crate::platform_impl::PlatformCustomCursor;

/// Use a custom image as a cursor (mouse pointer).
///
/// ## Platform-specific
///
/// - **Web** On Firefox, [a bug](https://bugzilla.mozilla.org/show_bug.cgi?id=1336764) causes
///     the cursor to not change until the mouse is moved.
///
///
/// # Examples
///
/// ```
/// use winit::window::CustomCursor;
///
/// let w = 10;
/// let h = 10;
/// let rgba = vec![0xff_u8; (w * h * 4) as usize];
/// let custom_cursor = CustomCursor::from_rgba(rgba, w, h, w / 2, h / 2).unwrap();
///
/// #[cfg(target_family = "wasm")]
/// let custom_cursor_url = {
///     use winit::platform::web::CustomCursorExtWebSys;
///     CustomCursor::from_url("http://localhost:3000/cursor.png", 0, 0).unwrap()
/// };
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CustomCursor {
    pub(crate) inner: Arc<PlatformCustomCursor>,
}

impl CustomCursor {
    /// Creates a new cursor from an rgba buffer.
    ///
    /// ## Platform-specific
    ///
    /// - **Web** Uses data URLs. They have limited maximum sizes in browsers. They are
    ///     generated asynchronously, so there can be latency when setting a cursor.
    pub fn from_rgba(
        rgba: impl Into<Vec<u8>>,
        width: u32,
        height: u32,
        hotspot_x: u32,
        hotspot_y: u32,
    ) -> Result<Self, BadImage> {
        Ok(Self {
            inner: PlatformCustomCursor::from_rgba(
                rgba.into(),
                width,
                height,
                hotspot_x,
                hotspot_y,
            )?
            .into(),
        })
    }
}

/// Platforms that don't support cursors will export this as `PlatformCustomCursor`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct NoCustomCursor;

#[allow(dead_code)]
impl NoCustomCursor {
    pub fn from_rgba(
        rgba: Vec<u8>,
        width: u32,
        height: u32,
        hotspot_x: u32,
        hotspot_y: u32,
    ) -> Result<Self, BadImage> {
        CursorImage::from_rgba(rgba, width, height, hotspot_x, hotspot_y)?;
        Ok(Self)
    }
}

#[derive(Debug)]
/// An error produced when using [`CustomCursor::from_rgba`] with invalid arguments.
pub enum BadImage {
    /// Produced when the length of the `rgba` argument isn't divisible by 4, thus `rgba` can't be
    /// safely interpreted as 32bpp RGBA pixels.
    ByteCountNotDivisibleBy4 { byte_count: usize },
    /// Produced when the number of pixels (`rgba.len() / 4`) isn't equal to `width * height`.
    /// At least one of your arguments is incorrect.
    DimensionsVsPixelCount {
        width: u32,
        height: u32,
        width_x_height: u64,
        pixel_count: u64,
    },
    /// Produced when the hotspot is outside the image bounds
    HotspotOutOfBounds {
        width: u32,
        height: u32,
        hotspot_x: u32,
        hotspot_y: u32,
    },
}

impl fmt::Display for BadImage {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            BadImage::ByteCountNotDivisibleBy4 { byte_count } => write!(f,
                "The length of the `rgba` argument ({byte_count:?}) isn't divisible by 4, making it impossible to interpret as 32bpp RGBA pixels.",
            ),
            BadImage::DimensionsVsPixelCount {
                width,
                height,
                width_x_height,
                pixel_count,
            } => write!(f,
                "The specified dimensions ({width:?}x{height:?}) don't match the number of pixels supplied by the `rgba` argument ({pixel_count:?}). For those dimensions, the expected pixel count is {width_x_height:?}.",
            ),
            BadImage::HotspotOutOfBounds {
                width,
                height,
                hotspot_x,
                hotspot_y,
            } => write!(f,
                "The specified hotspot ({hotspot_x:?}, {hotspot_y:?}) is outside the image bounds ({width:?}x{height:?}).",
            ),
        }
    }
}

impl Error for BadImage {}

/// Platforms export this directly as `PlatformCustomCursor` if they need to only work with images.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CursorImage {
    pub(crate) rgba: Vec<u8>,
    pub(crate) width: u32,
    pub(crate) height: u32,
    pub(crate) hotspot_x: u32,
    pub(crate) hotspot_y: u32,
}

pub const PIXEL_SIZE: usize = 4;

#[allow(dead_code)]
impl CursorImage {
    pub fn from_rgba(
        rgba: Vec<u8>,
        width: u32,
        height: u32,
        hotspot_x: u32,
        hotspot_y: u32,
    ) -> Result<Self, BadImage> {
        if rgba.len() % PIXEL_SIZE != 0 {
            return Err(BadImage::ByteCountNotDivisibleBy4 {
                byte_count: rgba.len(),
            });
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
            return Err(BadImage::HotspotOutOfBounds {
                width,
                height,
                hotspot_x,
                hotspot_y,
            });
        }

        Ok(CursorImage {
            rgba,
            width,
            height,
            hotspot_x,
            hotspot_y,
        })
    }
}
