use std::cell::RefCell;
use std::convert::TryInto;
use std::env;
use std::ffi::CString;
use std::fs::File;
use std::os::raw::c_char;
use std::os::unix::ffi::OsStringExt;
use std::ptr;

#[cfg(feature = "wayland")]
use memmap2::MmapOptions;
use xkbcommon_dl::{
    self as ffi, xkb_state_component, XKBCOMMON_COMPOSE_HANDLE as XKBCH, XKBCOMMON_HANDLE as XKBH,
};

pub use sctk::seat::keyboard::RMLVO;

use crate::{
    event::ElementState,
    keyboard::{Key, KeyCode, KeyLocation},
};

pub(crate) struct KbState {
    xkb_context: *mut ffi::xkb_context,
    xkb_keymap: *mut ffi::xkb_keymap,
    xkb_state: *mut ffi::xkb_state,
    xkb_compose_table: *mut ffi::xkb_compose_table,
    xkb_compose_state: *mut ffi::xkb_compose_state,
    mods_state: ModifiersState,
    locked: bool,
}

/// Represents the current state of the keyboard modifiers
///
/// Each field of this struct represents a modifier and is `true` if this modifier is active.
///
/// For some modifiers, this means that the key is currently pressed, others are toggled
/// (like caps lock).
#[derive(Copy, Clone, Debug, Default)]
pub struct ModifiersState {
    /// The "control" key
    pub ctrl: bool,
    /// The "alt" key
    pub alt: bool,
    /// The "shift" key
    pub shift: bool,
    /// The "Caps lock" key
    pub caps_lock: bool,
    /// The "logo" key
    ///
    /// Also known as the "windows" key on most keyboards
    pub logo: bool,
    /// The "Num lock" key
    pub num_lock: bool,
}

impl ModifiersState {
    fn new() -> ModifiersState {
        ModifiersState::default()
    }

    fn update_with(&mut self, state: *mut ffi::xkb_state) {
        self.ctrl = unsafe {
            (XKBH.xkb_state_mod_name_is_active)(
                state,
                ffi::XKB_MOD_NAME_CTRL.as_ptr() as *const c_char,
                xkb_state_component::XKB_STATE_MODS_EFFECTIVE,
            ) > 0
        };
        self.alt = unsafe {
            (XKBH.xkb_state_mod_name_is_active)(
                state,
                ffi::XKB_MOD_NAME_ALT.as_ptr() as *const c_char,
                xkb_state_component::XKB_STATE_MODS_EFFECTIVE,
            ) > 0
        };
        self.shift = unsafe {
            (XKBH.xkb_state_mod_name_is_active)(
                state,
                ffi::XKB_MOD_NAME_SHIFT.as_ptr() as *const c_char,
                xkb_state_component::XKB_STATE_MODS_EFFECTIVE,
            ) > 0
        };
        self.caps_lock = unsafe {
            (XKBH.xkb_state_mod_name_is_active)(
                state,
                ffi::XKB_MOD_NAME_CAPS.as_ptr() as *const c_char,
                xkb_state_component::XKB_STATE_MODS_EFFECTIVE,
            ) > 0
        };
        self.logo = unsafe {
            (XKBH.xkb_state_mod_name_is_active)(
                state,
                ffi::XKB_MOD_NAME_LOGO.as_ptr() as *const c_char,
                xkb_state_component::XKB_STATE_MODS_EFFECTIVE,
            ) > 0
        };
        self.num_lock = unsafe {
            (XKBH.xkb_state_mod_name_is_active)(
                state,
                ffi::XKB_MOD_NAME_NUM.as_ptr() as *const c_char,
                xkb_state_component::XKB_STATE_MODS_EFFECTIVE,
            ) > 0
        };
    }
}

impl KbState {
    pub(crate) fn update_modifiers(
        &mut self,
        mods_depressed: u32,
        mods_latched: u32,
        mods_locked: u32,
        group: u32,
    ) {
        if !self.ready() {
            return;
        }
        let mask = unsafe {
            (XKBH.xkb_state_update_mask)(
                self.xkb_state,
                mods_depressed,
                mods_latched,
                mods_locked,
                0,
                0,
                group,
            )
        };
        if mask.contains(xkb_state_component::XKB_STATE_MODS_EFFECTIVE) {
            // effective value of mods have changed, we need to update our state
            self.mods_state.update_with(self.xkb_state);
        }
    }

    pub(crate) fn get_one_sym_raw(&mut self, keycode: u32) -> u32 {
        if !self.ready() {
            return 0;
        }
        unsafe { (XKBH.xkb_state_key_get_one_sym)(self.xkb_state, keycode + 8) }
    }

