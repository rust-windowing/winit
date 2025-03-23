use std::ffi::c_void;
use std::path::Path;
use std::sync::Arc;
use std::{fmt, io, mem, ptr};

use cursor_icon::CursorIcon;
use windows_sys::core::PCWSTR;
use windows_sys::Win32::Graphics::Gdi::{
    CreateBitmap, CreateCompatibleBitmap, DeleteObject, GetDC, ReleaseDC, SetBitmapBits,
};
use windows_sys::Win32::UI::WindowsAndMessaging::{
    CreateIcon, CreateIconIndirect, DestroyCursor, DestroyIcon, LoadImageW, HCURSOR, HICON,
    ICONINFO, ICON_BIG, ICON_SMALL, IMAGE_ICON, LR_DEFAULTSIZE, LR_LOADFROMFILE,
};

use super::util;
use crate::cursor::{CursorImage, CustomCursorProvider};
use crate::dpi::PhysicalSize;
use crate::error::RequestError;
use crate::icon::*;
use crate::platform::windows::WinIcon;

unsafe impl Send for WinIcon {}

impl WinIcon {
    /// Create an icon from a file path.
    ///
    /// Specify `size` to load a specific icon size from the file, or `None` to load the default
    /// icon size from the file.
    ///
    /// In cases where the specified size does not exist in the file, Windows may perform scaling
    /// to get an icon of the desired size.
    pub fn from_path<P: AsRef<Path>>(
        path: P,
        size: Option<PhysicalSize<u32>>,
    ) -> Result<Self, BadIcon> {
        // width / height of 0 along with LR_DEFAULTSIZE tells windows to load the default icon size
        let (width, height) = size.map(Into::into).unwrap_or((0, 0));

        let wide_path = util::encode_wide(path.as_ref());

        let handle = unsafe {
            LoadImageW(
                ptr::null_mut(),
                wide_path.as_ptr(),
                IMAGE_ICON,
                width,
                height,
                LR_DEFAULTSIZE | LR_LOADFROMFILE,
            )
        };
        if !handle.is_null() {
            Ok(WinIcon::from_handle(handle as HICON))
        } else {
            Err(BadIcon::OsError(io::Error::last_os_error()))
        }
    }

    /// Create an icon from a resource embedded in this executable or library by its ordinal id.
    ///
    /// The valid `ordinal` values range from 1 to [`u16::MAX`] (inclusive). The value `0` is an
    /// invalid ordinal id, but it can be used with [`from_resource_name`] as `"0"`.
    ///
    /// [`from_resource_name`]: Self::from_resource_name
    ///
    /// Specify `size` to load a specific icon size from the file, or `None` to load the default
    /// icon size from the file.
    ///
    /// In cases where the specified size does not exist in the file, Windows may perform scaling
    /// to get an icon of the desired size.
    pub fn from_resource(
        resource_id: u16,
        size: Option<PhysicalSize<u32>>,
    ) -> Result<Self, BadIcon> {
        Self::from_resource_ptr(resource_id as PCWSTR, size)
    }

    /// Create an icon from a resource embedded in this executable or library by its name.
    ///
    /// Specify `size` to load a specific icon size from the file, or `None` to load the default
    /// icon size from the file.
    ///
    /// In cases where the specified size does not exist in the file, Windows may perform scaling
    /// to get an icon of the desired size.
    ///
    /// # Notes
    ///
    /// Consider the following resource definition statements:
    /// ```rc
    /// app     ICON "app.ico"
    /// 1       ICON "a.ico"
    /// 0027    ICON "custom.ico"
    /// 0       ICON "alt.ico"
    /// ```
    ///
    /// Due to some internal implementation details of the resource embedding/loading process on
    /// Windows platform, strings that can be interpreted as 16-bit unsigned integers (`"1"`,
    /// `"002"`, etc.) cannot be used as valid resource names, and instead should be passed into
    /// [`from_resource`]:
    ///
    /// [`from_resource`]: Self::from_resource
    ///
    /// ```rust,no_run
    /// use winit::platform::windows::IconExtWindows;
    /// use winit::window::Icon;
    ///
    /// assert!(Icon::from_resource_name("app", None).is_ok());
    /// assert!(Icon::from_resource(1, None).is_ok());
    /// assert!(Icon::from_resource(27, None).is_ok());
    /// assert!(Icon::from_resource_name("27", None).is_err());
    /// assert!(Icon::from_resource_name("0027", None).is_err());
    /// ```
    ///
    /// While `0` cannot be used as an ordinal id (see [`from_resource`]), it can be used as a
    /// name:
    ///
    /// [`from_resource`]: IconExtWindows::from_resource
    ///
    /// ```rust,no_run
    /// # use winit::platform::windows::IconExtWindows;
    /// # use winit::window::Icon;
    /// assert!(Icon::from_resource_name("0", None).is_ok());
    /// assert!(Icon::from_resource(0, None).is_err());
    /// ```
    pub fn from_resource_name(
        resource_name: &str,
        size: Option<PhysicalSize<u32>>,
    ) -> Result<Self, BadIcon> {
        let wide_name = util::encode_wide(resource_name);
        Self::from_resource_ptr(wide_name.as_ptr(), size)
    }

    fn from_resource_ptr(
        resource: PCWSTR,
        size: Option<PhysicalSize<u32>>,
    ) -> Result<Self, BadIcon> {
        // width / height of 0 along with LR_DEFAULTSIZE tells windows to load the default icon size
        let (width, height) = size.map(Into::into).unwrap_or((0, 0));
        let handle = unsafe {
            LoadImageW(
                util::get_instance_handle(),
                resource,
                IMAGE_ICON,
                width,
                height,
                LR_DEFAULTSIZE,
            )
        };
        if !handle.is_null() {
            Ok(WinIcon::from_handle(handle as HICON))
        } else {
            Err(BadIcon::OsError(io::Error::last_os_error()))
        }
    }

