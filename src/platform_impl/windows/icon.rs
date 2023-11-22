use std::{ffi::c_void, fmt, io, mem, path::Path, sync::Arc};

use cursor_icon::CursorIcon;
use windows_sys::{
    core::PCWSTR,
    Win32::{
        Foundation::HWND,
        Graphics::Gdi::{
            CreateBitmap, CreateCompatibleBitmap, DeleteObject, GetDC, ReleaseDC, SetBitmapBits,
        },
        UI::WindowsAndMessaging::{
            CreateIcon, CreateIconIndirect, DestroyCursor, DestroyIcon, LoadImageW, SendMessageW,
            HCURSOR, HICON, ICONINFO, ICON_BIG, ICON_SMALL, IMAGE_ICON, LR_DEFAULTSIZE,
            LR_LOADFROMFILE, WM_SETICON,
        },
    },
};

use crate::icon::*;
use crate::{cursor::CursorImage, dpi::PhysicalSize};

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

#[derive(Clone, Debug)]
pub struct WinCursor {
    handle: Arc<HCURSOR>,
}

impl WinCursor {
    pub fn as_raw_handle(&self) -> HCURSOR {
        *self.handle
    }

    fn from_handle(handle: HCURSOR) -> Self {
        Self {
            handle: Arc::new(handle),
        }
    }

    pub fn new(image: &CursorImage) -> Result<Self, io::Error> {
        let mut bgra = image.rgba.clone();
        bgra.chunks_exact_mut(4).for_each(|chunk| chunk.swap(0, 2));

        let w = image.width as i32;
        let h = image.height as i32;

        unsafe {
            let hdc_screen = GetDC(0);
            if hdc_screen == 0 {
                return Err(io::Error::last_os_error());
            }
            let hbm_color = CreateCompatibleBitmap(hdc_screen, w, h);
            ReleaseDC(0, hdc_screen);
            if hbm_color == 0 {
                return Err(io::Error::last_os_error());
            }
            if SetBitmapBits(hbm_color, bgra.len() as u32, bgra.as_ptr() as *const c_void) == 0 {
                DeleteObject(hbm_color);
                return Err(io::Error::last_os_error());
            };

            // Mask created according to https://learn.microsoft.com/en-us/windows/win32/api/wingdi/nf-wingdi-createbitmap#parameters
            let mask_bits: Vec<u8> = vec![0xff; ((((w + 15) >> 4) << 1) * h) as usize];
            let hbm_mask = CreateBitmap(w, h, 1, 1, mask_bits.as_ptr() as *const _);
            if hbm_mask == 0 {
                DeleteObject(hbm_color);
                return Err(io::Error::last_os_error());
            }

            let icon_info = ICONINFO {
                fIcon: 0,
                xHotspot: image.hotspot_x,
                yHotspot: image.hotspot_y,
                hbmMask: hbm_mask,
                hbmColor: hbm_color,
            };

            let handle = CreateIconIndirect(&icon_info as *const _);
            DeleteObject(hbm_color);
            DeleteObject(hbm_mask);
            if handle == 0 {
                return Err(io::Error::last_os_error());
            }

            Ok(Self::from_handle(handle))
        }
    }
}

impl Drop for WinCursor {
    fn drop(&mut self) {
        unsafe { DestroyCursor(self.as_raw_handle()) };
    }
}

#[derive(Debug, Clone)]
pub enum SelectedCursor {
    Named(CursorIcon),
    Custom(WinCursor),
}

impl Default for SelectedCursor {
    fn default() -> Self {
        Self::Named(Default::default())
    }
}
