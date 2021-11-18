use std::ffi::CString;
use std::os::raw::c_char;
use std::os::unix::ffi::OsStringExt;
use std::{char, env, ptr, slice, str};

#[cfg(feature = "wayland")]
use memmap2::MmapOptions;
#[cfg(feature = "wayland")]
pub use sctk::seat::keyboard::RMLVO;

#[cfg(feature = "x11")]
use xcb_dl::ffi::xcb_connection_t;
#[cfg(feature = "x11")]
use xkbcommon_dl::XKBCOMMON_X11_HANDLE as XKBXH;

use xkbcommon_dl::{
    self as ffi, xkb_compose_status, xkb_state_component, XKBCOMMON_COMPOSE_HANDLE as XKBCH,
    XKBCOMMON_HANDLE as XKBH,
};

use crate::{
    event::ElementState,
    keyboard::{Key, KeyCode, KeyLocation, ModifiersState},
};

pub(crate) struct KbState {
    #[cfg(feature = "x11")]
    xcb_connection: *mut xcb_connection_t,
    xkb_context: *mut ffi::xkb_context,
    xkb_keymap: *mut ffi::xkb_keymap,
    xkb_state: *mut ffi::xkb_state,
    xkb_compose_table: *mut ffi::xkb_compose_table,
    xkb_compose_state: *mut ffi::xkb_compose_state,
    mod_indices: ModIndices,
    mods_state: ModifiersState,
    #[cfg(feature = "wayland")]
    locked: bool,
    scratch_buffer: Vec<u8>,
}

#[derive(Default)]
struct ModIndices {
    ctrl: u32,
    alt: u32,
    shift: u32,
    logo: u32,
}

impl ModIndices {
    unsafe fn from_keymap(xkb_keymap: *mut ffi::xkb_keymap) -> Self {
        let ctrl = (XKBH.xkb_keymap_mod_get_index)(
            xkb_keymap,
            ffi::XKB_MOD_NAME_CTRL.as_ptr() as *const c_char,
        );
        let alt = (XKBH.xkb_keymap_mod_get_index)(
            xkb_keymap,
            ffi::XKB_MOD_NAME_ALT.as_ptr() as *const c_char,
        );
        let shift = (XKBH.xkb_keymap_mod_get_index)(
            xkb_keymap,
            ffi::XKB_MOD_NAME_SHIFT.as_ptr() as *const c_char,
        );
        let logo = (XKBH.xkb_keymap_mod_get_index)(
            xkb_keymap,
            ffi::XKB_MOD_NAME_LOGO.as_ptr() as *const c_char,
        );
        Self {
            ctrl,
            alt,
            shift,
            logo,
        }
    }
}

unsafe fn xkb_state_to_modifiers(
    state: *mut ffi::xkb_state,
    indices: &ModIndices,
) -> ModifiersState {
    let ctrl = (XKBH.xkb_state_mod_index_is_active)(
        state,
        indices.ctrl,
        xkb_state_component::XKB_STATE_MODS_EFFECTIVE,
    ) > 0;
    let alt = (XKBH.xkb_state_mod_index_is_active)(
        state,
        indices.alt,
        xkb_state_component::XKB_STATE_MODS_EFFECTIVE,
    ) > 0;
    let shift = (XKBH.xkb_state_mod_index_is_active)(
        state,
        indices.shift,
        xkb_state_component::XKB_STATE_MODS_EFFECTIVE,
    ) > 0;
    let logo = (XKBH.xkb_state_mod_index_is_active)(
        state,
        indices.logo,
        xkb_state_component::XKB_STATE_MODS_EFFECTIVE,
    ) > 0;

    let mut mods = ModifiersState::empty();
    mods.set(ModifiersState::SHIFT, shift);
    mods.set(ModifiersState::CONTROL, ctrl);
    mods.set(ModifiersState::ALT, alt);
    mods.set(ModifiersState::SUPER, logo);
    mods
}

