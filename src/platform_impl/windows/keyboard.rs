use std::{
    char,
    ffi::OsString,
    mem::MaybeUninit,
    os::windows::ffi::OsStringExt,
    sync::{
        atomic::{AtomicU32, Ordering::Relaxed},
        Mutex, MutexGuard,
    },
};

use windows_sys::Win32::{
    Foundation::{HWND, LPARAM, WPARAM},
    System::SystemServices::LANG_KOREAN,
    UI::{
        Input::KeyboardAndMouse::{
            GetAsyncKeyState, GetKeyState, GetKeyboardLayout, GetKeyboardState, MapVirtualKeyExW,
            MAPVK_VK_TO_VSC_EX, MAPVK_VSC_TO_VK_EX, VIRTUAL_KEY, VK_ABNT_C2, VK_ADD, VK_CAPITAL,
            VK_CLEAR, VK_CONTROL, VK_DECIMAL, VK_DELETE, VK_DIVIDE, VK_DOWN, VK_END, VK_F4,
            VK_HOME, VK_INSERT, VK_LCONTROL, VK_LEFT, VK_LMENU, VK_LSHIFT, VK_LWIN, VK_MENU,
            VK_MULTIPLY, VK_NEXT, VK_NUMLOCK, VK_NUMPAD0, VK_NUMPAD1, VK_NUMPAD2, VK_NUMPAD3,
            VK_NUMPAD4, VK_NUMPAD5, VK_NUMPAD6, VK_NUMPAD7, VK_NUMPAD8, VK_NUMPAD9, VK_PRIOR,
            VK_RCONTROL, VK_RETURN, VK_RIGHT, VK_RMENU, VK_RSHIFT, VK_RWIN, VK_SCROLL, VK_SHIFT,
            VK_SUBTRACT, VK_UP,
        },
        TextServices::HKL,
        WindowsAndMessaging::{
            PeekMessageW, MSG, PM_NOREMOVE, WM_CHAR, WM_DEADCHAR, WM_KEYDOWN, WM_KEYFIRST,
            WM_KEYLAST, WM_KEYUP, WM_KILLFOCUS, WM_SETFOCUS, WM_SYSCHAR, WM_SYSDEADCHAR,
            WM_SYSKEYDOWN, WM_SYSKEYUP,
        },
    },
};

use smol_str::SmolStr;
use unicode_segmentation::UnicodeSegmentation;

use crate::{
    event::{ElementState, KeyEvent},
    keyboard::{Key, KeyCode, KeyLocation, NamedKey, NativeKey, NativeKeyCode, PhysicalKey},
    platform::scancode::PhysicalKeyExtScancode,
    platform_impl::platform::{
        event_loop::ProcResult,
        keyboard_layout::{Layout, LayoutCache, WindowsModifiers, LAYOUT_CACHE},
        loword, primarylangid, KeyEventExtra,
    },
};

pub type ExScancode = u16;

pub struct MessageAsKeyEvent {
    pub event: KeyEvent,
    pub is_synthetic: bool,
}

