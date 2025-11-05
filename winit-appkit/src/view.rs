#![allow(clippy::unnecessary_cast)]
use std::cell::{Cell, RefCell};
use std::collections::{HashMap, VecDeque};
use std::ptr;
use std::rc::{Rc, Weak};

use dpi::{LogicalPosition, LogicalSize};
use objc2::rc::Retained;
use objc2::runtime::{AnyObject, Sel};
use objc2::{DefinedClass, MainThreadMarker, define_class, msg_send};
use objc2_app_kit::{
    NSApplication, NSCursor, NSEvent, NSEventPhase, NSResponder, NSTextInputClient,
    NSTrackingRectTag, NSView, NSWindow,
};
use objc2_core_foundation::CGRect;
use objc2_foundation::{
    NSArray, NSAttributedString, NSAttributedStringKey, NSCopying, NSMutableAttributedString,
    NSNotFound, NSObject, NSPoint, NSRange, NSRect, NSSize, NSString, NSUInteger,
};
use smol_str::SmolStr;
use winit_core::event::{
    DeviceEvent, ElementState, Ime, KeyEvent, Modifiers, MouseButton, MouseScrollDelta,
    PointerKind, PointerSource, TouchPhase, WindowEvent,
};
use winit_core::keyboard::{Key, KeyCode, KeyLocation, ModifiersState, NamedKey};
use winit_core::window::ImeCapabilities;

use super::app_state::AppState;
use super::cursor::{default_cursor, invisible_cursor};
use super::event::{
    code_to_key, code_to_location, create_key_event, event_mods, lalt_pressed, ralt_pressed,
    scancode_to_physicalkey,
};
use super::window::window_id;
use crate::OptionAsAlt;

#[derive(Debug)]
struct CursorState {
    visible: bool,
    cursor: Retained<NSCursor>,
}

impl Default for CursorState {
    fn default() -> Self {
        Self { visible: true, cursor: default_cursor() }
    }
}

/// A per-queued raw-character gate used to drop stale raw KeyboardInput events.
/// Each queued raw character captures an Rc<EventFilterToken> inside the runloop-dispatched
/// closure; when an IME Commit for the same key event arrives, we flip `deliver = false` so the
/// closure becomes a no-op.
#[derive(Debug)]
struct EventFilterToken {
    /// Whether this queued raw KeyboardInput should be delivered to the app.
    /// Set to `false` by `drop_conflicting_raw_characters` when an IME commit supersedes it.
    deliver: Cell<bool>,
}

impl EventFilterToken {
    fn new() -> Self {
        Self { deliver: Cell::new(true) }
    }
}

/// Bookkeeping for a raw-character KeyboardInput that was scheduled for dispatch.
/// - `serial`: monotonically increasing key-event serial so we can match it against an IME-handled
///   NSEvent in the same runloop tick.
/// - `text`: the raw character payload (e.g. ".") so we can compare with a subsequent IME commit
///   (e.g. "。").
/// - `token`: Weak reference allowing the IME path to cancel delivery without keeping the event
///   alive.
#[derive(Debug)]
struct PendingRawCharacter {
    /// Serial of the key event that produced this raw character.
    serial: u64,
    /// Raw character text captured from `KeyEvent.text`.
    text: SmolStr,
    /// Weak handle to the gate used by the dispatch closure.
    token: Weak<EventFilterToken>,
}

#[derive(Debug, Eq, PartialEq, Clone, Copy, Default)]
enum ImeState {
    #[default]
    /// The IME events are disabled, so only `ReceivedCharacter` is being sent to the user.
    Disabled,

    /// The ground state of enabled IME input. It means that both Preedit and regular keyboard
    /// input could be start from it.
    Ground,

    /// The IME is in preedit.
    Preedit,

    /// The text was just committed, so the next input from the keyboard must be ignored.
    Committed,
}

bitflags::bitflags! {
    #[derive(Debug, Clone, Copy, PartialEq)]
    struct ModLocationMask: u8 {
        const LEFT     = 0b0001;
        const RIGHT    = 0b0010;
    }
}
impl ModLocationMask {
    fn from_location(loc: KeyLocation) -> ModLocationMask {
        match loc {
            KeyLocation::Left => ModLocationMask::LEFT,
            KeyLocation::Right => ModLocationMask::RIGHT,
            _ => unreachable!(),
        }
    }
}

fn key_to_modifier(key: &Key) -> Option<ModifiersState> {
    match key {
        Key::Named(NamedKey::Alt) => Some(ModifiersState::ALT),
        Key::Named(NamedKey::Control) => Some(ModifiersState::CONTROL),
        Key::Named(NamedKey::Meta) => Some(ModifiersState::META),
        Key::Named(NamedKey::Shift) => Some(ModifiersState::SHIFT),
        _ => None,
    }
}

fn get_right_modifier_code(key: &Key) -> KeyCode {
    match key {
        Key::Named(NamedKey::Alt) => KeyCode::AltRight,
        Key::Named(NamedKey::Control) => KeyCode::ControlRight,
        Key::Named(NamedKey::Shift) => KeyCode::ShiftRight,
        Key::Named(NamedKey::Meta) => KeyCode::MetaRight,
        _ => unreachable!(),
    }
}

fn get_left_modifier_code(key: &Key) -> KeyCode {
    match key {
        Key::Named(NamedKey::Alt) => KeyCode::AltLeft,
        Key::Named(NamedKey::Control) => KeyCode::ControlLeft,
        Key::Named(NamedKey::Shift) => KeyCode::ShiftLeft,
        Key::Named(NamedKey::Meta) => KeyCode::MetaLeft,
        _ => unreachable!(),
    }
}

#[derive(Debug)]
pub struct ViewState {
    /// Strong reference to the global application state.
    app_state: Rc<AppState>,

    cursor_state: RefCell<CursorState>,
    ime_position: Cell<NSPoint>,
    ime_size: Cell<NSSize>,
    modifiers: Cell<Modifiers>,
    phys_modifiers: RefCell<HashMap<Key, ModLocationMask>>,
    tracking_rect: Cell<Option<NSTrackingRectTag>>,
    ime_state: Cell<ImeState>,
    input_source: RefCell<String>,

    /// True iff the application wants IME events.
    ///
    /// Can be set using `set_ime_allowed`
    ime_capabilities: Cell<Option<ImeCapabilities>>,

    /// True if the current key event should be forwarded
    /// to the application, even during IME
    forward_key_to_app: Cell<bool>,

