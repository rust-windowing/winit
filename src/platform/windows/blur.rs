//! This module contains the multiple implementations for controlling
//! both transparency and blur for windows.
//!
//! On `Windows` there are, depending on the version of the OS, different
//! APIs to use for enabling transparency and blur.
//! This modules abstracts over these different APIs and provides a single,
//! simple interface to use the correct API automatically.

use std::os::raw;
use std::ptr;

use libloading;
use winapi::shared::minwindef::ULONG;
use winapi::shared::windef::HWND;
use winapi::um::{dwmapi, errhandlingapi, winuser};

use platform::platform::window::WindowWrapper;

lazy_static! {
    static ref USER32_DLL: Option<libloading::Library> =
        { libloading::Library::new("user32.dll").ok() };
    static ref POINTERS: Option<AcrylicBlurPointers> = {
        Some(AcrylicBlurPointers {
            swca: unsafe {
                USER32_DLL
                    .as_ref()?
                    .get(b"SetWindowCompositionAttribute")
                    .ok()?
            },
        })
    };
}

static DWM: Dwm = Dwm;
static ACRYLIC_BLUR: AcrylicBlur = AcrylicBlur;

pub fn implementation() -> &'static Implementation {
    // unsafe {
    //     let ver = sysinfoapi::GetVersion();
    //     let (major, minor) = (ver & 0xFF, (ver & 0xFF00) >> 8);
    //     println!("Major: {}, minor: {}", major, minor);
    // }

    if let Some(_) = *POINTERS {
        &ACRYLIC_BLUR
    } else {
        &DWM
    }
}

pub trait Implementation {
    fn set_opaque(&self, window: &mut WindowWrapper);
    fn set_transparent(&self, window: &mut WindowWrapper);
    fn set_blur(&self, window: &mut WindowWrapper);
}

struct Dwm;

impl Implementation for Dwm {
    fn set_opaque(&self, window: &mut WindowWrapper) {
        let bb = dwmapi::DWM_BLURBEHIND {
            dwFlags: 0x1, // FIXME: DWM_BB_ENABLE;
            fEnable: 0,
            hRgnBlur: ptr::null_mut(),
            fTransitionOnMaximized: 0,
        };
        unsafe {
            dwmapi::DwmEnableBlurBehindWindow(window.0, &bb);
        }
    }

    fn set_transparent(&self, window: &mut WindowWrapper) {
        self.set_blur(window); // TODO: Can you even do actual transparency with DWM?
    }

    fn set_blur(&self, window: &mut WindowWrapper) {
        let bb = dwmapi::DWM_BLURBEHIND {
            dwFlags: 0x1, // FIXME: DWM_BB_ENABLE;
            fEnable: 1,
            hRgnBlur: ptr::null_mut(),
            fTransitionOnMaximized: 0,
        };
        unsafe {
            dwmapi::DwmEnableBlurBehindWindow(window.0, &bb);
        }
    }
}

struct AcrylicBlur;

/// SetWindowCompositionAttribute function pointer
type SwcaFn = unsafe extern "system" fn(
    hwnd: HWND,
    attribute: *const WindowCompositionAttributeData
) -> raw::c_int;

struct AcrylicBlurPointers {
    swca: libloading::Symbol<'static, SwcaFn>,
}

impl AcrylicBlurPointers {
    fn set_window_composite_attribute(
        &self,
        window: &mut WindowWrapper,
        attribute: &WindowCompositionAttributeData,
    ) -> raw::c_int {
        unsafe { (self.swca)(window.0, attribute as *const _) }
    }
}

#[repr(u32)]
enum AccentState {
    Disable = 0,
    EnableGradient = 1,
    EnableTransparentGradient = 2,
    EnableBlurBehind = 3,
    EnableAcrylicBlurBehind = 4,
    InvalidState = 5,
}

#[repr(C)]
struct AccentPolicy {
    state: AccentState,
    flags: raw::c_int,
    gradient_color: raw::c_uint,
    animation_id: raw::c_int,
}

#[repr(u32)]
enum WindowCompositionAttribute {
    AccentPolicy = 19,
}

#[repr(C)]
struct WindowCompositionAttributeData {
    attribute: WindowCompositionAttribute,
    policy: *const AccentPolicy,
    size: ULONG,
}

impl AcrylicBlur {}

impl Implementation for AcrylicBlur {
    fn set_opaque(&self, window: &mut WindowWrapper) {
        POINTERS.as_ref().unwrap().set_window_composite_attribute(
            window,
            &WindowCompositionAttributeData {
                attribute: WindowCompositionAttribute::AccentPolicy,
                policy: &AccentPolicy {
                    state: AccentState::Disable,
                    flags: 0,
                    gradient_color: 0x00000000,
                    animation_id: 0,
                } as *const _,
                size: ::std::mem::size_of::<AccentPolicy>() as _,
            },
        );
    }

    fn set_transparent(&self, window: &mut WindowWrapper) {
        self.set_blur(window); // Also, apparently no 'true' transparency support based on WCA.
    }

    fn set_blur(&self, window: &mut WindowWrapper) {
        POINTERS.as_ref().unwrap().set_window_composite_attribute(
            window,
            &WindowCompositionAttributeData {
                attribute: WindowCompositionAttribute::AccentPolicy,
                policy: &AccentPolicy {
                    // TODO: Decide between EnableBlurBehind and EnableAcrylicBlurBehind (since build 17063)
                    // based on the Windows version.
                    // When choosing EnableAcrylicBlurBehind, set gradient_color
                    // to something non-zero (apparently mandatory).
                    state: AccentState::EnableBlurBehind,
                    flags: 0,
                    gradient_color: 0x00000000,
                    animation_id: 0,
                } as *const _,
                size: ::std::mem::size_of::<AccentPolicy>() as _,
            },
        );
    }
}
