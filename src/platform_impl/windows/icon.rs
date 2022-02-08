use std::{fmt, io, iter::once, mem, os::windows::ffi::OsStrExt, path::Path, ptr, sync::Arc};

use winapi::{
    ctypes::{c_int, wchar_t},
    shared::{
        minwindef::{BYTE, LPARAM, WORD, WPARAM},
        windef::{HICON, HWND},
    },
    um::libloaderapi,
    um::winuser,
};

use crate::dpi::PhysicalSize;
use crate::icon::*;

impl Pixel {
    fn to_bgra(&mut self) {
        mem::swap(&mut self.r, &mut self.b);
    }
}

impl RgbaIcon {
    fn into_windows_icon(self) -> Result<WinIcon, BadIcon> {
        let mut rgba = self.rgba;
        let pixel_count = rgba.len() / PIXEL_SIZE;
        let mut and_mask = Vec::with_capacity(pixel_count);
        let pixels =
            unsafe { std::slice::from_raw_parts_mut(rgba.as_mut_ptr() as *mut Pixel, pixel_count) };
        for pixel in pixels {
            and_mask.push(pixel.a.wrapping_sub(std::u8::MAX)); // invert alpha channel
            pixel.to_bgra();
        }
        assert_eq!(and_mask.len(), pixel_count);
        let handle = unsafe {
            winuser::CreateIcon(
                ptr::null_mut(),
                self.width as c_int,
                self.height as c_int,
                1,
                (PIXEL_SIZE * 8) as BYTE,
                and_mask.as_ptr() as *const BYTE,
                rgba.as_ptr() as *const BYTE,
            ) as HICON
        };
        if !handle.is_null() {
            Ok(WinIcon::from_handle(handle))
        } else {
            Err(BadIcon::OsError(io::Error::last_os_error()))
        }
    }
}

#[derive(Debug)]
pub enum IconType {
    Small = winuser::ICON_SMALL as isize,
    Big = winuser::ICON_BIG as isize,
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
        let wide_path: Vec<u16> = path
            .as_ref()
            .as_os_str()
            .encode_wide()
            .chain(once(0))
            .collect();

        // width / height of 0 along with LR_DEFAULTSIZE tells windows to load the default icon size
        let (width, height) = size.map(Into::into).unwrap_or((0, 0));

        let handle = unsafe {
            winuser::LoadImageW(
                ptr::null_mut(),
                wide_path.as_ptr() as *const wchar_t,
                winuser::IMAGE_ICON,
                width as c_int,
                height as c_int,
                winuser::LR_DEFAULTSIZE | winuser::LR_LOADFROMFILE,
            ) as HICON
        };
        if !handle.is_null() {
            Ok(WinIcon::from_handle(handle))
        } else {
            Err(BadIcon::OsError(io::Error::last_os_error()))
        }
    }

    pub fn from_resource(
        resource_id: WORD,
        size: Option<PhysicalSize<u32>>,
    ) -> Result<Self, BadIcon> {
        // width / height of 0 along with LR_DEFAULTSIZE tells windows to load the default icon size
        let (width, height) = size.map(Into::into).unwrap_or((0, 0));
        let handle = unsafe {
            winuser::LoadImageW(
                libloaderapi::GetModuleHandleW(ptr::null_mut()),
                winuser::MAKEINTRESOURCEW(resource_id),
                winuser::IMAGE_ICON,
                width as c_int,
                height as c_int,
                winuser::LR_DEFAULTSIZE,
            ) as HICON
        };
        if !handle.is_null() {
            Ok(WinIcon::from_handle(handle))
        } else {
            Err(BadIcon::OsError(io::Error::last_os_error()))
        }
    }

    pub fn from_rgba(rgba: Vec<u8>, width: u32, height: u32) -> Result<Self, BadIcon> {
        let rgba_icon = RgbaIcon::from_rgba(rgba, width, height)?;
        rgba_icon.into_windows_icon()
    }

    pub fn set_for_window(&self, hwnd: HWND, icon_type: IconType) {
        unsafe {
            winuser::SendMessageW(
                hwnd,
                winuser::WM_SETICON,
                icon_type as WPARAM,
                self.as_raw_handle() as LPARAM,
            );
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
        unsafe { winuser::DestroyIcon(self.handle) };
    }
}

impl fmt::Debug for WinIcon {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> Result<(), fmt::Error> {
        (*self.inner).fmt(formatter)
    }
}

pub fn unset_for_window(hwnd: HWND, icon_type: IconType) {
    unsafe {
        winuser::SendMessageW(hwnd, winuser::WM_SETICON, icon_type as WPARAM, 0 as LPARAM);
    }
}