/// Stores information required to make `KeyEvent`s.
///
/// A single Winit `KeyEvent` contains information which the Windows API passes to the application
/// in multiple window messages. In other words: a Winit `KeyEvent` cannot be built from a single
/// window message. Therefore, this type keeps track of certain information from previous events so
/// that a `KeyEvent` can be constructed when the last event related to a keypress is received.
///
/// `PeekMessage` is sometimes used to determine whether the next window message still belongs to the
/// current keypress. If it doesn't and the current state represents a key event waiting to be
/// dispatched, then said event is considered complete and is dispatched.
///
/// The sequence of window messages for a key press event is the following:
/// - Exactly one WM_KEYDOWN / WM_SYSKEYDOWN
/// - Zero or one WM_DEADCHAR / WM_SYSDEADCHAR
/// - Zero or more WM_CHAR / WM_SYSCHAR. These messages each come with a UTF-16 code unit which when
///   put together in the sequence they arrived in, forms the text which is the result of pressing the
///   key.
///
/// Key release messages are a bit different due to the fact that they don't contribute to
/// text input. The "sequence" only consists of one WM_KEYUP / WM_SYSKEYUP event.
pub struct KeyEventBuilder {
    event_info: Mutex<Option<PartialKeyEventInfo>>,
    pending: PendingEventQueue<MessageAsKeyEvent>,
}
impl Default for KeyEventBuilder {
    fn default() -> Self {
        KeyEventBuilder {
            event_info: Mutex::new(None),
            pending: Default::default(),
        }
    }
}
impl KeyEventBuilder {
    /// Call this function for every window message.
    /// Returns Some() if this window message completes a KeyEvent.
    /// Returns None otherwise.
    pub(crate) fn process_message(
        &self,
        hwnd: HWND,
        msg_kind: u32,
        wparam: WPARAM,
        lparam: LPARAM,
        result: &mut ProcResult,
    ) -> Vec<MessageAsKeyEvent> {
        enum MatchResult {
            Nothing,
            TokenToRemove(PendingMessageToken),
            MessagesToDispatch(Vec<MessageAsKeyEvent>),
        }

        let mut matcher = || -> MatchResult {
            match msg_kind {
                WM_SETFOCUS => {
                    // synthesize keydown events
                    let kbd_state = get_async_kbd_state();
                    let key_events = Self::synthesize_kbd_state(ElementState::Pressed, &kbd_state);
                    MatchResult::MessagesToDispatch(self.pending.complete_multi(key_events))
                }
                WM_KILLFOCUS => {
                    // sythesize keyup events
                    let kbd_state = get_kbd_state();
                    let key_events = Self::synthesize_kbd_state(ElementState::Released, &kbd_state);
                    MatchResult::MessagesToDispatch(self.pending.complete_multi(key_events))
                }
                WM_KEYDOWN | WM_SYSKEYDOWN => {
                    if msg_kind == WM_SYSKEYDOWN && wparam as VIRTUAL_KEY == VK_F4 {
                        // Don't dispatch Alt+F4 to the application.
                        // This is handled in `event_loop.rs`
                        return MatchResult::Nothing;
                    }
                    let pending_token = self.pending.add_pending();
                    *result = ProcResult::Value(0);

                    let next_msg = next_kbd_msg(hwnd);

                    let mut layouts = LAYOUT_CACHE.lock().unwrap();
                    let mut finished_event_info = Some(PartialKeyEventInfo::from_message(
                        wparam,
                        lparam,
                        ElementState::Pressed,
                        &mut layouts,
                    ));
                    let mut event_info = self.event_info.lock().unwrap();
                    *event_info = None;
                    if let Some(next_msg) = next_msg {
                        let next_msg_kind = next_msg.message;
                        let next_belongs_to_this = !matches!(
                            next_msg_kind,
                            WM_KEYDOWN | WM_SYSKEYDOWN | WM_KEYUP | WM_SYSKEYUP
                        );
                        if next_belongs_to_this {
                            // The next OS event belongs to this Winit event, so let's just
                            // store the partial information, and add to it in the upcoming events
                            *event_info = finished_event_info.take();
                        } else {
                            let (_, layout) = layouts.get_current_layout();
                            let is_fake = {
                                let curr_event = finished_event_info.as_ref().unwrap();
                                is_current_fake(curr_event, next_msg, layout)
                            };
                            if is_fake {
                                finished_event_info = None;
                            }
                        }
                    }
                    if let Some(event_info) = finished_event_info {
                        let ev = event_info.finalize();
                        return MatchResult::MessagesToDispatch(self.pending.complete_pending(
                            pending_token,
                            MessageAsKeyEvent {
                                event: ev,
                                is_synthetic: false,
                            },
                        ));
                    }
                    MatchResult::TokenToRemove(pending_token)
                }
                WM_DEADCHAR | WM_SYSDEADCHAR => {
                    let pending_token = self.pending.add_pending();
                    *result = ProcResult::Value(0);
                    // At this point, we know that there isn't going to be any more events related to
                    // this key press
                    let event_info = self.event_info.lock().unwrap().take().unwrap();
                    let ev = event_info.finalize();
                    MatchResult::MessagesToDispatch(self.pending.complete_pending(
                        pending_token,
                        MessageAsKeyEvent {
                            event: ev,
                            is_synthetic: false,
                        },
                    ))
                }
                WM_CHAR | WM_SYSCHAR => {
                    let mut event_info = self.event_info.lock().unwrap();
                    if event_info.is_none() {
                        trace!("Received a CHAR message but no `event_info` was available. The message is probably IME, returning.");
                        return MatchResult::Nothing;
                    }
                    let pending_token = self.pending.add_pending();
                    *result = ProcResult::Value(0);
                    let is_high_surrogate = (0xD800..=0xDBFF).contains(&wparam);
                    let is_low_surrogate = (0xDC00..=0xDFFF).contains(&wparam);

                    let is_utf16 = is_high_surrogate || is_low_surrogate;

                    if is_utf16 {
                        if let Some(ev_info) = event_info.as_mut() {
                            ev_info.utf16parts.push(wparam as u16);
                        }
                    } else {
                        // In this case, wparam holds a UTF-32 character.
                        // Let's encode it as UTF-16 and append it to the end of `utf16parts`
                        let utf16parts = match event_info.as_mut() {
                            Some(ev_info) => &mut ev_info.utf16parts,
                            None => {
                                warn!("The event_info was None when it was expected to be some");
                                return MatchResult::TokenToRemove(pending_token);
                            }
                        };
                        let start_offset = utf16parts.len();
                        let new_size = utf16parts.len() + 2;
                        utf16parts.resize(new_size, 0);
                        if let Some(ch) = char::from_u32(wparam as u32) {
                            let encode_len = ch.encode_utf16(&mut utf16parts[start_offset..]).len();
                            let new_size = start_offset + encode_len;
                            utf16parts.resize(new_size, 0);
                        }
                    }
                    // It's important that we unlock the mutex, and create the pending event token before
                    // calling `next_msg`
                    std::mem::drop(event_info);
                    let next_msg = next_kbd_msg(hwnd);
                    let more_char_coming = next_msg
                        .map(|m| matches!(m.message, WM_CHAR | WM_SYSCHAR))
                        .unwrap_or(false);
                    if more_char_coming {
                        // No need to produce an event just yet, because there are still more characters that
                        // need to appended to this keyobard event
                        MatchResult::TokenToRemove(pending_token)
                    } else {
                        let mut event_info = self.event_info.lock().unwrap();
                        let mut event_info = match event_info.take() {
                            Some(ev_info) => ev_info,
                            None => {
                                warn!("The event_info was None when it was expected to be some");
                                return MatchResult::TokenToRemove(pending_token);
                            }
                        };
                        let mut layouts = LAYOUT_CACHE.lock().unwrap();
                        // It's okay to call `ToUnicode` here, because at this point the dead key
                        // is already consumed by the character.
                        let kbd_state = get_kbd_state();
                        let mod_state = WindowsModifiers::active_modifiers(&kbd_state);

                        let (_, layout) = layouts.get_current_layout();
                        let ctrl_on = if layout.has_alt_graph {
                            let alt_on = mod_state.contains(WindowsModifiers::ALT);
                            !alt_on && mod_state.contains(WindowsModifiers::CONTROL)
                        } else {
                            mod_state.contains(WindowsModifiers::CONTROL)
                        };

                        // If Ctrl is not pressed, just use the text with all
                        // modifiers because that already consumed the dead key. Otherwise,
                        // we would interpret the character incorrectly, missing the dead key.
                        if !ctrl_on {
                            event_info.text = PartialText::System(event_info.utf16parts.clone());
                        } else {
                            let mod_no_ctrl = mod_state.remove_only_ctrl();
                            let num_lock_on = kbd_state[VK_NUMLOCK as usize] & 1 != 0;
                            let vkey = event_info.vkey;
                            let physical_key = &event_info.physical_key;
                            let key = layout.get_key(mod_no_ctrl, num_lock_on, vkey, physical_key);
                            event_info.text = PartialText::Text(key.to_text().map(SmolStr::new));
                        }
                        let ev = event_info.finalize();
                        MatchResult::MessagesToDispatch(self.pending.complete_pending(
                            pending_token,
                            MessageAsKeyEvent {
                                event: ev,
                                is_synthetic: false,
                            },
                        ))
                    }
                }
                WM_KEYUP | WM_SYSKEYUP => {
                    let pending_token = self.pending.add_pending();
                    *result = ProcResult::Value(0);

                    let mut layouts = LAYOUT_CACHE.lock().unwrap();
                    let event_info = PartialKeyEventInfo::from_message(
                        wparam,
                        lparam,
                        ElementState::Released,
                        &mut layouts,
                    );
                    // We MUST release the layout lock before calling `next_kbd_msg`, otherwise it may deadlock
                    drop(layouts);
                    // It's important that we create the pending token before reading the next message.
                    let next_msg = next_kbd_msg(hwnd);
                    let mut valid_event_info = Some(event_info);
                    if let Some(next_msg) = next_msg {
                        let mut layouts = LAYOUT_CACHE.lock().unwrap();
                        let (_, layout) = layouts.get_current_layout();
                        let is_fake = {
                            let event_info = valid_event_info.as_ref().unwrap();
                            is_current_fake(event_info, next_msg, layout)
                        };
                        if is_fake {
                            valid_event_info = None;
                        }
                    }
                    if let Some(event_info) = valid_event_info {
                        let event = event_info.finalize();
                        return MatchResult::MessagesToDispatch(self.pending.complete_pending(
                            pending_token,
                            MessageAsKeyEvent {
                                event,
                                is_synthetic: false,
                            },
                        ));
                    }
                    MatchResult::TokenToRemove(pending_token)
                }
                _ => MatchResult::Nothing,
            }
        };
        let matcher_result = matcher();
        match matcher_result {
            MatchResult::TokenToRemove(t) => self.pending.remove_pending(t),
            MatchResult::MessagesToDispatch(m) => m,
            MatchResult::Nothing => Vec::new(),
        }
    }

