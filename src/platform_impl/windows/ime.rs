use std::{
    ffi::{c_void, OsString},
    mem::zeroed,
    os::windows::prelude::OsStringExt,
    ptr::null_mut,
};

use windows_sys::Win32::{
    Foundation::POINT,
    Globalization::HIMC,
    UI::Input::Ime::{
        ImmAssociateContext, ImmGetCompositionStringW, ImmGetContext, ImmReleaseContext,
        ImmSetCandidateWindow, CANDIDATEFORM, CFS_CANDIDATEPOS,
    },
};

use crate::{dpi::Position, platform::windows::HWND};

pub struct ImeContext {
    hwnd: HWND,
    himc: HIMC,
}

impl ImeContext {
    pub unsafe fn current(hwnd: HWND) -> Self {
        let himc = ImmGetContext(hwnd);
        ImeContext { hwnd, himc }
    }

    pub unsafe fn get_composition_string(&self, gcs_mode: u32) -> Option<String> {
        let size = ImmGetCompositionStringW(self.himc, gcs_mode, null_mut(), 0);
        if size <= 0 {
            return None;
        }
        let mut buf = Vec::<u8>::with_capacity(size as _);
        let size = ImmGetCompositionStringW(
            self.himc,
            gcs_mode,
            buf.as_mut_ptr() as *mut c_void,
            size as _,
        );
        if size <= 0 {
            return None;
        }
        buf.set_len(size as _);
        let (prefix, shorts, suffix) = buf.align_to::<u16>();

        if prefix.is_empty() && suffix.is_empty() {
            OsString::from_wide(&shorts).into_string().ok()
        } else {
            None
        }
    }

    pub unsafe fn set_ime_position(&self, spot: Position, scale_factor: f64) {
        let (x, y) = spot.to_physical::<i32>(scale_factor).into();

        let candidate_form = CANDIDATEFORM {
            dwIndex: 0,
            dwStyle: CFS_CANDIDATEPOS,
            ptCurrentPos: POINT { x, y },
            rcArea: zeroed(),
        };

        ImmSetCandidateWindow(self.himc, &candidate_form);
    }

    pub unsafe fn set_ime_allowed(&self, allowed: bool) {
        if allowed {
            ImmAssociateContext(self.hwnd, self.himc);
        } else {
            ImmAssociateContext(self.hwnd, 0);
        }
    }
}

impl Drop for ImeContext {
    fn drop(&mut self) {
        unsafe { ImmReleaseContext(self.hwnd, self.himc) };
    }
}