    marked_text: RefCell<Retained<NSMutableAttributedString>>,
    accepts_first_mouse: bool,

    /// Monotonic counter incremented per keyDown/keyUp; groups raw text and IME handling within
    /// the same runloop turn.
    current_event_serial: Cell<u64>,
    /// Serial of the last `NSEvent` that was handed to `NSTextInputContext::handleEvent`.
    last_handled_event_serial: Cell<u64>,
    /// Raw-character events queued for delivery; used to drop them if an IME Commit disagrees.
    pending_raw_characters: RefCell<Vec<PendingRawCharacter>>,

    /// The state of the `Option` as `Alt`.
    option_as_alt: Cell<OptionAsAlt>,

    /// Suppress the next character-bearing keyUp after an IME commit.
    suppress_next_keyup_char: Cell<bool>,

    /// True while handling keyUp; used to filter stray insertText from keyUp.
    handling_keyup: Cell<bool>,

    /// Serial of the last IME commit; used to drop stray ASCII from the same key cycle.
    last_commit_serial: Cell<u64>,
}

define_class!(
    #[unsafe(super(NSView, NSResponder, NSObject))]
    #[ivars = ViewState]
    #[name = "WinitView"]
    pub(super) struct WinitView;

    /// This documentation attribute makes rustfmt work for some reason?
    impl WinitView {
        #[unsafe(method(isFlipped))]
        fn is_flipped(&self) -> bool {
            // `winit` uses the upper-left corner as the origin.
            true
        }

        #[unsafe(method(viewDidMoveToWindow))]
        fn view_did_move_to_window(&self) {
            trace_scope!("viewDidMoveToWindow");
            if let Some(tracking_rect) = self.ivars().tracking_rect.take() {
                self.removeTrackingRect(tracking_rect);
            }

            let rect = self.frame();
            let tracking_rect = unsafe {
                self.addTrackingRect_owner_userData_assumeInside(rect, self, ptr::null_mut(), false)
            };
            assert_ne!(tracking_rect, 0, "failed adding tracking rect");
            self.ivars().tracking_rect.set(Some(tracking_rect));
        }

        // Not a normal method on `NSView`, it's triggered by `NSViewFrameDidChangeNotification`.
        #[unsafe(method(viewFrameDidChangeNotification:))]
        fn frame_did_change(&self, _notification: Option<&AnyObject>) {
            trace_scope!("NSViewFrameDidChangeNotification");
            if let Some(tracking_rect) = self.ivars().tracking_rect.take() {
                self.removeTrackingRect(tracking_rect);
            }

            let rect = self.frame();
            let tracking_rect = unsafe {
                self.addTrackingRect_owner_userData_assumeInside(rect, self, ptr::null_mut(), false)
            };
            assert_ne!(tracking_rect, 0, "failed adding tracking rect");
            self.ivars().tracking_rect.set(Some(tracking_rect));

            // Emit resize event here rather than from windowDidResize because:
            // 1. When a new window is created as a tab, the frame size may change without a window
            //    resize occurring.
            // 2. Even when a window resize does occur on a new tabbed window, it contains the wrong
            //    size (includes tab height).
            let logical_size = LogicalSize::new(rect.size.width as f64, rect.size.height as f64);
            let size = logical_size.to_physical::<u32>(self.scale_factor());
            self.queue_event(WindowEvent::SurfaceResized(size));
        }

        #[unsafe(method(drawRect:))]
        fn draw_rect(&self, _rect: NSRect) {
            trace_scope!("drawRect:");

            self.ivars().app_state.handle_redraw(window_id(&self.window()));

            // This is a direct subclass of NSView, no need to call superclass' drawRect:
        }

        #[unsafe(method(acceptsFirstResponder))]
        fn accepts_first_responder(&self) -> bool {
            trace_scope!("acceptsFirstResponder");
            true
        }

        // This is necessary to prevent a beefy terminal error on MacBook Pros:
        // IMKInputSession [0x7fc573576ff0
        // presentFunctionRowItemTextInputViewWithEndpoint:completionHandler:] : [self
        // textInputContext]=0x7fc573558e10 *NO* NSRemoteViewController to client, NSError=Error
        // Domain=NSCocoaErrorDomain Code=4099 "The connection from pid 0 was invalidated from this
        // process." UserInfo={NSDebugDescription=The connection from pid 0 was invalidated from
        // this process.}, com.apple.inputmethod.EmojiFunctionRowItem TODO: Add an API
        // extension for using `NSTouchBar`
        #[unsafe(method_id(touchBar))]
        fn touch_bar(&self) -> Option<Retained<NSObject>> {
            trace_scope!("touchBar");
            None
        }

        #[unsafe(method(resetCursorRects))]
        fn reset_cursor_rects(&self) {
            trace_scope!("resetCursorRects");
            let bounds = self.bounds();
            let cursor_state = self.ivars().cursor_state.borrow();
            // We correctly invoke `addCursorRect` only from inside `resetCursorRects`
            if cursor_state.visible {
                self.addCursorRect_cursor(bounds, &cursor_state.cursor);
            } else {
                self.addCursorRect_cursor(bounds, &invisible_cursor());
            }
        }
    }

    unsafe impl NSTextInputClient for WinitView {
        #[unsafe(method(hasMarkedText))]
        fn has_marked_text(&self) -> bool {
            trace_scope!("hasMarkedText");
            self.ivars().marked_text.borrow().length() > 0
        }

        #[unsafe(method(markedRange))]
        fn marked_range(&self) -> NSRange {
            trace_scope!("markedRange");
            let length = self.ivars().marked_text.borrow().length();
            if length > 0 {
                NSRange::new(0, length)
            } else {
                // Documented to return `{NSNotFound, 0}` if there is no marked range.
                NSRange::new(NSNotFound as NSUInteger, 0)
            }
        }

        #[unsafe(method(selectedRange))]
        fn selected_range(&self) -> NSRange {
            trace_scope!("selectedRange");
            // Documented to return `{NSNotFound, 0}` if there is no selection.
            NSRange::new(NSNotFound as NSUInteger, 0)
        }

        #[unsafe(method(setMarkedText:selectedRange:replacementRange:))]
        fn set_marked_text(
            &self,
            string: &NSObject,
            selected_range: NSRange,
            _replacement_range: NSRange,
        ) {
            // TODO: Use _replacement_range, requires changing the event to report surrounding text.
            trace_scope!("setMarkedText:selectedRange:replacementRange:");

            let (marked_text, string) = if let Some(string) =
                string.downcast_ref::<NSAttributedString>()
            {
                (NSMutableAttributedString::from_attributed_nsstring(string), string.string())
            } else if let Some(string) = string.downcast_ref::<NSString>() {
                (NSMutableAttributedString::from_nsstring(string), string.copy())
            } else {
                // This method is guaranteed to get either a `NSString` or a `NSAttributedString`.
                panic!("unexpected text {string:?}")
            };

            // Update marked text.
            *self.ivars().marked_text.borrow_mut() = marked_text;

            // Notify IME is active if application still doesn't know it.
            if self.ivars().ime_state.get() == ImeState::Disabled {
                *self.ivars().input_source.borrow_mut() = self.current_input_source();
                self.queue_event(WindowEvent::Ime(Ime::Enabled));
            }

            if self.hasMarkedText() {
                self.ivars().ime_state.set(ImeState::Preedit);
            } else {
                // In case the preedit was cleared, set IME into the Ground state.
                self.ivars().ime_state.set(ImeState::Ground);
            }

            let cursor_range = if string.is_empty() {
                // An empty string basically means that there's no preedit, so indicate that by
                // sending a `None` cursor range.
                None
            } else {
                // Convert the selected range from UTF-16 indices to UTF-8 indices.
                let sub_string_a = string.substringToIndex(selected_range.location);
                let sub_string_b = string.substringToIndex(selected_range.end());
                let lowerbound_utf8 = sub_string_a.len();
                let upperbound_utf8 = sub_string_b.len();
                Some((lowerbound_utf8, upperbound_utf8))
            };

            // Send WindowEvent for updating marked text
            self.queue_event(WindowEvent::Ime(Ime::Preedit(string.to_string(), cursor_range)));
        }

        #[unsafe(method(unmarkText))]
        fn unmark_text(&self) {
            trace_scope!("unmarkText");
            *self.ivars().marked_text.borrow_mut() = NSMutableAttributedString::new();

            let input_context = self.inputContext().expect("input context");
            input_context.discardMarkedText();

            self.queue_event(WindowEvent::Ime(Ime::Preedit(String::new(), None)));
            if self.is_ime_enabled() {
                // Leave the Preedit self.ivars()
                self.ivars().ime_state.set(ImeState::Ground);
            } else {
                tracing::warn!("Expected to have IME enabled when receiving unmarkText");
            }
        }

        #[unsafe(method_id(validAttributesForMarkedText))]
        fn valid_attributes_for_marked_text(&self) -> Retained<NSArray<NSAttributedStringKey>> {
            trace_scope!("validAttributesForMarkedText");
            // Advertise the winit version so IME clients can identify us.
            let client_identifier =
                NSString::from_str(concat!("_rust_winit_", env!("CARGO_PKG_VERSION")));
            NSArray::from_slice(&[client_identifier.as_ref()])
        }

        #[unsafe(method_id(attributedSubstringForProposedRange:actualRange:))]
        fn attributed_substring_for_proposed_range(
            &self,
            _range: NSRange,
            _actual_range: *mut NSRange,
        ) -> Option<Retained<NSAttributedString>> {
            trace_scope!("attributedSubstringForProposedRange:actualRange:");
            None
        }

        #[unsafe(method(characterIndexForPoint:))]
        fn character_index_for_point(&self, _point: NSPoint) -> NSUInteger {
            trace_scope!("characterIndexForPoint:");
            0
        }

        #[unsafe(method(firstRectForCharacterRange:actualRange:))]
        fn first_rect_for_character_range(
            &self,
            _range: NSRange,
            _actual_range: *mut NSRange,
        ) -> NSRect {
            trace_scope!("firstRectForCharacterRange:actualRange:");

            // Guard when the view is no longer in a window during teardown.
            let Some(window) = (**self).window() else {
                return CGRect::ZERO;
            };

            // Return value is expected to be in screen coordinates, so we need a conversion
            let rect = NSRect::new(self.ivars().ime_position.get(), self.ivars().ime_size.get());
            let view_rect = self.convertRect_toView(rect, None);
            window.convertRectToScreen(view_rect)
        }

        #[unsafe(method(insertText:replacementRange:))]
        fn insert_text(&self, string: &NSObject, _replacement_range: NSRange) {
            // TODO: Use _replacement_range, requires changing the event to report surrounding text.
            trace_scope!("insertText:replacementRange:");

            let string = if let Some(string) = string.downcast_ref::<NSAttributedString>() {
                string.string().to_string()
            } else if let Some(string) = string.downcast_ref::<NSString>() {
                string.to_string()
            } else {
                // This method is guaranteed to get either a `NSString` or a `NSAttributedString`.
                panic!("unexpected text {string:?}")
            };

            let is_control = string.chars().next().is_some_and(|c| c.is_control());

            // If we're in keyUp handling, drop stray ASCII single characters some IMEs/apps send.
            if self.ivars().handling_keyup.get() && string.is_ascii() && string.chars().count() == 1
            {
                return;
            }
            // Drop a stray ASCII '.' arriving on keyUp for the same key cycle as a commit.
            let current_serial = self.ivars().current_event_serial.get();
            if self.ivars().last_commit_serial.get() == current_serial && string == "." {
                return;
            }

            // If insertText equals the pending raw character, treat it as plain typing and avoid
            // IME commit.
            let mut same_as_pending = false;
            {
                let pending = self.ivars().pending_raw_characters.borrow();
                if let Some(entry) = pending.iter().rev().find(|e| e.token.upgrade().is_some()) {
                    same_as_pending = entry.text.as_str() == string;
                }
            }

            // Commit when IME is enabled; some IMEs commit punctuation without marked text.
            if self.is_ime_enabled() && !is_control && !same_as_pending {
                // Safety net: if a raw ReceivedCharacter from this tick exists and differs (e.g.
                // '.' vs '。'), drop it.
                self.drop_conflicting_raw_characters(&string);
                self.queue_event(WindowEvent::Ime(Ime::Commit(string)));
                self.ivars().ime_state.set(ImeState::Committed);
                // Ensure the following keyUp doesn't emit a raw character like '.'
                self.ivars().suppress_next_keyup_char.set(true);
            }
        }

        // Basically, we're sent this message whenever a keyboard event that doesn't generate a
        // "human readable" character happens, i.e. newlines, tabs, and Ctrl+C.
        #[unsafe(method(doCommandBySelector:))]
        fn do_command_by_selector(&self, command: Sel) {
            trace_scope!("doCommandBySelector:");

            // We shouldn't forward any character from just committed text, since we'll end up
            // sending it twice with some IMEs like Korean one. We'll also always send
            // `Enter` in that case, which is not desired given it was used to confirm
            // IME input.
            if self.ivars().ime_state.get() == ImeState::Committed {
                return;
            }

            self.ivars().forward_key_to_app.set(true);

            if self.hasMarkedText() && self.ivars().ime_state.get() == ImeState::Preedit {
                // Leave preedit so that we also report the key-up for this key.
                self.ivars().ime_state.set(ImeState::Ground);
            }

            // Send command action to user if they requested it.
            let window_id = window_id(&self.window());
            self.ivars().app_state.maybe_queue_with_handler(move |app, event_loop| {
                if let Some(handler) = app.macos_handler() {
                    handler.standard_key_binding(
                        event_loop,
                        window_id,
                        command.name().to_str().unwrap(),
                    );
                }
            });

            // The documentation for `-[NSTextInputClient doCommandBySelector:]` clearly states that
            // we should not be forwarding this event up the responder chain, so no calling `super`
            // here either.
        }
    }

    /// This documentation attribute makes rustfmt work for some reason?
    impl WinitView {
        #[unsafe(method(keyDown:))]
        fn key_down(&self, event: &NSEvent) {
            trace_scope!("keyDown:");
            self.begin_key_event();
            {
                let mut prev_input_source = self.ivars().input_source.borrow_mut();
                let current_input_source = self.current_input_source();
                if *prev_input_source != current_input_source {
                    *prev_input_source = current_input_source;
                }
            }

            // Get the characters from the event.
            let old_ime_state = self.ivars().ime_state.get();
            self.ivars().forward_key_to_app.set(false);
            // Opportunistically allow IME path for punctuation even if no preedit yet.
            if self.ivars().ime_state.get() == ImeState::Disabled {
                self.ivars().ime_state.set(ImeState::Ground);
            }
            let event = replace_event(event, self.option_as_alt());

            // The `interpretKeyEvents` function might call
            // `setMarkedText`, `insertText`, and `doCommandBySelector`.
            // It's important that we call this before queuing the KeyboardInput, because
            // we must send the `KeyboardInput` event during IME if it triggered
            // `doCommandBySelector`. (doCommandBySelector means that the keyboard input
            // is not handled by IME and should be handled by the application)
            // Route via Cocoa; interpretKeyEvents forwards to the input context/IME and then to
            // NSTextInputClient. This allows IMEs to transform punctuation and other keys.
            let events_for_nsview = NSArray::from_slice(&[&*event]);
            self.interpretKeyEvents(&events_for_nsview);

            if self.ivars().ime_state.get() == ImeState::Committed {
                *self.ivars().marked_text.borrow_mut() = NSMutableAttributedString::new();
            }

            self.update_modifiers(&event, false);

            let had_ime_input = match self.ivars().ime_state.get() {
                ImeState::Committed => {
                    // Allow normal input after the commit.
                    self.ivars().ime_state.set(ImeState::Ground);
                    true
                },
                ImeState::Preedit => true,
                // `key_down` could result in preedit clear, so compare old and current state.
                _ => old_ime_state != self.ivars().ime_state.get(),
            };

            // When IME is enabled, don't send character-bearing raw KeyDown; rely on
            // insertText/commit. Always allow doCommandBySelector path; when IME is
            // disabled, forward as before.
            let key_event = create_key_event(&event, true, event.isARepeat());
            let send_raw = if self.ivars().forward_key_to_app.get() {
                true
            } else if self.is_ime_enabled() {
                // Allow non-text keys through, and always pass through when Ctrl/Command is held
                // for shortcuts.
                let mods = self.ivars().modifiers.get().state();
                key_event.text.is_none()
                    || mods.intersects(ModifiersState::CONTROL | ModifiersState::META)
            } else {
                !had_ime_input
            };
            if send_raw {
                self.queue_keyboard_input_event(key_event, false);
            }
        }

        #[unsafe(method(keyUp:))]
        fn key_up(&self, event: &NSEvent) {
            trace_scope!("keyUp:");
            self.begin_key_event();
            // Let IME observe keyUp too; some IMEs compare keyDown/keyUp (e.g. Shift single-tap
            // detection). We don't use the boolean result, but handing the event over
            // ensures IME state is correct.
            let event = replace_event(event, self.option_as_alt());
            self.update_modifiers(&event, false);

            // Let IME observe keyUp too; some IMEs compare keyDown/keyUp (e.g. Shift single-tap
            // detection).
            self.ivars().handling_keyup.set(true);
            let ime_consumed_event = self.handle_text_input_event(&event);
            self.ivars().handling_keyup.set(false);

            let key_event = create_key_event(&event, false, false);
            let is_char_key = matches!(key_event.logical_key, Key::Character(_));
            let suppress_char_once = self.ivars().suppress_next_keyup_char.replace(false);
            if suppress_char_once && is_char_key {
                // After an IME commit, suppress the very next character keyUp universally.
                return;
            }

            // Route keyUp: forward non-character releases when IME is enabled and didn't consume,
            // or when Ctrl/Command is held (shortcuts).
            let mods = self.ivars().modifiers.get().state();
            if !self.is_ime_enabled()
                || (!is_char_key && !ime_consumed_event)
                || mods.intersects(ModifiersState::CONTROL | ModifiersState::META)
            {
                self.queue_keyboard_input_event(key_event, false);
            }
        }

        #[unsafe(method(flagsChanged:))]
        fn flags_changed(&self, event: &NSEvent) {
            trace_scope!("flagsChanged:");

            self.update_modifiers(event, true);
        }

        #[unsafe(method(insertTab:))]
        fn insert_tab(&self, _sender: Option<&AnyObject>) {
            trace_scope!("insertTab:");
            let window = self.window();
            if let Some(first_responder) = window.firstResponder() {
                if *first_responder == ***self {
                    window.selectNextKeyView(Some(self))
                }
            }
        }

        #[unsafe(method(insertBackTab:))]
        fn insert_back_tab(&self, _sender: Option<&AnyObject>) {
            trace_scope!("insertBackTab:");
            let window = self.window();
            if let Some(first_responder) = window.firstResponder() {
                if *first_responder == ***self {
                    window.selectPreviousKeyView(Some(self))
                }
            }
        }

        // Allows us to receive Cmd-. (the shortcut for closing a dialog)
        // https://bugs.eclipse.org/bugs/show_bug.cgi?id=300620#c6
        #[unsafe(method(cancelOperation:))]
        fn cancel_operation(&self, _sender: Option<&AnyObject>) {
            let mtm = MainThreadMarker::from(self);
            trace_scope!("cancelOperation:");

            let event = NSApplication::sharedApplication(mtm)
                .currentEvent()
                .expect("could not find current event");

            self.update_modifiers(&event, false);
            let event = create_key_event(&event, true, event.isARepeat());

            self.queue_keyboard_input_event(event, false);
        }

        // In the past (?), `mouseMoved:` events were not generated when the
        // user hovered over a window from a separate window, and as such the
        // application might not know the location of the mouse in the event.
        //
        // To fix this, we emit `mouse_motion` inside of mouse click, mouse
        // scroll, magnify and other gesture event handlers, to ensure that
        // the application's state of where the mouse click was located is up
        // to date.
        //
        // See https://github.com/rust-windowing/winit/pull/1490 for history.

        #[unsafe(method(mouseDown:))]
        fn mouse_down(&self, event: &NSEvent) {
            trace_scope!("mouseDown:");
            self.mouse_motion(event);
            self.mouse_click(event, ElementState::Pressed);
        }

        #[unsafe(method(mouseUp:))]
        fn mouse_up(&self, event: &NSEvent) {
            trace_scope!("mouseUp:");
            self.mouse_motion(event);
            self.mouse_click(event, ElementState::Released);
        }

        #[unsafe(method(rightMouseDown:))]
        fn right_mouse_down(&self, event: &NSEvent) {
            trace_scope!("rightMouseDown:");
            self.mouse_motion(event);
            self.mouse_click(event, ElementState::Pressed);
        }

        #[unsafe(method(rightMouseUp:))]
        fn right_mouse_up(&self, event: &NSEvent) {
            trace_scope!("rightMouseUp:");
            self.mouse_motion(event);
            self.mouse_click(event, ElementState::Released);
        }

        #[unsafe(method(otherMouseDown:))]
        fn other_mouse_down(&self, event: &NSEvent) {
            trace_scope!("otherMouseDown:");
            self.mouse_motion(event);
            self.mouse_click(event, ElementState::Pressed);
        }

        #[unsafe(method(otherMouseUp:))]
        fn other_mouse_up(&self, event: &NSEvent) {
            trace_scope!("otherMouseUp:");
            self.mouse_motion(event);
            self.mouse_click(event, ElementState::Released);
        }

        // No tracing on these because that would be overly verbose

        #[unsafe(method(mouseMoved:))]
        fn mouse_moved(&self, event: &NSEvent) {
            self.mouse_motion(event);
        }

        #[unsafe(method(mouseDragged:))]
        fn mouse_dragged(&self, event: &NSEvent) {
            self.mouse_motion(event);
        }

        #[unsafe(method(rightMouseDragged:))]
        fn right_mouse_dragged(&self, event: &NSEvent) {
            self.mouse_motion(event);
        }

        #[unsafe(method(otherMouseDragged:))]
        fn other_mouse_dragged(&self, event: &NSEvent) {
            self.mouse_motion(event);
        }

        #[unsafe(method(mouseEntered:))]
        fn mouse_entered(&self, event: &NSEvent) {
            trace_scope!("mouseEntered:");

            let position = self.mouse_view_point(event).to_physical(self.scale_factor());

            self.queue_event(WindowEvent::PointerEntered {
                device_id: None,
                primary: true,
                position,
                kind: PointerKind::Mouse,
            });
        }

        #[unsafe(method(mouseExited:))]
        fn mouse_exited(&self, event: &NSEvent) {
            trace_scope!("mouseExited:");

            let position = self.mouse_view_point(event).to_physical(self.scale_factor());

            self.queue_event(WindowEvent::PointerLeft {
                device_id: None,
                primary: true,
                position: Some(position),
                kind: PointerKind::Mouse,
            });
        }

        #[unsafe(method(scrollWheel:))]
        fn scroll_wheel(&self, event: &NSEvent) {
            trace_scope!("scrollWheel:");

            self.mouse_motion(event);

            let delta = {
                let (x, y) = (event.scrollingDeltaX(), event.scrollingDeltaY());
                if event.hasPreciseScrollingDeltas() {
                    let delta = LogicalPosition::new(x, y).to_physical(self.scale_factor());
                    MouseScrollDelta::PixelDelta(delta)
                } else {
                    MouseScrollDelta::LineDelta(x as f32, y as f32)
                }
            };

            // The "momentum phase," if any, has higher priority than touch phase (the two should
            // be mutually exclusive anyhow, which is why the API is rather incoherent). If no
            // momentum phase is recorded (or rather, the started/ended cases of the
            // momentum phase) then we report the touch phase.
            #[allow(non_upper_case_globals)]
            let phase = match event.momentumPhase() {
                NSEventPhase::MayBegin | NSEventPhase::Began => TouchPhase::Started,
                NSEventPhase::Ended | NSEventPhase::Cancelled => TouchPhase::Ended,
                _ => match event.phase() {
                    NSEventPhase::MayBegin | NSEventPhase::Began => TouchPhase::Started,
                    NSEventPhase::Ended | NSEventPhase::Cancelled => TouchPhase::Ended,
                    _ => TouchPhase::Moved,
                },
            };

            self.update_modifiers(event, false);

            self.ivars().app_state.maybe_queue_with_handler(move |app, event_loop| {
                app.device_event(event_loop, None, DeviceEvent::MouseWheel { delta })
            });
            self.queue_event(WindowEvent::MouseWheel { device_id: None, delta, phase });
        }

        #[unsafe(method(magnifyWithEvent:))]
        fn magnify_with_event(&self, event: &NSEvent) {
            trace_scope!("magnifyWithEvent:");

            self.mouse_motion(event);

            #[allow(non_upper_case_globals)]
            let phase = match event.phase() {
                NSEventPhase::Began => TouchPhase::Started,
                NSEventPhase::Changed => TouchPhase::Moved,
                NSEventPhase::Cancelled => TouchPhase::Cancelled,
                NSEventPhase::Ended => TouchPhase::Ended,
                _ => return,
            };

            self.queue_event(WindowEvent::PinchGesture {
                device_id: None,
                delta: event.magnification(),
                phase,
            });
        }

        #[unsafe(method(smartMagnifyWithEvent:))]
        fn smart_magnify_with_event(&self, event: &NSEvent) {
            trace_scope!("smartMagnifyWithEvent:");

            self.mouse_motion(event);

            self.queue_event(WindowEvent::DoubleTapGesture { device_id: None });
        }

        #[unsafe(method(rotateWithEvent:))]
        fn rotate_with_event(&self, event: &NSEvent) {
            trace_scope!("rotateWithEvent:");

            self.mouse_motion(event);

            #[allow(non_upper_case_globals)]
            let phase = match event.phase() {
                NSEventPhase::Began => TouchPhase::Started,
                NSEventPhase::Changed => TouchPhase::Moved,
                NSEventPhase::Cancelled => TouchPhase::Cancelled,
                NSEventPhase::Ended => TouchPhase::Ended,
                _ => return,
            };

            self.queue_event(WindowEvent::RotationGesture {
                device_id: None,
                delta: event.rotation(),
                phase,
            });
        }

        #[unsafe(method(pressureChangeWithEvent:))]
        fn pressure_change_with_event(&self, event: &NSEvent) {
            trace_scope!("pressureChangeWithEvent:");

            self.queue_event(WindowEvent::TouchpadPressure {
                device_id: None,
                pressure: event.pressure(),
                stage: event.stage() as i64,
            });
        }

        // Allows us to receive Ctrl-Tab and Ctrl-Esc.
        // Note that this *doesn't* help with any missing Cmd inputs.
        // https://github.com/chromium/chromium/blob/a86a8a6bcfa438fa3ac2eba6f02b3ad1f8e0756f/ui/views/cocoa/bridged_content_view.mm#L816
        #[unsafe(method(_wantsKeyDownForEvent:))]
        fn wants_key_down_for_event(&self, _event: &NSEvent) -> bool {
            trace_scope!("_wantsKeyDownForEvent:");
            true
        }

        #[unsafe(method(acceptsFirstMouse:))]
        fn accepts_first_mouse(&self, _event: &NSEvent) -> bool {
            trace_scope!("acceptsFirstMouse:");
            self.ivars().accepts_first_mouse
        }
    }
);