    // Alowing nominimal_bool lint because the `is_key_pressed` macro triggers this warning
    // and I don't know of another way to resolve it and also keeping the macro
    #[allow(clippy::nonminimal_bool)]
    fn synthesize_kbd_state(
        key_state: ElementState,
        kbd_state: &[u8; 256],
    ) -> Vec<MessageAsKeyEvent> {
        let mut key_events = Vec::new();

        let mut layouts = LAYOUT_CACHE.lock().unwrap();
        let (locale_id, _) = layouts.get_current_layout();

        macro_rules! is_key_pressed {
            ($vk:expr) => {
                kbd_state[$vk as usize] & 0x80 != 0
            };
        }

        // Is caps-lock active? Note that this is different from caps-lock
        // being held down.
        let caps_lock_on = kbd_state[VK_CAPITAL as usize] & 1 != 0;
        let num_lock_on = kbd_state[VK_NUMLOCK as usize] & 1 != 0;

        // We are synthesizing the press event for caps-lock first for the following reasons:
        // 1. If caps-lock is *not* held down but *is* active, then we have to
        //    synthesize all printable keys, respecting the caps-lock state.
        // 2. If caps-lock is held down, we could choose to sythesize its
        //    keypress after every other key, in which case all other keys *must*
        //    be sythesized as if the caps-lock state was be the opposite
        //    of what it currently is.
        // --
        // For the sake of simplicity we are choosing to always sythesize
        // caps-lock first, and always use the current caps-lock state
        // to determine the produced text
        if is_key_pressed!(VK_CAPITAL) {
            let event = Self::create_synthetic(
                VK_CAPITAL,
                key_state,
                caps_lock_on,
                num_lock_on,
                locale_id as HKL,
                &mut layouts,
            );
            if let Some(event) = event {
                key_events.push(event);
            }
        }
        let do_non_modifier = |key_events: &mut Vec<_>, layouts: &mut _| {
            for vk in 0..256 {
                match vk {
                    VK_CONTROL | VK_LCONTROL | VK_RCONTROL | VK_SHIFT | VK_LSHIFT | VK_RSHIFT
                    | VK_MENU | VK_LMENU | VK_RMENU | VK_CAPITAL => continue,
                    _ => (),
                }
                if !is_key_pressed!(vk) {
                    continue;
                }
                let event = Self::create_synthetic(
                    vk,
                    key_state,
                    caps_lock_on,
                    num_lock_on,
                    locale_id as HKL,
                    layouts,
                );
                if let Some(event) = event {
                    key_events.push(event);
                }
            }
        };
        let do_modifier = |key_events: &mut Vec<_>, layouts: &mut _| {
            const CLEAR_MODIFIER_VKS: [VIRTUAL_KEY; 6] = [
                VK_LCONTROL,
                VK_LSHIFT,
                VK_LMENU,
                VK_RCONTROL,
                VK_RSHIFT,
                VK_RMENU,
            ];
            for vk in CLEAR_MODIFIER_VKS.iter() {
                if is_key_pressed!(*vk) {
                    let event = Self::create_synthetic(
                        *vk,
                        key_state,
                        caps_lock_on,
                        num_lock_on,
                        locale_id as HKL,
                        layouts,
                    );
                    if let Some(event) = event {
                        key_events.push(event);
                    }
                }
            }
        };

        // Be cheeky and sequence modifier and non-modifier
        // key events such that non-modifier keys are not affected
        // by modifiers (except for caps-lock)
        match key_state {
            ElementState::Pressed => {
                do_non_modifier(&mut key_events, &mut layouts);
                do_modifier(&mut key_events, &mut layouts);
            }
            ElementState::Released => {
                do_modifier(&mut key_events, &mut layouts);
                do_non_modifier(&mut key_events, &mut layouts);
            }
        }

        key_events
    }

    fn create_synthetic(
        vk: VIRTUAL_KEY,
        key_state: ElementState,
        caps_lock_on: bool,
        num_lock_on: bool,
        locale_id: HKL,
        layouts: &mut MutexGuard<'_, LayoutCache>,
    ) -> Option<MessageAsKeyEvent> {
        let scancode = unsafe { MapVirtualKeyExW(vk as u32, MAPVK_VK_TO_VSC_EX, locale_id) };
        if scancode == 0 {
            return None;
        }
        let scancode = scancode as ExScancode;
        let physical_key = PhysicalKey::from_scancode(scancode as u32);
        let mods = if caps_lock_on {
            WindowsModifiers::CAPS_LOCK
        } else {
            WindowsModifiers::empty()
        };
        let layout = layouts.layouts.get(&(locale_id as u64)).unwrap();
        let logical_key = layout.get_key(mods, num_lock_on, vk, &physical_key);
        let key_without_modifiers =
            layout.get_key(WindowsModifiers::empty(), false, vk, &physical_key);
        let text = if key_state == ElementState::Pressed {
            logical_key.to_text().map(SmolStr::new)
        } else {
            None
        };
        let event_info = PartialKeyEventInfo {
            vkey: vk,
            logical_key: PartialLogicalKey::This(logical_key.clone()),
            key_without_modifiers,
            key_state,
            is_repeat: false,
            physical_key,
            location: get_location(scancode, locale_id),
            utf16parts: Vec::with_capacity(8),
            text: PartialText::Text(text.clone()),
        };

        let mut event = event_info.finalize();
        event.logical_key = logical_key;
        event.platform_specific.text_with_all_modifers = text;
        Some(MessageAsKeyEvent {
            event,
            is_synthetic: true,
        })
    }
}

enum PartialText {
    // Unicode
    System(Vec<u16>),
    Text(Option<SmolStr>),
}

