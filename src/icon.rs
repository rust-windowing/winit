use crate::{
    dpi::{PhysicalPosition, PhysicalSize},
    platform_impl::{PlatformCustomCursorIcon, PlatformCustomWindowIcon},
};
use std::{error::Error, fmt, io, mem, ops::Deref};

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
pub struct RgbaBuffer<I: Deref<Target = [u8]>> {
    pub(crate) rgba: I,
    pub(crate) size: PhysicalSize<u32>,
}

/// For platforms which don't have window icons (e.g. web)
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct NoWindowIcon;

/// For platforms which don't have cursor icons
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct NoCursorIcon;

/// An icon used for the window titlebar, taskbar, or cursor.
#[derive(Clone, PartialEq, Eq)]
pub struct CustomWindowIcon {
    pub(crate) inner: PlatformCustomWindowIcon,
}

#[derive(Clone, PartialEq, Eq)]
pub struct CustomCursorIcon {
    pub(crate) inner: PlatformCustomCursorIcon,
}

#[allow(dead_code)] // These are not used on every platform
mod constructors {
    use super::*;

    impl NoWindowIcon {
        pub fn from_rgba(_rgba: Vec<u8>, _size: PhysicalSize<u32>) -> Result<Self, io::Error> {
            Ok(Self)
        }

        pub fn from_rgba_fn<F>(_get_icon: F) -> Self
        where
            F: 'static
                + FnMut(
                    PhysicalSize<u32>,
                    f64,
                )
                    -> Result<RgbaBuffer<Box<[u8]>>, Box<dyn Error + Send + Sync>>,
        {
            Self
        }
    }

    impl NoCursorIcon {
        pub fn from_rgba(
            _rgba: Vec<u8>,
            _size: PhysicalSize<u32>,
            _hot_spot: PhysicalPosition<u32>,
        ) -> Result<Self, io::Error> {
            Ok(Self)
        }

        pub fn from_rgba_fn<F>(_get_icon: F) -> Self
        where
            F: 'static
                + FnMut(
                    PhysicalSize<u32>,
                    f64,
                ) -> Result<
                    (RgbaBuffer<Box<[u8]>>, PhysicalPosition<u32>),
                    Box<dyn Error + Send + Sync>,
                >,
        {
            Self
        }
    }
}

impl<I: Deref<Target = [u8]>> fmt::Debug for RgbaBuffer<I> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> Result<(), fmt::Error> {
        let RgbaBuffer { rgba, size } = self;
        f.debug_struct("RgbaBuffer")
            .field("size", &size)
            .field("rgba", &(&**rgba as *const [u8]))
            .finish()
    }
}
impl<I: Deref<Target = [u8]>> RgbaBuffer<I> {
    /// Creates a `RgbaBuffer` from 32bpp RGBA data.
    ///
    /// ## Panics
    /// Panics if the length of `rgba` is not divisible by 4, or if `width * height` doesn't
    /// equal `rgba.len() / 4`.
    pub fn from_rgba(rgba: I, size: PhysicalSize<u32>) -> Self {
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

        RgbaBuffer { rgba, size }
    }

    pub fn into_custom_window_icon(self) -> Result<CustomWindowIcon, io::Error> {
        CustomWindowIcon::from_rgba(&*self.rgba, self.size)
    }

    pub fn into_custom_cursor_icon(
        self,
        hot_spot: PhysicalPosition<u32>,
    ) -> Result<CustomCursorIcon, io::Error> {
        CustomCursorIcon::from_rgba(&*self.rgba, self.size, hot_spot)
    }
}

impl fmt::Debug for CustomWindowIcon {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> Result<(), fmt::Error> {
        fmt::Debug::fmt(&self.inner, formatter)
    }
}

impl fmt::Debug for CustomCursorIcon {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> Result<(), fmt::Error> {
        fmt::Debug::fmt(&self.inner, formatter)
    }
}

impl CustomWindowIcon {
    /// Creates an `Icon` from 32bpp RGBA data.
    ///
    /// ## Panics
    /// Panics if the length of `rgba` is not divisible by 4, or if `width * height` doesn't equal
    /// `rgba.len() / 4`.
    pub fn from_rgba(rgba: &[u8], size: PhysicalSize<u32>) -> Result<Self, io::Error> {
        Ok(CustomWindowIcon {
            inner: PlatformCustomWindowIcon::from_rgba(rgba.into(), size)?,
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
    pub fn from_rgba_fn<F, B>(mut get_icon: F) -> Self
    where
        F: 'static
            + FnMut(
                PhysicalSize<u32>,
                f64,
            ) -> Result<RgbaBuffer<B>, Box<dyn 'static + Error + Send + Sync>>,
        B: Deref<Target = [u8]> + Into<Box<[u8]>>,
    {
        CustomWindowIcon {
            inner: PlatformCustomWindowIcon::from_rgba_fn(move |size, scale_factor| {
                let icon = get_icon(size, scale_factor)?;
                Ok(RgbaBuffer {
                    rgba: icon.rgba.into(),
                    size: icon.size,
                })
            }),
        }
    }
}

impl CustomCursorIcon {
    /// Creates an `Icon` from 32bpp RGBA data, with a defined cursor hot spot. The hot spot is
    /// the exact pixel in the icon image where the cursor clicking point is, and is ignored when
    /// the icon is used as a window icon.
    ///
    /// ## Panics
    /// Panics if the length of `rgba` is not divisible by 4, or if `width * height` doesn't equal
    /// `rgba.len() / 4`.
    pub fn from_rgba(
        rgba: &[u8],
        size: PhysicalSize<u32>,
        hot_spot: PhysicalPosition<u32>,
    ) -> Result<Self, io::Error> {
        Ok(CustomCursorIcon {
            inner: PlatformCustomCursorIcon::from_rgba(rgba.into(), size, hot_spot)?,
        })
    }

    pub fn from_rgba_fn<F, B>(mut get_icon: F) -> Self
    where
        F: 'static
            + FnMut(
                PhysicalSize<u32>,
                f64,
            )
                -> Result<(RgbaBuffer<B>, PhysicalPosition<u32>), Box<dyn Error + Send + Sync>>,
        B: Deref<Target = [u8]> + Into<Box<[u8]>>,
    {
        CustomCursorIcon {
            inner: PlatformCustomCursorIcon::from_rgba_fn(move |size, scale_factor| {
                let (icon, hot_spot) = get_icon(size, scale_factor)?;
                Ok((
                    RgbaBuffer {
                        rgba: icon.rgba.into(),
                        size: icon.size,
                    },
                    hot_spot,
                ))
            }),
        }
    }
}
