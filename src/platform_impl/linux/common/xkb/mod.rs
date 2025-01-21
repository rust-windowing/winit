#[cfg(wayland_platform)]
use std::os::unix::io::OwnedFd;
use std::sync::atomic::{AtomicBool, Ordering};

use kbvm::xkb::compose;
use kbvm::xkb::compose::{ComposeTable, FeedResult};
use kbvm::xkb::diagnostic::WriteToLog;
use kbvm::{xkb, Keycode, Keysym};
use smol_str::SmolStr;
use tracing::warn;
#[cfg(x11_platform)]
use {kbvm::xkb::x11::KbvmX11Ext, x11rb::xcb_ffi::XCBConnection};

use crate::event::{ElementState, KeyEvent};
use crate::keyboard::{Key, KeyLocation};
use crate::platform_impl::KeyEventExtra;

mod keymap;
mod state;

use keymap::XkbKeymap;
pub use keymap::{keycode_to_physicalkey, physicalkey_to_scancode, scancode_to_physicalkey};
pub use state::XkbState;

// TODO: Wire this up without using a static `AtomicBool`.
static RESET_DEAD_KEYS: AtomicBool = AtomicBool::new(false);

#[inline(always)]
pub fn reset_dead_keys() {
    RESET_DEAD_KEYS.store(true, Ordering::SeqCst);
}

#[cfg(x11_platform)]
#[derive(Debug)]
pub enum Error {
    /// Could not initialize XKB
    InitializeXkb,
}

#[derive(Debug)]
struct ComposeContext {
    table: ComposeTable,
    state: compose::State,
}

#[derive(Debug)]
pub struct Context {
    // NOTE: field order matters.
    #[cfg(x11_platform)]
    pub core_keyboard_id: u16,
    state: Option<XkbState>,
    keymap: Option<XkbKeymap>,
    compose: Option<ComposeContext>,
    #[cfg(wayland_platform)]
    context: xkb::Context,
    scratch_buffer: String,
}

impl Context {
    pub fn new() -> Self {
        let context = xkb::Context::default();
        let compose = context
            .compose_table_builder()
            .build(WriteToLog)
            .map(|table| ComposeContext { state: table.create_state(), table });

        Self {
            state: None,
            keymap: None,
            #[cfg(x11_platform)]
            core_keyboard_id: 0,
            compose,
            #[cfg(wayland_platform)]
            context,
            scratch_buffer: String::with_capacity(8),
        }
    }

    #[cfg(feature = "x11")]
    pub fn from_x11_xkb(xcb: &XCBConnection) -> Result<Self, Error> {
        xcb.setup_xkb_extension().map_err(|_| Error::InitializeXkb)?;

        let mut this = Self::new();
        this.core_keyboard_id = xcb.get_xkb_core_device_id().map_err(|_| Error::InitializeXkb)?;
        this.set_keymap_from_x11(xcb);
        Ok(this)
    }

    pub fn state_mut(&mut self) -> Option<&mut XkbState> {
        self.state.as_mut()
    }

    pub fn keymap_mut(&mut self) -> Option<&mut XkbKeymap> {
        self.keymap.as_mut()
    }

    #[cfg(wayland_platform)]
    pub fn set_keymap_from_fd(&mut self, fd: OwnedFd, size: usize) {
        let keymap = XkbKeymap::from_fd(&self.context, fd, size);
        let state = keymap.as_ref().map(XkbState::new_wayland);
        if keymap.is_none() || state.is_none() {
            warn!("failed to update xkb keymap");
        }
        self.state = state;
        self.keymap = keymap;
    }

    #[cfg(x11_platform)]
    pub fn set_keymap_from_x11(&mut self, xcb: &XCBConnection) {
        let keymap = XkbKeymap::from_x11_keymap(xcb, self.core_keyboard_id);
        let state = keymap.as_ref().and_then(|keymap| XkbState::new_x11(xcb, keymap));
        if keymap.is_none() || state.is_none() {
            warn!("failed to update xkb keymap");
        }
        self.state = state;
        self.keymap = keymap;
    }

    /// Key builder context with the user provided xkb state.
    pub fn key_context(&mut self) -> Option<KeyContext<'_, '_>> {
        let state = self.state.as_mut()?;
        let keymap = self.keymap.as_mut()?;
        let compose = self
            .compose
            .as_mut()
            .map(|c| KeyComposeContext { table: &c.table, state: &mut c.state });
        let scratch_buffer = &mut self.scratch_buffer;
        Some(KeyContext { state, keymap, compose, scratch_buffer })
    }

    /// Key builder context with the user provided xkb state.
    ///
    /// Should be used when the original context must not be altered.
    #[cfg(x11_platform)]
    pub fn key_context_with_state<'a>(
        &'a mut self,
        state: &'a mut XkbState,
    ) -> Option<KeyContext<'a, 'a>> {
        let keymap = self.keymap.as_mut()?;
        let compose = self
            .compose
            .as_mut()
            .map(|c| KeyComposeContext { table: &c.table, state: &mut c.state });
        let scratch_buffer = &mut self.scratch_buffer;
        Some(KeyContext { state, keymap, compose, scratch_buffer })
    }
}

struct KeyComposeContext<'a, 'b> {
    table: &'b ComposeTable,
    state: &'a mut compose::State,
}

pub struct KeyContext<'a, 'b> {
    pub state: &'a mut XkbState,
    pub keymap: &'a mut XkbKeymap,
    compose: Option<KeyComposeContext<'a, 'b>>,
    scratch_buffer: &'a mut String,
}

