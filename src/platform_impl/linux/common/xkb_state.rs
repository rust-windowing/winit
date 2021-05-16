use std::convert::TryInto;
use std::env;
use std::ffi::CString;
use std::fs::File;
use std::os::raw::c_char;
use std::os::unix::ffi::OsStringExt;
use std::ptr;

#[cfg(feature = "wayland")]
use memmap2::MmapOptions;
#[cfg(feature = "wayland")]
pub use sctk::seat::keyboard::RMLVO;

#[cfg(feature = "x11")]
use x11_dl::xlib_xcb::xcb_connection_t;
#[cfg(feature = "x11")]
use xkbcommon_dl::x11::XKBCOMMON_X11_HANDLE as XKBXH;

use xkbcommon_dl::{
    self as ffi, xkb_state_component, XKBCOMMON_COMPOSE_HANDLE as XKBCH, XKBCOMMON_HANDLE as XKBH,
};

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
    xkb_compose_state_2: *mut ffi::xkb_compose_state,
    mods_state: ModifiersState,
    locked: bool,
    scratch_buffer: Vec<u8>,
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
        depressed_group: u32,
        latched_group: u32,
        locked_group: u32,
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
                depressed_group,
                latched_group,
                locked_group,
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

    pub(crate) fn get_utf8_raw(&mut self, keycode: u32) -> Option<&'static str> {
        if !self.ready() {
            return None;
        }
        let size = unsafe {
            (XKBH.xkb_state_key_get_utf8)(self.xkb_state, keycode + 8, ptr::null_mut(), 0)
        } + 1;
        if size <= 1 {
            return None;
        };
        self.scratch_buffer.clear();
        let size = size.try_into().unwrap();
        self.scratch_buffer.reserve(size);
        unsafe {
            self.scratch_buffer.set_len(size);
            (XKBH.xkb_state_key_get_utf8)(
                self.xkb_state,
                keycode + 8,
                self.scratch_buffer.as_mut_ptr() as *mut _,
                size,
            );
        };
        // remove the final `\0`
        self.scratch_buffer.pop();
        Some(byte_slice_to_cached_string(&self.scratch_buffer))
    }

    fn compose_feed_normal(&mut self, keysym: u32) -> Option<ffi::xkb_compose_feed_result> {
        self.compose_feed(self.xkb_compose_state, keysym)
    }

    fn compose_feed_2(&mut self, keysym: u32) -> Option<ffi::xkb_compose_feed_result> {
        self.compose_feed(self.xkb_compose_state_2, keysym)
    }

    fn compose_feed(
        &mut self,
        xkb_compose_state: *mut ffi::xkb_compose_state,
        keysym: u32,
    ) -> Option<ffi::xkb_compose_feed_result> {
        if !self.ready() || self.xkb_compose_state.is_null() {
            return None;
        }
        Some(unsafe { (XKBCH.xkb_compose_state_feed)(xkb_compose_state, keysym) })
    }

    fn compose_status_normal(&mut self) -> Option<ffi::xkb_compose_status> {
        self.compose_status(self.xkb_compose_state)
    }

    #[allow(dead_code)]
    fn compose_status_2(&mut self) -> Option<ffi::xkb_compose_status> {
        self.compose_status(self.xkb_compose_state_2)
    }

    fn compose_status(
        &mut self,
        xkb_compose_state: *mut ffi::xkb_compose_state,
    ) -> Option<ffi::xkb_compose_status> {
        if !self.ready() || xkb_compose_state.is_null() {
            return None;
        }
        Some(unsafe { (XKBCH.xkb_compose_state_get_status)(xkb_compose_state) })
    }

    fn compose_get_utf8_normal(&mut self) -> Option<&'static str> {
        self.compose_get_utf8(self.xkb_compose_state)
    }

    fn compose_get_utf8_2(&mut self) -> Option<&'static str> {
        self.compose_get_utf8(self.xkb_compose_state_2)
    }

    fn compose_get_utf8(
        &mut self,
        xkb_compose_state: *mut ffi::xkb_compose_state,
    ) -> Option<&'static str> {
        if !self.ready() || xkb_compose_state.is_null() {
            return None;
        }
        let size =
            unsafe { (XKBCH.xkb_compose_state_get_utf8)(xkb_compose_state, ptr::null_mut(), 0) }
                + 1;
        if size <= 1 {
            return None;
        };
        self.scratch_buffer.clear();
        let size = size.try_into().unwrap();
        self.scratch_buffer.reserve(size);
        unsafe {
            self.scratch_buffer.set_len(size);
            (XKBCH.xkb_compose_state_get_utf8)(
                xkb_compose_state,
                self.scratch_buffer.as_mut_ptr() as *mut _,
                size as usize,
            );
        };
        // remove the final `\0`
        self.scratch_buffer.pop();
        Some(byte_slice_to_cached_string(&self.scratch_buffer))
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
            xkb_compose_state_2: ptr::null_mut(),
            mods_state: ModifiersState::new(),
            locked: false,
            scratch_buffer: Vec::new(),
        };

        unsafe { me.init_compose() };

        Ok(me)
    }
}

