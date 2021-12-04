#![allow(non_upper_case_globals)]

use super::evdev::*;
use super::keysyms::*;
use crate::backends::x11::XConnection;
use crate::keyboard::Layout::{self, *};
use std::collections::HashMap;
use std::mem::MaybeUninit;
use uapi::as_maybe_uninit_bytes;
use xcb_dl::{ffi, XcbXkb};

pub struct Layouts {
    pub msg1: Msg,
    pub msg2: Msg,
}

pub struct Msg {
    pub header: ffi::xcb_xkb_set_map_request_t,
    pub body: Vec<MaybeUninit<u8>>,
}

pub fn layouts() -> Layouts {
    Layouts {
        msg1: create_msg(&[keymap(Layout::Qwerty), keymap(Layout::Azerty)]),
        msg2: create_msg(&[keymap(Layout::QwertySwapped)]),
    }
}

const KEY_OFFSET: u32 = 8;
const FIRST_KEY: u32 = KEY_ESC;
const LAST_KEY: u32 = KEY_MENU;
const NUM_KEYS: u32 = LAST_KEY - FIRST_KEY + 1;

const ONE_LEVEL: u8 = 0;
const TWO_LEVEL: u8 = 1;
const ALPHABETIC: u8 = 2;
const KEYPAD: u8 = 3;
const FOUR_LEVEL: u8 = 4;
const FOUR_LEVEL_SEMIALPHABETIC: u8 = 5;

const NUM_TYPES: u8 = FOUR_LEVEL_SEMIALPHABETIC - ONE_LEVEL + 1;
const NUM_LEVELS: usize = 15;

pub(super) fn set_names(
    xkb: &XcbXkb,
    c: &XConnection,
    slave: ffi::xcb_input_device_id_t,
) -> ffi::xcb_void_cookie_t {
    let mut levels_per_type: [u8; NUM_TYPES as usize] = [1, 2, 2, 2, 4, 4];
    let mut level_names: [u32; NUM_LEVELS] = [0; NUM_LEVELS];
    assert_eq!(levels_per_type.into_iter().sum::<u8>(), NUM_LEVELS as u8);
    let values = ffi::xcb_xkb_set_names_values_t {
        n_levels_per_type: levels_per_type.as_mut_ptr(),
        kt_level_names: level_names.as_mut_ptr(),
        ..Default::default()
    };
    unsafe {
        xkb.xcb_xkb_set_names_aux_checked(
            c.c,
            slave,
            0,
            ffi::XCB_XKB_NAME_DETAIL_KT_LEVEL_NAMES,
            0,
            NUM_TYPES,
            0,
            NUM_TYPES,
            0,
            0,
            0,
            0,
            0,
            0,
            NUM_LEVELS as _,
            &values,
        )
    }
}

fn classify_keysyms(keysyms: &[u32]) -> u8 {
    if keysyms.len() < 2 {
        return ONE_LEVEL;
    }
    if keysyms.len() == 2 {
        if xkb_keysym_is_lower(keysyms[0]) && xkb_keysym_is_upper(keysyms[1]) {
            return ALPHABETIC;
        }
        if xkb_keysym_is_keypad(keysyms[0]) && xkb_keysym_is_keypad(keysyms[1]) {
            return KEYPAD;
        }
        return TWO_LEVEL;
    }
    if keysyms.len() == 4 {
        if xkb_keysym_is_lower(keysyms[0]) && xkb_keysym_is_upper(keysyms[1]) {
            return FOUR_LEVEL_SEMIALPHABETIC;
        }
        return FOUR_LEVEL;
    }
    unreachable!();
}

