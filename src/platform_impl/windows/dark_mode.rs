use std::os::windows::ffi::OsStrExt;
/// This is a simple implementation of support for Windows Dark Mode,
/// which is inspired by the solution in https://github.com/ysc3839/win32-darkmode
use std::{ffi::OsStr, iter::once};

use winapi::Windows::Win32::{
    System::{
        SystemServices::{
            GetProcAddress, LoadLibraryA, BOOL, PSTR, PWSTR, VER_EQUAL, VER_GREATER_EQUAL,
        },
        WindowsProgramming::{
            VerSetConditionMask, VerifyVersionInfoW, OSVERSIONINFOEXW, VER_BUILDNUMBER,
            VER_MAJORVERSION, VER_MINORVERSION,
        },
    },
    UI::{
        Accessibility::{HCF_HIGHCONTRASTON, HIGHCONTRASTA},
        Controls::SetWindowTheme,
        WindowsAndMessaging::{
            SystemParametersInfoA, HWND, SPI_GETHIGHCONTRAST, SYSTEM_PARAMETERS_INFO_UPDATE_FLAGS,
        },
    },
};

use crate::window::Theme;

lazy_static! {
    static ref DARK_MODE_SUPPORTED: bool = {
        let mut condition_mask = 0;
        condition_mask = VerSetConditionMask(condition_mask, VER_MAJORVERSION, VER_EQUAL as u8);
        condition_mask = VerSetConditionMask(condition_mask, VER_MINORVERSION, VER_EQUAL as u8);
        condition_mask =
            VerSetConditionMask(condition_mask, VER_BUILDNUMBER, VER_GREATER_EQUAL as u8);

        VerifyVersionInfoW(
            &mut OSVERSIONINFOEXW {
                dwMajorVersion: 10,
                dwMinorVersion: 0,
                dwBuildNumber: 17763,
                ..Default::default()
            },
            VER_MAJORVERSION | VER_MINORVERSION | VER_BUILDNUMBER,
            condition_mask,
        )
        .as_bool()
    };
    static ref DARK_THEME_NAME: Vec<u16> = widestring("DarkMode_Explorer");
    static ref LIGHT_THEME_NAME: Vec<u16> = widestring("");
}

/// Attempt to set a theme on a window, if necessary.
/// Returns the theme that was picked
pub fn try_theme(hwnd: HWND, preferred_theme: Option<Theme>) -> Theme {
    if *DARK_MODE_SUPPORTED {
        let is_dark_mode = match preferred_theme {
            Some(theme) => theme == Theme::Dark,
            None => should_use_dark_mode(),
        };

        let theme = if is_dark_mode {
            Theme::Dark
        } else {
            Theme::Light
        };
        let theme_name = match theme {
            Theme::Dark => DARK_THEME_NAME.as_mut_ptr(),
            Theme::Light => LIGHT_THEME_NAME.as_mut_ptr(),
        };

        let status = unsafe { SetWindowTheme(hwnd, PWSTR(theme_name), None) };

        if status.is_ok() && set_dark_mode_for_window(hwnd, is_dark_mode) {
            return theme;
        }
    }

    Theme::Light
}

fn set_dark_mode_for_window(hwnd: HWND, is_dark_mode: bool) -> bool {
    // Uses Windows undocumented API SetWindowCompositionAttribute,
    // as seen in win32-darkmode example linked at top of file.

    type SetWindowCompositionAttribute =
        unsafe extern "system" fn(HWND, *mut WINDOWCOMPOSITIONATTRIBDATA) -> BOOL;

    #[allow(non_snake_case)]
    type WINDOWCOMPOSITIONATTRIB = u32;
    const WCA_USEDARKMODECOLORS: WINDOWCOMPOSITIONATTRIB = 26;

    #[allow(non_snake_case)]
    #[repr(C)]
    struct WINDOWCOMPOSITIONATTRIBDATA {
        Attrib: WINDOWCOMPOSITIONATTRIB,
        pvData: *mut std::os::raw::c_void,
        cbData: usize,
    }

    lazy_static! {
        static ref SET_WINDOW_COMPOSITION_ATTRIBUTE: Option<SetWindowCompositionAttribute> =
            get_function!("user32.dll", SetWindowCompositionAttribute);
    }

    if let Some(set_window_composition_attribute) = *SET_WINDOW_COMPOSITION_ATTRIBUTE {
        unsafe {
            // SetWindowCompositionAttribute needs a bigbool (i32), not bool.
            let mut is_dark_mode_bigbool: BOOL = is_dark_mode.into();

            let mut data = WINDOWCOMPOSITIONATTRIBDATA {
                Attrib: WCA_USEDARKMODECOLORS,
                pvData: &mut is_dark_mode_bigbool as *mut _ as _,
                cbData: std::mem::size_of_val(&is_dark_mode_bigbool) as _,
            };

            set_window_composition_attribute(hwnd, &mut data as *mut _).as_bool()
        }
    } else {
        false
    }
}

fn should_use_dark_mode() -> bool {
    should_apps_use_dark_mode() && !is_high_contrast()
}

fn should_apps_use_dark_mode() -> bool {
    type ShouldAppsUseDarkMode = unsafe extern "system" fn() -> bool;
    lazy_static! {
        static ref SHOULD_APPS_USE_DARK_MODE: Option<ShouldAppsUseDarkMode> = {
            const UXTHEME_SHOULDAPPSUSEDARKMODE_ORDINAL: u32 = 132;

            let module = unsafe { LoadLibraryA(PSTR("uxtheme.dll".as_mut_ptr())) };
            if module.is_null() {
                return None;
            }

            unsafe {
                GetProcAddress(
                    module,
                    PSTR(UXTHEME_SHOULDAPPSUSEDARKMODE_ORDINAL as *mut u8),
                )
            }
            .map(|func| std::mem::transmute(func))
        };
    }

    SHOULD_APPS_USE_DARK_MODE
        .map(|should_apps_use_dark_mode| unsafe { (should_apps_use_dark_mode)() })
        .unwrap_or(false)
}

fn is_high_contrast() -> bool {
    let mut hc: HIGHCONTRASTA = unsafe { std::mem::zeroed() };

    let ok = unsafe {
        SystemParametersInfoA(
            SPI_GETHIGHCONTRAST,
            std::mem::size_of_val(&hc) as _,
            &mut hc as *mut _ as _,
            SYSTEM_PARAMETERS_INFO_UPDATE_FLAGS(0),
        )
    };

    !ok.as_bool() && (HCF_HIGHCONTRASTON & hc.dwFlags) == HCF_HIGHCONTRASTON
}

fn widestring(src: &'static str) -> Vec<u16> {
    OsStr::new(src).encode_wide().chain(once(0)).collect()
}