impl WinitView {
    pub(super) fn new(
        app_state: &Rc<AppState>,
        accepts_first_mouse: bool,
        option_as_alt: OptionAsAlt,
        mtm: MainThreadMarker,
    ) -> Retained<Self> {
        let this = mtm.alloc().set_ivars(ViewState {
            app_state: Rc::clone(app_state),
            cursor_state: Default::default(),
            ime_position: Default::default(),
            ime_size: Default::default(),
            modifiers: Default::default(),
            phys_modifiers: Default::default(),
            tracking_rect: Default::default(),
            ime_state: Default::default(),
            input_source: Default::default(),
            ime_capabilities: Default::default(),
            forward_key_to_app: Default::default(),
            marked_text: Default::default(),
            accepts_first_mouse,
            current_event_serial: Cell::new(0),
            last_handled_event_serial: Cell::new(0),
            pending_raw_characters: RefCell::new(Vec::new()),
            option_as_alt: Cell::new(option_as_alt),
            suppress_next_keyup_char: Cell::new(false),
            handling_keyup: Cell::new(false),
            last_commit_serial: Cell::new(0),
        });
        let this: Retained<Self> = unsafe { msg_send![super(this), init] };

        *this.ivars().input_source.borrow_mut() = this.current_input_source();

        this
    }

