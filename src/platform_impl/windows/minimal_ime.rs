use std::mem::MaybeUninit;

use winapi::{
    shared::{
        minwindef::{LPARAM, LRESULT, WPARAM},
        windef::HWND,
    },
    um::winuser,
};

pub fn is_msg_ime_related(msg_kind: u32) -> bool {
    match msg_kind {
        winuser::WM_IME_COMPOSITION
        | winuser::WM_IME_COMPOSITIONFULL
        | winuser::WM_IME_STARTCOMPOSITION
        | winuser::WM_IME_ENDCOMPOSITION
        | winuser::WM_IME_CHAR
        | winuser::WM_CHAR
        | winuser::WM_SYSCHAR => true,
        _ => false,
    }
}

pub struct MinimalIme {
    // True if currently receiving messages belonging to finished IME session.
    getting_ime_text: bool,

    utf16parts: Vec<u16>,
}
impl Default for MinimalIme {
    fn default() -> Self {
        MinimalIme {
            getting_ime_text: false,
            utf16parts: Vec::with_capacity(16),
        }
    }
}
impl MinimalIme {
    pub fn process_message(
        &mut self,
        hwnd: HWND,
        msg_kind: u32,
        wparam: WPARAM,
        _lparam: LPARAM,
        retval: &mut Option<LRESULT>,
    ) -> Option<String> {
        match msg_kind {
            winuser::WM_IME_ENDCOMPOSITION => {
                self.getting_ime_text = true;
            }
            winuser::WM_CHAR | winuser::WM_SYSCHAR => {
                if self.getting_ime_text {
                    *retval = Some(0);
                    self.utf16parts.push(wparam as u16);

                    let more_char_coming;
                    unsafe {
                        let mut next_msg = MaybeUninit::uninit();
                        let has_message = winuser::PeekMessageW(
                            next_msg.as_mut_ptr(),
                            hwnd,
                            winuser::WM_KEYFIRST,
                            winuser::WM_KEYLAST,
                            winuser::PM_NOREMOVE,
                        );
                        let has_message = has_message != 0;
                        if !has_message {
                            more_char_coming = false;
                        } else {
                            let next_msg = next_msg.assume_init().message;
                            if next_msg == winuser::WM_CHAR || next_msg == winuser::WM_SYSCHAR {
                                more_char_coming = true;
                            } else {
                                more_char_coming = false;
                            }
                        }
                    }
                    if !more_char_coming {
                        let result = String::from_utf16(&self.utf16parts).unwrap();
                        self.utf16parts.clear();
                        self.getting_ime_text = false;
                        return Some(result);
                    }
                }
            }
            _ => (),
        }

        None
    }
}