enum PartialLogicalKey {
    /// Use the text provided by the WM_CHAR messages and report that as a `Character` variant. If
    /// the text consists of multiple grapheme clusters (user-precieved characters) that means that
    /// dead key could not be combined with the second input, and in that case we should fall back
    /// to using what would have without a dead-key input.
    TextOr(Key),

    /// Use the value directly provided by this variant
    This(Key),
}

struct PartialKeyEventInfo {
    vkey: VIRTUAL_KEY,
    key_state: ElementState,
    is_repeat: bool,
    physical_key: PhysicalKey,
    location: KeyLocation,
    logical_key: PartialLogicalKey,

    key_without_modifiers: Key,

    /// The UTF-16 code units of the text that was produced by the keypress event.
    /// This take all modifiers into account. Including CTRL
    utf16parts: Vec<u16>,

    text: PartialText,
}

impl PartialKeyEventInfo {
    fn from_message(
        wparam: WPARAM,
        lparam: LPARAM,
        state: ElementState,
        layouts: &mut MutexGuard<'_, LayoutCache>,
    ) -> Self {
        const NO_MODS: WindowsModifiers = WindowsModifiers::empty();

        let (_, layout) = layouts.get_current_layout();
        let lparam_struct = destructure_key_lparam(lparam);
        let vkey = wparam as VIRTUAL_KEY;
        let scancode = if lparam_struct.scancode == 0 {
            // In some cases (often with media keys) the device reports a scancode of 0 but a
            // valid virtual key. In these cases we obtain the scancode from the virtual key.
            unsafe { MapVirtualKeyExW(vkey as u32, MAPVK_VK_TO_VSC_EX, layout.hkl as HKL) as u16 }
        } else {
            new_ex_scancode(lparam_struct.scancode, lparam_struct.extended)
        };
        let physical_key = PhysicalKey::from_scancode(scancode as u32);
        let location = get_location(scancode, layout.hkl as HKL);

        let kbd_state = get_kbd_state();
        let mods = WindowsModifiers::active_modifiers(&kbd_state);
        let mods_without_ctrl = mods.remove_only_ctrl();
        let num_lock_on = kbd_state[VK_NUMLOCK as usize] & 1 != 0;

        // On Windows Ctrl+NumLock = Pause (and apparently Ctrl+Pause -> NumLock). In these cases
        // the KeyCode still stores the real key, so in the name of consistency across platforms, we
        // circumvent this mapping and force the key values to match the keycode.
        // For more on this, read the article by Raymond Chen, titled:
        // "Why does Ctrl+ScrollLock cancel dialogs?"
        // https://devblogs.microsoft.com/oldnewthing/20080211-00/?p=23503
        let code_as_key = if mods.contains(WindowsModifiers::CONTROL) {
            match physical_key {
                PhysicalKey::Code(KeyCode::NumLock) => Some(Key::Named(NamedKey::NumLock)),
                PhysicalKey::Code(KeyCode::Pause) => Some(Key::Named(NamedKey::Pause)),
                _ => None,
            }
        } else {
            None
        };

        let preliminary_logical_key =
            layout.get_key(mods_without_ctrl, num_lock_on, vkey, &physical_key);
        let key_is_char = matches!(preliminary_logical_key, Key::Character(_));
        let is_pressed = state == ElementState::Pressed;

        let logical_key = if let Some(key) = code_as_key.clone() {
            PartialLogicalKey::This(key)
        } else if is_pressed && key_is_char && !mods.contains(WindowsModifiers::CONTROL) {
            // In some cases we want to use the UNICHAR text for logical_key in order to allow
            // dead keys to have an effect on the character reported by `logical_key`.
            PartialLogicalKey::TextOr(preliminary_logical_key)
        } else {
            PartialLogicalKey::This(preliminary_logical_key)
        };
        let key_without_modifiers = if let Some(key) = code_as_key {
            key
        } else {
            match layout.get_key(NO_MODS, false, vkey, &physical_key) {
                // We convert dead keys into their character.
                // The reason for this is that `key_without_modifiers` is designed for key-bindings,
                // but the US International layout treats `'` (apostrophe) as a dead key and the
                // reguar US layout treats it a character. In order for a single binding
                // configuration to work with both layouts, we forward each dead key as a character.
                Key::Dead(k) => {
                    if let Some(ch) = k {
                        // I'm avoiding the heap allocation. I don't want to talk about it :(
                        let mut utf8 = [0; 4];
                        let s = ch.encode_utf8(&mut utf8);
                        Key::Character(SmolStr::new(s))
                    } else {
                        Key::Unidentified(NativeKey::Unidentified)
                    }
                }
                key => key,
            }
        };

        PartialKeyEventInfo {
            vkey,
            key_state: state,
            logical_key,
            key_without_modifiers,
            is_repeat: lparam_struct.is_repeat,
            physical_key,
            location,
            utf16parts: Vec::with_capacity(8),
            text: PartialText::System(Vec::new()),
        }
    }

    fn finalize(self) -> KeyEvent {
        let mut char_with_all_modifiers = None;
        if !self.utf16parts.is_empty() {
            let os_string = OsString::from_wide(&self.utf16parts);
            if let Ok(string) = os_string.into_string() {
                char_with_all_modifiers = Some(SmolStr::new(string));
            }
        }

        // The text without Ctrl
        let mut text = None;
        match self.text {
            PartialText::System(wide) => {
                if !wide.is_empty() {
                    let os_string = OsString::from_wide(&wide);
                    if let Ok(string) = os_string.into_string() {
                        text = Some(SmolStr::new(string));
                    }
                }
            }
            PartialText::Text(s) => {
                text = s.map(SmolStr::new);
            }
        }

        let logical_key = match self.logical_key {
            PartialLogicalKey::TextOr(fallback) => match text.as_ref() {
                Some(s) => {
                    if s.grapheme_indices(true).count() > 1 {
                        fallback
                    } else {
                        Key::Character(s.clone())
                    }
                }
                None => Key::Unidentified(NativeKey::Windows(self.vkey)),
            },
            PartialLogicalKey::This(v) => v,
        };

        KeyEvent {
            physical_key: self.physical_key,
            logical_key,
            text,
            location: self.location,
            state: self.key_state,
            repeat: self.is_repeat,
            platform_specific: KeyEventExtra {
                text_with_all_modifers: char_with_all_modifiers,
                key_without_modifiers: self.key_without_modifiers,
            },
        }
    }
}

#[derive(Debug, Copy, Clone)]
struct KeyLParam {
    pub scancode: u8,
    pub extended: bool,

    /// This is `previous_state XOR transition_state`. See the lParam for WM_KEYDOWN and WM_KEYUP for further details.
    pub is_repeat: bool,
}

