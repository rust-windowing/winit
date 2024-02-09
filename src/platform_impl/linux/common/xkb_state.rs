use std::convert::TryInto;
use std::env;
use std::ffi::CString;
use std::os::raw::c_char;
use std::os::unix::ffi::OsStringExt;
use std::ptr;
use std::sync::atomic::{AtomicBool, Ordering};

use once_cell::sync::Lazy;
use smol_str::SmolStr;
use xkbcommon_dl::{
    self as ffi, xkb_state_component, xkbcommon_compose_handle, xkbcommon_handle, XkbCommon,
    XkbCommonCompose,
};
#[cfg(feature = "wayland")]
use {memmap2::MmapOptions, std::os::unix::io::OwnedFd};
#[cfg(feature = "x11")]
use {x11_dl::xlib_xcb::xcb_connection_t, xkbcommon_dl::x11::xkbcommon_x11_handle};

use crate::event::KeyEvent;
use crate::platform_impl::common::keymap;
use crate::platform_impl::KeyEventExtra;
use crate::{
    event::ElementState,
    keyboard::{Key, KeyLocation, PhysicalKey},
};

// TODO: Wire this up without using a static `AtomicBool`.
static RESET_DEAD_KEYS: AtomicBool = AtomicBool::new(false);

#[inline(always)]
pub fn reset_dead_keys() {
    RESET_DEAD_KEYS.store(true, Ordering::SeqCst);
}

static XKBH: Lazy<&'static XkbCommon> = Lazy::new(xkbcommon_handle);
static XKBCH: Lazy<&'static XkbCommonCompose> = Lazy::new(xkbcommon_compose_handle);
#[cfg(feature = "x11")]
static XKBXH: Lazy<&'static ffi::x11::XkbCommonX11> = Lazy::new(xkbcommon_x11_handle);

#[derive(Debug)]
pub struct KbdState {
    #[cfg(feature = "x11")]
    xcb_connection: *mut xcb_connection_t,
    xkb_context: *mut ffi::xkb_context,
    xkb_keymap: *mut ffi::xkb_keymap,
    xkb_state: *mut ffi::xkb_state,
    xkb_compose_table: *mut ffi::xkb_compose_table,
    xkb_compose_state: *mut ffi::xkb_compose_state,
    xkb_compose_state_2: *mut ffi::xkb_compose_state,
    mods_state: ModifiersState,
    #[cfg(feature = "x11")]
    pub core_keyboard_id: i32,
    scratch_buffer: Vec<u8>,
}

