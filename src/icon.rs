use crate::{
    dpi::{PhysicalPosition, PhysicalSize},
    platform_impl::PlatformIcon,
};
use std::{fmt, io, mem, ops::Deref};

#[repr(C)]
#[derive(Debug)]
pub(crate) struct Pixel {
    pub(crate) r: u8,
    pub(crate) g: u8,
    pub(crate) b: u8,
    pub(crate) a: u8,
}

pub(crate) const PIXEL_SIZE: usize = mem::size_of::<Pixel>();

#[derive(Clone, PartialEq, Eq, Hash)]
pub struct RgbaIcon<I: Deref<Target = [u8]>> {
    pub(crate) rgba: I,
    pub(crate) size: PhysicalSize<u32>,
    pub(crate) hot_spot: PhysicalPosition<u32>,
}

/// For platforms which don't have window icons (e.g. web)
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct NoIcon;

/// An icon used for the window titlebar, taskbar, or cursor.
#[derive(Clone, PartialEq, Eq)]
pub struct Icon {
    pub(crate) inner: PlatformIcon,
}

#[allow(dead_code)] // These are not used on every platform
mod constructors {
    use super::*;

    impl NoIcon {
        pub fn from_rgba(_rgba: Vec<u8>, _size: PhysicalSize<u32>) -> Result<Self, io::Error> {
            Ok(NoIcon)
        }

        pub fn from_rgba_with_hot_spot(
            _rgba: Vec<u8>,
            _size: PhysicalSize<u32>,
            _hot_spot: PhysicalPosition<u32>,
        ) -> Result<Self, io::Error> {
            Ok(NoIcon)
        }

        pub fn from_rgba_fn<F>(_get_icon: F) -> Result<Self, io::Error>
        where
            F: 'static
                + FnMut(
                    PhysicalSize<u32>,
                    f64,
                )
                    -> Result<RgbaIcon<Box<[u8]>>, Box<dyn std::error::Error + Send + Sync>>,
        {
            Ok(NoIcon)
        }
    }
}

impl<I: Deref<Target = [u8]>> fmt::Debug for RgbaIcon<I> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> Result<(), fmt::Error> {
        let RgbaIcon {
            rgba,
            size,
            hot_spot,
        } = self;
        f.debug_struct("RgbaIcon")
            .field("size", &size)
            .field("hot_spot", &hot_spot)
            .field("rgba", &(&**rgba as *const [u8]))
            .finish()
    }
}
impl<I: Deref<Target = [u8]>> RgbaIcon<I> {
    /// Creates a `RgbaIcon` from 32bpp RGBA data.
    ///
    /// ## Panics
    /// Panics if the length of `rgba` is not divisible by 4, or if `width * height` doesn't
    /// equal `rgba.len() / 4`.
    pub fn from_rgba(rgba: I, size: PhysicalSize<u32>) -> Self {
        Self::from_rgba_with_hot_spot(
            rgba,
            size,
            PhysicalPosition::new(size.width / 2, size.height / 2),
        )
    }

    /// Creates a `RgbaIcon` from 32bpp RGBA data, with a defined cursor hot spot. The hot spot is
    /// the exact pixel in the icon image where the cursor clicking point is, and is ignored when
    /// the icon is used as a window icon.
    ///
    /// ## Panics
    /// Panics if the length of `rgba` is not divisible by 4, or if `width * height` doesn't equal
    /// `rgba.len() / 4`.
    pub fn from_rgba_with_hot_spot(
        rgba: I,
        size: PhysicalSize<u32>,
        hot_spot: PhysicalPosition<u32>,
    ) -> Self {
        let PhysicalSize { width, height } = size;
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
                width,
                height,
                pixel_count,
                width * height,
            )
        }

        RgbaIcon {
            rgba,
            size,
            hot_spot,
        }
    }
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
    /// Panics if the length of `rgba` is not divisible by 4, or if `width * height` doesn't equal
    /// `rgba.len() / 4`.
    pub fn from_rgba(rgba: &[u8], size: PhysicalSize<u32>) -> Result<Self, io::Error> {
        Ok(Icon {
            inner: PlatformIcon::from_rgba(rgba.into(), size)?,
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
        rgba: &[u8],
        size: PhysicalSize<u32>,
        hot_spot: PhysicalPosition<u32>,
    ) -> Result<Self, io::Error> {
        Ok(Icon {
            inner: PlatformIcon::from_rgba_with_hot_spot(rgba.into(), size, hot_spot)?,
        })
    }

    /// Lazily create an icon from several scaled source images.
    ///
    /// `get_icon` will be lazily called for a particular icon size whenever the window manager
    /// needs an icon of that size. The `PhysicalSize<u32>` parameter specifies the window manager's
    /// suggested icon size for a particular scale factor, and will always be a square. The `f64`
    /// parameter specifies the scale factor that the window manager is requesting the icon with.
    /// `get_icon` will only be called once for any given suggested icon size.
    ///
    /// If `get_icon` returns `Err(e)` for a given size, Winit will invoke `warn!` on the returned
    /// error and will try to retrieve a differently-sized icon from `get_icon`.
    pub fn from_rgba_fn<F, B>(mut get_icon: F) -> Result<Self, std::io::Error>
    where
        F: 'static
            + FnMut(
                PhysicalSize<u32>,
                f64,
            )
                -> Result<RgbaIcon<B>, Box<dyn 'static + std::error::Error + Send + Sync>>,
        B: Deref<Target = [u8]> + Into<Box<[u8]>>,
    {
        Ok(Icon {
            inner: PlatformIcon::from_rgba_fn(move |size, scale_factor| {
                let icon = get_icon(size, scale_factor)?;
                Ok(RgbaIcon {
                    rgba: icon.rgba.into(),
                    size: icon.size,
                    hot_spot: icon.hot_spot,
                })
            })?,
        })
    }
}