fn destructure_key_lparam(lparam: LPARAM) -> KeyLParam {
    let previous_state = (lparam >> 30) & 0x01;
    let transition_state = (lparam >> 31) & 0x01;
    KeyLParam {
        scancode: ((lparam >> 16) & 0xFF) as u8,
        extended: ((lparam >> 24) & 0x01) != 0,
        is_repeat: (previous_state ^ transition_state) != 0,
    }
}

#[inline]
fn new_ex_scancode(scancode: u8, extended: bool) -> ExScancode {
    (scancode as u16) | (if extended { 0xE000 } else { 0 })
}

#[inline]
fn ex_scancode_from_lparam(lparam: LPARAM) -> ExScancode {
    let lparam = destructure_key_lparam(lparam);
    new_ex_scancode(lparam.scancode, lparam.extended)
}

/// Gets the keyboard state as reported by messages that have been removed from the event queue.
/// See also: get_async_kbd_state
fn get_kbd_state() -> [u8; 256] {
    unsafe {
        let mut kbd_state: MaybeUninit<[u8; 256]> = MaybeUninit::uninit();
        GetKeyboardState(kbd_state.as_mut_ptr() as *mut u8);
        kbd_state.assume_init()
    }
}

/// Gets the current keyboard state regardless of whether the corresponding keyboard events have
/// been removed from the event queue. See also: get_kbd_state
#[allow(clippy::uninit_assumed_init)]
fn get_async_kbd_state() -> [u8; 256] {
    unsafe {
        let mut kbd_state: [u8; 256] = [0; 256];
        for (vk, state) in kbd_state.iter_mut().enumerate() {
            let vk = vk as VIRTUAL_KEY;
            let async_state = GetAsyncKeyState(vk as i32);
            let is_down = (async_state & (1 << 15)) != 0;
            *state = if is_down { 0x80 } else { 0 };

            if matches!(vk, VK_CAPITAL | VK_NUMLOCK | VK_SCROLL) {
                // Toggle states aren't reported by `GetAsyncKeyState`
                let toggle_state = GetKeyState(vk as i32);
                let is_active = (toggle_state & 1) != 0;
                *state |= u8::from(is_active);
            }
        }
        kbd_state
    }
}

/// On windows, AltGr == Ctrl + Alt
///
/// Due to this equivalence, the system generates a fake Ctrl key-press (and key-release) preceeding
/// every AltGr key-press (and key-release). We check if the current event is a Ctrl event and if
/// the next event is a right Alt (AltGr) event. If this is the case, the current event must be the
/// fake Ctrl event.
fn is_current_fake(curr_info: &PartialKeyEventInfo, next_msg: MSG, layout: &Layout) -> bool {
    let curr_is_ctrl = matches!(
        curr_info.logical_key,
        PartialLogicalKey::This(Key::Named(NamedKey::Control))
    );
    if layout.has_alt_graph {
        let next_code = ex_scancode_from_lparam(next_msg.lParam);
        let next_is_altgr = next_code == 0xE038; // 0xE038 is right alt
        if curr_is_ctrl && next_is_altgr {
            return true;
        }
    }
    false
}

enum PendingMessage<T> {
    Incomplete,
    Complete(T),
}
struct IdentifiedPendingMessage<T> {
    token: PendingMessageToken,
    msg: PendingMessage<T>,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PendingMessageToken(u32);

/// While processing keyboard events, we sometimes need
/// to call `PeekMessageW` (`next_msg`). But `PeekMessageW`
/// can also call the event handler, which means that the new event
/// gets processed before finishing to process the one that came before.
///
/// This would mean that the application receives events in the wrong order.
/// To avoid this, we keep track whether we are in the middle of processing
/// an event. Such an event is an "incomplete pending event". A
/// "complete pending event" is one that has already finished processing, but
/// hasn't been dispatched to the application because there still are incomplete
/// pending events that came before it.
///
/// When we finish processing an event, we call `complete_pending`,
/// which returns an empty array if there are incomplete pending events, but
/// if all pending events are complete, then it returns all pending events in
/// the order they were encountered. These can then be dispatched to the application
pub struct PendingEventQueue<T> {
    pending: Mutex<Vec<IdentifiedPendingMessage<T>>>,
    next_id: AtomicU32,
}
impl<T> PendingEventQueue<T> {
    /// Add a new pending event to the "pending queue"
    pub fn add_pending(&self) -> PendingMessageToken {
        let token = self.next_token();
        let mut pending = self.pending.lock().unwrap();
        pending.push(IdentifiedPendingMessage {
            token,
            msg: PendingMessage::Incomplete,
        });
        token
    }

    /// Returns all finished pending events
    ///
    /// If the return value is non empty, it's guaranteed to contain `msg`
    ///
    /// See also: `add_pending`
    pub fn complete_pending(&self, token: PendingMessageToken, msg: T) -> Vec<T> {
        let mut pending = self.pending.lock().unwrap();
        let mut target_is_first = false;
        for (i, pending_msg) in pending.iter_mut().enumerate() {
            if pending_msg.token == token {
                pending_msg.msg = PendingMessage::Complete(msg);
                if i == 0 {
                    target_is_first = true;
                }
                break;
            }
        }
        if target_is_first {
            // If the message that we just finished was the first one in the pending queue,
            // then we can empty the queue, and dispatch all of the messages.
            Self::drain_pending(&mut *pending)
        } else {
            Vec::new()
        }
    }

    pub fn complete_multi(&self, msgs: Vec<T>) -> Vec<T> {
        let mut pending = self.pending.lock().unwrap();
        if pending.is_empty() {
            return msgs;
        }
        pending.reserve(msgs.len());
        for msg in msgs {
            pending.push(IdentifiedPendingMessage {
                token: self.next_token(),
                msg: PendingMessage::Complete(msg),
            });
        }
        Vec::new()
    }

    /// Returns all finished pending events
    ///
    /// It's safe to call this even if the element isn't in the list anymore
    ///
    /// See also: `add_pending`
    pub fn remove_pending(&self, token: PendingMessageToken) -> Vec<T> {
        let mut pending = self.pending.lock().unwrap();
        let mut was_first = false;
        if let Some(m) = pending.first() {
            if m.token == token {
                was_first = true;
            }
        }
        pending.retain(|m| m.token != token);
        if was_first {
            Self::drain_pending(&mut *pending)
        } else {
            Vec::new()
        }
    }