fn create_msg(layouts: &[HashMap<u32, Vec<u32>>]) -> Msg {
    let mut body = vec![];
    {
        // ONE_LEVEL
        let set_key_type = ffi::xcb_xkb_set_key_type_t {
            real_mods: 0,
            num_levels: 1,
            n_map_entries: 0,
            ..Default::default()
        };
        body.extend_from_slice(as_maybe_uninit_bytes(&set_key_type));
    }
    {
        // TWO_LEVEL
        let set_map_entries = &[ffi::xcb_xkb_kt_set_map_entry_t {
            level: 1,
            real_mods: ffi::XCB_MOD_MASK_SHIFT as _,
            ..Default::default()
        }];
        let set_key_type = ffi::xcb_xkb_set_key_type_t {
            real_mods: ffi::XCB_MOD_MASK_SHIFT as _,
            num_levels: 2,
            n_map_entries: set_map_entries.len() as _,
            ..Default::default()
        };
        body.extend_from_slice(as_maybe_uninit_bytes(&set_key_type));
        body.extend_from_slice(as_maybe_uninit_bytes(set_map_entries));
    }
    {
        // ALPHABETIC
        let set_map_entries = &[
            ffi::xcb_xkb_kt_set_map_entry_t {
                level: 1,
                real_mods: ffi::XCB_MOD_MASK_SHIFT as _,
                ..Default::default()
            },
            ffi::xcb_xkb_kt_set_map_entry_t {
                level: 1,
                real_mods: ffi::XCB_MOD_MASK_LOCK as _,
                ..Default::default()
            },
        ];
        let set_key_type = ffi::xcb_xkb_set_key_type_t {
            real_mods: (ffi::XCB_MOD_MASK_SHIFT | ffi::XCB_MOD_MASK_LOCK) as _,
            num_levels: 2,
            n_map_entries: set_map_entries.len() as _,
            ..Default::default()
        };
        body.extend_from_slice(as_maybe_uninit_bytes(&set_key_type));
        body.extend_from_slice(as_maybe_uninit_bytes(set_map_entries));
    }
    {
        // KEYPAD
        let set_map_entries = &[
            ffi::xcb_xkb_kt_set_map_entry_t {
                level: 1,
                real_mods: ffi::XCB_MOD_MASK_SHIFT as _,
                ..Default::default()
            },
            ffi::xcb_xkb_kt_set_map_entry_t {
                level: 1,
                real_mods: ffi::XCB_MOD_MASK_3 as _,
                ..Default::default()
            },
        ];
        let set_key_type = ffi::xcb_xkb_set_key_type_t {
            real_mods: (ffi::XCB_MOD_MASK_SHIFT | ffi::XCB_MOD_MASK_3) as _,
            num_levels: 2,
            n_map_entries: set_map_entries.len() as _,
            ..Default::default()
        };
        body.extend_from_slice(as_maybe_uninit_bytes(&set_key_type));
        body.extend_from_slice(as_maybe_uninit_bytes(set_map_entries));
    }
    {
        // FOUR_LEVEL
        let set_map_entries = &[
            ffi::xcb_xkb_kt_set_map_entry_t {
                level: 1,
                real_mods: ffi::XCB_MOD_MASK_SHIFT as _,
                ..Default::default()
            },
            ffi::xcb_xkb_kt_set_map_entry_t {
                level: 2,
                real_mods: ffi::XCB_MOD_MASK_2 as _,
                ..Default::default()
            },
            ffi::xcb_xkb_kt_set_map_entry_t {
                level: 3,
                real_mods: (ffi::XCB_MOD_MASK_SHIFT | ffi::XCB_MOD_MASK_2) as _,
                ..Default::default()
            },
        ];
        let set_key_type = ffi::xcb_xkb_set_key_type_t {
            real_mods: (ffi::XCB_MOD_MASK_SHIFT | ffi::XCB_MOD_MASK_2) as _,
            num_levels: 4,
            n_map_entries: set_map_entries.len() as _,
            ..Default::default()
        };
        body.extend_from_slice(as_maybe_uninit_bytes(&set_key_type));
        body.extend_from_slice(as_maybe_uninit_bytes(set_map_entries));
    }
    {
        // FOUR_LEVEL_SEMIALPHABETIC
        let set_map_entries = &[
            ffi::xcb_xkb_kt_set_map_entry_t {
                level: 1,
                real_mods: ffi::XCB_MOD_MASK_SHIFT as _,
                ..Default::default()
            },
            ffi::xcb_xkb_kt_set_map_entry_t {
                level: 1,
                real_mods: ffi::XCB_MOD_MASK_LOCK as _,
                ..Default::default()
            },
            ffi::xcb_xkb_kt_set_map_entry_t {
                level: 2,
                real_mods: ffi::XCB_MOD_MASK_2 as _,
                ..Default::default()
            },
            ffi::xcb_xkb_kt_set_map_entry_t {
                level: 3,
                real_mods: (ffi::XCB_MOD_MASK_SHIFT | ffi::XCB_MOD_MASK_2) as _,
                ..Default::default()
            },
            ffi::xcb_xkb_kt_set_map_entry_t {
                level: 2,
                real_mods: (ffi::XCB_MOD_MASK_LOCK | ffi::XCB_MOD_MASK_2) as _,
                ..Default::default()
            },
            ffi::xcb_xkb_kt_set_map_entry_t {
                level: 3,
                real_mods: (ffi::XCB_MOD_MASK_SHIFT | ffi::XCB_MOD_MASK_LOCK | ffi::XCB_MOD_MASK_2)
                    as _,
                ..Default::default()
            },
        ];
        let set_key_type = ffi::xcb_xkb_set_key_type_t {
            real_mods: (ffi::XCB_MOD_MASK_SHIFT | ffi::XCB_MOD_MASK_LOCK | ffi::XCB_MOD_MASK_2)
                as _,
            num_levels: 4,
            n_map_entries: set_map_entries.len() as _,
            ..Default::default()
        };
        body.extend_from_slice(as_maybe_uninit_bytes(&set_key_type));
        body.extend_from_slice(as_maybe_uninit_bytes(set_map_entries));
    }
    struct Syms<'a> {
        syms: Vec<&'a [u32]>,
        width: usize,
        has_actions: bool,
    }
    let mut syms_by_key = HashMap::new();
    for key in FIRST_KEY..=LAST_KEY {
        let syms: Vec<_> = layouts
            .iter()
            .map(|l| l.get(&key).map(|s| s.as_slice()).unwrap_or(&[]))
            .collect();
        let width = syms.iter().map(|s| s.len()).max().unwrap_or(0);
        syms_by_key.insert(
            key,
            Syms {
                syms,
                width,
                has_actions: false,
            },
        );
    }
    let mut total_syms = 0;
    for key in FIRST_KEY..=LAST_KEY {
        let syms = &syms_by_key[&key];
        let width = syms.width;
        let syms = &syms.syms;
        let n_syms = (width * layouts.len()) as _;
        total_syms += n_syms;
        let mut key_sym_map = ffi::xcb_xkb_key_sym_map_t {
            kt_index: [ONE_LEVEL, ONE_LEVEL, ONE_LEVEL, ONE_LEVEL],
            group_info: if width > 0 { layouts.len() as _ } else { 0 },
            width: width as _,
            n_syms,
        };
        for (i, syms) in syms.iter().copied().enumerate() {
            key_sym_map.kt_index[i] = classify_keysyms(syms);
        }
        body.extend_from_slice(as_maybe_uninit_bytes(&key_sym_map));
        for syms in syms {
            body.extend_from_slice(as_maybe_uninit_bytes::<[u32]>(syms));
            for _ in syms.len()..width {
                body.extend_from_slice(as_maybe_uninit_bytes::<u32>(&0));
            }
        }
    }
    let mut total_actions = 0;
    for key in FIRST_KEY..=LAST_KEY {
        let syms = syms_by_key.get_mut(&key).unwrap();
        'outer: for s in &syms.syms {
            for &sym in *s {
                if matches!(
                    sym,
                    XK_Num_Lock
                        | XK_ISO_Level3_Shift
                        | XK_Alt_L
                        | XK_Alt_R
                        | XK_Shift_L
                        | XK_Shift_R
                        | XK_Caps_Lock
                        | XK_Control_L
                        | XK_Control_R
                ) {
                    syms.has_actions = true;
                    break 'outer;
                }
            }
        }
        let num_actions = syms.has_actions as usize * syms.syms.len() * syms.width;
        total_actions += num_actions;
        body.push(MaybeUninit::new(num_actions as u8));
    }
    for _ in 0..NUM_KEYS.wrapping_neg() & 3 {
        body.push(MaybeUninit::new(0));
    }
    let no_action = ffi::xcb_xkb_action_t {
        type_: ffi::XCB_XKB_SA_TYPE_NO_ACTION as _,
    };
    for key in FIRST_KEY..=LAST_KEY {
        let syms = &syms_by_key[&key];
        if !syms.has_actions {
            continue;
        }
        for s in &syms.syms {
            for &sym in *s {
                let action = match sym {
                    XK_Num_Lock | XK_Caps_Lock => ffi::xcb_xkb_action_t {
                        lockmods: ffi::xcb_xkb_sa_lock_mods_t {
                            type_: ffi::XCB_XKB_SA_TYPE_LOCK_MODS as _,
                            real_mods: if sym == XK_Num_Lock {
                                ffi::XCB_MOD_MASK_3
                            } else {
                                ffi::XCB_MOD_MASK_LOCK
                            } as _,
                            ..Default::default()
                        },
                    },
                    XK_ISO_Level3_Shift | XK_Alt_L | XK_Alt_R | XK_Shift_L | XK_Shift_R
                    | XK_Control_L | XK_Control_R => ffi::xcb_xkb_action_t {
                        setmods: ffi::xcb_xkb_sa_set_mods_t {
                            type_: ffi::XCB_XKB_SA_TYPE_SET_MODS as _,
                            real_mods: match sym {
                                XK_ISO_Level3_Shift => ffi::XCB_MOD_MASK_2,
                                XK_Alt_L | XK_Alt_R => ffi::XCB_MOD_MASK_1,
                                XK_Shift_L | XK_Shift_R => ffi::XCB_MOD_MASK_SHIFT,
                                XK_Control_L | XK_Control_R => ffi::XCB_MOD_MASK_CONTROL,
                                _ => unreachable!(),
                            } as _,
                            ..Default::default()
                        },
                    },
                    _ => no_action,
                };
                body.extend_from_slice(as_maybe_uninit_bytes(&action));
            }
            for _ in syms.syms.len()..syms.width {
                body.extend_from_slice(as_maybe_uninit_bytes(&no_action));
            }
        }
    }
    let header = ffi::xcb_xkb_set_map_request_t {
        present: 0xff,
        flags: (ffi::XCB_XKB_SET_MAP_FLAGS_RECOMPUTE_ACTIONS
            | ffi::XCB_XKB_SET_MAP_FLAGS_RESIZE_TYPES) as _,
        min_key_code: KEY_OFFSET as _,
        max_key_code: 255,
        first_type: ONE_LEVEL,
        n_types: NUM_TYPES,
        first_key_sym: (FIRST_KEY + KEY_OFFSET) as _,
        n_key_syms: NUM_KEYS as _,
        total_syms,
        first_key_action: (FIRST_KEY + KEY_OFFSET) as _,
        n_key_actions: NUM_KEYS as _,
        total_actions: total_actions as _,
        ..Default::default()
    };
    Msg { header, body }
}

