use std::{ffi, fmt, io, mem, path::Path, ptr, sync::Arc};

use windows_sys::Win32::Graphics::Gdi::CreateBitmap;
use windows_sys::Win32::UI::WindowsAndMessaging::{CreateIconIndirect, ICONINFO};
use windows_sys::{
    core::PCWSTR,
    Win32::{
        Foundation::HWND,
        UI::WindowsAndMessaging::{
            DestroyIcon, LoadImageW, SendMessageW, HICON, ICON_BIG, ICON_SMALL, IMAGE_ICON,
            LR_DEFAULTSIZE, LR_LOADFROMFILE, WM_SETICON,
        },
    },
};

use crate::dpi::PhysicalSize;
use crate::icon::*;

use super::util;

impl Pixel {
    fn convert_to_bgra(&mut self) {
        mem::swap(&mut self.r, &mut self.b);
    }
}

impl RgbaIcon {
    fn into_windows_icon(self) -> Result<WinIcon, BadIcon> {
        let rgba = self.rgba;
        let pixel_count = rgba.len() / PIXEL_SIZE;
        let pixels =
            unsafe { std::slice::from_raw_parts_mut(rgba.as_ptr() as *mut Pixel, pixel_count) };
        for pixel in pixels {
            pixel.convert_to_bgra();
        }
        let handle = unsafe {
            let hbm_mask = CreateBitmap(self.width as i32, self.height as i32, 1, 1, ptr::null());
            let hbm_color = CreateBitmap(
                self.width as i32,
                self.height as i32,
                1,
                (PIXEL_SIZE * 8) as u32,
                rgba.as_ptr() as *const ffi::c_void,
            );

            let icon_info = ICONINFO {
                fIcon: 0,
                xHotspot: self.hotspot_x,
                yHotspot: self.hotspot_y,
                hbmMask: hbm_mask,
                hbmColor: hbm_color,
            };

            CreateIconIndirect(&icon_info as *const _)
        };
        if handle != 0 {
            Ok(WinIcon::from_handle(handle))
        } else {
            Err(BadIcon::OsError(io::Error::last_os_error()))
        }
    }
}

#[derive(Debug)]
pub enum IconType {
    Small = ICON_SMALL as isize,
    Big = ICON_BIG as isize,
}

#[derive(Debug)]
struct RaiiIcon {
    handle: HICON,
}

#[derive(Clone)]
pub struct WinIcon {
    inner: Arc<RaiiIcon>,
}

unsafe impl Send for WinIcon {}

impl WinIcon {
    pub fn as_raw_handle(&self) -> HICON {
        self.inner.handle
    }

    pub fn from_path<P: AsRef<Path>>(
        path: P,
        size: Option<PhysicalSize<u32>>,
    ) -> Result<Self, BadIcon> {
        // width / height of 0 along with LR_DEFAULTSIZE tells windows to load the default icon size
        let (width, height) = size.map(Into::into).unwrap_or((0, 0));

        let wide_path = util::encode_wide(path.as_ref());

        let handle = unsafe {
            LoadImageW(
                0,
                wide_path.as_ptr(),
                IMAGE_ICON,
                width,
                height,
                LR_DEFAULTSIZE | LR_LOADFROMFILE,
            )
        };
        if handle != 0 {
            Ok(WinIcon::from_handle(handle as HICON))
        } else {
            Err(BadIcon::OsError(io::Error::last_os_error()))
        }
    }

    pub fn from_resource(
        resource_id: u16,
        size: Option<PhysicalSize<u32>>,
    ) -> Result<Self, BadIcon> {
        // width / height of 0 along with LR_DEFAULTSIZE tells windows to load the default icon size
        let (width, height) = size.map(Into::into).unwrap_or((0, 0));
        let handle = unsafe {
            LoadImageW(
                util::get_instance_handle(),
                resource_id as PCWSTR,
                IMAGE_ICON,
                width,
                height,
                LR_DEFAULTSIZE,
            )
        };
        if handle != 0 {
            Ok(WinIcon::from_handle(handle as HICON))
        } else {
            Err(BadIcon::OsError(io::Error::last_os_error()))
        }
    }

    pub fn from_rgba(
        rgba: Vec<u8>,
        width: u32,
        height: u32,
        hotspot_x: u32,
        hotspot_y: u32,
    ) -> Result<Self, BadIcon> {
        let rgba_icon = RgbaIcon::from_rgba(rgba, width, height, hotspot_x, hotspot_y)?;
        rgba_icon.into_windows_icon()
    }

    pub fn set_for_window(&self, hwnd: HWND, icon_type: IconType) {
        unsafe {
            SendMessageW(hwnd, WM_SETICON, icon_type as usize, self.as_raw_handle());
        }
    }

    fn from_handle(handle: HICON) -> Self {
        Self {
            inner: Arc::new(RaiiIcon { handle }),
        }
    }
}

impl Drop for RaiiIcon {
    fn drop(&mut self) {
        unsafe { DestroyIcon(self.handle) };
    }
}

impl fmt::Debug for WinIcon {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> Result<(), fmt::Error> {
        (*self.inner).fmt(formatter)
    }
}

pub fn unset_for_window(hwnd: HWND, icon_type: IconType) {
    unsafe {
        SendMessageW(hwnd, WM_SETICON, icon_type as usize, 0);
    }
}
