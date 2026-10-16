use std::ffi::{OsString, c_void};
use std::os::windows::prelude::OsStringExt;
use std::ptr::null_mut;

use dpi::{Position, Size};
use windows_sys::Win32::Foundation::{POINT, RECT};
use windows_sys::Win32::UI::Input::Ime::{
    ATTR_TARGET_CONVERTED, ATTR_TARGET_NOTCONVERTED, CANDIDATEFORM, CFS_EXCLUDE, CFS_POINT,
    COMPOSITIONFORM, GCS_COMPATTR, GCS_COMPSTR, GCS_CURSORPOS, GCS_RESULTSTR, HIMC, IACE_CHILDREN,
    IACE_DEFAULT, ImmAssociateContextEx, ImmGetCompositionStringW, ImmGetContext,
    ImmReleaseContext, ImmSetCandidateWindow, ImmSetCompositionWindow,
};
use windows_sys::Win32::UI::WindowsAndMessaging::{GetSystemMetrics, SM_IMMENABLED};

use crate::HWND;

pub struct ImeContext {
    hwnd: HWND,
    himc: HIMC,
}

impl ImeContext {
    pub unsafe fn current(hwnd: HWND) -> Self {
        let himc = unsafe { ImmGetContext(hwnd) };
        ImeContext { hwnd, himc }
    }

    pub unsafe fn get_composing_text_and_cursor(
        &self,
    ) -> Option<(String, Option<usize>, Option<usize>)> {
        let text = unsafe { self.get_composition_string(GCS_COMPSTR) }?;
        let attrs = unsafe { self.get_composition_data(GCS_COMPATTR) }.unwrap_or_default();

        let mut first = None;
        let mut last = None;
        let mut boundary_before_char = 0;
        let mut attr_idx = 0;

        for chr in text.chars() {
            let Some(attr) = attrs.get(attr_idx).copied() else {
                break;
            };

            let char_is_targeted =
                attr as u32 == ATTR_TARGET_CONVERTED || attr as u32 == ATTR_TARGET_NOTCONVERTED;

            if first.is_none() && char_is_targeted {
                first = Some(boundary_before_char);
            } else if first.is_some() && last.is_none() && !char_is_targeted {
                last = Some(boundary_before_char);
            }

            boundary_before_char += chr.len_utf8();
            attr_idx += chr.len_utf16();
        }

        if first.is_some() && last.is_none() {
            last = Some(text.len());
        } else if first.is_none() {
            // IME haven't split words and select any clause yet, so trying to retrieve normal
            // cursor.
            let cursor = unsafe { self.get_composition_cursor(&text) };
            first = cursor;
            last = cursor;
        }

        Some((text, first, last))
    }

    pub unsafe fn get_composed_text(&self) -> Option<String> {
        unsafe { self.get_composition_string(GCS_RESULTSTR) }
    }

    unsafe fn get_composition_cursor(&self, text: &str) -> Option<usize> {
        let cursor = unsafe { ImmGetCompositionStringW(self.himc, GCS_CURSORPOS, null_mut(), 0) };
        // GCS_CURSORPOS counts UTF-16 code units, while Ime::Preedit uses UTF-8 byte offsets.
        (cursor >= 0).then(|| utf16_to_utf8_offset(text, cursor as usize))
    }

    unsafe fn get_composition_string(&self, gcs_mode: u32) -> Option<String> {
        let data = unsafe { self.get_composition_data(gcs_mode) }?;
        let (prefix, shorts, suffix) = unsafe { data.align_to::<u16>() };
        if prefix.is_empty() && suffix.is_empty() {
            OsString::from_wide(shorts).into_string().ok()
        } else {
            None
        }
    }

    unsafe fn get_composition_data(&self, gcs_mode: u32) -> Option<Vec<u8>> {
        let size = match unsafe { ImmGetCompositionStringW(self.himc, gcs_mode, null_mut(), 0) } {
            0 => return Some(Vec::new()),
            size if size < 0 => return None,
            size => size,
        };

        let mut buf = Vec::<u8>::with_capacity(size as _);
        let size = unsafe {
            ImmGetCompositionStringW(
                self.himc,
                gcs_mode,
                buf.as_mut_ptr() as *mut c_void,
                size as _,
            )
        };

        if size < 0 {
            None
        } else {
            unsafe { buf.set_len(size as _) };
            Some(buf)
        }
    }