    fn drain_pending(pending: &mut Vec<IdentifiedPendingMessage<T>>) -> Vec<T> {
        pending.drain(..).map(|m| {
            match m.msg {
                PendingMessage::Complete(msg) => msg,
                PendingMessage::Incomplete => {
                    panic!("Found an incomplete pending message when collecting messages. This indicates a bug in winit.")
                }
            }
        }).collect()
    }
    fn next_token(&self) -> PendingMessageToken {
        // It's okay for the u32 to overflow here. Yes, that could mean
        // that two different messages have the same token,
        // but that would only happen after having about 4 billion
        // messages sitting in the pending queue.
        //
        // In that case, having two identical tokens is the least of your concerns.
        let id = self.next_id.fetch_add(1, Relaxed);
        PendingMessageToken(id)
    }
}
impl<T> Default for PendingEventQueue<T> {
    fn default() -> Self {
        PendingEventQueue {
            pending: Mutex::new(Vec::new()),
            next_id: AtomicU32::new(0),
        }
    }
}

/// WARNING: Due to using PeekMessage, the event handler
/// function may get called during this function.
/// (Re-entrance to the event handler)
///
/// This can cause a deadlock if calling this function
/// while having a mutex locked.
///
/// It can also cause code to get executed in a surprising order.
pub fn next_kbd_msg(hwnd: HWND) -> Option<MSG> {
    unsafe {
        let mut next_msg = MaybeUninit::uninit();
        let peek_retval = PeekMessageW(
            next_msg.as_mut_ptr(),
            hwnd,
            WM_KEYFIRST,
            WM_KEYLAST,
            PM_NOREMOVE,
        );
        (peek_retval != 0).then(|| next_msg.assume_init())
    }
}

fn get_location(scancode: ExScancode, hkl: HKL) -> KeyLocation {
    const ABNT_C2: VIRTUAL_KEY = VK_ABNT_C2 as VIRTUAL_KEY;

    let extension = 0xE000;
    let extended = (scancode & extension) == extension;
    let vkey = unsafe { MapVirtualKeyExW(scancode as u32, MAPVK_VSC_TO_VK_EX, hkl) as VIRTUAL_KEY };

    // Use the native VKEY and the extended flag to cover most cases
    // This is taken from the `druid` GUI library, specifically
    // druid-shell/src/platform/windows/keyboard.rs
    match vkey {
        VK_LSHIFT | VK_LCONTROL | VK_LMENU | VK_LWIN => KeyLocation::Left,
        VK_RSHIFT | VK_RCONTROL | VK_RMENU | VK_RWIN => KeyLocation::Right,
        VK_RETURN if extended => KeyLocation::Numpad,
        VK_INSERT | VK_DELETE | VK_END | VK_DOWN | VK_NEXT | VK_LEFT | VK_CLEAR | VK_RIGHT
        | VK_HOME | VK_UP | VK_PRIOR => {
            if extended {
                KeyLocation::Standard
            } else {
                KeyLocation::Numpad
            }
        }
        VK_NUMPAD0 | VK_NUMPAD1 | VK_NUMPAD2 | VK_NUMPAD3 | VK_NUMPAD4 | VK_NUMPAD5
        | VK_NUMPAD6 | VK_NUMPAD7 | VK_NUMPAD8 | VK_NUMPAD9 | VK_DECIMAL | VK_DIVIDE
        | VK_MULTIPLY | VK_SUBTRACT | VK_ADD | ABNT_C2 => KeyLocation::Numpad,
        _ => KeyLocation::Standard,
    }
}

impl PhysicalKeyExtScancode for PhysicalKey {
    fn to_scancode(self) -> Option<u32> {
        // See `from_scancode` for more info

        let hkl = unsafe { GetKeyboardLayout(0) };

        let primary_lang_id = primarylangid(loword(hkl as u32));
        let is_korean = primary_lang_id as u32 == LANG_KOREAN;

        let code = match self {
            PhysicalKey::Code(code) => code,
            PhysicalKey::Unidentified(code) => {
                return match code {
                    NativeKeyCode::Windows(scancode) => Some(scancode as u32),
                    _ => None,
                };
            }
        };

        match code {
            KeyCode::Backquote => Some(0x0029),
            KeyCode::Backslash => Some(0x002B),
            KeyCode::Backspace => Some(0x000E),
            KeyCode::BracketLeft => Some(0x001A),
            KeyCode::BracketRight => Some(0x001B),
            KeyCode::Comma => Some(0x0033),
            KeyCode::Digit0 => Some(0x000B),
            KeyCode::Digit1 => Some(0x0002),
            KeyCode::Digit2 => Some(0x0003),
            KeyCode::Digit3 => Some(0x0004),
            KeyCode::Digit4 => Some(0x0005),
            KeyCode::Digit5 => Some(0x0006),
            KeyCode::Digit6 => Some(0x0007),
            KeyCode::Digit7 => Some(0x0008),
            KeyCode::Digit8 => Some(0x0009),
            KeyCode::Digit9 => Some(0x000A),
            KeyCode::Equal => Some(0x000D),
            KeyCode::IntlBackslash => Some(0x0056),
            KeyCode::IntlRo => Some(0x0073),
            KeyCode::IntlYen => Some(0x007D),
            KeyCode::KeyA => Some(0x001E),
            KeyCode::KeyB => Some(0x0030),
            KeyCode::KeyC => Some(0x002E),
            KeyCode::KeyD => Some(0x0020),
            KeyCode::KeyE => Some(0x0012),
            KeyCode::KeyF => Some(0x0021),
            KeyCode::KeyG => Some(0x0022),
            KeyCode::KeyH => Some(0x0023),
            KeyCode::KeyI => Some(0x0017),
            KeyCode::KeyJ => Some(0x0024),
            KeyCode::KeyK => Some(0x0025),
            KeyCode::KeyL => Some(0x0026),
            KeyCode::KeyM => Some(0x0032),
            KeyCode::KeyN => Some(0x0031),
            KeyCode::KeyO => Some(0x0018),
            KeyCode::KeyP => Some(0x0019),
            KeyCode::KeyQ => Some(0x0010),
            KeyCode::KeyR => Some(0x0013),
            KeyCode::KeyS => Some(0x001F),
            KeyCode::KeyT => Some(0x0014),
            KeyCode::KeyU => Some(0x0016),
            KeyCode::KeyV => Some(0x002F),
            KeyCode::KeyW => Some(0x0011),
            KeyCode::KeyX => Some(0x002D),
            KeyCode::KeyY => Some(0x0015),
            KeyCode::KeyZ => Some(0x002C),
            KeyCode::Minus => Some(0x000C),
            KeyCode::Period => Some(0x0034),
            KeyCode::Quote => Some(0x0028),
            KeyCode::Semicolon => Some(0x0027),
            KeyCode::Slash => Some(0x0035),
            KeyCode::AltLeft => Some(0x0038),
            KeyCode::AltRight => Some(0xE038),
            KeyCode::CapsLock => Some(0x003A),
            KeyCode::ContextMenu => Some(0xE05D),
            KeyCode::ControlLeft => Some(0x001D),
            KeyCode::ControlRight => Some(0xE01D),
            KeyCode::Enter => Some(0x001C),
            KeyCode::SuperLeft => Some(0xE05B),
            KeyCode::SuperRight => Some(0xE05C),
            KeyCode::ShiftLeft => Some(0x002A),
            KeyCode::ShiftRight => Some(0x0036),
            KeyCode::Space => Some(0x0039),
            KeyCode::Tab => Some(0x000F),
            KeyCode::Convert => Some(0x0079),
            KeyCode::Lang1 => {
                if is_korean {
                    Some(0xE0F2)
                } else {
                    Some(0x0072)
                }
            }
            KeyCode::Lang2 => {
                if is_korean {
                    Some(0xE0F1)
                } else {
                    Some(0x0071)
                }
            }
            KeyCode::KanaMode => Some(0x0070),
            KeyCode::NonConvert => Some(0x007B),
            KeyCode::Delete => Some(0xE053),
            KeyCode::End => Some(0xE04F),
            KeyCode::Home => Some(0xE047),
            KeyCode::Insert => Some(0xE052),
            KeyCode::PageDown => Some(0xE051),
            KeyCode::PageUp => Some(0xE049),
            KeyCode::ArrowDown => Some(0xE050),
            KeyCode::ArrowLeft => Some(0xE04B),
            KeyCode::ArrowRight => Some(0xE04D),
            KeyCode::ArrowUp => Some(0xE048),
            KeyCode::NumLock => Some(0xE045),
            KeyCode::Numpad0 => Some(0x0052),
            KeyCode::Numpad1 => Some(0x004F),
            KeyCode::Numpad2 => Some(0x0050),
            KeyCode::Numpad3 => Some(0x0051),
            KeyCode::Numpad4 => Some(0x004B),
            KeyCode::Numpad5 => Some(0x004C),
            KeyCode::Numpad6 => Some(0x004D),
            KeyCode::Numpad7 => Some(0x0047),
            KeyCode::Numpad8 => Some(0x0048),
            KeyCode::Numpad9 => Some(0x0049),
            KeyCode::NumpadAdd => Some(0x004E),
            KeyCode::NumpadComma => Some(0x007E),
            KeyCode::NumpadDecimal => Some(0x0053),
            KeyCode::NumpadDivide => Some(0xE035),
            KeyCode::NumpadEnter => Some(0xE01C),
            KeyCode::NumpadEqual => Some(0x0059),
            KeyCode::NumpadMultiply => Some(0x0037),
            KeyCode::NumpadSubtract => Some(0x004A),
            KeyCode::Escape => Some(0x0001),
            KeyCode::F1 => Some(0x003B),
            KeyCode::F2 => Some(0x003C),
            KeyCode::F3 => Some(0x003D),
            KeyCode::F4 => Some(0x003E),
            KeyCode::F5 => Some(0x003F),
            KeyCode::F6 => Some(0x0040),
            KeyCode::F7 => Some(0x0041),
            KeyCode::F8 => Some(0x0042),
            KeyCode::F9 => Some(0x0043),
            KeyCode::F10 => Some(0x0044),
            KeyCode::F11 => Some(0x0057),
            KeyCode::F12 => Some(0x0058),
            KeyCode::F13 => Some(0x0064),
            KeyCode::F14 => Some(0x0065),
            KeyCode::F15 => Some(0x0066),
            KeyCode::F16 => Some(0x0067),
            KeyCode::F17 => Some(0x0068),
            KeyCode::F18 => Some(0x0069),
            KeyCode::F19 => Some(0x006A),
            KeyCode::F20 => Some(0x006B),
            KeyCode::F21 => Some(0x006C),
            KeyCode::F22 => Some(0x006D),
            KeyCode::F23 => Some(0x006E),
            KeyCode::F24 => Some(0x0076),
            KeyCode::PrintScreen => Some(0xE037),
            //KeyCode::PrintScreen => Some(0x0054), // Alt + PrintScreen
            KeyCode::ScrollLock => Some(0x0046),
            KeyCode::Pause => Some(0x0045),
            //KeyCode::Pause => Some(0xE046), // Ctrl + Pause
            KeyCode::BrowserBack => Some(0xE06A),
            KeyCode::BrowserFavorites => Some(0xE066),
            KeyCode::BrowserForward => Some(0xE069),
            KeyCode::BrowserHome => Some(0xE032),
            KeyCode::BrowserRefresh => Some(0xE067),
            KeyCode::BrowserSearch => Some(0xE065),
            KeyCode::BrowserStop => Some(0xE068),
            KeyCode::LaunchApp1 => Some(0xE06B),
            KeyCode::LaunchApp2 => Some(0xE021),
            KeyCode::LaunchMail => Some(0xE06C),
            KeyCode::MediaPlayPause => Some(0xE022),
            KeyCode::MediaSelect => Some(0xE06D),
            KeyCode::MediaStop => Some(0xE024),
            KeyCode::MediaTrackNext => Some(0xE019),
            KeyCode::MediaTrackPrevious => Some(0xE010),
            KeyCode::Power => Some(0xE05E),
            KeyCode::AudioVolumeDown => Some(0xE02E),
            KeyCode::AudioVolumeMute => Some(0xE020),
            KeyCode::AudioVolumeUp => Some(0xE030),
            _ => None,
        }
    }

