use std::{fmt, io, iter::once, mem, os::windows::ffi::OsStrExt, path::Path, ptr, sync::Arc};

use winapi::{
    ctypes::{c_int, wchar_t},
    shared::{
        minwindef::{LPARAM, UINT, WORD, WPARAM},
        windef::{HICON, HWND},
    },
    um::libloaderapi,
    um::{wingdi, winuser},
};

use crate::dpi::{PhysicalSize, PhysicalPosition};
use crate::icon::*;

impl Pixel {
    fn to_bgra(&mut self) {
        mem::swap(&mut self.r, &mut self.b);
    }
}

impl RgbaIcon {
    fn into_windows_icon(self) -> Result<WinIcon, io::Error> {
        unsafe {
            let mut rgba = self.rgba;
            let pixel_count = rgba.len() / PIXEL_SIZE;
            let mut and_mask = Vec::with_capacity(pixel_count);
            let pixels =
                std::slice::from_raw_parts_mut(rgba.as_mut_ptr() as *mut Pixel, pixel_count);
            for pixel in pixels {
                and_mask.push(pixel.a.wrapping_sub(std::u8::MAX)); // invert alpha channel
                pixel.to_bgra();
            }
            assert_eq!(and_mask.len(), pixel_count);

            let width = self.size.width as c_int;
            let height = self.size.height as c_int;
            let and_bitmap = wingdi::CreateBitmap(
                width, height,
                1,
                (PIXEL_SIZE * 8) as UINT,
                and_mask.as_ptr() as *const _,
            );
            let color_bitmap = wingdi::CreateBitmap(
                width, height,
                1,
                (PIXEL_SIZE * 8) as UINT,
                rgba.as_ptr() as *const _,
            );

            let mut icon_info = winuser::ICONINFO {
                // technically a value of 0 means this is always a cursor even for window icons
                // but it doesn't seem to cause any issues so ¯\_(ツ)_/¯
                fIcon: 0,
                xHotspot: self.hot_spot.x,
                yHotspot: self.hot_spot.y,
                hbmMask: and_bitmap,
                hbmColor: color_bitmap,
            };
            let handle = winuser::CreateIconIndirect(&mut icon_info);

            wingdi::DeleteObject(and_bitmap as _);
            wingdi::DeleteObject(color_bitmap as _);

            if !handle.is_null() {
                Ok(WinIcon::from_handle(handle))
            } else {
                Err(io::Error::last_os_error())
            }
        }
    }
}

#[derive(Debug)]
pub enum IconType {
    Small = winuser::ICON_SMALL as isize,
    Big = winuser::ICON_BIG as isize,
}

#[derive(Debug, PartialEq, Eq)]
struct RaiiIcon {
    handle: HICON,
}

#[derive(Clone, PartialEq, Eq)]
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
    ) -> Result<Self, io::Error> {
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
            Err(io::Error::last_os_error())
        }
    }

    pub fn from_resource(
        resource_id: WORD,
        size: Option<PhysicalSize<u32>>,
    ) -> Result<Self, io::Error> {
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
            Err(io::Error::last_os_error())
        }
    }

    pub fn from_rgba(rgba: Vec<u8>, size: PhysicalSize<u32>) -> Result<Self, io::Error> {
        RgbaIcon::from_rgba(rgba, size)
            .into_windows_icon()
    }

    pub fn from_rgba_with_hot_spot(
        rgba: Vec<u8>,
        size: PhysicalSize<u32>,
        hot_spot: PhysicalPosition<u32>
    ) -> Result<Self, io::Error> {
        RgbaIcon::from_rgba_with_hot_spot(rgba, size, hot_spot)
            .into_windows_icon()
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