fn keymap(layout: Layout) -> HashMap<u32, Vec<u32>> {
    let mut res = HashMap::new();
    match layout {
        Qwerty | Azerty => {
            res.insert(KEY_ESC, vec![XK_Escape]);
            res.insert(KEY_CAPSLOCK, vec![XK_Caps_Lock]);
            res.insert(KEY_LEFTSHIFT, vec![XK_Shift_L]);
            res.insert(KEY_RIGHTSHIFT, vec![XK_Shift_R]);
        }
        QwertySwapped => {
            res.insert(KEY_ESC, vec![XK_Caps_Lock]);
            res.insert(KEY_CAPSLOCK, vec![XK_Escape]);
            res.insert(KEY_LEFTSHIFT, vec![XK_Shift_R]);
            res.insert(KEY_RIGHTSHIFT, vec![XK_Shift_L]);
        }
    }
    match layout {
        Qwerty | QwertySwapped => {
            res.insert(KEY_1, vec![XK_1, XK_exclam]);
            res.insert(KEY_2, vec![XK_2, XK_at]);
            res.insert(KEY_3, vec![XK_3, XK_numbersign]);
            res.insert(KEY_4, vec![XK_4, XK_dollar]);
            res.insert(KEY_5, vec![XK_5, XK_percent]);
            res.insert(KEY_6, vec![XK_6, XK_asciicircum]);
            res.insert(KEY_7, vec![XK_7, XK_ampersand]);
            res.insert(KEY_8, vec![XK_8, XK_asterisk]);
            res.insert(KEY_9, vec![XK_9, XK_parenleft]);
            res.insert(KEY_0, vec![XK_0, XK_parenright]);
            res.insert(KEY_MINUS, vec![XK_minus, XK_underscore]);
            res.insert(KEY_EQUAL, vec![XK_equal, XK_plus]);
            res.insert(KEY_Q, vec![XK_q, XK_Q]);
            res.insert(KEY_W, vec![XK_w, XK_W]);
            res.insert(KEY_E, vec![XK_e, XK_E]);
            res.insert(KEY_LEFTBRACE, vec![XK_bracketleft, XK_braceleft]);
            res.insert(KEY_RIGHTBRACE, vec![XK_bracketright, XK_braceright]);
            res.insert(KEY_A, vec![XK_a, XK_A]);
            res.insert(KEY_SEMICOLON, vec![XK_semicolon, XK_colon]);
            res.insert(KEY_APOSTROPHE, vec![XK_apostrophe, XK_quotedbl]);
            res.insert(KEY_GRAVE, vec![XK_grave, XK_asciitilde]);
            res.insert(KEY_BACKSLASH, vec![XK_backslash, XK_bar]);
            res.insert(KEY_Z, vec![XK_z, XK_Z]);
            res.insert(KEY_M, vec![XK_m, XK_M]);
            res.insert(KEY_COMMA, vec![XK_comma, XK_less]);
            res.insert(KEY_DOT, vec![XK_period, XK_greater]);
            res.insert(KEY_SLASH, vec![XK_slash, XK_question]);
            res.insert(KEY_RIGHTALT, vec![XK_Alt_R]);
        }
        Azerty => {
            res.insert(KEY_1, vec![XK_ampersand, XK_1]);
            res.insert(KEY_2, vec![XK_eacute, XK_2, XK_asciitilde, 0]);
            res.insert(KEY_3, vec![XK_quotedbl, XK_3, XK_numbersign, 0]);
            res.insert(KEY_4, vec![XK_apostrophe, XK_4, XK_braceleft, 0]);
            res.insert(KEY_5, vec![XK_parenleft, XK_5, XK_bracketleft, 0]);
            res.insert(KEY_6, vec![XK_minus, XK_6, XK_bar, 0]);
            res.insert(KEY_7, vec![XK_egrave, XK_7, XK_grave, 0]);
            res.insert(KEY_8, vec![XK_underscore, XK_8, XK_backslash, 0]);
            res.insert(KEY_9, vec![XK_ccedilla, XK_9, XK_asciicircum, 0]);
            res.insert(KEY_0, vec![XK_agrave, XK_0, XK_at, 0]);
            res.insert(
                KEY_MINUS,
                vec![XK_parenright, XK_degree, XK_bracketright, 0],
            );
            res.insert(KEY_EQUAL, vec![XK_equal, XK_plus, XK_braceright, 0]);
            res.insert(KEY_Q, vec![XK_a, XK_A]);
            res.insert(KEY_W, vec![XK_z, XK_Z]);
            res.insert(KEY_E, vec![XK_e, XK_E, XK_EuroSign, 0]);
            res.insert(KEY_LEFTBRACE, vec![XK_dead_circumflex, XK_dead_diaeresis]);
            res.insert(KEY_RIGHTBRACE, vec![XK_dollar, XK_sterling, XK_currency, 0]);
            res.insert(KEY_A, vec![XK_q, XK_Q]);
            res.insert(KEY_SEMICOLON, vec![XK_m, XK_M]);
            res.insert(KEY_APOSTROPHE, vec![XK_ugrave, XK_percent]);
            res.insert(KEY_GRAVE, vec![XK_twosuperior]);
            res.insert(KEY_BACKSLASH, vec![XK_asterisk, XK_mu]);
            res.insert(KEY_Z, vec![XK_w, XK_W]);
            res.insert(KEY_M, vec![XK_comma, XK_question]);
            res.insert(KEY_COMMA, vec![XK_semicolon, XK_period]);
            res.insert(KEY_DOT, vec![XK_colon, XK_slash]);
            res.insert(KEY_SLASH, vec![XK_exclam, XK_section]);
            res.insert(KEY_RIGHTALT, vec![XK_ISO_Level3_Shift]);
        }
    }
    res.insert(KEY_BACKSPACE, vec![XK_BackSpace]);
    res.insert(KEY_TAB, vec![XK_Tab, XK_ISO_Left_Tab]);
    res.insert(KEY_R, vec![XK_r, XK_R]);
    res.insert(KEY_T, vec![XK_t, XK_T]);
    res.insert(KEY_Y, vec![XK_y, XK_Y]);
    res.insert(KEY_U, vec![XK_u, XK_U]);
    res.insert(KEY_I, vec![XK_i, XK_I]);
    res.insert(KEY_O, vec![XK_o, XK_O]);
    res.insert(KEY_P, vec![XK_p, XK_P]);
    res.insert(KEY_ENTER, vec![XK_Return]);
    res.insert(KEY_LEFTCTRL, vec![XK_Control_L]);
    res.insert(KEY_S, vec![XK_s, XK_S]);
    res.insert(KEY_D, vec![XK_d, XK_D]);
    res.insert(KEY_F, vec![XK_f, XK_F]);
    res.insert(KEY_G, vec![XK_g, XK_G]);
    res.insert(KEY_H, vec![XK_h, XK_H]);
    res.insert(KEY_J, vec![XK_j, XK_J]);
    res.insert(KEY_K, vec![XK_k, XK_K]);
    res.insert(KEY_L, vec![XK_l, XK_L]);
    res.insert(KEY_X, vec![XK_x, XK_X]);
    res.insert(KEY_C, vec![XK_c, XK_C]);
    res.insert(KEY_V, vec![XK_v, XK_V]);
    res.insert(KEY_B, vec![XK_b, XK_B]);
    res.insert(KEY_N, vec![XK_n, XK_N]);
    res.insert(KEY_KPASTERISK, vec![XK_KP_Multiply]);
    res.insert(KEY_LEFTALT, vec![XK_Alt_L]);
    res.insert(KEY_SPACE, vec![XK_space]);
    res.insert(KEY_F1, vec![XK_F1]);
    res.insert(KEY_F2, vec![XK_F2]);
    res.insert(KEY_F3, vec![XK_F3]);
    res.insert(KEY_F4, vec![XK_F4]);
    res.insert(KEY_F5, vec![XK_F5]);
    res.insert(KEY_F6, vec![XK_F6]);
    res.insert(KEY_F7, vec![XK_F7]);
    res.insert(KEY_F8, vec![XK_F8]);
    res.insert(KEY_F9, vec![XK_F9]);
    res.insert(KEY_F10, vec![XK_F10]);
    res.insert(KEY_NUMLOCK, vec![XK_Num_Lock]);
    res.insert(KEY_SCROLLLOCK, vec![XK_Scroll_Lock]);
    res.insert(KEY_KP7, vec![XK_KP_Home, XK_KP_7]);
    res.insert(KEY_KP8, vec![XK_KP_Up, XK_KP_8]);
    res.insert(KEY_KP9, vec![XK_KP_Prior, XK_KP_9]);
    res.insert(KEY_KPMINUS, vec![XK_KP_Subtract]);
    res.insert(KEY_KP4, vec![XK_KP_Left, XK_KP_4]);
    res.insert(KEY_KP5, vec![XK_KP_Begin, XK_KP_5]);
    res.insert(KEY_KP6, vec![XK_KP_Right, XK_KP_6]);
    res.insert(KEY_KPPLUS, vec![XK_KP_Add]);
    res.insert(KEY_KP1, vec![XK_KP_End, XK_KP_1]);
    res.insert(KEY_KP2, vec![XK_KP_Down, XK_KP_2]);
    res.insert(KEY_KP3, vec![XK_KP_Next, XK_KP_3]);
    res.insert(KEY_KP0, vec![XK_KP_Insert, XK_KP_0]);
    res.insert(KEY_KPDOT, vec![XK_KP_Delete, XK_KP_Decimal]);
    res.insert(KEY_F11, vec![XK_F11]);
    res.insert(KEY_F12, vec![XK_F12]);
    res.insert(KEY_KPENTER, vec![XK_KP_Enter]);
    res.insert(KEY_RIGHTCTRL, vec![XK_Control_R]);
    res.insert(KEY_KPSLASH, vec![XK_KP_Divide]);
    res.insert(KEY_SYSRQ, vec![XK_Print]);
    res.insert(KEY_HOME, vec![XK_Home]);
    res.insert(KEY_UP, vec![XK_Up]);
    res.insert(KEY_PAGEUP, vec![XK_Prior]);
    res.insert(KEY_LEFT, vec![XK_Left]);
    res.insert(KEY_RIGHT, vec![XK_Right]);
    res.insert(KEY_END, vec![XK_End]);
    res.insert(KEY_DOWN, vec![XK_Down]);
    res.insert(KEY_PAGEDOWN, vec![XK_Next]);
    res.insert(KEY_INSERT, vec![XK_Insert]);
    res.insert(KEY_DELETE, vec![XK_Delete]);
    res.insert(KEY_PAUSE, vec![XK_Pause]);
    res.insert(KEY_LEFTMETA, vec![XK_Super_L]);
    res.insert(KEY_RIGHTMETA, vec![XK_Super_R]);
    res.insert(KEY_MENU, vec![XK_Menu]);
    res
}

