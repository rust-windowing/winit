/// This is a simple implementation of support for Windows Dark Mode,
/// which is a more or less straight translation of the implemenation
/// in Windows Terminal (which was originally in C++).
use std::ffi::OsStr;
use std::os::windows::ffi::OsStrExt;

use winapi::{
    shared::{
        minwindef::{BOOL, DWORD, UINT, WORD},
        ntdef::LPSTR,
        windef::HWND,
    },
    um::{dwmapi, libloaderapi, uxtheme, winuser},
};

const DWMWA_USE_IMMERSIVE_DARK_MODE: DWORD = 19;
const UXTHEME_DLL_NAME: &'static str = "uxtheme.dll";
const UXTHEME_SHOULDAPPSUSEDARKMODE_ORDINAL: WORD = 132;

type ShouldAppsUseDarkMode = unsafe extern "system" fn() -> bool;

lazy_static! {
    static ref SHOULD_APPS_USE_DARK_MODE: ShouldAppsUseDarkMode = {
        unsafe {
            let module = libloaderapi::LoadLibraryExW(
                widestring(UXTHEME_DLL_NAME).as_ptr(),
                std::ptr::null_mut(),
                libloaderapi::LOAD_LIBRARY_SEARCH_SYSTEM32,
            );

            let handle = libloaderapi::GetProcAddress(
                module,
                winuser::MAKEINTRESOURCEA(UXTHEME_SHOULDAPPSUSEDARKMODE_ORDINAL),
            );

            if handle.is_null() {
                unsafe extern "system" fn always_false() -> bool {
                    false
                }
                always_false
            } else {
                std::mem::transmute(handle)
            }
        }
    };
    static ref DARK_THEME_NAME: Vec<u16> = widestring("DarkMode_Explorer");
    static ref LIGHT_THEME_NAME: Vec<u16> = widestring("");
}

/// Attempt to set dark mode on a window, if necessary.
/// Returns true if dark mode was set, false if not.
pub fn try_dark_mode(hwnd: HWND) -> bool {
    // According to Windows Terminal source, should be BOOL (32-bit int)
    // to be appropriately sized as a parameter for DwmSetWindowAttribute
    let is_dark_mode = should_use_dark_mode();
    let is_dark_mode_bigbool = is_dark_mode as BOOL;

    let theme_name = if is_dark_mode {
        DARK_THEME_NAME.as_ptr()
    } else {
        LIGHT_THEME_NAME.as_ptr()
    };

    unsafe {
        assert_eq!(
            0,
            uxtheme::SetWindowTheme(hwnd, theme_name as _, std::ptr::null())
        );

        assert_eq!(
            0,
            dwmapi::DwmSetWindowAttribute(
                hwnd,
                DWMWA_USE_IMMERSIVE_DARK_MODE,
                &is_dark_mode_bigbool as *const _ as _,
                std::mem::size_of_val(&is_dark_mode_bigbool) as _
            )
        );
    }

    is_dark_mode
}

fn should_use_dark_mode() -> bool {
    should_apps_use_dark_mode() && !is_high_contrast()
}

fn should_apps_use_dark_mode() -> bool {
    unsafe { SHOULD_APPS_USE_DARK_MODE() }
}

// FIXME: This definition was missing from winapi. Can remove from
// here and use winapi once the following PR is released:
// https://github.com/retep998/winapi-rs/pull/815
#[repr(C)]
#[allow(non_snake_case)]
struct HIGHCONTRASTA {
    cbSize: UINT,
    dwFlags: DWORD,
    lpszDefaultScheme: LPSTR,
}

const HCF_HIGHCONTRASTON: DWORD = 1;

fn is_high_contrast() -> bool {
    let mut hc = HIGHCONTRASTA {
        cbSize: 0,
        dwFlags: 0,
        lpszDefaultScheme: std::ptr::null_mut(),
    };

    let ok = unsafe {
        winuser::SystemParametersInfoA(
            winuser::SPI_GETHIGHCONTRAST,
            std::mem::size_of_val(&hc) as _,
            &mut hc as *mut _ as _,
            0,
        )
    };

    (ok > 0) && ((HCF_HIGHCONTRASTON & hc.dwFlags) == 1)
}

fn widestring(src: &'static str) -> Vec<u16> {
    OsStr::new(src)
        .encode_wide()
        .chain(Some(0).into_iter())
        .collect()
}
