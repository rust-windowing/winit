use std::{ffi::c_void, fmt, io, mem, path::Path, sync::Arc};

use windows_sys::{
    core::PCWSTR,
    Win32::{
        Foundation::HWND,
        Graphics::Gdi::{
            CreateBitmap, CreateCompatibleBitmap, DeleteObject, GetDC, ReleaseDC, SetBitmapBits,
        },
        UI::WindowsAndMessaging::{
            CreateIcon, CreateIconIndirect, DestroyIcon, LoadImageW, SendMessageW, HICON, ICONINFO,
            ICON_BIG, ICON_SMALL, IMAGE_ICON, LR_DEFAULTSIZE, LR_LOADFROMFILE, WM_SETICON,
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

impl Icon {
    /// Creates an cursor icon from 32bpp RGBA data with hotspot
    ///
    /// The length of `rgba` must be divisible by 4, and `width * height` must equal
    /// `rgba.len() / 4`. Otherwise, this will return a `BadIcon` error.
    pub fn from_rgba_cursor(
        rgba: Vec<u8>,
        width: u32,
        height: u32,
        hotspot_x: u32,
        hotspot_y: u32,
    ) -> Result<Self, BadIcon> {
        Ok(Icon {
            inner: WinIcon::from_rgba_cursor(rgba, width, height, hotspot_x, hotspot_y)?,
        })
    }
}

impl RgbaIcon {
    fn into_windows_icon(self) -> Result<WinIcon, BadIcon> {
        let rgba = self.rgba;
        let pixel_count = rgba.len() / PIXEL_SIZE;
        let mut and_mask = Vec::with_capacity(pixel_count);
        let pixels =
            unsafe { std::slice::from_raw_parts_mut(rgba.as_ptr() as *mut Pixel, pixel_count) };
        for pixel in pixels {
            and_mask.push(pixel.a.wrapping_sub(std::u8::MAX)); // invert alpha channel
            pixel.convert_to_bgra();
        }
        assert_eq!(and_mask.len(), pixel_count);
        let handle = unsafe {
            CreateIcon(
                0,
                self.width as i32,
                self.height as i32,
                1,
                (PIXEL_SIZE * 8) as u8,
                and_mask.as_ptr(),
                rgba.as_ptr(),
            )
        };
        if handle != 0 {
            Ok(WinIcon::from_handle(handle))
        } else {
            Err(BadIcon::OsError(io::Error::last_os_error()))
        }
    }
    fn into_windows_cursor_icon(self, hotspot_x: u32, hotspot_y: u32) -> Result<WinIcon, BadIcon> {
        let mut rgba = self.rgba;

        // convert to bgra
        rgba.chunks_mut(4).for_each(|chunk| {
            chunk.swap(0, 2);
        });

        let handle = unsafe {
            // always visible for each pixel
            let mask_bits: Vec<u8> = vec![0xff; (((self.width + 15) >> 3) * self.height) as usize];
            let hbm_mask = CreateBitmap(
                self.width as i32,
                self.height as i32,
                1,
                1,
                mask_bits.as_ptr() as *const _,
            );
            if hbm_mask == 0 {
                return Err(BadIcon::OsError(io::Error::last_os_error()));
            }

            let hdc_screen = GetDC(0);
            if hdc_screen == 0 {
                return Err(BadIcon::OsError(io::Error::last_os_error()));
            }

            // uses CreateCompatibleBitmap instead of CreateBitmap according to MSDN
            // https://learn.microsoft.com/en-us/windows/win32/api/wingdi/nf-wingdi-createbitmap
            let hbm_color =
                CreateCompatibleBitmap(hdc_screen, self.width as i32, self.height as i32);

            if hbm_color == 0 {
                DeleteObject(hbm_mask);
                ReleaseDC(0, hdc_screen);
                return Err(BadIcon::OsError(io::Error::last_os_error()));
            }

            SetBitmapBits(hbm_color, rgba.len() as u32, rgba.as_ptr() as *const c_void);

            ReleaseDC(0, hdc_screen);

            let icon_info = ICONINFO {
                fIcon: 0,
                xHotspot: hotspot_x,
                yHotspot: hotspot_y,
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

    pub fn from_rgba(rgba: Vec<u8>, width: u32, height: u32) -> Result<Self, BadIcon> {
        let rgba_icon = RgbaIcon::from_rgba(rgba, width, height)?;
        rgba_icon.into_windows_icon()
    }

    pub fn from_rgba_cursor(
        rgba: Vec<u8>,
        width: u32,
        height: u32,
        hotspot_x: u32,
        hotspot_y: u32,
    ) -> Result<Self, BadIcon> {
        let rgba_icon = RgbaIcon::from_rgba(rgba, width, height)?;
        rgba_icon.into_windows_cursor_icon(hotspot_x, hotspot_y)
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