fn xkb_keysym_is_keypad(keysym: u32) -> bool {
    keysym >= XK_KP_Space && keysym <= XK_KP_Equal
}

fn xkb_keysym_is_lower(ks: u32) -> bool {
    let (lower, upper) = x_convert_case(ks);
    if lower == upper {
        return false;
    }
    ks == lower
}

fn xkb_keysym_is_upper(ks: u32) -> bool {
    let (lower, upper) = x_convert_case(ks);
    if lower == upper {
        return false;
    }
    ks == upper
}

fn ucsconvert_case(code: u32, lower: &mut u32, upper: &mut u32) {
    /* Case conversion for UCS, as in Unicode Data version 4.0.0 */
    /* NB: Only converts simple one-to-one mappings. */

    /* Tables are used where they take less space than     */
    /* the code to work out the mappings. Zero values mean */
    /* undefined code points.                              */

    static IPAEXT_UPPER_MAPPING: &[u32] = &[
        /* part only */
        0x0181, 0x0186, 0x0255, 0x0189, 0x018A, 0x0258, 0x018F, 0x025A, 0x0190, 0x025C, 0x025D,
        0x025E, 0x025F, 0x0193, 0x0261, 0x0262, 0x0194, 0x0264, 0x0265, 0x0266, 0x0267, 0x0197,
        0x0196, 0x026A, 0x026B, 0x026C, 0x026D, 0x026E, 0x019C, 0x0270, 0x0271, 0x019D, 0x0273,
        0x0274, 0x019F, 0x0276, 0x0277, 0x0278, 0x0279, 0x027A, 0x027B, 0x027C, 0x027D, 0x027E,
        0x027F, 0x01A6, 0x0281, 0x0282, 0x01A9, 0x0284, 0x0285, 0x0286, 0x0287, 0x01AE, 0x0289,
        0x01B1, 0x01B2, 0x028C, 0x028D, 0x028E, 0x028F, 0x0290, 0x0291, 0x01B7,
    ];

    static LATIN_EXT_B_UPPER_MAPPING: &[u16] = &[
        /* first part only */
        0x0180, 0x0181, 0x0182, 0x0182, 0x0184, 0x0184, 0x0186, 0x0187, 0x0187, 0x0189, 0x018A,
        0x018B, 0x018B, 0x018D, 0x018E, 0x018F, 0x0190, 0x0191, 0x0191, 0x0193, 0x0194, 0x01F6,
        0x0196, 0x0197, 0x0198, 0x0198, 0x019A, 0x019B, 0x019C, 0x019D, 0x0220, 0x019F, 0x01A0,
        0x01A0, 0x01A2, 0x01A2, 0x01A4, 0x01A4, 0x01A6, 0x01A7, 0x01A7, 0x01A9, 0x01AA, 0x01AB,
        0x01AC, 0x01AC, 0x01AE, 0x01AF, 0x01AF, 0x01B1, 0x01B2, 0x01B3, 0x01B3, 0x01B5, 0x01B5,
        0x01B7, 0x01B8, 0x01B8, 0x01BA, 0x01BB, 0x01BC, 0x01BC, 0x01BE, 0x01F7, 0x01C0, 0x01C1,
        0x01C2, 0x01C3, 0x01C4, 0x01C4, 0x01C4, 0x01C7, 0x01C7, 0x01C7, 0x01CA, 0x01CA, 0x01CA,
    ];

    static LATIN_EXT_B_LOWER_MAPPING: &[u16] = &[
        /* first part only */
        0x0180, 0x0253, 0x0183, 0x0183, 0x0185, 0x0185, 0x0254, 0x0188, 0x0188, 0x0256, 0x0257,
        0x018C, 0x018C, 0x018D, 0x01DD, 0x0259, 0x025B, 0x0192, 0x0192, 0x0260, 0x0263, 0x0195,
        0x0269, 0x0268, 0x0199, 0x0199, 0x019A, 0x019B, 0x026F, 0x0272, 0x019E, 0x0275, 0x01A1,
        0x01A1, 0x01A3, 0x01A3, 0x01A5, 0x01A5, 0x0280, 0x01A8, 0x01A8, 0x0283, 0x01AA, 0x01AB,
        0x01AD, 0x01AD, 0x0288, 0x01B0, 0x01B0, 0x028A, 0x028B, 0x01B4, 0x01B4, 0x01B6, 0x01B6,
        0x0292, 0x01B9, 0x01B9, 0x01BA, 0x01BB, 0x01BD, 0x01BD, 0x01BE, 0x01BF, 0x01C0, 0x01C1,
        0x01C2, 0x01C3, 0x01C6, 0x01C6, 0x01C6, 0x01C9, 0x01C9, 0x01C9, 0x01CC, 0x01CC, 0x01CC,
    ];

    static GREEK_UPPER_MAPPING: &[u16] = &[
        0x0000, 0x0000, 0x0000, 0x0000, 0x0374, 0x0375, 0x0000, 0x0000, 0x0000, 0x0000, 0x037A,
        0x0000, 0x0000, 0x0000, 0x037E, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0384, 0x0385,
        0x0386, 0x0387, 0x0388, 0x0389, 0x038A, 0x0000, 0x038C, 0x0000, 0x038E, 0x038F, 0x0390,
        0x0391, 0x0392, 0x0393, 0x0394, 0x0395, 0x0396, 0x0397, 0x0398, 0x0399, 0x039A, 0x039B,
        0x039C, 0x039D, 0x039E, 0x039F, 0x03A0, 0x03A1, 0x0000, 0x03A3, 0x03A4, 0x03A5, 0x03A6,
        0x03A7, 0x03A8, 0x03A9, 0x03AA, 0x03AB, 0x0386, 0x0388, 0x0389, 0x038A, 0x03B0, 0x0391,
        0x0392, 0x0393, 0x0394, 0x0395, 0x0396, 0x0397, 0x0398, 0x0399, 0x039A, 0x039B, 0x039C,
        0x039D, 0x039E, 0x039F, 0x03A0, 0x03A1, 0x03A3, 0x03A3, 0x03A4, 0x03A5, 0x03A6, 0x03A7,
        0x03A8, 0x03A9, 0x03AA, 0x03AB, 0x038C, 0x038E, 0x038F, 0x0000, 0x0392, 0x0398, 0x03D2,
        0x03D3, 0x03D4, 0x03A6, 0x03A0, 0x03D7, 0x03D8, 0x03D8, 0x03DA, 0x03DA, 0x03DC, 0x03DC,
        0x03DE, 0x03DE, 0x03E0, 0x03E0, 0x03E2, 0x03E2, 0x03E4, 0x03E4, 0x03E6, 0x03E6, 0x03E8,
        0x03E8, 0x03EA, 0x03EA, 0x03EC, 0x03EC, 0x03EE, 0x03EE, 0x039A, 0x03A1, 0x03F9, 0x03F3,
        0x03F4, 0x0395, 0x03F6, 0x03F7, 0x03F7, 0x03F9, 0x03FA, 0x03FA, 0x0000, 0x0000, 0x0000,
        0x0000,
    ];

    static GREEK_LOWER_MAPPING: &[u16] = &[
        0x0000, 0x0000, 0x0000, 0x0000, 0x0374, 0x0375, 0x0000, 0x0000, 0x0000, 0x0000, 0x037A,
        0x0000, 0x0000, 0x0000, 0x037E, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0384, 0x0385,
        0x03AC, 0x0387, 0x03AD, 0x03AE, 0x03AF, 0x0000, 0x03CC, 0x0000, 0x03CD, 0x03CE, 0x0390,
        0x03B1, 0x03B2, 0x03B3, 0x03B4, 0x03B5, 0x03B6, 0x03B7, 0x03B8, 0x03B9, 0x03BA, 0x03BB,
        0x03BC, 0x03BD, 0x03BE, 0x03BF, 0x03C0, 0x03C1, 0x0000, 0x03C3, 0x03C4, 0x03C5, 0x03C6,
        0x03C7, 0x03C8, 0x03C9, 0x03CA, 0x03CB, 0x03AC, 0x03AD, 0x03AE, 0x03AF, 0x03B0, 0x03B1,
        0x03B2, 0x03B3, 0x03B4, 0x03B5, 0x03B6, 0x03B7, 0x03B8, 0x03B9, 0x03BA, 0x03BB, 0x03BC,
        0x03BD, 0x03BE, 0x03BF, 0x03C0, 0x03C1, 0x03C2, 0x03C3, 0x03C4, 0x03C5, 0x03C6, 0x03C7,
        0x03C8, 0x03C9, 0x03CA, 0x03CB, 0x03CC, 0x03CD, 0x03CE, 0x0000, 0x03D0, 0x03D1, 0x03D2,
        0x03D3, 0x03D4, 0x03D5, 0x03D6, 0x03D7, 0x03D9, 0x03D9, 0x03DB, 0x03DB, 0x03DD, 0x03DD,
        0x03DF, 0x03DF, 0x03E1, 0x03E1, 0x03E3, 0x03E3, 0x03E5, 0x03E5, 0x03E7, 0x03E7, 0x03E9,
        0x03E9, 0x03EB, 0x03EB, 0x03ED, 0x03ED, 0x03EF, 0x03EF, 0x03F0, 0x03F1, 0x03F2, 0x03F3,
        0x03B8, 0x03F5, 0x03F6, 0x03F8, 0x03F8, 0x03F2, 0x03FB, 0x03FB, 0x0000, 0x0000, 0x0000,
        0x0000,
    ];

    static GREEK_EXT_LOWER_MAPPING: &[u16] = &[
        0x1F00, 0x1F01, 0x1F02, 0x1F03, 0x1F04, 0x1F05, 0x1F06, 0x1F07, 0x1F00, 0x1F01, 0x1F02,
        0x1F03, 0x1F04, 0x1F05, 0x1F06, 0x1F07, 0x1F10, 0x1F11, 0x1F12, 0x1F13, 0x1F14, 0x1F15,
        0x0000, 0x0000, 0x1F10, 0x1F11, 0x1F12, 0x1F13, 0x1F14, 0x1F15, 0x0000, 0x0000, 0x1F20,
        0x1F21, 0x1F22, 0x1F23, 0x1F24, 0x1F25, 0x1F26, 0x1F27, 0x1F20, 0x1F21, 0x1F22, 0x1F23,
        0x1F24, 0x1F25, 0x1F26, 0x1F27, 0x1F30, 0x1F31, 0x1F32, 0x1F33, 0x1F34, 0x1F35, 0x1F36,
        0x1F37, 0x1F30, 0x1F31, 0x1F32, 0x1F33, 0x1F34, 0x1F35, 0x1F36, 0x1F37, 0x1F40, 0x1F41,
        0x1F42, 0x1F43, 0x1F44, 0x1F45, 0x0000, 0x0000, 0x1F40, 0x1F41, 0x1F42, 0x1F43, 0x1F44,
        0x1F45, 0x0000, 0x0000, 0x1F50, 0x1F51, 0x1F52, 0x1F53, 0x1F54, 0x1F55, 0x1F56, 0x1F57,
        0x0000, 0x1F51, 0x0000, 0x1F53, 0x0000, 0x1F55, 0x0000, 0x1F57, 0x1F60, 0x1F61, 0x1F62,
        0x1F63, 0x1F64, 0x1F65, 0x1F66, 0x1F67, 0x1F60, 0x1F61, 0x1F62, 0x1F63, 0x1F64, 0x1F65,
        0x1F66, 0x1F67, 0x1F70, 0x1F71, 0x1F72, 0x1F73, 0x1F74, 0x1F75, 0x1F76, 0x1F77, 0x1F78,
        0x1F79, 0x1F7A, 0x1F7B, 0x1F7C, 0x1F7D, 0x0000, 0x0000, 0x1F80, 0x1F81, 0x1F82, 0x1F83,
        0x1F84, 0x1F85, 0x1F86, 0x1F87, 0x1F80, 0x1F81, 0x1F82, 0x1F83, 0x1F84, 0x1F85, 0x1F86,
        0x1F87, 0x1F90, 0x1F91, 0x1F92, 0x1F93, 0x1F94, 0x1F95, 0x1F96, 0x1F97, 0x1F90, 0x1F91,
        0x1F92, 0x1F93, 0x1F94, 0x1F95, 0x1F96, 0x1F97, 0x1FA0, 0x1FA1, 0x1FA2, 0x1FA3, 0x1FA4,
        0x1FA5, 0x1FA6, 0x1FA7, 0x1FA0, 0x1FA1, 0x1FA2, 0x1FA3, 0x1FA4, 0x1FA5, 0x1FA6, 0x1FA7,
        0x1FB0, 0x1FB1, 0x1FB2, 0x1FB3, 0x1FB4, 0x0000, 0x1FB6, 0x1FB7, 0x1FB0, 0x1FB1, 0x1F70,
        0x1F71, 0x1FB3, 0x1FBD, 0x1FBE, 0x1FBF, 0x1FC0, 0x1FC1, 0x1FC2, 0x1FC3, 0x1FC4, 0x0000,
        0x1FC6, 0x1FC7, 0x1F72, 0x1F73, 0x1F74, 0x1F75, 0x1FC3, 0x1FCD, 0x1FCE, 0x1FCF, 0x1FD0,
        0x1FD1, 0x1FD2, 0x1FD3, 0x0000, 0x0000, 0x1FD6, 0x1FD7, 0x1FD0, 0x1FD1, 0x1F76, 0x1F77,
        0x0000, 0x1FDD, 0x1FDE, 0x1FDF, 0x1FE0, 0x1FE1, 0x1FE2, 0x1FE3, 0x1FE4, 0x1FE5, 0x1FE6,
        0x1FE7, 0x1FE0, 0x1FE1, 0x1F7A, 0x1F7B, 0x1FE5, 0x1FED, 0x1FEE, 0x1FEF, 0x0000, 0x0000,
        0x1FF2, 0x1FF3, 0x1FF4, 0x0000, 0x1FF6, 0x1FF7, 0x1F78, 0x1F79, 0x1F7C, 0x1F7D, 0x1FF3,
        0x1FFD, 0x1FFE, 0x0000,
    ];

    static GREEK_EXT_UPPER_MAPPING: &[u16] = &[
        0x1F08, 0x1F09, 0x1F0A, 0x1F0B, 0x1F0C, 0x1F0D, 0x1F0E, 0x1F0F, 0x1F08, 0x1F09, 0x1F0A,
        0x1F0B, 0x1F0C, 0x1F0D, 0x1F0E, 0x1F0F, 0x1F18, 0x1F19, 0x1F1A, 0x1F1B, 0x1F1C, 0x1F1D,
        0x0000, 0x0000, 0x1F18, 0x1F19, 0x1F1A, 0x1F1B, 0x1F1C, 0x1F1D, 0x0000, 0x0000, 0x1F28,
        0x1F29, 0x1F2A, 0x1F2B, 0x1F2C, 0x1F2D, 0x1F2E, 0x1F2F, 0x1F28, 0x1F29, 0x1F2A, 0x1F2B,
        0x1F2C, 0x1F2D, 0x1F2E, 0x1F2F, 0x1F38, 0x1F39, 0x1F3A, 0x1F3B, 0x1F3C, 0x1F3D, 0x1F3E,
        0x1F3F, 0x1F38, 0x1F39, 0x1F3A, 0x1F3B, 0x1F3C, 0x1F3D, 0x1F3E, 0x1F3F, 0x1F48, 0x1F49,
        0x1F4A, 0x1F4B, 0x1F4C, 0x1F4D, 0x0000, 0x0000, 0x1F48, 0x1F49, 0x1F4A, 0x1F4B, 0x1F4C,
        0x1F4D, 0x0000, 0x0000, 0x1F50, 0x1F59, 0x1F52, 0x1F5B, 0x1F54, 0x1F5D, 0x1F56, 0x1F5F,
        0x0000, 0x1F59, 0x0000, 0x1F5B, 0x0000, 0x1F5D, 0x0000, 0x1F5F, 0x1F68, 0x1F69, 0x1F6A,
        0x1F6B, 0x1F6C, 0x1F6D, 0x1F6E, 0x1F6F, 0x1F68, 0x1F69, 0x1F6A, 0x1F6B, 0x1F6C, 0x1F6D,
        0x1F6E, 0x1F6F, 0x1FBA, 0x1FBB, 0x1FC8, 0x1FC9, 0x1FCA, 0x1FCB, 0x1FDA, 0x1FDB, 0x1FF8,
        0x1FF9, 0x1FEA, 0x1FEB, 0x1FFA, 0x1FFB, 0x0000, 0x0000, 0x1F88, 0x1F89, 0x1F8A, 0x1F8B,
        0x1F8C, 0x1F8D, 0x1F8E, 0x1F8F, 0x1F88, 0x1F89, 0x1F8A, 0x1F8B, 0x1F8C, 0x1F8D, 0x1F8E,
        0x1F8F, 0x1F98, 0x1F99, 0x1F9A, 0x1F9B, 0x1F9C, 0x1F9D, 0x1F9E, 0x1F9F, 0x1F98, 0x1F99,
        0x1F9A, 0x1F9B, 0x1F9C, 0x1F9D, 0x1F9E, 0x1F9F, 0x1FA8, 0x1FA9, 0x1FAA, 0x1FAB, 0x1FAC,
        0x1FAD, 0x1FAE, 0x1FAF, 0x1FA8, 0x1FA9, 0x1FAA, 0x1FAB, 0x1FAC, 0x1FAD, 0x1FAE, 0x1FAF,
        0x1FB8, 0x1FB9, 0x1FB2, 0x1FBC, 0x1FB4, 0x0000, 0x1FB6, 0x1FB7, 0x1FB8, 0x1FB9, 0x1FBA,
        0x1FBB, 0x1FBC, 0x1FBD, 0x0399, 0x1FBF, 0x1FC0, 0x1FC1, 0x1FC2, 0x1FCC, 0x1FC4, 0x0000,
        0x1FC6, 0x1FC7, 0x1FC8, 0x1FC9, 0x1FCA, 0x1FCB, 0x1FCC, 0x1FCD, 0x1FCE, 0x1FCF, 0x1FD8,
        0x1FD9, 0x1FD2, 0x1FD3, 0x0000, 0x0000, 0x1FD6, 0x1FD7, 0x1FD8, 0x1FD9, 0x1FDA, 0x1FDB,
        0x0000, 0x1FDD, 0x1FDE, 0x1FDF, 0x1FE8, 0x1FE9, 0x1FE2, 0x1FE3, 0x1FE4, 0x1FEC, 0x1FE6,
        0x1FE7, 0x1FE8, 0x1FE9, 0x1FEA, 0x1FEB, 0x1FEC, 0x1FED, 0x1FEE, 0x1FEF, 0x0000, 0x0000,
        0x1FF2, 0x1FFC, 0x1FF4, 0x0000, 0x1FF6, 0x1FF7, 0x1FF8, 0x1FF9, 0x1FFA, 0x1FFB, 0x1FFC,
        0x1FFD, 0x1FFE, 0x0000,
    ];

    *lower = code;
    *upper = code;

    /* Basic Latin and Latin-1 Supplement, U+0000 to U+00FF */
    if code <= 0x00ff {
        if code >= 0x0041 && code <= 0x005a
        /* A-Z */
        {
            *lower += 0x20;
        } else if code >= 0x0061 && code <= 0x007a
        /* a-z */
        {
            *upper -= 0x20;
        } else if (code >= 0x00c0 && code <= 0x00d6) || (code >= 0x00d8 && code <= 0x00de) {
            *lower += 0x20;
        } else if (code >= 0x00e0 && code <= 0x00f6) || (code >= 0x00f8 && code <= 0x00fe) {
            *upper -= 0x20;
        } else if code == 0x00ff
        /* y with diaeresis */
        {
            *upper = 0x0178;
        } else if code == 0x00b5
        /* micro sign */
        {
            *upper = 0x039c;
        } else if code == 0x00df
        /* ssharp */
        {
            *upper = 0x1e9e;
        }
        return;
    }

    /* Latin Extended-A, U+0100 to U+017F */
    if code >= 0x0100 && code <= 0x017f {
        if (code >= 0x0100 && code <= 0x012f)
            || (code >= 0x0132 && code <= 0x0137)
            || (code >= 0x014a && code <= 0x0177)
        {
            *upper = code & !1;
            *lower = code | 1;
        } else if (code >= 0x0139 && code <= 0x0148) || (code >= 0x0179 && code <= 0x017e) {
            if (code & 1) != 0 {
                *lower += 1;
            } else {
                *upper -= 1;
            }
        } else if code == 0x0130 {
            *lower = 0x0069;
        } else if code == 0x0131 {
            *upper = 0x0049;
        } else if code == 0x0178 {
            *lower = 0x00ff;
        } else if code == 0x017f {
            *upper = 0x0053;
        }
        return;
    }

    /* Latin Extended-B, U+0180 to U+024F */
    if code >= 0x0180 && code <= 0x024f {
        if code >= 0x01cd && code <= 0x01dc {
            if (code & 1) != 0 {
                *lower += 1;
            } else {
                *upper -= 1;
            }
        } else if (code >= 0x01de && code <= 0x01ef)
            || (code >= 0x01f4 && code <= 0x01f5)
            || (code >= 0x01f8 && code <= 0x021f)
            || (code >= 0x0222 && code <= 0x0233)
        {
            *lower |= 1;
            *upper &= !1;
        } else if code >= 0x0180 && code <= 0x01cc {
            *lower = LATIN_EXT_B_LOWER_MAPPING[code as usize - 0x0180] as _;
            *upper = LATIN_EXT_B_UPPER_MAPPING[code as usize - 0x0180] as _;
        } else if code == 0x01dd {
            *upper = 0x018e;
        } else if code == 0x01f1 || code == 0x01f2 {
            *lower = 0x01f3;
            *upper = 0x01f1;
        } else if code == 0x01f3 {
            *upper = 0x01f1;
        } else if code == 0x01f6 {
            *lower = 0x0195;
        } else if code == 0x01f7 {
            *lower = 0x01bf;
        } else if code == 0x0220 {
            *lower = 0x019e;
        }
        return;
    }

    /* IPA Extensions, U+0250 to U+02AF */
    if code >= 0x0253 && code <= 0x0292 {
        *upper = IPAEXT_UPPER_MAPPING[code as usize - 0x0253];
    }

    /* Combining Diacritical Marks, U+0300 to U+036F */
    if code == 0x0345 {
        *upper = 0x0399;
    }

    /* Greek and Coptic, U+0370 to U+03FF */
    if code >= 0x0370 && code <= 0x03ff {
        *lower = GREEK_LOWER_MAPPING[code as usize - 0x0370] as _;
        *upper = GREEK_UPPER_MAPPING[code as usize - 0x0370] as _;
        if *upper == 0 {
            *upper = code;
        }
        if *lower == 0 {
            *lower = code;
        }
    }

    /* Cyrillic and Cyrillic Supplementary, U+0400 to U+052F */
    if (code >= 0x0400 && code <= 0x04ff) || (code >= 0x0500 && code <= 0x052f) {
        if code >= 0x0400 && code <= 0x040f {
            *lower += 0x50;
        } else if code >= 0x0410 && code <= 0x042f {
            *lower += 0x20;
        } else if code >= 0x0430 && code <= 0x044f {
            *upper -= 0x20;
        } else if code >= 0x0450 && code <= 0x045f {
            *upper -= 0x50;
        } else if (code >= 0x0460 && code <= 0x0481)
            || (code >= 0x048a && code <= 0x04bf)
            || (code >= 0x04d0 && code <= 0x04f5)
            || (code >= 0x04f8 && code <= 0x04f9)
            || (code >= 0x0500 && code <= 0x050f)
        {
            *upper &= !1;
            *lower |= 1;
        } else if code >= 0x04c1 && code <= 0x04ce {
            if code & 1 != 0 {
                *lower += 1;
            } else {
                *upper -= 1;
            }
        }
    }

    /* Armenian, U+0530 to U+058F */
    if code >= 0x0530 && code <= 0x058f {
        if code >= 0x0531 && code <= 0x0556 {
            *lower += 0x30;
        } else if code >= 0x0561 && code <= 0x0586 {
            *upper -= 0x30;
        }
    }

    /* Latin Extended Additional, U+1E00 to U+1EFF */
    if code >= 0x1e00 && code <= 0x1eff {
        if (code >= 0x1e00 && code <= 0x1e95) || (code >= 0x1ea0 && code <= 0x1ef9) {
            *upper &= !1;
            *lower |= 1;
        } else if code == 0x1e9b {
            *upper = 0x1e60;
        } else if code == 0x1e9e {
            *lower = 0x00df; /* ssharp */
        }
    }

    /* Greek Extended, U+1F00 to U+1FFF */
    if code >= 0x1f00 && code <= 0x1fff {
        *lower = GREEK_EXT_LOWER_MAPPING[code as usize - 0x1f00] as _;
        *upper = GREEK_EXT_UPPER_MAPPING[code as usize - 0x1f00] as _;
        if *upper == 0 {
            *upper = code;
        }
        if *lower == 0 {
            *lower = code;
        }
    }

    /* Letterlike Symbols, U+2100 to U+214F */
    if code >= 0x2100 && code <= 0x214f {
        match code {
            0x2126 => *lower = 0x03c9,
            0x212a => *lower = 0x006b,
            0x212b => *lower = 0x00e5,
            _ => {}
        }
    }
    /* Number Forms, U+2150 to U+218F */
    else if code >= 0x2160 && code <= 0x216f {
        *lower += 0x10;
    } else if code >= 0x2170 && code <= 0x217f {
        *upper -= 0x10;
    }
    /* Enclosed Alphanumerics, U+2460 to U+24FF */
    else if code >= 0x24b6 && code <= 0x24cf {
        *lower += 0x1a;
    } else if code >= 0x24d0 && code <= 0x24e9 {
        *upper -= 0x1a;
    }
    /* Halfwidth and Fullwidth Forms, U+FF00 to U+FFEF */
    else if code >= 0xff21 && code <= 0xff3a {
        *lower += 0x20;
    } else if code >= 0xff41 && code <= 0xff5a {
        *upper -= 0x20;
    }
    /* Deseret, U+10400 to U+104FF */
    else if code >= 0x10400 && code <= 0x10427 {
        *lower += 0x28;
    } else if code >= 0x10428 && code <= 0x1044f {
        *upper -= 0x28;
    }
}