    fn from_scancode(scancode: u32) -> PhysicalKey {
        // See: https://www.win.tue.nl/~aeb/linux/kbd/scancodes-1.html
        // and: https://www.w3.org/TR/uievents-code/
        // and: The widget/NativeKeyToDOMCodeName.h file in the firefox source

        PhysicalKey::Code(match scancode {
            0x0029 => KeyCode::Backquote,
            0x002B => KeyCode::Backslash,
            0x000E => KeyCode::Backspace,
            0x001A => KeyCode::BracketLeft,
            0x001B => KeyCode::BracketRight,
            0x0033 => KeyCode::Comma,
            0x000B => KeyCode::Digit0,
            0x0002 => KeyCode::Digit1,
            0x0003 => KeyCode::Digit2,
            0x0004 => KeyCode::Digit3,
            0x0005 => KeyCode::Digit4,
            0x0006 => KeyCode::Digit5,
            0x0007 => KeyCode::Digit6,
            0x0008 => KeyCode::Digit7,
            0x0009 => KeyCode::Digit8,
            0x000A => KeyCode::Digit9,
            0x000D => KeyCode::Equal,
            0x0056 => KeyCode::IntlBackslash,
            0x0073 => KeyCode::IntlRo,
            0x007D => KeyCode::IntlYen,
            0x001E => KeyCode::KeyA,
            0x0030 => KeyCode::KeyB,
            0x002E => KeyCode::KeyC,
            0x0020 => KeyCode::KeyD,
            0x0012 => KeyCode::KeyE,
            0x0021 => KeyCode::KeyF,
            0x0022 => KeyCode::KeyG,
            0x0023 => KeyCode::KeyH,
            0x0017 => KeyCode::KeyI,
            0x0024 => KeyCode::KeyJ,
            0x0025 => KeyCode::KeyK,
            0x0026 => KeyCode::KeyL,
            0x0032 => KeyCode::KeyM,
            0x0031 => KeyCode::KeyN,
            0x0018 => KeyCode::KeyO,
            0x0019 => KeyCode::KeyP,
            0x0010 => KeyCode::KeyQ,
            0x0013 => KeyCode::KeyR,
            0x001F => KeyCode::KeyS,
            0x0014 => KeyCode::KeyT,
            0x0016 => KeyCode::KeyU,
            0x002F => KeyCode::KeyV,
            0x0011 => KeyCode::KeyW,
            0x002D => KeyCode::KeyX,
            0x0015 => KeyCode::KeyY,
            0x002C => KeyCode::KeyZ,
            0x000C => KeyCode::Minus,
            0x0034 => KeyCode::Period,
            0x0028 => KeyCode::Quote,
            0x0027 => KeyCode::Semicolon,
            0x0035 => KeyCode::Slash,
            0x0038 => KeyCode::AltLeft,
            0xE038 => KeyCode::AltRight,
            0x003A => KeyCode::CapsLock,
            0xE05D => KeyCode::ContextMenu,
            0x001D => KeyCode::ControlLeft,
            0xE01D => KeyCode::ControlRight,
            0x001C => KeyCode::Enter,
            0xE05B => KeyCode::SuperLeft,
            0xE05C => KeyCode::SuperRight,
            0x002A => KeyCode::ShiftLeft,
            0x0036 => KeyCode::ShiftRight,
            0x0039 => KeyCode::Space,
            0x000F => KeyCode::Tab,
            0x0079 => KeyCode::Convert,
            0x0072 => KeyCode::Lang1, // for non-Korean layout
            0xE0F2 => KeyCode::Lang1, // for Korean layout
            0x0071 => KeyCode::Lang2, // for non-Korean layout
            0xE0F1 => KeyCode::Lang2, // for Korean layout
            0x0070 => KeyCode::KanaMode,
            0x007B => KeyCode::NonConvert,
            0xE053 => KeyCode::Delete,
            0xE04F => KeyCode::End,
            0xE047 => KeyCode::Home,
            0xE052 => KeyCode::Insert,
            0xE051 => KeyCode::PageDown,
            0xE049 => KeyCode::PageUp,
            0xE050 => KeyCode::ArrowDown,
            0xE04B => KeyCode::ArrowLeft,
            0xE04D => KeyCode::ArrowRight,
            0xE048 => KeyCode::ArrowUp,
            0xE045 => KeyCode::NumLock,
            0x0052 => KeyCode::Numpad0,
            0x004F => KeyCode::Numpad1,
            0x0050 => KeyCode::Numpad2,
            0x0051 => KeyCode::Numpad3,
            0x004B => KeyCode::Numpad4,
            0x004C => KeyCode::Numpad5,
            0x004D => KeyCode::Numpad6,
            0x0047 => KeyCode::Numpad7,
            0x0048 => KeyCode::Numpad8,
            0x0049 => KeyCode::Numpad9,
            0x004E => KeyCode::NumpadAdd,
            0x007E => KeyCode::NumpadComma,
            0x0053 => KeyCode::NumpadDecimal,
            0xE035 => KeyCode::NumpadDivide,
            0xE01C => KeyCode::NumpadEnter,
            0x0059 => KeyCode::NumpadEqual,
            0x0037 => KeyCode::NumpadMultiply,
            0x004A => KeyCode::NumpadSubtract,
            0x0001 => KeyCode::Escape,
            0x003B => KeyCode::F1,
            0x003C => KeyCode::F2,
            0x003D => KeyCode::F3,
            0x003E => KeyCode::F4,
            0x003F => KeyCode::F5,
            0x0040 => KeyCode::F6,
            0x0041 => KeyCode::F7,
            0x0042 => KeyCode::F8,
            0x0043 => KeyCode::F9,
            0x0044 => KeyCode::F10,
            0x0057 => KeyCode::F11,
            0x0058 => KeyCode::F12,
            0x0064 => KeyCode::F13,
            0x0065 => KeyCode::F14,
            0x0066 => KeyCode::F15,
            0x0067 => KeyCode::F16,
            0x0068 => KeyCode::F17,
            0x0069 => KeyCode::F18,
            0x006A => KeyCode::F19,
            0x006B => KeyCode::F20,
            0x006C => KeyCode::F21,
            0x006D => KeyCode::F22,
            0x006E => KeyCode::F23,
            0x0076 => KeyCode::F24,
            0xE037 => KeyCode::PrintScreen,
            0x0054 => KeyCode::PrintScreen, // Alt + PrintScreen
            0x0046 => KeyCode::ScrollLock,
            0x0045 => KeyCode::Pause,
            0xE046 => KeyCode::Pause, // Ctrl + Pause
            0xE06A => KeyCode::BrowserBack,
            0xE066 => KeyCode::BrowserFavorites,
            0xE069 => KeyCode::BrowserForward,
            0xE032 => KeyCode::BrowserHome,
            0xE067 => KeyCode::BrowserRefresh,
            0xE065 => KeyCode::BrowserSearch,
            0xE068 => KeyCode::BrowserStop,
            0xE06B => KeyCode::LaunchApp1,
            0xE021 => KeyCode::LaunchApp2,
            0xE06C => KeyCode::LaunchMail,
            0xE022 => KeyCode::MediaPlayPause,
            0xE06D => KeyCode::MediaSelect,
            0xE024 => KeyCode::MediaStop,
            0xE019 => KeyCode::MediaTrackNext,
            0xE010 => KeyCode::MediaTrackPrevious,
            0xE05E => KeyCode::Power,
            0xE02E => KeyCode::AudioVolumeDown,
            0xE020 => KeyCode::AudioVolumeMute,
            0xE030 => KeyCode::AudioVolumeUp,
            _ => return PhysicalKey::Unidentified(NativeKeyCode::Windows(scancode as u16)),
        })
    }
}