    fn window(&self) -> Retained<NSWindow> {
        (**self).window().expect("view must be installed in a window")
    }

    fn queue_event(&self, event: WindowEvent) {
        let window_id = window_id(&self.window());
        self.ivars().app_state.maybe_queue_with_handler(move |app, event_loop| {
            app.window_event(event_loop, window_id, event);
        });
    }

    /// Queue a KeyboardInput for delivery, with an option to drop it later if an IME commit
    /// supersedes it.
    ///
    /// Rationale: when an IME is active, macOS can generate both a raw character (e.g. '.') and an
    /// IME commit (e.g. '。') for the same physical key press. We tentatively enqueue the raw
    /// event, but guard it with a token so `drop_conflicting_raw_characters` can cancel
    /// delivery in the same runloop turn.
    fn queue_keyboard_input_event(&self, key_event: KeyEvent, is_synthetic: bool) {
        // Trim any stale entries whose tokens have already been dropped by dispatched closures.
        self.cleanup_pending_raw_characters();

        // Associate this event with the current key event serial.
        let serial = self.ivars().current_event_serial.get();
        // Only character-bearing events participate in the safety net; non-text key events bypass
        // the filter.
        let token = key_event.text.as_ref().map(|text| {
            let token = Rc::new(EventFilterToken::new());
            self.ivars().pending_raw_characters.borrow_mut().push(PendingRawCharacter {
                serial,
                text: text.clone(),
                token: Rc::downgrade(&token),
            });
            token
        });

        let window_event =
            WindowEvent::KeyboardInput { device_id: None, event: key_event, is_synthetic };
        let window_id = window_id(&self.window());

        if let Some(token) = token {
            // Defer dispatch and drop the event if IME said to supersede it.
            let event_to_dispatch = window_event.clone();
            self.ivars().app_state.maybe_queue_with_handler(move |app, event_loop| {
                // The IME path may have flipped `deliver` to false.
                if !token.deliver.get() {
                    return;
                }
                app.window_event(event_loop, window_id, event_to_dispatch.clone());
            });
        } else {
            // No text payload: dispatch as-is.
            let event_to_dispatch = window_event;
            self.ivars().app_state.maybe_queue_with_handler(move |app, event_loop| {
                app.window_event(event_loop, window_id, event_to_dispatch.clone());
            });
        }
    }

