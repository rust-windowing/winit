use std::{
    fmt, io, iter::once, mem,
    os::windows::ffi::OsStrExt, path::Path, ptr,
    sync::{Arc, atomic::{AtomicPtr, Ordering}},
    cmp::{PartialEq, Eq}, ffi::OsStr,
};
use parking_lot::Mutex;
use winapi::{
    ctypes::{c_int, wchar_t},
    shared::{
        minwindef::{LPARAM, UINT, WORD, WPARAM},
        windef::{HICON, HCURSOR, HICON__, HWND},
    },
    um::libloaderapi,
    um::{wingdi, winuser},
};
use crate::{
    dpi::{PhysicalPosition, PhysicalSize},
    icon::{Pixel, PIXEL_SIZE, RgbaIcon},
    platform_impl::platform::{util, monitor},
};

impl Pixel {
    fn to_bgra(&mut self) {
        mem::swap(&mut self.r, &mut self.b);
    }
}

impl RgbaIcon<Box<[u8]>> {
    fn into_windows_icon(self) -> Result<HICON, io::Error> {
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
                width,
                height,
                1,
                (PIXEL_SIZE * 8) as UINT,
                and_mask.as_ptr() as *const _,
            );
            let color_bitmap = wingdi::CreateBitmap(
                width,
                height,
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
                Ok(handle)
            } else {
                Err(io::Error::last_os_error())
            }
        }
    }
}

#[derive(Debug, PartialEq, Eq)]
enum RaiiIcon {
    Path(PathIcon),
    Resource(ResourceIcon),
    Single(HICON),
    Function(FunctionIcon),
}

#[derive(Debug)]
struct PathIcon {
    wide_path: Vec<u16>,
    cursor_icon_set: LazyIconSet,
    window_icon_set: LazyIconSet,
}

#[derive(Debug)]
struct ResourceIcon {
    resource_id: WORD,
    cursor_icon_set: LazyIconSet,
    window_icon_set: LazyIconSet,
}

struct FunctionIcon {
    get_icon: Box<Mutex<dyn FnMut(PhysicalSize<u32>, f64) -> Result<RgbaIcon<Box<[u8]>>, Box<dyn std::error::Error + Send + Sync>>>>,
    icon_set: LazyIconSet,
}

impl Eq for PathIcon {}
impl PartialEq for PathIcon {
    fn eq(&self, other: &Self) -> bool {
        self.wide_path == other.wide_path
    }
}

impl Eq for ResourceIcon {}
impl PartialEq for ResourceIcon {
    fn eq(&self, other: &Self) -> bool {
        self.resource_id == other.resource_id
    }
}

impl Eq for FunctionIcon {}
impl PartialEq for FunctionIcon {
    fn eq(&self, other: &Self) -> bool {
        &*self.get_icon as *const _ == &*other.get_icon as *const _
    }
}

impl fmt::Debug for FunctionIcon {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> Result<(), fmt::Error> {
        f.debug_tuple("FunctionIcon")
            .field(&(&*self.get_icon as *const Mutex<_>))
            .finish()
    }
}

type AtomicHICON = AtomicPtr<HICON__>;