impl KbState {
    #[cfg(feature = "x11")]
    pub(crate) fn from_x11_xkb(connection: *mut xcb_connection_t) -> Result<KbState, Error> {
        let mut me = Self::new()?;

        let result = unsafe {
            (XKBXH.xkb_x11_setup_xkb_extension)(
                connection,
                1,
                2,
                xkbcommon_dl::x11::xkb_x11_setup_xkb_extension_flags::XKB_X11_SETUP_XKB_EXTENSION_NO_FLAGS,
                ptr::null_mut(),
                ptr::null_mut(),
                ptr::null_mut(),
                ptr::null_mut(),
            )
        };
        assert_eq!(result, 1, "Failed to initialize libxkbcommon");

        // TODO: Support keyboards other than the "virtual core keyboard device".
        let core_keyboard_id = unsafe { (XKBXH.xkb_x11_get_core_keyboard_device_id)(connection) };
        let keymap = unsafe {
            (XKBXH.xkb_x11_keymap_new_from_device)(
                me.xkb_context,
                connection,
                core_keyboard_id,
                xkbcommon_dl::xkb_keymap_compile_flags::XKB_KEYMAP_COMPILE_NO_FLAGS,
            )
        };
        assert_ne!(keymap, ptr::null_mut());
        me.xkb_keymap = keymap;

        unsafe { me.post_init(keymap) };

        Ok(me)
    }

    #[cfg(feature = "wayland")]
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

    unsafe fn init_compose(&mut self) {
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

        let compose_state_2 = (XKBCH.xkb_compose_state_new)(
            compose_table,
            ffi::xkb_compose_state_flags::XKB_COMPOSE_STATE_NO_FLAGS,
        );

        if compose_state.is_null() || compose_state_2.is_null() {
            // init of compose state failed, continue without compose
            (XKBCH.xkb_compose_table_unref)(compose_table);
            return;
        }

        self.xkb_compose_table = compose_table;
        self.xkb_compose_state = compose_state;
        self.xkb_compose_state_2 = compose_state_2;
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

    #[cfg(feature = "wayland")]
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
}

impl KbState {
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

    fn keysym_to_utf8_raw(&mut self, keysym: u32) -> Option<&'static str> {
        self.scratch_buffer.clear();
        self.scratch_buffer.reserve(8);
        loop {
            unsafe { self.scratch_buffer.set_len(8) };
            let bytes_written = unsafe {
                (XKBH.xkb_keysym_to_utf8)(
                    keysym,
                    self.scratch_buffer.as_mut_ptr().cast(),
                    self.scratch_buffer.capacity(),
                )
            };
            if bytes_written == 0 {
                return None;
            } else if bytes_written == -1 {
                self.scratch_buffer.reserve(8);
            } else {
                unsafe {
                    self.scratch_buffer
                        .set_len(bytes_written.try_into().unwrap())
                };
                break;
            }
        }

        // remove the final `\0`
        self.scratch_buffer.pop();
        Some(byte_slice_to_cached_string(&self.scratch_buffer))
    }
}

#[derive(Copy, Clone, Debug)]
enum XkbCompose {
    Accepted(ffi::xkb_compose_status),
    Ignored,
    Uninitialized,
}

pub(crate) struct KeyEventResults<'a> {
    state: &'a mut KbState,
    keycode: u32,
    keysym: u32,
    compose: Option<XkbCompose>,
}

impl<'a> KeyEventResults<'a> {
    fn new(state: &'a mut KbState, keycode: u32, compose: bool) -> Self {
        let keysym = state.get_one_sym_raw(keycode);

        let compose = if compose {
            Some(match state.compose_feed_normal(keysym) {
                Some(ffi::xkb_compose_feed_result::XKB_COMPOSE_FEED_ACCEPTED) => {
                    // Unwrapping is safe here, as `compose_feed` returns `None` when composition is uninitialized.
                    XkbCompose::Accepted(state.compose_status_normal().unwrap())
                }
                Some(ffi::xkb_compose_feed_result::XKB_COMPOSE_FEED_IGNORED) => XkbCompose::Ignored,
                None => XkbCompose::Uninitialized,
            })
        } else {
            None
        };

        KeyEventResults {
            state,
            keycode,
            keysym,
            compose,
        }
    }