impl KbState {
    pub(crate) fn update_state(
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
            self.mods_state = unsafe { xkb_state_to_modifiers(self.xkb_state, &self.mod_indices) };
        }
    }

    pub(crate) fn get_one_sym_raw(&mut self, keycode: u32) -> u32 {
        if !self.ready() {
            return 0;
        }
        unsafe { (XKBH.xkb_state_key_get_one_sym)(self.xkb_state, keycode) }
    }

    pub(crate) fn get_utf8_raw(&mut self, keycode: u32) -> Option<&'static str> {
        if !self.ready() {
            return None;
        }
        let utf32 = unsafe { (XKBH.xkb_state_key_get_utf32)(self.xkb_state, keycode) };
        char_to_str(utf32)
    }

    fn compose_feed(&mut self, keysym: u32) -> Option<ffi::xkb_compose_feed_result> {
        if !self.ready() || self.xkb_compose_state.is_null() {
            return None;
        }
        Some(unsafe { (XKBCH.xkb_compose_state_feed)(self.xkb_compose_state, keysym) })
    }

    pub fn reset_dead_keys(&mut self) {
        unsafe {
            (XKBCH.xkb_compose_state_reset)(self.xkb_compose_state);
        }
    }

    fn compose_status(&mut self) -> Option<ffi::xkb_compose_status> {
        if !self.ready() || self.xkb_compose_state.is_null() {
            return None;
        }
        Some(unsafe { (XKBCH.xkb_compose_state_get_status)(self.xkb_compose_state) })
    }

    fn compose_get_utf8(&mut self) -> Option<&'static str> {
        if !self.ready() || self.xkb_compose_state.is_null() {
            return None;
        }
        self.scratch_buffer.truncate(0);
        loop {
            unsafe {
                let size = (XKBCH.xkb_compose_state_get_utf8)(
                    self.xkb_compose_state,
                    self.scratch_buffer.as_mut_ptr() as *mut _,
                    self.scratch_buffer.capacity(),
                );
                if size < 0 {
                    return None;
                }
                let size = size as usize;
                if size >= self.scratch_buffer.capacity() {
                    self.scratch_buffer.reserve(size + 1);
                    continue;
                }
                self.scratch_buffer.set_len(size);
                return Some(byte_slice_to_cached_string(&self.scratch_buffer));
            }
        }
    }

    pub(crate) fn new() -> Result<Self, Error> {
        if ffi::XKBCOMMON_OPTION.as_ref().is_none() {
            return Err(Error::XKBNotFound);
        }

        let context =
            unsafe { (XKBH.xkb_context_new)(ffi::xkb_context_flags::XKB_CONTEXT_NO_FLAGS) };
        if context.is_null() {
            return Err(Error::XKBNotFound);
        }

        // let level = if log::log_enabled!(log::Level::Debug) {
        //     ffi::xkb_log_level::XKB_LOG_LEVEL_DEBUG
        // } else if log::log_enabled!(log::Level::Info) {
        //     ffi::xkb_log_level::XKB_LOG_LEVEL_INFO
        // } else if log::log_enabled!(log::Level::Warn) {
        //     ffi::xkb_log_level::XKB_LOG_LEVEL_WARNING
        // } else if log::log_enabled!(log::Level::Error) {
        //     ffi::xkb_log_level::XKB_LOG_LEVEL_ERROR
        // } else {
        //     ffi::xkb_log_level::XKB_LOG_LEVEL_CRITICAL
        // };
        // unsafe {
        //     (XKBH.xkb_context_set_log_level)(context, level);
        // }

        let mut me = Self {
            #[cfg(feature = "x11")]
            xcb_connection: ptr::null_mut(),
            xkb_context: context,
            xkb_keymap: ptr::null_mut(),
            xkb_state: ptr::null_mut(),
            xkb_compose_table: ptr::null_mut(),
            xkb_compose_state: ptr::null_mut(),
            mod_indices: Default::default(),
            mods_state: ModifiersState::empty(),
            #[cfg(feature = "wayland")]
            locked: false,
            scratch_buffer: Vec::with_capacity(5),
        };

        unsafe { me.init_compose() };

        Ok(me)
    }
}