    pub(crate) fn as_raw_handle(&self) -> HICON {
        self.inner.handle
    }

    pub(crate) fn from_rgba(rgba: &RgbaIcon) -> Result<Self, BadIcon> {
        let pixel_count = rgba.rgba.len() / PIXEL_SIZE;
        let mut and_mask = Vec::with_capacity(pixel_count);
        let pixels = unsafe {
            std::slice::from_raw_parts_mut(rgba.rgba.as_ptr() as *mut Pixel, pixel_count)
        };
        for pixel in pixels {
            and_mask.push(pixel.a.wrapping_sub(u8::MAX)); // invert alpha channel
            pixel.convert_to_bgra();
        }
        assert_eq!(and_mask.len(), pixel_count);
        let handle = unsafe {
            CreateIcon(
                ptr::null_mut(),
                rgba.width as i32,
                rgba.height as i32,
                1,
                (PIXEL_SIZE * 8) as u8,
                and_mask.as_ptr(),
                rgba.rgba.as_ptr(),
            )
        };
        if !handle.is_null() {
            Ok(WinIcon::from_handle(handle))
        } else {
            Err(BadIcon::OsError(io::Error::last_os_error()))
        }
    }

    fn from_handle(handle: HICON) -> Self {
        Self { inner: Arc::new(RaiiIcon { handle }) }
    }
}

impl IconProvider for WinIcon {}

impl fmt::Debug for WinIcon {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> Result<(), fmt::Error> {
        (*self.inner).fmt(formatter)
    }
}

impl Pixel {
    fn convert_to_bgra(&mut self) {
        mem::swap(&mut self.r, &mut self.b);
    }
}

#[derive(Debug, Clone, Copy)]
pub enum IconType {
    Small = ICON_SMALL as isize,
    Big = ICON_BIG as isize,
}

#[derive(Debug, PartialEq, Eq, Hash)]
pub(crate) struct RaiiIcon {
    handle: HICON,
}

unsafe impl Send for RaiiIcon {}
unsafe impl Sync for RaiiIcon {}

impl Drop for RaiiIcon {
    fn drop(&mut self) {
        unsafe { DestroyIcon(self.handle) };
    }
}

#[derive(Debug, Clone)]
pub enum SelectedCursor {
    Named(CursorIcon),
    Custom(Arc<RaiiCursor>),
}

impl Default for SelectedCursor {
    fn default() -> Self {
        Self::Named(Default::default())
    }
}

#[derive(Clone, Debug, Hash, Eq, PartialEq)]
pub struct WinCursor(pub(super) Arc<RaiiCursor>);

impl CustomCursorProvider for WinCursor {
    fn is_animated(&self) -> bool {
        false
    }
}

impl WinCursor {
    pub(crate) fn new(image: &CursorImage) -> Result<Self, RequestError> {
        let mut bgra = image.rgba.clone();
        bgra.chunks_exact_mut(4).for_each(|chunk| chunk.swap(0, 2));

        let w = image.width as i32;
        let h = image.height as i32;

        unsafe {
            let hdc_screen = GetDC(ptr::null_mut());
            if hdc_screen.is_null() {
                return Err(os_error!(io::Error::last_os_error()).into());
            }
            let hbm_color = CreateCompatibleBitmap(hdc_screen, w, h);
            ReleaseDC(ptr::null_mut(), hdc_screen);
            if hbm_color.is_null() {
                return Err(os_error!(io::Error::last_os_error()).into());
            }
            if SetBitmapBits(hbm_color, bgra.len() as u32, bgra.as_ptr() as *const c_void) == 0 {
                DeleteObject(hbm_color);
                return Err(os_error!(io::Error::last_os_error()).into());
            };

            // Mask created according to https://learn.microsoft.com/en-us/windows/win32/api/wingdi/nf-wingdi-createbitmap#parameters
            let mask_bits: Vec<u8> = vec![0xff; ((((w + 15) >> 4) << 1) * h) as usize];
            let hbm_mask = CreateBitmap(w, h, 1, 1, mask_bits.as_ptr() as *const _);
            if hbm_mask.is_null() {
                DeleteObject(hbm_color);
                return Err(os_error!(io::Error::last_os_error()).into());
            }

            let icon_info = ICONINFO {
                fIcon: 0,
                xHotspot: image.hotspot_x as u32,
                yHotspot: image.hotspot_y as u32,
                hbmMask: hbm_mask,
                hbmColor: hbm_color,
            };

            let handle = CreateIconIndirect(&icon_info as *const _);
            DeleteObject(hbm_color);
            DeleteObject(hbm_mask);
            if handle.is_null() {
                return Err(os_error!(io::Error::last_os_error()).into());
            }

            Ok(Self(Arc::new(RaiiCursor { handle })))
        }
    }
}

#[derive(Debug, Hash, Eq, PartialEq)]
pub struct RaiiCursor {
    handle: HCURSOR,
}

unsafe impl Send for RaiiCursor {}
unsafe impl Sync for RaiiCursor {}

impl Drop for RaiiCursor {
    fn drop(&mut self) {
        unsafe { DestroyCursor(self.handle) };
    }
}

impl RaiiCursor {
    pub fn as_raw_handle(&self) -> HICON {
        self.handle
    }
}
