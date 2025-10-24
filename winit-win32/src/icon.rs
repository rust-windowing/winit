use std::ffi::c_void;
use std::path::Path;
use std::sync::Arc;
use std::{fmt, io, mem, ptr};

use cursor_icon::CursorIcon;
use dpi::PhysicalSize;
use windows_sys::Win32::Graphics::Gdi::{
    CreateBitmap, CreateCompatibleBitmap, DeleteObject, GetDC, ReleaseDC, SetBitmapBits,
};
use windows_sys::Win32::UI::WindowsAndMessaging::{
    CreateIcon, CreateIconIndirect, DestroyCursor, DestroyIcon, HCURSOR, HICON, ICON_BIG,
    ICON_SMALL, ICONINFO, IMAGE_ICON, LR_DEFAULTSIZE, LR_LOADFROMFILE, LoadImageW,
};
use windows_sys::core::PCWSTR;
use winit_core::cursor::{CursorImage, CustomCursorProvider};
use winit_core::error::RequestError;
use winit_core::icon::*;

use super::util;
use crate::WinIcon;

pub(crate) const PIXEL_SIZE: usize = mem::size_of::<Pixel>();

unsafe impl Send for WinIcon {}

impl WinIcon {
    pub(crate) fn from_path_impl<P: AsRef<Path>>(
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

    pub(crate) fn from_resource_impl(
        resource_id: u16,
        size: Option<PhysicalSize<u32>>,
    ) -> Result<Self, BadIcon> {
        Self::from_resource_ptr(resource_id as PCWSTR, size)
    }

    pub(crate) fn from_resource_name_impl(
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
        let pixel_count = rgba.buffer().len() / PIXEL_SIZE;
        let mut and_mask = Vec::with_capacity(pixel_count);
        let pixels = unsafe {
            std::slice::from_raw_parts_mut(rgba.buffer().as_ptr() as *mut Pixel, pixel_count)
        };
        for pixel in pixels {
            and_mask.push(pixel.a.wrapping_sub(u8::MAX)); // invert alpha channel
            pixel.convert_to_bgra();
        }
        assert_eq!(and_mask.len(), pixel_count);
        let handle = unsafe {
            CreateIcon(
                ptr::null_mut(),
                rgba.width() as i32,
                rgba.height() as i32,
                1,
                (PIXEL_SIZE * 8) as u8,
                and_mask.as_ptr(),
                rgba.buffer().as_ptr(),
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
        let mut bgra = Vec::from(image.buffer());
        bgra.chunks_exact_mut(4).for_each(|chunk| chunk.swap(0, 2));

        let w = image.width() as i32;
        let h = image.height() as i32;

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
                xHotspot: image.hotspot_x() as u32,
                yHotspot: image.hotspot_y() as u32,
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

#[repr(C)]
#[derive(Debug)]
pub(crate) struct Pixel {
    pub(crate) r: u8,
    pub(crate) g: u8,
    pub(crate) b: u8,
    pub(crate) a: u8,
}