    pub unsafe fn set_ime_cursor_area(&self, spot: Position, size: Size, scale_factor: f64) {
        if !unsafe { ImeContext::system_has_ime() } {
            return;
        }

        let (x, y) = spot.to_physical::<i32>(scale_factor).into();
        let (width, height): (i32, i32) = size.to_physical::<i32>(scale_factor).into();
        let rc_area = RECT { left: x, top: y, right: x + width, bottom: y + height };
        let candidate_form = CANDIDATEFORM {
            dwIndex: 0,
            dwStyle: CFS_EXCLUDE,
            ptCurrentPos: POINT { x, y },
            rcArea: rc_area,
        };
        let composition_form = COMPOSITIONFORM {
            dwStyle: CFS_POINT,
            ptCurrentPos: POINT { x, y: y + height },
            rcArea: rc_area,
        };

        unsafe {
            ImmSetCompositionWindow(self.himc, &composition_form);
            ImmSetCandidateWindow(self.himc, &candidate_form);
        }
    }

    pub unsafe fn set_ime_allowed(hwnd: HWND, allowed: bool) {
        if !unsafe { ImeContext::system_has_ime() } {
            return;
        }

        if allowed {
            unsafe { ImmAssociateContextEx(hwnd, null_mut(), IACE_DEFAULT) };
        } else {
            unsafe { ImmAssociateContextEx(hwnd, null_mut(), IACE_CHILDREN) };
        }
    }

    pub unsafe fn system_has_ime() -> bool {
        unsafe { GetSystemMetrics(SM_IMMENABLED) != 0 }
    }
}

impl Drop for ImeContext {
    fn drop(&mut self) {
        unsafe { ImmReleaseContext(self.hwnd, self.himc) };
    }
}

/// Convert a UTF-16 code unit offset into a UTF-8 byte offset.
/// Offsets inside a surrogate pair round down; offsets past the end clamp to the string's length.
fn utf16_to_utf8_offset(text: &str, units: usize) -> usize {
    let mut consumed = 0;
    for (byte, ch) in text.char_indices() {
        if consumed >= units || consumed + ch.len_utf16() > units {
            return byte;
        }
        consumed += ch.len_utf16();
    }
    text.len()
}

#[cfg(test)]
mod tests {
    use super::utf16_to_utf8_offset;

    #[test]
    fn cursor_offsets_use_utf16_units() {
        for (text, units, expected) in [
            // A valid cursor after the emoji must not advance past the following character.
            ("a😀b", 3, 5),
            ("😀😀", 2, 4),
            // Empty, ASCII and BMP text retain their existing offsets.
            ("", 0, 0),
            ("", 1, 0),
            ("ab", 0, 0),
            ("ab", 1, 1),
            ("ab", 2, 2),
            ("ab", 3, 2),
            ("あい", 1, 3),
            ("あい", 2, 6),
            ("éあ", 1, 2),
            ("éあ", 2, 5),
            // A position inside a surrogate pair rounds down to that character's start.
            ("😀😀", 1, 0),
            ("😀😀", 3, 4),
            ("a😀b", 2, 1),
            // End-of-string and oversized positions clamp to the UTF-8 length.
            ("a😀b", 4, 6),
            ("a😀b", 5, 6),
            ("😀😀", 4, 8),
            ("😀😀", usize::MAX, 8),
        ] {
            let byte = utf16_to_utf8_offset(text, units);
            assert_eq!(byte, expected, "{text:?}, UTF-16 offset {units}");
            assert!(text.is_char_boundary(byte));
        }
    }

    #[test]
    fn cursor_offsets_match_decoded_utf16_prefixes() {
        for text in ["", "ab", "あい", "éあ", "😀😀", "a😀b", "👨‍👩‍👧🇯🇵", "e\u{301}", "مرحبا"]
        {
            let encoded: Vec<u16> = text.encode_utf16().collect();
            let mut previous = 0;
            for units in 0..=encoded.len() + 3 {
                // An independent oracle: truncate the UTF-16 representation, drop any
                // trailing high surrogate, then decode the complete prefix back to UTF-8.
                let mut prefix = encoded[..units.min(encoded.len())].to_vec();
                if matches!(prefix.last(), Some(0xd800..=0xdbff)) {
                    prefix.pop();
                }
                let expected = String::from_utf16(&prefix).unwrap().len();
                let byte = utf16_to_utf8_offset(text, units);
                assert_eq!(byte, expected, "{text:?}, UTF-16 offset {units}");
                assert!(text.is_char_boundary(byte));
                assert!(byte >= previous);
                previous = byte;
            }
        }
    }
}