impl KbState {
    #[cfg(feature = "x11")]
    pub(crate) fn from_x11_xkb(
        connection: *mut xcb_connection_t,
        device_id: xcb_dl::ffi::xcb_input_device_id_t,
    ) -> Result<Self, Error> {
        let mut me = Self::new()?;
        me.xcb_connection = connection;

        unsafe { me.init_with_x11_keymap(device_id) };

        Ok(me)
    }

    #[cfg(feature = "wayland")]
    pub(crate) fn from_rmlvo(rmlvo: RMLVO) -> Result<Self, Error> {
        fn to_cstring(s: Option<String>) -> Result<Option<CString>, Error> {
            s.map_or(Ok(None), |s| CString::new(s).map(Option::Some))
                .map_err(|_| Error::BadNames)
        }

        let mut state = Self::new()?;

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

        if compose_state.is_null() {
            // init of compose state failed, continue without compose
            (XKBCH.xkb_compose_table_unref)(compose_table);
            return;
        }

        self.xkb_compose_table = compose_table;
        self.xkb_compose_state = compose_state;
    }

    unsafe fn post_init(&mut self, state: *mut ffi::xkb_state, keymap: *mut ffi::xkb_keymap) {
        self.xkb_keymap = keymap;
        self.mod_indices = ModIndices::from_keymap(keymap);
        self.xkb_state = state;
        self.mods_state = xkb_state_to_modifiers(state, &self.mod_indices);
    }

    unsafe fn de_init(&mut self) {
        (XKBH.xkb_state_unref)(self.xkb_state);
        self.xkb_state = ptr::null_mut();
        (XKBH.xkb_keymap_unref)(self.xkb_keymap);
        self.xkb_keymap = ptr::null_mut();
    }

    #[cfg(feature = "x11")]
    pub(crate) unsafe fn init_with_x11_keymap(
        &mut self,
        device_id: xcb_dl::ffi::xcb_input_device_id_t,
    ) {
        if !self.xkb_keymap.is_null() {
            self.de_init();
        }

        let keymap = (XKBXH.xkb_x11_keymap_new_from_device)(
            self.xkb_context,
            self.xcb_connection as _,
            device_id as _,
            xkbcommon_dl::xkb_keymap_compile_flags::XKB_KEYMAP_COMPILE_NO_FLAGS,
        );
        assert_ne!(keymap, ptr::null_mut());

        let state =
            (XKBXH.xkb_x11_state_new_from_device)(keymap, self.xcb_connection as _, device_id as _);
        self.post_init(state, keymap);
    }

    #[cfg(feature = "wayland")]
    pub(crate) unsafe fn init_with_fd(&mut self, fd: std::fs::File, size: usize) {
        if !self.xkb_keymap.is_null() {
            self.de_init();
        }

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

        let state = (XKBH.xkb_state_new)(keymap);
        self.post_init(state, keymap);
    }

    #[cfg(feature = "wayland")]
    pub(crate) unsafe fn init_with_rmlvo(
        &mut self,
        names: ffi::xkb_rule_names,
    ) -> Result<(), Error> {
        if !self.xkb_keymap.is_null() {
            self.de_init();
        }

        let keymap = (XKBH.xkb_keymap_new_from_names)(
            self.xkb_context,
            &names,
            ffi::xkb_keymap_compile_flags::XKB_KEYMAP_COMPILE_NO_FLAGS,
        );

        if keymap.is_null() {
            return Err(Error::BadNames);
        }

        let state = (XKBH.xkb_state_new)(keymap);
        self.post_init(state, keymap);

        Ok(())
    }
}

impl KbState {
    #[cfg(feature = "wayland")]
    pub(crate) unsafe fn key_repeats(&mut self, keycode: ffi::xkb_keycode_t) -> bool {
        (XKBH.xkb_keymap_key_repeats)(self.xkb_keymap, keycode) == 1
    }

