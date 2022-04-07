use std::{
    ffi::{c_void, OsString},
    mem::zeroed,
    os::windows::prelude::OsStringExt,
    ptr::null_mut,
};

use windows_sys::Win32::{
    Foundation::POINT,
    Globalization::HIMC,
    UI::{
        Input::Ime::{
            ImmAssociateContext, ImmGetCompositionStringW, ImmGetContext, ImmReleaseContext,
            ImmSetCandidateWindow, ATTR_TARGET_CONVERTED, ATTR_TARGET_NOTCONVERTED, CANDIDATEFORM,
            CFS_CANDIDATEPOS, GCS_COMPATTR, GCS_COMPSTR, GCS_RESULTSTR,
        },
        WindowsAndMessaging::{GetSystemMetrics, SM_IMMENABLED},
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

    pub unsafe fn get_composing_text_and_cursor(
        &self,
    ) -> Option<(String, Option<usize>, Option<usize>)> {
        if let Some(text) = self.get_composition_string(GCS_COMPSTR) {
            if let Some(attrs) = self.get_composition_data(GCS_COMPATTR) {
                let mut first: Option<usize> = None;
                let mut last: Option<usize> = None;
                let mut boundary_before_char = 0;

                for (attr, chr) in attrs.into_iter().zip(text.chars()) {
                    let char_is_targetted = attr as u32 == ATTR_TARGET_CONVERTED
                        || attr as u32 == ATTR_TARGET_NOTCONVERTED;

                    if first == None && char_is_targetted {
                        first = Some(boundary_before_char);
                    } else if first != None && last == None && !char_is_targetted {
                        last = Some(boundary_before_char);
                    }

                    boundary_before_char += chr.len_utf8();
                }
                if first != None && last == None {
                    last = Some(text.len());
                } else if first == None {
                    // IME haven't split words and select any clause yet.
                    first = None;
                    last = None;
                }

                Some((text, first, last))
            } else {
                Some((text, None, None))
            }
        } else {
            None
        }
    }

    pub unsafe fn get_composed_text(&self) -> Option<String> {
        self.get_composition_string(GCS_RESULTSTR)
    }

    unsafe fn get_composition_string(&self, gcs_mode: u32) -> Option<String> {
        if let Some(data) = self.get_composition_data(gcs_mode) {
            let (prefix, shorts, suffix) = data.align_to::<u16>();
            if prefix.is_empty() && suffix.is_empty() {
                OsString::from_wide(&shorts).into_string().ok()
            } else {
                None
            }
        } else {
            None
        }
    }

    unsafe fn get_composition_data(&self, gcs_mode: u32) -> Option<Vec<u8>> {
        let size = ImmGetCompositionStringW(self.himc, gcs_mode, null_mut(), 0);
        if size < 0 {
            return None;
        }
        if size == 0 {
            return Some(Vec::new());
        }
        let mut buf = Vec::<u8>::with_capacity(size as _);
        let size = ImmGetCompositionStringW(
            self.himc,
            gcs_mode,
            buf.as_mut_ptr() as *mut c_void,
            size as _,
        );
        if size < 0 {
            return None;
        }
        buf.set_len(size as _);
        return Some(buf);
    }

    pub unsafe fn set_ime_position(&self, spot: Position, scale_factor: f64) {
        if ImeContext::system_has_ime() {
            let (x, y) = spot.to_physical::<i32>(scale_factor).into();

            let candidate_form = CANDIDATEFORM {
                dwIndex: 0,
                dwStyle: CFS_CANDIDATEPOS,
                ptCurrentPos: POINT { x, y },
                rcArea: zeroed(),
            };

            ImmSetCandidateWindow(self.himc, &candidate_form);
        }
    }

    pub unsafe fn set_ime_allowed(&self, allowed: bool) {
        if ImeContext::system_has_ime() {
            if allowed {
                ImmAssociateContext(self.hwnd, self.himc);
            } else {
                ImmAssociateContext(self.hwnd, 0);
            }
        }
    }

    unsafe fn system_has_ime() -> bool {
        return GetSystemMetrics(SM_IMMENABLED) != 0;
    }
}

impl Drop for ImeContext {
    fn drop(&mut self) {
        unsafe { ImmReleaseContext(self.hwnd, self.himc) };
    }
}