#[derive(Default, Debug)]
struct LazyIconSet {
    i_16: AtomicHICON,
    i_24: AtomicHICON,
    i_32: AtomicHICON,
    i_48: AtomicHICON,
    i_64: AtomicHICON,
    i_96: AtomicHICON,
    i_128: AtomicHICON,
    i_256: AtomicHICON,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum IconSize {
    I16,
    I24,
    I32,
    I48,
    I64,
    I96,
    I128,
    I256,
}

fn cursor_scale_factor() -> f64 {
    let cursor_position = util::get_cursor_position();
    let monitor = monitor::monitor_from_position(cursor_position);
    monitor.scale_factor()
}

impl IconSize {
    pub fn adjust_for_scale_factor(&self, scale_factor: f64) -> IconSize {
        use IconSize::*;

        let num = match *self {
            I16 => 16,
            I24 => 24,
            I32 => 32,
            I48 => 48,
            I64 => 64,
            I96 => 96,
            I128 => 128,
            I256 => 256,
        };
        let scaled_num = (num as f64 * scale_factor) as u32;
        match scaled_num / 8 {
            0 |
            1 |
            2 => I16,
            3 => I24,
            4 |
            5 => I32,
            6 |
            7 => I48,
            8 |
            9 |
            10 |
            11 => I64,
            12 |
            13 |
            14 |
            15 => I96,
            16 |
            17 |
            18 |
            19 |
            20 |
            21 |
            22 |
            23 |
            24 => I128,
            25 |
            26 |
            27 |
            28 |
            29 |
            30 |
            31 |
            32 |
            _ => I256,
        }
    }
}

impl LazyIconSet {
    fn load_icon<F>(&self, icon_size: IconSize, mut load_icon: F) -> Result<HICON, io::Error>
        where F: FnMut(c_int) -> Result<HICON, io::Error>,
    {
        use IconSize::*;
        let attempt_order = match icon_size {
            I16  => [I16, I24, I32, I48, I64, I96, I128, I256],
            I24  => [I24, I16, I32, I48, I64, I96, I128, I256],
            I32  => [I32, I24, I16, I48, I64, I96, I128, I256],
            I48  => [I48, I32, I24, I16, I64, I96, I128, I256],
            I64  => [I64, I48, I32, I24, I16, I96, I128, I256],
            I96  => [I96, I64, I48, I32, I24, I16, I128, I256],
            I128 => [I128, I96, I64, I48, I32, I24, I16, I256],
            I256 => [I256, I128, I96, I64, I48, I32, I24, I16],
        };
        let mut error = None;
        for icon_size in attempt_order.iter().cloned() {
            let (hicon, dim) = match icon_size {
                I16 => (&self.i_16, 16),
                I24 => (&self.i_24, 24),
                I32 => (&self.i_32, 32),
                I48 => (&self.i_48, 48),
                I64 => (&self.i_64, 64),
                I96 => (&self.i_96, 96),
                I128 => (&self.i_128, 128),
                I256 => (&self.i_256, 256),
            };

            let current_icon = hicon.load(Ordering::SeqCst);
            let is_valid = |icon: HICON| !(icon.is_null() || icon == (1 as HICON));

            if current_icon.is_null() {
                match load_icon(dim) {
                    Ok(icon_loaded) => {
                        let old_icon = hicon.swap(icon_loaded, Ordering::SeqCst);
                        if is_valid(old_icon) {
                            unsafe{ winuser::DestroyIcon(old_icon) };
                        }
                        return Ok(icon_loaded)
                    },
                    Err(e) => {
                        warn!("could not load icon at size {0}x{0}: {1}", dim, e);
                        error = Some(e);
                        let old_icon = hicon.swap(1 as HICON, Ordering::SeqCst);
                        if is_valid(old_icon) {
                            unsafe{ winuser::DestroyIcon(old_icon) };
                        }
                        continue;
                    }
                }
            } else if current_icon == 1 as HICON {
                continue;
            } else {
                return Ok(current_icon);
            }
        }
        Err(error.unwrap_or_else(|| io::Error::new(io::ErrorKind::Other, "icon loading alreay failed")))
    }
}

impl PathIcon {
    fn load_icon(&self, icon_size: IconSize) -> Result<HICON, io::Error> {
        self.window_icon_set.load_icon(
            icon_size,
            |dim| {
                let icon = unsafe{ winuser::LoadImageW(
                    libloaderapi::GetModuleHandleW(ptr::null_mut()),
                    self.wide_path.as_ptr() as *const wchar_t,
                    winuser::IMAGE_ICON,
                    dim, dim,
                    winuser::LR_LOADFROMFILE,
                ) as HICON };
                if icon.is_null() {
                    Err(io::Error::last_os_error())
                } else {
                    Ok(icon)
                }
            }
        )
    }

    fn load_cursor(&self, icon_size: IconSize) -> Result<HCURSOR, io::Error> {
        self.window_icon_set.load_icon(
            icon_size,
            |_| {
                // Windows will automatically select an appropriately-scaled cursor from the cursor
                // file based on where the cursor is on the screen. Passing in a size manually
                // actually interferes with that, so we don't do that.
                let cursor = unsafe{ winuser::LoadImageW(
                    libloaderapi::GetModuleHandleW(ptr::null_mut()),
                    self.wide_path.as_ptr() as *const wchar_t,
                    winuser::IMAGE_CURSOR,
                    0, 0,
                    winuser::LR_LOADFROMFILE | winuser::LR_DEFAULTSIZE,
                ) as HCURSOR };
                if cursor.is_null() {
                    Err(io::Error::last_os_error())
                } else {
                    Ok(cursor)
                }
            }
        )
    }
}

impl ResourceIcon {
    fn load_icon(&self, icon_size: IconSize) -> Result<HICON, io::Error> {
        self.window_icon_set.load_icon(
            icon_size,
            |dim| {
                let icon = unsafe{ winuser::LoadImageW(
                    libloaderapi::GetModuleHandleW(ptr::null_mut()),
                    winuser::MAKEINTRESOURCEW(self.resource_id),
                    winuser::IMAGE_ICON,
                    dim, dim,
                    0,
                ) as HICON };
                if icon.is_null() {
                    Err(io::Error::last_os_error())
                } else {
                    Ok(icon)
                }
            }
        )
    }