    #[inline]
    pub(crate) fn ready(&self) -> bool {
        !self.xkb_state.is_null()
    }

    #[inline]
    #[cfg(feature = "wayland")]
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
            // TODO: Simplify this. We can currently only safely assume that the `xkb_context`
            //       is always valid. If we can somehow guarantee the same for `xkb_state` and
            //       `xkb_keymap`, then we could omit their null-checks.
            if !self.xkb_compose_state.is_null() {
                (XKBCH.xkb_compose_state_unref)(self.xkb_compose_state);
            }
            if !self.xkb_compose_table.is_null() {
                (XKBCH.xkb_compose_table_unref)(self.xkb_compose_table);
            }
            if !self.xkb_state.is_null() {
                (XKBH.xkb_state_unref)(self.xkb_state);
            }
            if !self.xkb_keymap.is_null() {
                (XKBH.xkb_keymap_unref)(self.xkb_keymap);
            }
            (XKBH.xkb_context_unref)(self.xkb_context);
        }
    }
}

#[derive(Debug)]
pub enum Error {
    /// libxkbcommon is not available
    XKBNotFound,
    /// Provided RMLVO specified a keymap that would not be loaded
    #[cfg(feature = "wayland")]
    BadNames,
}

impl KbState {
    pub fn process_key_event(
        &mut self,
        keycode: u32,
        group: u32,
        state: ElementState,
    ) -> KeyEventResults {
        let keysym = self.get_one_sym_raw(keycode);

        let (text, text_with_all_modifiers);

        if state == ElementState::Pressed {
            // This is a press or repeat event. Feed the keysym to the compose engine.
            match self.compose_feed(keysym) {
                Some(ffi::xkb_compose_feed_result::XKB_COMPOSE_FEED_ACCEPTED) => {
                    // The keysym potentially affected the compose state. Check it.
                    match self.compose_status().unwrap() {
                        xkb_compose_status::XKB_COMPOSE_NOTHING => {
                            // There is no ongoing composing. Use the keysym on its own.
                            text = keysym_to_utf8_raw(keysym);
                            text_with_all_modifiers = self.get_utf8_raw(keycode);
                        }
                        xkb_compose_status::XKB_COMPOSE_COMPOSING => {
                            // Composing is ongoing and not yet completed. No text is produced.
                            text = None;
                            text_with_all_modifiers = None;
                        }
                        xkb_compose_status::XKB_COMPOSE_COMPOSED => {
                            // This keysym completed the sequence. The text is the result.
                            text = self.compose_get_utf8();
                            // The current behaviour makes it so composing a character overrides attempts to input a
                            // control character with the `Ctrl` key. We can potentially add a configuration option
                            // if someone specifically wants the opposite behaviour.
                            text_with_all_modifiers = text;
                        }
                        xkb_compose_status::XKB_COMPOSE_CANCELLED => {
                            // Before this keysym, composing was ongoing. This keysym was not a possible
                            // continuation of the sequence and thus aborted composing. The standard
                            // behavior on linux in this case is to ignore both the sequence and this keysym.
                            text = None;
                            text_with_all_modifiers = None;
                        }
                    }
                }
                Some(ffi::xkb_compose_feed_result::XKB_COMPOSE_FEED_IGNORED) => {
                    // This keysym is a modifier and thus has no effect on the engine. Nor does it produce
                    // text.
                    text = None;
                    text_with_all_modifiers = None;
                }
                _ => {
                    // The compose engine is disabled. Use the keysym on its own.
                    text = keysym_to_utf8_raw(keysym);
                    text_with_all_modifiers = self.get_utf8_raw(keycode);
                }
            }
        } else {
            // This is a key release. No text is produced.
            text = None;
            text_with_all_modifiers = None;
        }

        let key_without_modifiers = {
            // This will become a pointer to an array which libxkbcommon owns, so we don't need to deallocate it.
            let mut keysyms = ptr::null();
            let keysym_count = unsafe {
                (XKBH.xkb_keymap_key_get_syms_by_level)(
                    self.xkb_keymap,
                    keycode,
                    group,
                    0,
                    &mut keysyms,
                )
            };
            let keysym = if keysym_count == 1 {
                unsafe { *keysyms }
            } else {
                0
            };
            keysym_to_key(keysym)
        };

        let res = KeyEventResults {
            keycode: super::keymap::raw_keycode_to_keycode(keycode),
            location: super::keymap::keysym_location(keysym),
            key: keysym_to_key(keysym),
            key_without_modifiers,
            text,
            text_with_all_modifiers,
        };

        // log::trace!("{:?}", res);

        res
    }
}

