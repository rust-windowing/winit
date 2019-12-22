/// This is a simple implementation of support for Windows Dark Mode,
/// which is inspired by the solution in https://github.com/ysc3839/win32-darkmode
use std::ffi::OsStr;
use std::os::windows::ffi::OsStrExt;

use winapi::{
    shared::{
        basetsd::SIZE_T,
        minwindef::{BOOL, DWORD, UINT, ULONG, WORD},
        ntdef::{LPSTR, NTSTATUS, NT_SUCCESS, PVOID, WCHAR},
        windef::HWND,
    },
    um::{libloaderapi, uxtheme, winuser},
};

lazy_static! {
    static ref WIN10_BUILD_VERSION: Option<DWORD> = {
        // FIXME: RtlGetVersion is a documented windows API,
        // should be part of winapi!

        #[allow(non_snake_case)]
        #[repr(C)]
        struct OSVERSIONINFOW {
            dwOSVersionInfoSize: ULONG,
            dwMajorVersion: ULONG,
            dwMinorVersion: ULONG,
            dwBuildNumber: ULONG,
            dwPlatformId: ULONG,
            szCSDVersion: [WCHAR; 128],
        }

        type RtlGetVersion = unsafe extern "system" fn (*mut OSVERSIONINFOW) -> NTSTATUS;
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

                let status = (rtl_get_version)(&mut vi as _);
                assert!(NT_SUCCESS(status));

                if vi.dwMajorVersion == 10 && vi.dwMinorVersion == 0 {
                    Some(vi.dwBuildNumber)
                } else {
                    None
                }
            }
        } else {
            None
        }
    };

    static ref DARK_MODE_SUPPORTED: bool = {
        // We won't try to do anything for windows versions < 17763
        // (Windows 10 October 2018 update)
        match *WIN10_BUILD_VERSION {
            Some(v) => v >= 17763,
            None => false
        }
    };

    static ref DARK_THEME_NAME: Vec<u16> = widestring("DarkMode_Explorer");
    static ref LIGHT_THEME_NAME: Vec<u16> = widestring("");
}

/// Attempt to set dark mode on a window, if necessary.
/// Returns true if dark mode was set, false if not.
pub fn try_dark_mode(hwnd: HWND) -> bool {
    if *DARK_MODE_SUPPORTED {
        let is_dark_mode = should_use_dark_mode();

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

            set_dark_mode_for_window(hwnd, is_dark_mode)
        }

        is_dark_mode
    } else {
        false
    }
}

fn set_dark_mode_for_window(hwnd: HWND, is_dark_mode: bool) {
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
        pvData: PVOID,
        cbData: SIZE_T,
    }

    lazy_static! {
        static ref SET_WINDOW_COMPOSITION_ATTRIBUTE: Option<SetWindowCompositionAttribute> =
            get_function!("user32.dll", SetWindowCompositionAttribute);
    }

    if let Some(set_window_composition_attribute) = *SET_WINDOW_COMPOSITION_ATTRIBUTE {
        unsafe {
            // SetWindowCompositionAttribute needs a bigbool (i32), not bool.
            let mut is_dark_mode_bigbool = is_dark_mode as BOOL;

            let mut data = WINDOWCOMPOSITIONATTRIBDATA {
                Attrib: WCA_USEDARKMODECOLORS,
                pvData: &mut is_dark_mode_bigbool as *mut _ as _,
                cbData: std::mem::size_of_val(&is_dark_mode_bigbool) as _,
            };

            assert_eq!(
                1,
                set_window_composition_attribute(hwnd, &mut data as *mut _)
            );
        }
    }
}

fn should_use_dark_mode() -> bool {
    should_apps_use_dark_mode() && !is_high_contrast()
}

fn should_apps_use_dark_mode() -> bool {
    type ShouldAppsUseDarkMode = unsafe extern "system" fn() -> bool;
    lazy_static! {
        static ref SHOULD_APPS_USE_DARK_MODE: Option<ShouldAppsUseDarkMode> = {
            unsafe {
                const UXTHEME_SHOULDAPPSUSEDARKMODE_ORDINAL: WORD = 132;

                let module = libloaderapi::LoadLibraryA("uxtheme.dll\0".as_ptr() as _);

                if module.is_null() {
                    return None;
                }

                let handle = libloaderapi::GetProcAddress(
                    module,
                    winuser::MAKEINTRESOURCEA(UXTHEME_SHOULDAPPSUSEDARKMODE_ORDINAL),
                );

                if handle.is_null() {
                    None
                } else {
                    Some(std::mem::transmute(handle))
                }
            }
        };
    }

    SHOULD_APPS_USE_DARK_MODE
        .map(|should_apps_use_dark_mode| unsafe { (should_apps_use_dark_mode)() })
        .unwrap_or(false)
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