    /// Start a new serial for the current keyDown/keyUp pair.
    ///
    /// We use this to correlate raw-character events with IME handling within the same runloop
    /// tick.
    fn begin_key_event(&self) {
        let next = self.ivars().current_event_serial.get().wrapping_add(1);
        self.ivars().current_event_serial.set(next);
    }

    /// Let IME observe the native NSEvent via `NSTextInputContext::handleEvent` and record the
    /// serial.
    ///
    /// Returns true when the IME consumed the event; in that case we should suppress raw character
    /// delivery.
    fn handle_text_input_event(&self, event: &NSEvent) -> bool {
        let Some(input_context) = self.inputContext() else {
            return false;
        };

        // Record which serial was seen by the IME so `drop_conflicting_raw_characters` knows what
        // to cancel.
        let serial = self.ivars().current_event_serial.get();
        self.ivars().last_handled_event_serial.set(serial);

        input_context.handleEvent(event)
    }

    /// Drop the most relevant queued raw-character event if its text disagrees with the IME commit.
    ///
    /// Strategy:
    /// - Prefer the raw character queued in the same serial (same key event) as the IME-handled
    ///   NSEvent.
    /// - If none is found (ordering nuances), fall back to the newest still-alive raw character.
    /// - If its text != `commit`, flip its token to prevent delivery.
    fn drop_conflicting_raw_characters(&self, commit: &str) {
        let serial = self.ivars().last_handled_event_serial.get();
        let mut pending = self.ivars().pending_raw_characters.borrow_mut();

        let mut target: Option<(Weak<EventFilterToken>, SmolStr)> = None;

        // Search from newest to oldest to find a match in the same serial.
        for entry in pending.iter().rev() {
            if entry.token.upgrade().is_none() {
                continue;
            }

            if entry.serial == serial {
                target = Some((entry.token.clone(), entry.text.clone()));
                break;
            }
        }

        // If we didn't find one in the same serial, take the newest alive entry.
        if target.is_none() {
            if let Some(entry) = pending.iter().rev().find(|entry| entry.token.upgrade().is_some())
            {
                target = Some((entry.token.clone(), entry.text.clone()));
            }
        }

        if let Some((token, text)) = target {
            if text.as_str() != commit {
                if let Some(token) = token.upgrade() {
                    // Cancel delivery of the stale raw character.
                    token.deliver.set(false);
                }
            }
        }

        // GC: keep only entries whose tokens are still alive.
        pending.retain(|entry| entry.token.upgrade().is_some());
    }