    fn load_cursor(&self, icon_size: IconSize) -> Result<HCURSOR, io::Error> {
        self.window_icon_set.load_icon(
            icon_size,
            |_| {
                // Windows will automatically select an appropriately-scaled cursor from the cursor
                // file based on where the cursor is on the screen. Passing in a size manually
                // actually interferes with that, so we don't do that.
                let icon = unsafe{ winuser::LoadImageW(
                    libloaderapi::GetModuleHandleW(ptr::null_mut()),
                    winuser::MAKEINTRESOURCEW(self.resource_id),
                    winuser::IMAGE_CURSOR,
                    0, 0,
                    winuser::LR_DEFAULTSIZE,
                ) as HICON };
                if icon.is_null() {
                    Err(io::Error::last_os_error())
                } else {
                    Ok(icon)
                }
            }
        )
    }
}

impl FunctionIcon {
    fn load_icon(&self, icon_size: IconSize, scale_factor: f64) -> Result<HICON, io::Error> {
        self.icon_set.load_icon(
            icon_size,
            |dim| {
                let mut get_icon = self.get_icon.lock();
                let rgba_icon = (&mut *get_icon)(PhysicalSize::new(dim as u32, dim as u32), scale_factor);
                rgba_icon
                    .map_err(|mut e| if let Some(ioe) = e.downcast_mut::<io::Error>() {
                        mem::replace(ioe, io::Error::from_raw_os_error(0))
                    } else {
                        io::Error::new(io::ErrorKind::Other, e)
                    })
                    .and_then(|i| i.into_windows_icon())
            }
        )
    }
}

#[derive(Clone, PartialEq, Eq)]
pub struct WinIcon {
    inner: Arc<RaiiIcon>,
}

unsafe impl Send for WinIcon {}

impl WinIcon {
    pub fn as_raw_scaled_cursor_handle(&self) -> Option<HCURSOR> {
        let scale_factor = cursor_scale_factor();
        let cursor_size = IconSize::I32.adjust_for_scale_factor(scale_factor);

        match &*self.inner {
            RaiiIcon::Path(icon) => icon.load_cursor(cursor_size).ok(),
            RaiiIcon::Resource(icon) => icon.load_cursor(cursor_size).ok(),
            RaiiIcon::Single(icon) => Some(*icon),
            RaiiIcon::Function(icon) => icon.load_icon(cursor_size, scale_factor).ok(),
        }
    }

    fn as_raw_icon_handle(&self, icon_size: IconSize, scale_factor: f64) -> Option<HICON> {
        let icon_size = icon_size.adjust_for_scale_factor(scale_factor);
        match &*self.inner {
            RaiiIcon::Path(icon) => icon.load_icon(icon_size).ok(),
            RaiiIcon::Resource(icon) => icon.load_icon(icon_size).ok(),
            RaiiIcon::Single(icon) => Some(*icon),
            RaiiIcon::Function(icon) => icon.load_icon(icon_size, scale_factor).ok(),
        }
    }

    pub fn from_path<P: AsRef<Path>>(path: P) -> Result<Self, io::Error> {
        let path = path.as_ref();
        let wide_path: Vec<u16> = path
            .as_os_str()
            .encode_wide()
            .chain(once(0))
            .collect();

        let path_icon = PathIcon {
            wide_path,
            cursor_icon_set: LazyIconSet::default(),
            window_icon_set: LazyIconSet::default(),
        };
        if path.extension() == Some(OsStr::new("cur")) || path.extension() == Some(OsStr::new("ani")) {
            path_icon.load_cursor(IconSize::I32.adjust_for_scale_factor(cursor_scale_factor()))?;
        } else {
            path_icon.load_icon(IconSize::I24.adjust_for_scale_factor(cursor_scale_factor()))?;
        }
        Ok(WinIcon {
            inner: Arc::new(RaiiIcon::Path(path_icon))
        })
    }