    pub(crate) fn get_utf8_raw(&mut self, keycode: u32) -> Option<String> {
        if !self.ready() {
            return None;
        }
        let size = unsafe {
            (XKBH.xkb_state_key_get_utf8)(self.xkb_state, keycode + 8, ptr::null_mut(), 0)
        } + 1;
        if size <= 1 {
            return None;
        };
        let mut buffer = Vec::with_capacity(size as usize);
        unsafe {
            buffer.set_len(size as usize);
            (XKBH.xkb_state_key_get_utf8)(
                self.xkb_state,
                keycode + 8,
                buffer.as_mut_ptr() as *mut _,
                size as usize,
            );
        };
        // remove the final `\0`
        buffer.pop();
        // libxkbcommon will always provide valid UTF8
        Some(unsafe { String::from_utf8_unchecked(buffer) })
    }

    pub(crate) fn compose_feed(&mut self, keysym: u32) -> Option<ffi::xkb_compose_feed_result> {
        if !self.ready() || self.xkb_compose_state.is_null() {
            return None;
        }
        Some(unsafe { (XKBCH.xkb_compose_state_feed)(self.xkb_compose_state, keysym) })
    }

    pub(crate) fn compose_status(&mut self) -> Option<ffi::xkb_compose_status> {
        if !self.ready() || self.xkb_compose_state.is_null() {
            return None;
        }
        Some(unsafe { (XKBCH.xkb_compose_state_get_status)(self.xkb_compose_state) })
    }

    pub(crate) fn compose_get_utf8(&mut self) -> Option<String> {
        if !self.ready() || self.xkb_compose_state.is_null() {
            return None;
        }
        let size = unsafe {
            (XKBCH.xkb_compose_state_get_utf8)(self.xkb_compose_state, ptr::null_mut(), 0)
        } + 1;
        if size <= 1 {
            return None;
        };
        let mut buffer = Vec::with_capacity(size as usize);
        unsafe {
            buffer.set_len(size as usize);
            (XKBCH.xkb_compose_state_get_utf8)(
                self.xkb_compose_state,
                buffer.as_mut_ptr() as *mut _,
                size as usize,
            );
        };
        // remove the final `\0`
        buffer.pop();
        // libxkbcommon will always provide valid UTF8
        Some(unsafe { String::from_utf8_unchecked(buffer) })
    }

    pub(crate) fn new() -> Result<KbState, Error> {
        {
            if ffi::XKBCOMMON_OPTION.as_ref().is_none() {
                return Err(Error::XKBNotFound);
            }
        }
        let context =
            unsafe { (XKBH.xkb_context_new)(ffi::xkb_context_flags::XKB_CONTEXT_NO_FLAGS) };
        if context.is_null() {
            return Err(Error::XKBNotFound);
        }

        let mut me = KbState {
            xkb_context: context,
            xkb_keymap: ptr::null_mut(),
            xkb_state: ptr::null_mut(),
            xkb_compose_table: ptr::null_mut(),
            xkb_compose_state: ptr::null_mut(),
            mods_state: ModifiersState::new(),
            locked: false,
        };

        unsafe {
            me.init_compose();
        }

        Ok(me)
    }

    pub(crate) fn from_rmlvo(rmlvo: RMLVO) -> Result<KbState, Error> {
        fn to_cstring(s: Option<String>) -> Result<Option<CString>, Error> {
            s.map_or(Ok(None), |s| CString::new(s).map(Option::Some))
                .map_err(|_| Error::BadNames)
        }

        let mut state = KbState::new()?;

        let rules = to_cstring(rmlvo.rules)?;
        let model = to_cstring(rmlvo.model)?;
        let layout = to_cstring(rmlvo.layout)?;
        let variant = to_cstring(rmlvo.variant)?;
        let options = to_cstring(rmlvo.options)?;

        let xkb_names = ffi::xkb_rule_names {
            rules: rules.map_or(ptr::null(), |s| s.as_ptr()),
            model: model.map_or(ptr::null(), |s| s.as_ptr()),
            layout: layout.map_or(ptr::null(), |s| s.as_ptr()),
            variant: variant.map_or(ptr::null(), |s| s.as_ptr()),
            options: options.map_or(ptr::null(), |s| s.as_ptr()),
        };

        unsafe {
            state.init_with_rmlvo(xkb_names)?;
        }

        state.locked = true;
        Ok(state)
    }