    /// Remove bookkeeping entries whose dispatch tokens have already been dropped.
    fn cleanup_pending_raw_characters(&self) {
        self.ivars()
            .pending_raw_characters
            .borrow_mut()
            .retain(|entry| entry.token.upgrade().is_some());
    }

    fn scale_factor(&self) -> f64 {
        self.window().backingScaleFactor() as f64
    }

    fn is_ime_enabled(&self) -> bool {
        !matches!(self.ivars().ime_state.get(), ImeState::Disabled)
    }

    fn current_input_source(&self) -> String {
        self.inputContext()
            .expect("input context")
            .selectedKeyboardInputSource()
            .map(|input_source| input_source.to_string())
            .unwrap_or_default()
    }

    pub(super) fn cursor_icon(&self) -> Retained<NSCursor> {
        self.ivars().cursor_state.borrow().cursor.clone()
    }

    pub(super) fn set_cursor_icon(&self, icon: Retained<NSCursor>) {
        let mut cursor_state = self.ivars().cursor_state.borrow_mut();
        cursor_state.cursor = icon;
    }

    /// Set whether the cursor should be visible or not.
    ///
    /// Returns whether the state changed.
    pub(super) fn set_cursor_visible(&self, visible: bool) -> bool {
        let mut cursor_state = self.ivars().cursor_state.borrow_mut();
        if visible != cursor_state.visible {
            cursor_state.visible = visible;
            true
        } else {
            false
        }
    }

