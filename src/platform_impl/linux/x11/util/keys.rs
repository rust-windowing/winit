use std::{iter::Enumerate, ptr, slice::Iter};
use x11rb::protocol::xproto;

use super::*;

pub struct Keymap {
    keys: [u8; 32],
}

pub struct KeymapIter<'a> {
    iter: Enumerate<Iter<'a, u8>>,
    index: usize,
    item: Option<u8>,
}

impl Keymap {
    pub fn iter(&self) -> KeymapIter<'_> {
        KeymapIter {
            iter: self.keys.iter().enumerate(),
            index: 0,
            item: None,
        }
    }
}

impl<'a> IntoIterator for &'a Keymap {
    type Item = ffi::KeyCode;
    type IntoIter = KeymapIter<'a>;

    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}

impl Iterator for KeymapIter<'_> {
    type Item = ffi::KeyCode;

    fn next(&mut self) -> Option<ffi::KeyCode> {
        if self.item.is_none() {
            for (index, &item) in self.iter.by_ref() {
                if item != 0 {
                    self.index = index;
                    self.item = Some(item);
                    break;
                }
            }
        }

        self.item.take().map(|item| {
            debug_assert!(item != 0);

            let bit = first_bit(item);

            if item != bit {
                // Remove the first bit; save the rest for further iterations
                self.item = Some(item ^ bit);
            }

            let shift = bit.trailing_zeros() + (self.index * 8) as u32;
            shift as ffi::KeyCode
        })
    }
}

impl XConnection {
    pub fn keycode_to_keysym(&self, keycode: ffi::KeyCode) -> ffi::KeySym {
        unsafe { (self.xlib.XKeycodeToKeysym)(self.display.as_ptr(), keycode, 0) }
    }

    pub fn lookup_keysym(&self, xkev: &xproto::KeyPressEvent) -> ffi::KeySym {
        let mut keysym = 0;

        // TODO: Reimplement this using libxcb. For now, just reconstruct the Xlib event.
        let mut xp = self.convert_keyevent(xkev);

        unsafe {
            (self.xlib.XLookupString)(&mut xp, ptr::null_mut(), 0, &mut keysym, ptr::null_mut());
        }

        keysym
    }

    pub fn query_keymap(&self) -> Keymap {
        let mut keys = [0; 32];

        unsafe {
            (self.xlib.XQueryKeymap)(self.display.as_ptr(), keys.as_mut_ptr() as *mut c_char);
        }

        Keymap { keys }
    }

    pub fn convert_keyevent(&self, xkev: &xproto::KeyPressEvent) -> ffi::XKeyEvent {
        ffi::XKeyEvent {
            type_: (xkev.response_type & 0x7F) as _,
            serial: xkev.sequence as _,
            send_event: (xkev.response_type & 0x80 != 0) as _,
            display: self.display.as_ptr(),
            window: xkev.event as _,
            root: xkev.root as _,
            subwindow: xkev.child as _,
            x_root: xkev.root_x as _,
            y_root: xkev.root_y as _,
            x: xkev.event_x as _,
            y: xkev.event_y as _,
            time: xkev.time as _,
            state: xkev.state.into(),
            keycode: xkev.detail as _,
            same_screen: xkev.same_screen as _,
        }
    }
}

fn first_bit(b: u8) -> u8 {
    1 << b.trailing_zeros()
}