    pub(crate) unsafe fn init_compose(&mut self) {
        let locale = env::var_os("LC_ALL")
            .and_then(|v| if v.is_empty() { None } else { Some(v) })
            .or_else(|| env::var_os("LC_CTYPE"))
            .and_then(|v| if v.is_empty() { None } else { Some(v) })
            .or_else(|| env::var_os("LANG"))
            .and_then(|v| if v.is_empty() { None } else { Some(v) })
            .unwrap_or_else(|| "C".into());
        let locale = CString::new(locale.into_vec()).unwrap();

        let compose_table = (XKBCH.xkb_compose_table_new_from_locale)(
            self.xkb_context,
            locale.as_ptr(),
            ffi::xkb_compose_compile_flags::XKB_COMPOSE_COMPILE_NO_FLAGS,
        );

        if compose_table.is_null() {
            // init of compose table failed, continue without compose
            return;
        }

        let compose_state = (XKBCH.xkb_compose_state_new)(
            compose_table,
            ffi::xkb_compose_state_flags::XKB_COMPOSE_STATE_NO_FLAGS,
        );

        if compose_state.is_null() {
            // init of compose state failed, continue without compose
            (XKBCH.xkb_compose_table_unref)(compose_table);
            return;
        }

        self.xkb_compose_table = compose_table;
        self.xkb_compose_state = compose_state;
    }

    pub(crate) unsafe fn post_init(&mut self, keymap: *mut ffi::xkb_keymap) {
        let state = (XKBH.xkb_state_new)(keymap);
        self.xkb_keymap = keymap;
        self.xkb_state = state;
        self.mods_state.update_with(state);
    }

    pub(crate) unsafe fn de_init(&mut self) {
        (XKBH.xkb_state_unref)(self.xkb_state);
        self.xkb_state = ptr::null_mut();
        (XKBH.xkb_keymap_unref)(self.xkb_keymap);
        self.xkb_keymap = ptr::null_mut();
    }

    #[cfg(feature = "wayland")]
    pub(crate) unsafe fn init_with_fd(&mut self, fd: File, size: usize) {
        let map = MmapOptions::new().len(size).map(&fd).unwrap();

        let keymap = (XKBH.xkb_keymap_new_from_string)(
            self.xkb_context,
            map.as_ptr() as *const _,
            ffi::xkb_keymap_format::XKB_KEYMAP_FORMAT_TEXT_V1,
            ffi::xkb_keymap_compile_flags::XKB_KEYMAP_COMPILE_NO_FLAGS,
        );

        if keymap.is_null() {
            panic!("Received invalid keymap from compositor.");
        }

        self.post_init(keymap);
    }

    pub(crate) unsafe fn init_with_rmlvo(
        &mut self,
        names: ffi::xkb_rule_names,
    ) -> Result<(), Error> {
        let keymap = (XKBH.xkb_keymap_new_from_names)(
            self.xkb_context,
            &names,
            ffi::xkb_keymap_compile_flags::XKB_KEYMAP_COMPILE_NO_FLAGS,
        );

        if keymap.is_null() {
            return Err(Error::BadNames);
        }

        self.post_init(keymap);

        Ok(())
    }

    pub(crate) unsafe fn key_repeats(&mut self, keycode: ffi::xkb_keycode_t) -> bool {
        (XKBH.xkb_keymap_key_repeats)(self.xkb_keymap, keycode + 8) == 1
    }

    #[inline]
    pub(crate) fn ready(&self) -> bool {
        !self.xkb_state.is_null()
    }

    #[inline]
    pub(crate) fn locked(&self) -> bool {
        self.locked
    }

    #[inline]
    pub(crate) fn mods_state(&self) -> ModifiersState {
        self.mods_state
    }
}

impl Drop for KbState {
    fn drop(&mut self) {
        unsafe {
            (XKBCH.xkb_compose_state_unref)(self.xkb_compose_state);
            (XKBCH.xkb_compose_table_unref)(self.xkb_compose_table);
            (XKBH.xkb_state_unref)(self.xkb_state);
            (XKBH.xkb_keymap_unref)(self.xkb_keymap);
            (XKBH.xkb_context_unref)(self.xkb_context);
        }
    }
}

#[derive(Debug)]
pub enum Error {
    /// libxkbcommon is not available
    XKBNotFound,
    /// Provided RMLVO specified a keymap that would not be loaded
    BadNames,
}

impl KbState {
    pub fn process_key_event(&mut self, keycode: u32, state: ElementState) -> KeyEventResults<'_> {
        KeyEventResults::new(self, keycode, state == ElementState::Pressed)
    }

    pub fn process_key_repeat_event(&mut self, keycode: u32) -> KeyEventResults<'_> {
        KeyEventResults::new(self, keycode, false)
    }
}