    pub(super) fn set_ime_allowed(&self, capabilities: Option<ImeCapabilities>) {
        if self.ivars().ime_capabilities.get().is_some() {
            return;
        }
        self.ivars().ime_capabilities.set(capabilities);

        if capabilities.is_some() {
            return;
        }

        // Clear markedText
        *self.ivars().marked_text.borrow_mut() = NSMutableAttributedString::new();

        if self.ivars().ime_state.get() != ImeState::Disabled {
            self.ivars().ime_state.set(ImeState::Disabled);
            self.queue_event(WindowEvent::Ime(Ime::Disabled));
        }
    }

    pub(super) fn ime_capabilities(&self) -> Option<ImeCapabilities> {
        self.ivars().ime_capabilities.get()
    }

    pub(super) fn set_ime_cursor_area(&self, position: NSPoint, size: NSSize) {
        self.ivars().ime_position.set(position);
        self.ivars().ime_size.set(size);
        let input_context = self.inputContext().expect("input context");
        input_context.invalidateCharacterCoordinates();
    }

    /// Reset modifiers and emit a synthetic ModifiersChanged event if deemed necessary.
    pub(super) fn reset_modifiers(&self) {
        if !self.ivars().modifiers.get().state().is_empty() {
            self.ivars().modifiers.set(Modifiers::default());
            self.queue_event(WindowEvent::ModifiersChanged(self.ivars().modifiers.get()));
        }
    }

    pub(super) fn set_option_as_alt(&self, value: OptionAsAlt) {
        self.ivars().option_as_alt.set(value)
    }

    pub(super) fn option_as_alt(&self) -> OptionAsAlt {
        self.ivars().option_as_alt.get()
    }