impl KbdState {
    pub fn update_modifiers(
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

    pub fn get_one_sym_raw(&mut self, keycode: u32) -> u32 {
        if !self.ready() {
            return 0;
        }
        unsafe { (XKBH.xkb_state_key_get_one_sym)(self.xkb_state, keycode) }
    }

    pub fn get_utf8_raw(&mut self, keycode: u32) -> Option<SmolStr> {
        if !self.ready() {
            return None;
        }
        let xkb_state = self.xkb_state;
        self.make_string_with({
            |ptr, len| unsafe { (XKBH.xkb_state_key_get_utf8)(xkb_state, keycode, ptr, len) }
        })
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
        if RESET_DEAD_KEYS.swap(false, Ordering::SeqCst) {
            unsafe { self.init_compose() };
        }
        Some(unsafe { (XKBCH.xkb_compose_state_feed)(xkb_compose_state, keysym) })
    }

    fn compose_status_normal(&mut self) -> Option<ffi::xkb_compose_status> {
        self.compose_status(self.xkb_compose_state)
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

    fn compose_get_utf8_normal(&mut self) -> Option<SmolStr> {
        self.compose_get_utf8(self.xkb_compose_state)
    }

    fn compose_get_utf8_2(&mut self) -> Option<SmolStr> {
        self.compose_get_utf8(self.xkb_compose_state_2)
    }

    fn compose_get_utf8(
        &mut self,
        xkb_compose_state: *mut ffi::xkb_compose_state,
    ) -> Option<SmolStr> {
        if !self.ready() || xkb_compose_state.is_null() {
            return None;
        }
        self.make_string_with(|ptr, len| unsafe {
            (XKBCH.xkb_compose_state_get_utf8)(xkb_compose_state, ptr, len)
        })
    }

    /// Shared logic for constructing a string with `xkb_compose_state_get_utf8` and
    /// `xkb_state_key_get_utf8`.
    fn make_string_with<F>(&mut self, mut f: F) -> Option<SmolStr>
    where
        F: FnMut(*mut c_char, usize) -> i32,
    {
        let size = f(ptr::null_mut(), 0);
        if size == 0 {
            return None;
        }
        let size = usize::try_from(size).unwrap();
        self.scratch_buffer.clear();
        // The allocated buffer must include space for the null-terminator
        self.scratch_buffer.reserve(size + 1);
        unsafe {
            let written = f(
                self.scratch_buffer.as_mut_ptr().cast(),
                self.scratch_buffer.capacity(),
            );
            if usize::try_from(written).unwrap() != size {
                // This will likely never happen
                return None;
            }
            self.scratch_buffer.set_len(size);
        };
        byte_slice_to_smol_str(&self.scratch_buffer)
    }

    pub fn new() -> Result<Self, Error> {
        if ffi::xkbcommon_option().is_none() {
            return Err(Error::XKBNotFound);
        }

        let context =
            unsafe { (XKBH.xkb_context_new)(ffi::xkb_context_flags::XKB_CONTEXT_NO_FLAGS) };
        if context.is_null() {
            return Err(Error::XKBNotFound);
        }

        let mut me = Self {
            #[cfg(feature = "x11")]
            xcb_connection: ptr::null_mut(),
            xkb_context: context,
            xkb_keymap: ptr::null_mut(),
            xkb_state: ptr::null_mut(),
            xkb_compose_table: ptr::null_mut(),
            xkb_compose_state: ptr::null_mut(),
            xkb_compose_state_2: ptr::null_mut(),
            mods_state: ModifiersState::new(),
            #[cfg(feature = "x11")]
            core_keyboard_id: 0,
            scratch_buffer: Vec::new(),
        };

        unsafe { me.init_compose() };

        Ok(me)
    }

    #[cfg(feature = "x11")]
    pub fn from_x11_xkb(connection: *mut xcb_connection_t) -> Result<Self, Error> {
        let mut me = Self::new()?;
        me.xcb_connection = connection;

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

        unsafe { me.init_with_x11_keymap() };

        Ok(me)
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

        let compose_table = unsafe {
            (XKBCH.xkb_compose_table_new_from_locale)(
                self.xkb_context,
                locale.as_ptr(),
                ffi::xkb_compose_compile_flags::XKB_COMPOSE_COMPILE_NO_FLAGS,
            )
        };

        if compose_table.is_null() {
            // init of compose table failed, continue without compose
            return;
        }

        let compose_state = unsafe {
            (XKBCH.xkb_compose_state_new)(
                compose_table,
                ffi::xkb_compose_state_flags::XKB_COMPOSE_STATE_NO_FLAGS,
            )
        };

        if compose_state.is_null() {
            // init of compose state failed, continue without compose
            unsafe { (XKBCH.xkb_compose_table_unref)(compose_table) };
            return;
        }

        let compose_state_2 = unsafe {
            (XKBCH.xkb_compose_state_new)(
                compose_table,
                ffi::xkb_compose_state_flags::XKB_COMPOSE_STATE_NO_FLAGS,
            )
        };

        if compose_state_2.is_null() {
            // init of compose state failed, continue without compose
            unsafe { (XKBCH.xkb_compose_table_unref)(compose_table) };
            unsafe { (XKBCH.xkb_compose_state_unref)(compose_state) };
            return;
        }

        self.xkb_compose_table = compose_table;
        self.xkb_compose_state = compose_state;
        self.xkb_compose_state_2 = compose_state_2;
    }

    unsafe fn post_init(&mut self, state: *mut ffi::xkb_state, keymap: *mut ffi::xkb_keymap) {
        self.xkb_keymap = keymap;
        self.xkb_state = state;
        self.mods_state.update_with(state);
    }

    unsafe fn de_init(&mut self) {
        unsafe { (XKBH.xkb_state_unref)(self.xkb_state) };
        self.xkb_state = ptr::null_mut();
        unsafe { (XKBH.xkb_keymap_unref)(self.xkb_keymap) };
        self.xkb_keymap = ptr::null_mut();
    }

    #[cfg(feature = "x11")]
    pub unsafe fn init_with_x11_keymap(&mut self) {
        if !self.xkb_keymap.is_null() {
            unsafe { self.de_init() };
        }

        // TODO: Support keyboards other than the "virtual core keyboard device".
        self.core_keyboard_id =
            unsafe { (XKBXH.xkb_x11_get_core_keyboard_device_id)(self.xcb_connection) };
        let keymap = unsafe {
            (XKBXH.xkb_x11_keymap_new_from_device)(
                self.xkb_context,
                self.xcb_connection,
                self.core_keyboard_id,
                xkbcommon_dl::xkb_keymap_compile_flags::XKB_KEYMAP_COMPILE_NO_FLAGS,
            )
        };
        if keymap.is_null() {
            panic!("Failed to get keymap from X11 server.");
        }

        let state = unsafe {
            (XKBXH.xkb_x11_state_new_from_device)(
                keymap,
                self.xcb_connection,
                self.core_keyboard_id,
            )
        };
        unsafe { self.post_init(state, keymap) };
    }

    #[cfg(feature = "wayland")]
    pub unsafe fn init_with_fd(&mut self, fd: OwnedFd, size: usize) {
        if !self.xkb_keymap.is_null() {
            unsafe { self.de_init() };
        }

        let map = unsafe {
            MmapOptions::new()
                .len(size)
                .map_copy_read_only(&fd)
                .unwrap()
        };

        let keymap = unsafe {
            (XKBH.xkb_keymap_new_from_string)(
                self.xkb_context,
                map.as_ptr() as *const _,
                ffi::xkb_keymap_format::XKB_KEYMAP_FORMAT_TEXT_V1,
                ffi::xkb_keymap_compile_flags::XKB_KEYMAP_COMPILE_NO_FLAGS,
            )
        };

        if keymap.is_null() {
            panic!("Received invalid keymap from compositor.");
        }

        let state = unsafe { (XKBH.xkb_state_new)(keymap) };
        unsafe { self.post_init(state, keymap) };
    }

    pub fn key_repeats(&mut self, keycode: ffi::xkb_keycode_t) -> bool {
        unsafe { (XKBH.xkb_keymap_key_repeats)(self.xkb_keymap, keycode) == 1 }
    }

    #[inline]
    pub fn ready(&self) -> bool {
        !self.xkb_state.is_null()
    }

    #[inline]
    pub fn mods_state(&self) -> ModifiersState {
        self.mods_state
    }

    pub fn process_key_event(
        &mut self,
        keycode: u32,
        state: ElementState,
        repeat: bool,
    ) -> KeyEvent {
        let mut event =
            KeyEventResults::new(self, keycode, !repeat && state == ElementState::Pressed);
        let physical_key = event.physical_key();
        let (logical_key, location) = event.key();
        let text = event.text();
        let (key_without_modifiers, _) = event.key_without_modifiers();
        let text_with_all_modifiers = event.text_with_all_modifiers();

        let platform_specific = KeyEventExtra {
            key_without_modifiers,
            text_with_all_modifiers,
        };

        KeyEvent {
            physical_key,
            logical_key,
            text,
            location,
            state,
            repeat,
            platform_specific,
        }
    }

    fn keysym_to_utf8_raw(&mut self, keysym: u32) -> Option<SmolStr> {
        self.scratch_buffer.clear();
        self.scratch_buffer.reserve(8);
        loop {
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

        // Remove the null-terminator
        self.scratch_buffer.pop();
        byte_slice_to_smol_str(&self.scratch_buffer)
    }
}

impl Drop for KbdState {
    fn drop(&mut self) {
        unsafe {
            if !self.xkb_compose_state.is_null() {
                (XKBCH.xkb_compose_state_unref)(self.xkb_compose_state);
            }
            if !self.xkb_compose_state_2.is_null() {
                (XKBCH.xkb_compose_state_unref)(self.xkb_compose_state_2);
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

struct KeyEventResults<'a> {
    state: &'a mut KbdState,
    keycode: u32,
    keysym: u32,
    compose: Option<XkbCompose>,
}

impl<'a> KeyEventResults<'a> {
    fn new(state: &'a mut KbdState, keycode: u32, compose: bool) -> Self {
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

    fn physical_key(&self) -> PhysicalKey {
        keymap::raw_keycode_to_physicalkey(self.keycode)
    }

    pub fn key(&mut self) -> (Key, KeyLocation) {
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
                                    .map(|s| s.chars().next().unwrap()),
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

    pub fn key_without_modifiers(&mut self) -> (Key, KeyLocation) {
        // This will become a pointer to an array which libxkbcommon owns, so we don't need to deallocate it.
        let mut keysyms = ptr::null();
        let keysym_count = unsafe {
            let layout = (XKBH.xkb_state_key_get_layout)(self.state.xkb_state, self.keycode);
            (XKBH.xkb_keymap_key_get_syms_by_level)(
                self.state.xkb_keymap,
                self.keycode,
                layout,
                // NOTE: The level should be zero to ignore modifiers.
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

    fn keysym_to_key(&self, keysym: u32) -> Result<(Key, KeyLocation), (Key, KeyLocation)> {
        let location = super::keymap::keysym_location(keysym);
        let key = super::keymap::keysym_to_key(keysym);
        if matches!(key, Key::Unidentified(_)) {
            Err((key, location))
        } else {
            Ok((key, location))
        }
    }

    pub fn text(&mut self) -> Option<SmolStr> {
        self.composed_text()
            .unwrap_or_else(|_| self.state.keysym_to_utf8_raw(self.keysym))
    }

    pub fn text_with_all_modifiers(&mut self) -> Option<SmolStr> {
        // The current behaviour makes it so composing a character overrides attempts to input a
        // control character with the `Ctrl` key. We can potentially add a configuration option
        // if someone specifically wants the oppsite behaviour.
        self.composed_text()
            .unwrap_or_else(|_| self.state.get_utf8_raw(self.keycode))
    }

    fn composed_text(&mut self) -> Result<Option<SmolStr>, ()> {
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

/// Represents the current state of the keyboard modifiers
///
/// Each field of this struct represents a modifier and is `true` if this modifier is active.
///
/// For some modifiers, this means that the key is currently pressed, others are toggled
/// (like caps lock).
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
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
    fn new() -> Self {
        Self::default()
    }

    fn update_with(&mut self, state: *mut ffi::xkb_state) {
        let mod_name_is_active = |mod_name: &[u8]| unsafe {
            (XKBH.xkb_state_mod_name_is_active)(
                state,
                mod_name.as_ptr() as *const c_char,
                xkb_state_component::XKB_STATE_MODS_EFFECTIVE,
            ) > 0
        };
        self.ctrl = mod_name_is_active(ffi::XKB_MOD_NAME_CTRL);
        self.alt = mod_name_is_active(ffi::XKB_MOD_NAME_ALT);
        self.shift = mod_name_is_active(ffi::XKB_MOD_NAME_SHIFT);
        self.caps_lock = mod_name_is_active(ffi::XKB_MOD_NAME_CAPS);
        self.logo = mod_name_is_active(ffi::XKB_MOD_NAME_LOGO);
        self.num_lock = mod_name_is_active(ffi::XKB_MOD_NAME_NUM);
    }
}

impl From<ModifiersState> for crate::keyboard::ModifiersState {
    fn from(mods: ModifiersState) -> crate::keyboard::ModifiersState {
        let mut to_mods = crate::keyboard::ModifiersState::empty();
        to_mods.set(crate::keyboard::ModifiersState::SHIFT, mods.shift);
        to_mods.set(crate::keyboard::ModifiersState::CONTROL, mods.ctrl);
        to_mods.set(crate::keyboard::ModifiersState::ALT, mods.alt);
        to_mods.set(crate::keyboard::ModifiersState::SUPER, mods.logo);
        to_mods
    }
}

#[derive(Debug)]
pub enum Error {
    /// libxkbcommon is not available
    XKBNotFound,
}

#[derive(Copy, Clone, Debug)]
enum XkbCompose {
    Accepted(ffi::xkb_compose_status),
    Ignored,
    Uninitialized,
}

// Note: This is track_caller so we can have more informative line numbers when logging
#[track_caller]
fn byte_slice_to_smol_str(bytes: &[u8]) -> Option<SmolStr> {
    std::str::from_utf8(bytes)
        .map(SmolStr::new)
        .map_err(|e| {
            warn!(
                "UTF-8 received from libxkbcommon ({:?}) was invalid: {e}",
                bytes
            )
        })
        .ok()
}