    pub fn keycode(&mut self) -> KeyCode {
        super::keymap::rawkey_to_keycode(self.keycode)
    }

    pub fn key(&mut self) -> (Key<'static>, KeyLocation) {
        self.keysym_to_key(self.keysym)
            .unwrap_or_else(|(key, location)| match self.compose {
                Some(XkbCompose::Accepted(ffi::xkb_compose_status::XKB_COMPOSE_COMPOSING)) => {
                    // When pressing a dead key twice, the non-combining variant of that character will be
                    // produced. Since this function only concerns itself with a single keypress, we simulate
                    // this double press here by feeding the keysym to the compose state twice.
                    self.state.compose_feed_2(self.keysym);
                    match self.state.compose_feed_2(self.keysym) {
                        Some(ffi::xkb_compose_feed_result::XKB_COMPOSE_FEED_ACCEPTED) => (
                            // Extracting only a single `char` here *should* be fine, assuming that no dead
                            // key's non-combining variant ever occupies more than one `char`.
                            Key::Dead(
                                self.state
                                    .compose_get_utf8_2()
                                    .map(|s| s.chars().nth(0).unwrap()),
                            ),
                            location,
                        ),
                        _ => (key, location),
                    }
                }
                _ => (
                    self.composed_text()
                        .unwrap_or_else(|_| self.state.keysym_to_utf8_raw(self.keysym))
                        .map(Key::Character)
                        .unwrap_or(key),
                    location,
                ),
            })
    }

    pub fn key_without_modifiers(&mut self) -> (Key<'static>, KeyLocation) {
        // This will become a pointer to an array which libxkbcommon owns, so we don't need to deallocate it.
        let mut keysyms = ptr::null();
        let keysym_count = unsafe {
            (XKBH.xkb_keymap_key_get_syms_by_level)(
                self.state.xkb_keymap,
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
        self.keysym_to_key(keysym)
            .unwrap_or_else(|(key, location)| {
                (
                    self.state
                        .keysym_to_utf8_raw(keysym)
                        .map(Key::Character)
                        .unwrap_or(key),
                    location,
                )
            })
    }

    fn keysym_to_key(
        &mut self,
        keysym: u32,
    ) -> Result<(Key<'static>, KeyLocation), (Key<'static>, KeyLocation)> {
        let location = super::keymap::keysym_location(keysym);
        let key = super::keymap::keysym_to_key(keysym);
        if matches!(key, Key::Unidentified(_)) {
            Err((key, location))
        } else {
            Ok((key, location))
        }
    }

    pub fn text(&mut self) -> Option<&'static str> {
        self.composed_text()
            .unwrap_or_else(|_| self.state.keysym_to_utf8_raw(self.keysym))
    }

    pub fn text_with_all_modifiers(&mut self) -> Option<&'static str> {
        // TODO: Should Ctrl override any attempts to compose text?
        //       gnome-terminal agrees, but konsole disagrees.
        //       Should it be configurable instead?
        self.composed_text()
            .unwrap_or_else(|_| self.state.get_utf8_raw(self.keycode))
    }

    fn composed_text(&mut self) -> Result<Option<&'static str>, ()> {
        if let Some(compose) = &self.compose {
            match compose {
                XkbCompose::Accepted(status) => match status {
                    ffi::xkb_compose_status::XKB_COMPOSE_COMPOSED => {
                        Ok(self.state.compose_get_utf8_normal())
                    }
                    ffi::xkb_compose_status::XKB_COMPOSE_NOTHING => Err(()),
                    _ => Ok(None),
                },
                XkbCompose::Ignored | XkbCompose::Uninitialized => Err(()),
            }
        } else {
            Err(())
        }
    }
}

fn byte_slice_to_cached_string(bytes: &[u8]) -> &'static str {
    use std::cell::RefCell;
    use std::collections::HashSet;

    thread_local! {
        static STRING_CACHE: RefCell<HashSet<&'static str>> = RefCell::new(HashSet::new());
    }

    let string = std::str::from_utf8(bytes).unwrap();

    STRING_CACHE.with(|cache| {
        let mut cache = cache.borrow_mut();
        if let Some(string) = cache.get(string) {
            *string
        } else {
            // borrowck couldn't quite figure out this one on its own
            let string: &'static str = Box::leak(String::from(string).into_boxed_str());
            cache.insert(string);
            string
        }
    })
}
