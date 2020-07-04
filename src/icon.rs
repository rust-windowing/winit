use crate::{
    dpi::{PhysicalSize, PhysicalPosition},
    platform_impl::PlatformIcon,
};
use std::{fmt, io, mem};

#[repr(C)]
#[derive(Debug)]
pub(crate) struct Pixel {
    pub(crate) r: u8,
    pub(crate) g: u8,
    pub(crate) b: u8,
    pub(crate) a: u8,
}

pub(crate) const PIXEL_SIZE: usize = mem::size_of::<Pixel>();

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct RgbaIcon {
    pub(crate) rgba: Vec<u8>,
    pub(crate) size: PhysicalSize<u32>,
    pub(crate) hot_spot: PhysicalPosition<u32>,
}

/// For platforms which don't have window icons (e.g. web)
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct NoIcon;

#[allow(dead_code)] // These are not used on every platform
mod constructors {
    use super::*;

    impl RgbaIcon {
        /// Creates an `Icon` from 32bpp RGBA data.
        ///
        /// ## Panics
        /// Panics if the length of `rgba` must be divisible by 4, or if `width * height` doesn't equal
        /// `rgba.len() / 4`.
        pub fn from_rgba(rgba: Vec<u8>, size: PhysicalSize<u32>) -> Self {
            Self::from_rgba_with_hot_spot(
                rgba,
                size,
                PhysicalPosition::new(size.width / 2, size.height / 2),
            )
        }

        pub fn from_rgba_with_hot_spot(
            rgba: Vec<u8>,
            size: PhysicalSize<u32>,
            hot_spot: PhysicalPosition<u32>
        ) -> Self {
            let PhysicalSize{ width, height } = size;
            if rgba.len() % PIXEL_SIZE != 0 {
                panic!(
                    "The length of the `rgba` argument ({:?}) isn't divisible by 4, making \
                    it impossible to interpret as 32bpp RGBA pixels.",
                    rgba.len(),
                );
            }
            let pixel_count = rgba.len() / PIXEL_SIZE;
            if pixel_count != (width * height) as usize {
                panic!(
                    "The specified dimensions ({:?}x{:?}) don't match the number of pixels \
                    supplied by the `rgba` argument ({:?}). For those dimensions, the expected \
                    pixel count is {:?}.",
                    width, height, pixel_count, width * height,
                )
            }

            RgbaIcon {
                rgba,
                size,
                hot_spot,
            }
        }
    }

    impl NoIcon {
        pub fn from_rgba(_rgba: Vec<u8>, _size: PhysicalSize<u32>) -> Self {
            NoIcon
        }
    }
}

/// An icon used for the window titlebar, taskbar, etc.
#[derive(Clone, PartialEq, Eq)]
pub struct Icon {
    pub(crate) inner: PlatformIcon,
}

impl fmt::Debug for Icon {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> Result<(), fmt::Error> {
        fmt::Debug::fmt(&self.inner, formatter)
    }
}

impl Icon {
    /// Creates an `Icon` from 32bpp RGBA data.
    ///
    /// ## Panics
    /// Panics if the length of `rgba` must be divisible by 4, or if `width * height` doesn't equal
    /// `rgba.len() / 4`.
    pub fn from_rgba(rgba: Vec<u8>, size: PhysicalSize<u32>) -> Result<Self, io::Error> {
        Ok(Icon {
            inner: PlatformIcon::from_rgba(rgba, size)?,
        })
    }

    /// Creates an `Icon` from 32bpp RGBA data, with a defined cursor hot spot. The hot spot is
    /// the exact pixel in the icon image where the cursor clicking point is, and is ignored when
    /// the icon is used as a window icon.
    ///
    /// ## Panics
    /// Panics if the length of `rgba` must be divisible by 4, or if `width * height` doesn't equal
    /// `rgba.len() / 4`.
    pub fn from_rgba_with_hot_spot(
        rgba: Vec<u8>,
        size: PhysicalSize<u32>,
        hot_spot: PhysicalPosition<u32>
    ) -> Result<Self, io::Error> {
        Ok(Icon {
            inner: PlatformIcon::from_rgba_with_hot_spot(rgba, size, hot_spot)?,
        })
    }
}