    pub fn from_resource(
        resource_id: WORD,
    ) -> Result<Self, io::Error> {
        let resource_icon = ResourceIcon {
            resource_id,
            cursor_icon_set: LazyIconSet::default(),
            window_icon_set: LazyIconSet::default(),
        };
        resource_icon.load_icon(IconSize::I24.adjust_for_scale_factor(cursor_scale_factor()))?;
        Ok(WinIcon {
            inner: Arc::new(RaiiIcon::Resource(resource_icon))
        })
    }

    pub fn from_rgba(rgba: &[u8], size: PhysicalSize<u32>) -> Result<Self, io::Error> {
        Ok(WinIcon {
            inner: Arc::new(RaiiIcon::Single(RgbaIcon::from_rgba(Box::from(rgba), size).into_windows_icon()?))
        })
    }

    pub fn from_rgba_with_hot_spot(
        rgba: &[u8],
        size: PhysicalSize<u32>,
        hot_spot: PhysicalPosition<u32>,
    ) -> Result<Self, io::Error> {
        Ok(WinIcon {
            inner: Arc::new(RaiiIcon::Single(RgbaIcon::from_rgba_with_hot_spot(Box::from(rgba), size, hot_spot).into_windows_icon()?))
        })
    }

    pub fn from_rgba_fn<F>(get_icon: F) -> Self
        where F: 'static + FnMut(PhysicalSize<u32>, f64) -> Result<RgbaIcon<Box<[u8]>>, Box<dyn std::error::Error + Send + Sync>>,
    {
        let function_icon = FunctionIcon {
            get_icon: Box::new(Mutex::new(get_icon)),
            icon_set: LazyIconSet::default(),
        };
        WinIcon {
            inner: Arc::new(RaiiIcon::Function(function_icon))
        }
    }

    pub fn set_for_window(&self, hwnd: HWND, scale_factor: f64) {
        unsafe {
            let small_icon = self.as_raw_icon_handle(IconSize::I16, scale_factor).map(|i| i as LPARAM).unwrap_or(0);
            let big_icon = self.as_raw_icon_handle(IconSize::I24, scale_factor).map(|i| i as LPARAM).unwrap_or(0);
            winuser::SendMessageW(
                hwnd,
                winuser::WM_SETICON,
                winuser::ICON_SMALL as WPARAM,
                small_icon as LPARAM,
            );
            winuser::SendMessageW(
                hwnd,
                winuser::WM_SETICON,
                winuser::ICON_BIG as WPARAM,
                big_icon as LPARAM,
            );
        }
    }
}

impl Drop for LazyIconSet {
    fn drop(&mut self) {
        unsafe {
            let LazyIconSet {
                i_16,
                i_24,
                i_32,
                i_48,
                i_64,
                i_96,
                i_128,
                i_256,
            } = self;

            let i_16 = i_16.load(Ordering::SeqCst);
            if !i_16.is_null() { winuser::DestroyIcon(i_16); }
            let i_24 = i_24.load(Ordering::SeqCst);
            if !i_24.is_null() { winuser::DestroyIcon(i_24); }
            let i_32 = i_32.load(Ordering::SeqCst);
            if !i_32.is_null() { winuser::DestroyIcon(i_32); }
            let i_48 = i_48.load(Ordering::SeqCst);
            if !i_48.is_null() { winuser::DestroyIcon(i_48); }
            let i_64 = i_64.load(Ordering::SeqCst);
            if !i_64.is_null() { winuser::DestroyIcon(i_64); }
            let i_96 = i_96.load(Ordering::SeqCst);
            if !i_96.is_null() { winuser::DestroyIcon(i_96); }
            let i_128 = i_128.load(Ordering::SeqCst);
            if !i_128.is_null() { winuser::DestroyIcon(i_128); }
            let i_256 = i_256.load(Ordering::SeqCst);
            if !i_256.is_null() { winuser::DestroyIcon(i_256); }
        }
    }
}

impl fmt::Debug for WinIcon {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> Result<(), fmt::Error> {
        (*self.inner).fmt(formatter)
    }
}

pub fn unset_for_window(hwnd: HWND) {
    unsafe {
        winuser::SendMessageW(
            hwnd,
            winuser::WM_SETICON,
            winuser::ICON_SMALL as WPARAM,
            0,
        );
        winuser::SendMessageW(
            hwnd,
            winuser::WM_SETICON,
            winuser::ICON_BIG as WPARAM,
            0,
        );
    }
}