enum XkbCompose {
    Accepted(ffi::xkb_compose_status),
    Ignored,
    Uninitialized,
}

pub(crate) struct KeyEventResults<'a> {
    state: RefCell<&'a mut KbState>,
    keycode: u32,
    keysym: u32,
    compose: Option<XkbCompose>,
}

impl<'a> KeyEventResults<'a> {
    fn new(state: &'a mut KbState, keycode: u32, compose: bool) -> Self {
        let keysym = state.get_one_sym_raw(keycode);

        let compose = if compose {
            Some(match state.compose_feed(keysym) {
                Some(ffi::xkb_compose_feed_result::XKB_COMPOSE_FEED_ACCEPTED) => {
                    // Unwrapping is safe here, as `compose_feed` returns `None` when composition is uninitialized.
                    XkbCompose::Accepted(state.compose_status().unwrap())
                }
                Some(ffi::xkb_compose_feed_result::XKB_COMPOSE_FEED_IGNORED) => XkbCompose::Ignored,
                None => XkbCompose::Uninitialized,
            })
        } else {
            None
        };

        KeyEventResults {
            state: RefCell::new(state),
            keycode,
            keysym,
            compose,
        }
    }

    pub fn keycode(&mut self) -> KeyCode {
        super::keymap::rawkey_to_keycode(self.keycode)
    }

    pub fn key(&mut self) -> (Key<'static>, KeyLocation) {
        Self::keysym_to_key(self.keysym)
    }

    pub fn key_without_modifiers(&mut self) -> (Key<'static>, KeyLocation) {
        // This will become a pointer to an array which libxkbcommon owns, so we don't need to deallocate it.
        let mut keysyms = ptr::null();
        let keysym_count = unsafe {
            (XKBH.xkb_keymap_key_get_syms_by_level)(
                self.state.borrow_mut().xkb_keymap,
                self.keycode + 8,
                0,
                0,
                &mut keysyms,
            )
        };
        let keysym = if keysym_count == 1 {
            unsafe { *keysyms }
        } else {
            0
        };
        Self::keysym_to_key(keysym)
    }

    fn keysym_to_key(keysym: u32) -> (Key<'static>, KeyLocation) {
        let location = super::keymap::keysym_location(keysym);
        let mut key = super::keymap::keysym_to_key(keysym);
        if matches!(key, Key::Unidentified(_)) {
            if let Some(string) = keysym_to_utf8_raw(keysym) {
                key = Key::Character(cached_string(string));
            }
        }
        (key, location)
    }

    pub fn text(&mut self) -> Option<&'static str> {
        let keysym = self.keysym;
        self._text(|| keysym_to_utf8_raw(keysym))
    }

    pub fn text_with_all_modifiers(&mut self) -> Option<&'static str> {
        // TODO: Should Ctrl override any attempts to compose text?
        //       gnome-terminal agrees, but konsole disagrees.
        //       Should it be configurable instead?
        let keycode = self.keycode;
        self._text(|| self.state.borrow_mut().get_utf8_raw(keycode))
    }

    fn _text<F>(&self, fallback: F) -> Option<&'static str>
    where
        F: FnOnce() -> Option<String>,
    {
        if let Some(compose) = &self.compose {
            match compose {
                XkbCompose::Accepted(status) => match status {
                    ffi::xkb_compose_status::XKB_COMPOSE_COMPOSED => {
                        self.state.borrow_mut().compose_get_utf8()
                    }
                    ffi::xkb_compose_status::XKB_COMPOSE_NOTHING => fallback(),
                    _ => None,
                },
                XkbCompose::Ignored | XkbCompose::Uninitialized => fallback(),
            }
        } else {
            fallback()
        }
        .map(cached_string)
    }
}

fn keysym_to_utf8_raw(keysym: u32) -> Option<String> {
    let mut buffer: Vec<u8> = Vec::with_capacity(8);
    loop {
        let bytes_written = unsafe {
            (XKBH.xkb_keysym_to_utf8)(keysym, buffer.as_mut_ptr().cast(), buffer.capacity())
        };
        if bytes_written == 0 {
            return None;
        } else if bytes_written == -1 {
            buffer.reserve(8);
        } else {
            unsafe { buffer.set_len(bytes_written.try_into().unwrap()) };
            break;
        }
    }

    // remove the final `\0`
    buffer.pop();
    // libxkbcommon will always provide valid UTF8
    Some(unsafe { String::from_utf8_unchecked(buffer) })
}

fn cached_string<S: Into<String>>(string: S) -> &'static str {
    Box::leak(string.into().into_boxed_str())
}