impl KeyContext<'_, '_> {
    pub fn process_key_event(
        &mut self,
        keycode: Keycode,
        state: ElementState,
        repeat: bool,
    ) -> KeyEvent {
        let mut event =
            KeyEventResults::new(self, keycode, !repeat && state == ElementState::Pressed);
        let physical_key = keycode_to_physicalkey(keycode);
        let (logical_key, location) = event.key();
        let text = event.text();
        let (key_without_modifiers, _) = event.key_without_modifiers();
        let text_with_all_modifiers = event.text_with_all_modifiers();

        let platform_specific = KeyEventExtra { text_with_all_modifiers, key_without_modifiers };

        KeyEvent { physical_key, logical_key, text, location, state, repeat, platform_specific }
    }

    fn keysym_to_utf8_raw(&mut self, keysym: Keysym) -> Option<SmolStr> {
        let c = keysym.char()?;
        Some(char_to_smol_str(c))
    }
}

struct KeyEventResults<'a, 'b, 'c> {
    context: &'a mut KeyContext<'b, 'c>,
    keycode: Keycode,
    keysym: Keysym,
    feed_result: Option<FeedResult<'c>>,
}

impl<'a, 'b, 'c> KeyEventResults<'a, 'b, 'c> {
    fn new(context: &'a mut KeyContext<'b, 'c>, keycode: Keycode, compose: bool) -> Self {
        let keysym = context.state.get_one_sym_raw(keycode);

        let feed_result = if let Some(state) = context.compose.as_mut().filter(|_| compose) {
            if RESET_DEAD_KEYS.swap(false, Ordering::SeqCst) {
                *state.state = state.table.create_state();
            }
            state.table.feed(state.state, keysym)
        } else {
            None
        };

        KeyEventResults { context, keycode, keysym, feed_result }
    }

    pub fn key(&mut self) -> (Key, KeyLocation) {
        let (key, location) = match self.keysym_to_key(self.keysym) {
            Ok(known) => return known,
            Err(undefined) => undefined,
        };

        if let Some(FeedResult::Pending) = &self.feed_result {
            // When pressing a dead key twice, the non-combining variant of that character will
            // be produced. Since this function only concerns itself with a single keypress, we
            // simulate this double press here by feeding the keysym to the compose state
            // twice.

            let compose = self.context.compose.as_ref().unwrap();
            let mut state = compose.state.clone();
            let res = compose.table.feed(&mut state, self.keysym);
            // Extracting only a single `char` here *should* be fine, assuming that no
            // dead key's non-combining variant ever occupies more than one `char`.
            let text = composed_text(res.as_ref()).ok().flatten();
            let key = Key::Dead(text.and_then(|s| s.chars().next()));
            (key, location)
        } else {
            let key = self
                .composed_text()
                .unwrap_or_else(|_| self.context.keysym_to_utf8_raw(self.keysym))
                .map(Key::Character)
                .unwrap_or(key);
            (key, location)
        }
    }

    pub fn key_without_modifiers(&mut self) -> (Key, KeyLocation) {
        // This will become a pointer to an array which libxkbcommon owns, so we don't need to
        // deallocate it.
        let layout = self.context.state.layout(self.keycode);
        let keysym = self.context.keymap.first_keysym_by_level(layout, self.keycode);

        match self.keysym_to_key(keysym) {
            Ok((key, location)) => (key, location),
            Err((key, location)) => {
                let key =
                    self.context.keysym_to_utf8_raw(keysym).map(Key::Character).unwrap_or(key);
                (key, location)
            },
        }
    }

    fn keysym_to_key(&self, keysym: Keysym) -> Result<(Key, KeyLocation), (Key, KeyLocation)> {
        let location = keymap::keysym_location(keysym);
        let key = keymap::keysym_to_key(keysym);
        if matches!(key, Key::Unidentified(_)) {
            Err((key, location))
        } else {
            Ok((key, location))
        }
    }

    pub fn text(&mut self) -> Option<SmolStr> {
        self.composed_text().unwrap_or_else(|_| self.context.keysym_to_utf8_raw(self.keysym))
    }

    // The current behaviour makes it so composing a character overrides attempts to input a
    // control character with the `Ctrl` key. We can potentially add a configuration option
    // if someone specifically wants the oppsite behaviour.
    pub fn text_with_all_modifiers(&mut self) -> Option<SmolStr> {
        match self.composed_text() {
            Ok(text) => text,
            Err(_) => self.context.state.get_utf8_raw(self.keycode, self.context.scratch_buffer),
        }
    }

    fn composed_text(&mut self) -> Result<Option<SmolStr>, ()> {
        composed_text(self.feed_result.as_ref())
    }
}

fn char_to_smol_str(c: char) -> SmolStr {
    let mut buf = [0; 4];
    SmolStr::new(c.encode_utf8(&mut buf))
}

fn composed_text(feed_result: Option<&FeedResult<'_>>) -> Result<Option<SmolStr>, ()> {
    let Some(feed_result) = feed_result else {
        return Err(());
    };
    let FeedResult::Composed { string, keysym } = feed_result else {
        return Ok(None);
    };
    if let Some(s) = string {
        return Ok(Some(SmolStr::new(s)));
    }
    if let Some(keysym) = keysym {
        if let Some(c) = keysym.char() {
            return Ok(Some(char_to_smol_str(c)));
        }
    }
    Ok(Some(SmolStr::default()))
}