fn x_convert_case(sym: u32) -> (u32, u32) {
    let mut lower = 0;
    let mut upper = 0;
    x_convert_case2(sym, &mut lower, &mut upper);
    (lower, upper)
}

fn x_convert_case2(sym: u32, lower: &mut u32, upper: &mut u32) {
    /* Latin 1 keysym */
    if sym < 0x100 {
        ucsconvert_case(sym, lower, upper);
        return;
    }

    /* Unicode keysym */
    if (sym & 0xff000000) == 0x01000000 {
        ucsconvert_case(sym & 0x00ffffff, lower, upper);
        *upper |= 0x01000000;
        *lower |= 0x01000000;
        return;
    }

    /* Legacy keysym */

    *lower = sym;
    *upper = sym;

    match sym >> 8 {
        1 => {
            /* Latin 2 */
            /* Assume the KeySym is a legal value (ignore discontinuities) */
            if sym == XK_Aogonek {
                *lower = XK_aogonek;
            } else if sym >= XK_Lstroke && sym <= XK_Sacute {
                *lower += XK_lstroke - XK_Lstroke;
            } else if sym >= XK_Scaron && sym <= XK_Zacute {
                *lower += XK_scaron - XK_Scaron;
            } else if sym >= XK_Zcaron && sym <= XK_Zabovedot {
                *lower += XK_zcaron - XK_Zcaron;
            } else if sym == XK_aogonek {
                *upper = XK_Aogonek;
            } else if sym >= XK_lstroke && sym <= XK_sacute {
                *upper -= XK_lstroke - XK_Lstroke;
            } else if sym >= XK_scaron && sym <= XK_zacute {
                *upper -= XK_scaron - XK_Scaron;
            } else if sym >= XK_zcaron && sym <= XK_zabovedot {
                *upper -= XK_zcaron - XK_Zcaron;
            } else if sym >= XK_Racute && sym <= XK_Tcedilla {
                *lower += XK_racute - XK_Racute;
            } else if sym >= XK_racute && sym <= XK_tcedilla {
                *upper -= XK_racute - XK_Racute;
            }
        }
        2 => {
            /* Latin 3 */
            /* Assume the KeySym is a legal value (ignore discontinuities) */
            if sym >= XK_Hstroke && sym <= XK_Hcircumflex {
                *lower += XK_hstroke - XK_Hstroke;
            } else if sym >= XK_Gbreve && sym <= XK_Jcircumflex {
                *lower += XK_gbreve - XK_Gbreve;
            } else if sym >= XK_hstroke && sym <= XK_hcircumflex {
                *upper -= XK_hstroke - XK_Hstroke;
            } else if sym >= XK_gbreve && sym <= XK_jcircumflex {
                *upper -= XK_gbreve - XK_Gbreve;
            } else if sym >= XK_Cabovedot && sym <= XK_Scircumflex {
                *lower += XK_cabovedot - XK_Cabovedot;
            } else if sym >= XK_cabovedot && sym <= XK_scircumflex {
                *upper -= XK_cabovedot - XK_Cabovedot;
            }
        }
        3 => {
            /* Latin 4 */
            /* Assume the KeySym is a legal value (ignore discontinuities) */
            if sym >= XK_Rcedilla && sym <= XK_Tslash {
                *lower += XK_rcedilla - XK_Rcedilla;
            } else if sym >= XK_rcedilla && sym <= XK_tslash {
                *upper -= XK_rcedilla - XK_Rcedilla;
            } else if sym == XK_ENG {
                *lower = XK_eng;
            } else if sym == XK_eng {
                *upper = XK_ENG;
            } else if sym >= XK_Amacron && sym <= XK_Umacron {
                *lower += XK_amacron - XK_Amacron;
            } else if sym >= XK_amacron && sym <= XK_umacron {
                *upper -= XK_amacron - XK_Amacron;
            }
        }
        6 => {
            /* Cyrillic */
            /* Assume the KeySym is a legal value (ignore discontinuities) */
            if sym >= XK_Serbian_DJE && sym <= XK_Serbian_DZE {
                *lower -= XK_Serbian_DJE - XK_Serbian_dje;
            } else if sym >= XK_Serbian_dje && sym <= XK_Serbian_dze {
                *upper += XK_Serbian_DJE - XK_Serbian_dje;
            } else if sym >= XK_Cyrillic_YU && sym <= XK_Cyrillic_HARDSIGN {
                *lower -= XK_Cyrillic_YU - XK_Cyrillic_yu;
            } else if sym >= XK_Cyrillic_yu && sym <= XK_Cyrillic_hardsign {
                *upper += XK_Cyrillic_YU - XK_Cyrillic_yu;
            }
        }
        7 => {
            /* Greek */
            /* Assume the KeySym is a legal value (ignore discontinuities) */
            if sym >= XK_Greek_ALPHAaccent && sym <= XK_Greek_OMEGAaccent {
                *lower += XK_Greek_alphaaccent - XK_Greek_ALPHAaccent;
            } else if sym >= XK_Greek_alphaaccent
                && sym <= XK_Greek_omegaaccent
                && sym != XK_Greek_iotaaccentdieresis
                && sym != XK_Greek_upsilonaccentdieresis
            {
                *upper -= XK_Greek_alphaaccent - XK_Greek_ALPHAaccent;
            } else if sym >= XK_Greek_ALPHA && sym <= XK_Greek_OMEGA {
                *lower += XK_Greek_alpha - XK_Greek_ALPHA;
            } else if sym >= XK_Greek_alpha
                && sym <= XK_Greek_omega
                && sym != XK_Greek_finalsmallsigma
            {
                *upper -= XK_Greek_alpha - XK_Greek_ALPHA;
            }
        }
        0x13 => {
            /* Latin 9 */
            if sym == XK_OE {
                *lower = XK_oe;
            } else if sym == XK_oe {
                *upper = XK_OE;
            } else if sym == XK_Ydiaeresis {
                *lower = XK_ydiaeresis;
            }
        }
        _ => {}
    }
}
