use std::{self, mem, ptr};
use std::os::windows::ffi::OsStrExt;
use std::path::Path;

use winapi::ctypes::{c_int, wchar_t};
use winapi::shared::minwindef::{BYTE, LPARAM, WPARAM};
use winapi::shared::windef::{HICON, HWND};
use winapi::um::winuser;

use {Pixel, PIXEL_SIZE, Icon};
use platform::platform::util;

impl Pixel {
    fn to_bgra(&mut self) {
        mem::swap(&mut self.r, &mut self.b);
    }
}

#[derive(Debug)]
pub enum IconType {
    Small = winuser::ICON_SMALL as isize,
    Big = winuser::ICON_BIG as isize,
}

#[derive(Debug)]
pub struct WinIcon {
    pub handle: HICON,
}

impl WinIcon {
    #[allow(dead_code)]
    pub fn from_path<P: AsRef<Path>>(path: P) -> Result<Self, util::WinError> {
        let wide_path: Vec<u16> = path.as_ref().as_os_str().encode_wide().collect();
        let handle = unsafe {
            winuser::LoadImageW(
                ptr::null_mut(),
                wide_path.as_ptr() as *const wchar_t,
                winuser::IMAGE_ICON,
                0, // 0 indicates that we want to use the actual width
                0, // and height
                winuser::LR_LOADFROMFILE,
            ) as HICON
        };
        if !handle.is_null() {
            Ok(WinIcon { handle })
        } else {
            Err(util::WinError::from_last_error())
        }
    }

    pub fn from_icon(icon: Icon) -> Result<Self, util::WinError> {
        Self::from_rgba(icon.rgba, icon.width, icon.height)
    }

    pub fn from_rgba(mut rgba: Vec<u8>, width: u32, height: u32) -> Result<Self, util::WinError> {
        assert_eq!(rgba.len() % PIXEL_SIZE, 0);
        let pixel_count = rgba.len() / PIXEL_SIZE;
        assert_eq!(pixel_count, (width * height) as usize);
        let mut and_mask = Vec::with_capacity(pixel_count);
        let pixels = rgba.as_mut_ptr() as *mut Pixel; // how not to write idiomatic Rust
        for pixel_index in 0..pixel_count {
            let pixel = unsafe { &mut *pixels.offset(pixel_index as isize) };
            and_mask.push(pixel.a.wrapping_sub(std::u8::MAX)); // invert alpha channel
            pixel.to_bgra();
        }
        assert_eq!(and_mask.len(), pixel_count);
        let handle = unsafe {
            winuser::CreateIcon(
                ptr::null_mut(),
                width as c_int,
                height as c_int,
                1,
                (PIXEL_SIZE * 8) as BYTE,
                and_mask.as_ptr() as *const BYTE,
                rgba.as_ptr() as *const BYTE,
            ) as HICON
        };
        if !handle.is_null() {
            Ok(WinIcon { handle })
        } else {
            Err(util::WinError::from_last_error())
        }
    }

    pub fn set_for_window(&self, hwnd: HWND, icon_type: IconType) {
        unsafe {
            winuser::SendMessageW(
                hwnd,
                winuser::WM_SETICON,
                icon_type as WPARAM,
                self.handle as LPARAM,
            );
        }
    }
}

impl Drop for WinIcon {
    fn drop(&mut self) {
        unsafe { winuser::DestroyIcon(self.handle) };
    }
}

pub fn unset_for_window(hwnd: HWND, icon_type: IconType) {
    unsafe {
        winuser::SendMessageW(
            hwnd,
            winuser::WM_SETICON,
            icon_type as WPARAM,
            0 as LPARAM,
        );
    }
}