#[derive(Debug)]
pub(crate) struct KeyEventResults {
    pub keycode: KeyCode,
    pub location: KeyLocation,
    pub key: Key<'static>,
    pub key_without_modifiers: Key<'static>,
    pub text: Option<&'static str>,
    pub text_with_all_modifiers: Option<&'static str>,
}

fn keysym_to_key(keysym: u32) -> Key<'static> {
    let key = super::keymap::keysym_to_key(keysym);
    if let Key::Unidentified(_) = key {
        keysym_to_utf8_raw(keysym)
            .map(Key::Character)
            .unwrap_or(key)
    } else {
        key
    }
}

fn keysym_to_utf8_raw(keysym: u32) -> Option<&'static str> {
    let utf32 = unsafe { (XKBH.xkb_keysym_to_utf32)(keysym) };
    char_to_str(utf32)
}

fn char_to_str(utf32: u32) -> Option<&'static str> {
    use std::cell::RefCell;
    use std::collections::HashMap;

    if utf32 == 0 {
        return None;
    }

    if utf32 < 128 {
        static ASCII: [u8; 128] = [
            0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21, 22, 23,
            24, 25, 26, 27, 28, 29, 30, 31, 32, 33, 34, 35, 36, 37, 38, 39, 40, 41, 42, 43, 44, 45,
            46, 47, 48, 49, 50, 51, 52, 53, 54, 55, 56, 57, 58, 59, 60, 61, 62, 63, 64, 65, 66, 67,
            68, 69, 70, 71, 72, 73, 74, 75, 76, 77, 78, 79, 80, 81, 82, 83, 84, 85, 86, 87, 88, 89,
            90, 91, 92, 93, 94, 95, 96, 97, 98, 99, 100, 101, 102, 103, 104, 105, 106, 107, 108,
            109, 110, 111, 112, 113, 114, 115, 116, 117, 118, 119, 120, 121, 122, 123, 124, 125,
            126, 127,
        ];
        unsafe {
            debug_assert_eq!(ASCII[utf32 as usize], utf32 as u8);
            return Some(str::from_utf8_unchecked(slice::from_raw_parts(
                &ASCII[utf32 as usize],
                1,
            )));
        }
    }

    thread_local! {
        static STRING_CACHE: RefCell<HashMap<u32, &'static str>> = RefCell::new(HashMap::new());
    }

    return STRING_CACHE.with(|cache| {
        let mut cache = cache.borrow_mut();
        if let Some(string) = cache.get(&utf32) {
            Some(*string)
        } else {
            let mut buf = [0; 4];
            let char = char::from_u32(utf32).unwrap();
            let string: &'static str =
                Box::leak(char.encode_utf8(&mut buf).to_string().into_boxed_str());
            cache.insert(utf32, string);
            Some(string)
        }
    });
}

fn byte_slice_to_cached_string(bytes: &[u8]) -> &'static str {
    use std::cell::RefCell;
    use std::collections::HashSet;

    thread_local! {
        static STRING_CACHE: RefCell<HashSet<&'static str>> = RefCell::new(HashSet::new());
    }

    let string = str::from_utf8(bytes).unwrap();

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
