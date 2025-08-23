use std::sync::LazyLock;
/// This is a simple implementation of support for Windows Dark Mode,
/// which is inspired by the solution in https://github.com/ysc3839/win32-darkmode
use std::{ffi::c_void, ptr};

use windows_sys::core::{PCSTR, PCWSTR};
use windows_sys::w;
use windows_sys::Win32::Foundation::{BOOL, HWND, LPARAM, NTSTATUS, S_OK, WPARAM};
use windows_sys::Win32::System::LibraryLoader::{GetProcAddress, LoadLibraryA};
use windows_sys::Win32::System::SystemInformation::OSVERSIONINFOW;
use windows_sys::Win32::UI::Accessibility::{HCF_HIGHCONTRASTON, HIGHCONTRASTA};
use windows_sys::Win32::UI::Controls::SetWindowTheme;
use windows_sys::Win32::UI::Input::KeyboardAndMouse::GetActiveWindow;
use windows_sys::Win32::UI::WindowsAndMessaging::{
    DefWindowProcW, SystemParametersInfoA, SPI_GETHIGHCONTRAST, WM_NCACTIVATE,
};
use winit_core::window::Theme;

use super::util;

static WIN10_BUILD_VERSION: LazyLock<Option<u32>> = LazyLock::new(|| {
    type RtlGetVersion = unsafe extern "system" fn(*mut OSVERSIONINFOW) -> NTSTATUS;
    let handle = get_function!("ntdll.dll", RtlGetVersion);

    if let Some(rtl_get_version) = handle {
        unsafe {
            let mut vi = OSVERSIONINFOW {
                dwOSVersionInfoSize: 0,
                dwMajorVersion: 0,
                dwMinorVersion: 0,
                dwBuildNumber: 0,
                dwPlatformId: 0,
                szCSDVersion: [0; 128],
            };

            let status = (rtl_get_version)(&mut vi);

            if status >= 0 && vi.dwMajorVersion == 10 && vi.dwMinorVersion == 0 {
                Some(vi.dwBuildNumber)
            } else {
                None
            }
        }
    } else {
        None
    }
});

static DARK_MODE_SUPPORTED: LazyLock<bool> = LazyLock::new(|| {
    // We won't try to do anything for windows versions < 17763
    // (Windows 10 October 2018 update)
    match *WIN10_BUILD_VERSION {
        Some(v) => v >= 17763,
        None => false,
    }
});

const DARK_THEME_NAME: PCWSTR = w!("DarkMode_Explorer");
const LIGHT_THEME_NAME: PCWSTR = w!("");

/// Attempt to set a theme on a window, if necessary.
/// Returns the theme that was picked
///
/// `refresh_title_bar` is only needed when the system doesn't do it by itself,
/// for cases like on window creation or system settings changes,
/// the system will refresh the title bar automatically,
/// if we always refresh the title bar, it will blink it on system settings changes
pub fn try_theme(hwnd: HWND, preferred_theme: Option<Theme>, refresh_title_bar: bool) -> Theme {
    if *DARK_MODE_SUPPORTED {
        let is_dark_mode = match preferred_theme {
            Some(theme) => theme == Theme::Dark,
            None => should_use_dark_mode(),
        };

        let theme = if is_dark_mode { Theme::Dark } else { Theme::Light };
        let theme_name = match theme {
            Theme::Dark => DARK_THEME_NAME,
            Theme::Light => LIGHT_THEME_NAME,
        };

        let status = unsafe { SetWindowTheme(hwnd, theme_name, ptr::null()) };

        if status == S_OK && set_dark_mode_for_window(hwnd, is_dark_mode) {
            if refresh_title_bar {
                unsafe { refresh_titlebar_theme_color(hwnd) };
            }
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

    #[allow(clippy::upper_case_acronyms)]
    type WINDOWCOMPOSITIONATTRIB = u32;
    const WCA_USEDARKMODECOLORS: WINDOWCOMPOSITIONATTRIB = 26;

    #[allow(non_snake_case)]
    #[allow(clippy::upper_case_acronyms)]
    #[repr(C)]
    struct WINDOWCOMPOSITIONATTRIBDATA {
        Attrib: WINDOWCOMPOSITIONATTRIB,
        pvData: *mut c_void,
        cbData: usize,
    }

    static SET_WINDOW_COMPOSITION_ATTRIBUTE: LazyLock<Option<SetWindowCompositionAttribute>> =
        LazyLock::new(|| get_function!("user32.dll", SetWindowCompositionAttribute));

    if let Some(set_window_composition_attribute) = *SET_WINDOW_COMPOSITION_ATTRIBUTE {
        unsafe {
            // SetWindowCompositionAttribute needs a bigbool (i32), not bool.
            let mut is_dark_mode_bigbool = BOOL::from(is_dark_mode);

            let mut data = WINDOWCOMPOSITIONATTRIBDATA {
                Attrib: WCA_USEDARKMODECOLORS,
                pvData: &mut is_dark_mode_bigbool as *mut _ as _,
                cbData: std::mem::size_of_val(&is_dark_mode_bigbool) as _,
            };

            let status = set_window_composition_attribute(hwnd, &mut data);

            status != false.into()
        }
    } else {
        false
    }
}

unsafe fn refresh_titlebar_theme_color(hwnd: HWND) {
    unsafe {
        if GetActiveWindow() == hwnd {
            DefWindowProcW(hwnd, WM_NCACTIVATE, WPARAM::default(), LPARAM::default());
            DefWindowProcW(hwnd, WM_NCACTIVATE, true as _, LPARAM::default());
        } else {
            DefWindowProcW(hwnd, WM_NCACTIVATE, true as _, LPARAM::default());
            DefWindowProcW(hwnd, WM_NCACTIVATE, WPARAM::default(), LPARAM::default());
        }
    }
}

pub fn should_use_dark_mode() -> bool {
    should_apps_use_dark_mode() && !is_high_contrast()
}

fn should_apps_use_dark_mode() -> bool {
    type ShouldAppsUseDarkMode = unsafe extern "system" fn() -> bool;
    static SHOULD_APPS_USE_DARK_MODE: LazyLock<Option<ShouldAppsUseDarkMode>> =
        LazyLock::new(|| unsafe {
            const UXTHEME_SHOULDAPPSUSEDARKMODE_ORDINAL: PCSTR = 132 as PCSTR;

            // We won't try to do anything for windows versions < 17763
            // (Windows 10 October 2018 update)
            if !*DARK_MODE_SUPPORTED {
                return None;
            }

            let module = LoadLibraryA(c"uxtheme.dll".as_ptr().cast());

            if module.is_null() {
                return None;
            }

            let handle = GetProcAddress(module, UXTHEME_SHOULDAPPSUSEDARKMODE_ORDINAL);

            handle.map(|handle| std::mem::transmute(handle))
        });

    SHOULD_APPS_USE_DARK_MODE
        .map(|should_apps_use_dark_mode| unsafe { (should_apps_use_dark_mode)() })
        .unwrap_or(false)
}

fn is_high_contrast() -> bool {
    let mut hc = HIGHCONTRASTA { cbSize: 0, dwFlags: 0, lpszDefaultScheme: ptr::null_mut() };

    let ok = unsafe {
        SystemParametersInfoA(
            SPI_GETHIGHCONTRAST,
            std::mem::size_of_val(&hc) as _,
            &mut hc as *mut _ as _,
            0,
        )
    };

    ok != false.into() && util::has_flag(hc.dwFlags, HCF_HIGHCONTRASTON)
}