    /// Update modifiers if `event` has something different
    fn update_modifiers(&self, ns_event: &NSEvent, is_flags_changed_event: bool) {
        use ElementState::{Pressed, Released};

        let current_modifiers = event_mods(ns_event);
        let prev_modifiers = self.ivars().modifiers.get();
        self.ivars().modifiers.set(current_modifiers);

        // This function was called form the flagsChanged event, which is triggered
        // when the user presses/releases a modifier even if the same kind of modifier
        // has already been pressed.
        //
        // When flags changed event has key code of zero it means that event doesn't carry any key
        // event, thus we can't generate regular presses based on that. The `ModifiersChanged`
        // later will work though, since the flags are attached to the event and contain valid
        // information.
        'send_event: {
            if is_flags_changed_event && ns_event.keyCode() != 0 {
                let scancode = ns_event.keyCode();
                let physical_key = scancode_to_physicalkey(scancode as u32);

                let logical_key = code_to_key(physical_key, scancode);
                // Ignore processing of unknown modifiers because we can't determine whether
                // it was pressed or release reliably.
                //
                // Furthermore, sometimes normal keys are reported inside flagsChanged:, such as
                // when holding Caps Lock while pressing another key, see:
                // https://github.com/alacritty/alacritty/issues/8268
                let Some(event_modifier) = key_to_modifier(&logical_key) else {
                    break 'send_event;
                };

                let mut event = KeyEvent {
                    location: code_to_location(physical_key),
                    logical_key: logical_key.clone(),
                    physical_key,
                    repeat: false,
                    // We'll correct this later.
                    state: Pressed,
                    text: None,
                    text_with_all_modifiers: None,
                    key_without_modifiers: logical_key.clone(),
                };

                let location_mask = ModLocationMask::from_location(event.location);

                let mut phys_mod_state = self.ivars().phys_modifiers.borrow_mut();
                let phys_mod =
                    phys_mod_state.entry(logical_key).or_insert(ModLocationMask::empty());

                let is_active = current_modifiers.state().contains(event_modifier);
                let mut events = VecDeque::with_capacity(2);

                // There is no API for getting whether the button was pressed or released
                // during this event. For this reason we have to do a bit of magic below
                // to come up with a good guess whether this key was pressed or released.
                // (This is not trivial because there are multiple buttons that may affect
                // the same modifier)
                if !is_active {
                    event.state = Released;
                    if phys_mod.contains(ModLocationMask::LEFT) {
                        let mut event = event.clone();
                        event.location = KeyLocation::Left;
                        event.physical_key = get_left_modifier_code(&event.logical_key).into();
                        events.push_back(WindowEvent::KeyboardInput {
                            device_id: None,
                            event,
                            is_synthetic: false,
                        });
                    }
                    if phys_mod.contains(ModLocationMask::RIGHT) {
                        event.location = KeyLocation::Right;
                        event.physical_key = get_right_modifier_code(&event.logical_key).into();
                        events.push_back(WindowEvent::KeyboardInput {
                            device_id: None,
                            event,
                            is_synthetic: false,
                        });
                    }
                    *phys_mod = ModLocationMask::empty();
                } else {
                    if *phys_mod == location_mask {
                        // Here we hit a contradiction:
                        // The modifier state was "changed" to active,
                        // yet the only pressed modifier key was the one that we
                        // just got a change event for.
                        // This seemingly means that the only pressed modifier is now released,
                        // but at the same time the modifier became active.
                        //
                        // But this scenario is possible if we released modifiers
                        // while the application was not in focus. (Because we don't
                        // get informed of modifier key events while the application
                        // is not focused)

                        // In this case we prioritize the information
                        // about the current modifier state which means
                        // that the button was pressed.
                        event.state = Pressed;
                    } else {
                        phys_mod.toggle(location_mask);
                        let is_pressed = phys_mod.contains(location_mask);
                        event.state = if is_pressed { Pressed } else { Released };
                    }

                    events.push_back(WindowEvent::KeyboardInput {
                        device_id: None,
                        event,
                        is_synthetic: false,
                    });
                }

                drop(phys_mod_state);

                for event in events {
                    match event {
                        // Route synthesized modifier presses through the same filtering path to
                        // honor IME safety net.
                        WindowEvent::KeyboardInput { event: key_event, is_synthetic, .. } => {
                            self.queue_keyboard_input_event(key_event, is_synthetic);
                        },
                        other => self.queue_event(other),
                    }
                }
            }
        }

        if prev_modifiers == current_modifiers {
            return;
        }

        self.queue_event(WindowEvent::ModifiersChanged(self.ivars().modifiers.get()));
    }

    fn mouse_click(&self, event: &NSEvent, button_state: ElementState) {
        let position = self.mouse_view_point(event).to_physical(self.scale_factor());
        let button = mouse_button(event);

        self.update_modifiers(event, false);

        self.queue_event(WindowEvent::PointerButton {
            device_id: None,
            primary: true,
            state: button_state,
            position,
            button: button.into(),
        });
    }

    fn mouse_motion(&self, event: &NSEvent) {
        let view_point = self.mouse_view_point(event);
        let frame = self.frame();

        if view_point.x.is_sign_negative()
            || view_point.y.is_sign_negative()
            || view_point.x > frame.size.width
            || view_point.y > frame.size.height
        {
            let mouse_buttons_down = NSEvent::pressedMouseButtons();
            if mouse_buttons_down == 0 {
                // Point is outside of the client area (view) and no buttons are pressed
                return;
            }
        }

        self.update_modifiers(event, false);

        self.queue_event(WindowEvent::PointerMoved {
            device_id: None,
            primary: true,
            position: view_point.to_physical(self.scale_factor()),
            source: PointerSource::Mouse,
        });
    }

    fn mouse_view_point(&self, event: &NSEvent) -> LogicalPosition<f64> {
        let window_point = event.locationInWindow();
        let view_point = self.convertPoint_fromView(window_point, None);

        LogicalPosition::new(view_point.x, view_point.y)
    }
}

/// Get the mouse button from the NSEvent.
fn mouse_button(event: &NSEvent) -> MouseButton {
    // The buttonNumber property only makes sense for the mouse events:
    // NSLeftMouse.../NSRightMouse.../NSOtherMouse...
    // For the other events, it's always set to 0.
    // MacOS only defines the left, right and middle buttons, 3..=31 are left as generic buttons,
    // but 3 and 4 are very commonly used as Back and Forward by hardware vendors and applications.
    let b: isize = event.buttonNumber();
    b.try_into()
        .ok()
        .and_then(MouseButton::try_from_u8)
        .expect("expected MacOS button number in the range 0..=31")
}

// NOTE: to get option as alt working we need to rewrite events
// we're getting from the operating system, which makes it
// impossible to provide such events as extra in `KeyEvent`.
fn replace_event(event: &NSEvent, option_as_alt: OptionAsAlt) -> Retained<NSEvent> {
    let ev_mods = event_mods(event).state();
    let ignore_alt_characters = match option_as_alt {
        OptionAsAlt::OnlyLeft if lalt_pressed(event) => true,
        OptionAsAlt::OnlyRight if ralt_pressed(event) => true,
        OptionAsAlt::Both if ev_mods.alt_key() => true,
        _ => false,
    } && !ev_mods.control_key()
        && !ev_mods.meta_key();

    if ignore_alt_characters {
        let ns_chars =
            event.charactersIgnoringModifiers().expect("expected characters to be non-null");

        NSEvent::keyEventWithType_location_modifierFlags_timestamp_windowNumber_context_characters_charactersIgnoringModifiers_isARepeat_keyCode(
                event.r#type(),
                event.locationInWindow(),
                event.modifierFlags(),
                event.timestamp(),
                event.windowNumber(),
                None,
                &ns_chars,
                &ns_chars,
                event.isARepeat(),
                event.keyCode(),
            )
            .unwrap()
    } else {
        event.copy()
    }
}
