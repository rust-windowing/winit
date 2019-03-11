use std::{fmt, mem};
use std::error::Error;

#[repr(C)]
#[derive(Debug)]
pub(crate) struct Pixel {
    pub(crate) r: u8,
    pub(crate) g: u8,
    pub(crate) b: u8,
    pub(crate) a: u8,
}

pub(crate) const PIXEL_SIZE: usize = mem::size_of::<Pixel>();

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
/// An error produced when using `Icon::from_rgba` with invalid arguments.
pub enum BadIcon {
    /// Produced when the length of the `rgba` argument isn't divisible by 4, thus `rgba` can't be
    /// safely interpreted as 32bpp RGBA pixels.
    ByteCountNotDivisibleBy4 {
        byte_count: usize,
    },
    /// Produced when the number of pixels (`rgba.len() / 4`) isn't equal to `width * height`.
    /// At least one of your arguments is incorrect.
    DimensionsVsPixelCount {
        width: u32,
        height: u32,
        width_x_height: usize,
        pixel_count: usize,
    },
}

impl fmt::Display for BadIcon {
    fn fmt(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        let msg = match self {
            &BadIcon::ByteCountNotDivisibleBy4 { byte_count } => format!(
                "The length of the `rgba` argument ({:?}) isn't divisible by 4, making it impossible to interpret as 32bpp RGBA pixels.",
                byte_count,
            ),
            &BadIcon::DimensionsVsPixelCount {
                width,
                height,
                width_x_height,
                pixel_count,
            } => format!(
                "The specified dimensions ({:?}x{:?}) don't match the number of pixels supplied by the `rgba` argument ({:?}). For those dimensions, the expected pixel count is {:?}.",
                width, height, pixel_count, width_x_height,
            ),
        };
        write!(formatter, "{}", msg)
    }
}

impl Error for BadIcon {
    fn description(&self) -> &str {
        "A valid icon cannot be created from these arguments"
    }

    fn cause(&self) -> Option<&Error> {
        Some(self)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
/// An icon used for the window titlebar, taskbar, etc.
pub struct Icon {
    pub(crate) rgba: Vec<u8>,
    pub(crate) width: u32,
    pub(crate) height: u32,
}

impl Icon {
    /// Creates an `Icon` from 32bpp RGBA data.
    ///
    /// The length of `rgba` must be divisible by 4, and `width * height` must equal
    /// `rgba.len() / 4`. Otherwise, this will return a `BadIcon` error.
    pub fn from_rgba(rgba: Vec<u8>, width: u32, height: u32) -> Result<Self, BadIcon> {
        if rgba.len() % PIXEL_SIZE != 0 {
            return Err(BadIcon::ByteCountNotDivisibleBy4 { byte_count: rgba.len() });
        }
        let pixel_count = rgba.len() / PIXEL_SIZE;
        if pixel_count != (width * height) as usize {
            Err(BadIcon::DimensionsVsPixelCount {
                width,
                height,
                width_x_height: (width * height) as usize,
                pixel_count,
            })
        } else {
            Ok(Icon { rgba, width, height })
        }
    }
}
