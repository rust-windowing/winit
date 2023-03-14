#![allow(clippy::unnecessary_cast)]

use std::{boxed::Box, os::raw::*, ptr, str, sync::Mutex};

use objc2::declare::{Ivar, IvarDrop};
use objc2::foundation::{
    NSArray, NSAttributedString, NSAttributedStringKey, NSCopying, NSMutableAttributedString,
    NSObject, NSPoint, NSRange, NSRect, NSSize, NSString, NSUInteger,
};
use objc2::rc::{Id, Owned, Shared, WeakId};
use objc2::runtime::{Object, Sel};
use objc2::{class, declare_class, msg_send, msg_send_id, sel, ClassType};

use super::appkit::{
    NSApp, NSCursor, NSEvent, NSEventModifierFlags, NSEventPhase, NSResponder, NSTrackingRectTag,
    NSView,
};
use crate::platform::macos::{OptionAsAlt, WindowExtMacOS};
use crate::{
    dpi::{LogicalPosition, LogicalSize},
    event::{
        DeviceEvent, ElementState, Event, Ime, KeyboardInput, ModifiersState, MouseButton,
        MouseScrollDelta, TouchPhase, VirtualKeyCode, WindowEvent,
    },
    platform_impl::platform::{
        app_state::AppState,
        event::{
            char_to_keycode, check_function_keys, event_mods, modifier_event, scancode_to_keycode,
            EventWrapper,
        },
        util,
        window::WinitWindow,
        DEVICE_ID,
    },
    window::WindowId,
};

#[derive(Debug)]
pub struct CursorState {
    pub visible: bool,
    pub(super) cursor: Id<NSCursor, Shared>,
}

impl Default for CursorState {
    fn default() -> Self {
        Self {
            visible: true,
            cursor: Default::default(),
        }
    }
}

#[derive(Debug, Eq, PartialEq)]
enum ImeState {
    /// The IME events are disabled, so only `ReceivedCharacter` is being sent to the user.
    Disabled,

    /// The IME events are enabled.
    Enabled,

    /// The IME is in preedit.
    Preedit,

    /// The text was just commited, so the next input from the keyboard must be ignored.
    Commited,
}

#[derive(Debug)]
pub(super) struct ViewState {
    pub cursor_state: Mutex<CursorState>,
    ime_position: LogicalPosition<f64>,
    pub(super) modifiers: ModifiersState,
    tracking_rect: Option<NSTrackingRectTag>,
    ime_state: ImeState,
    input_source: String,

    /// True iff the application wants IME events.
    ///
    /// Can be set using `set_ime_allowed`
    ime_allowed: bool,

    /// True if the current key event should be forwarded
    /// to the application, even during IME
    forward_key_to_app: bool,
}

fn get_characters(event: &NSEvent, ignore_modifiers: bool) -> String {
    if ignore_modifiers {
        event.charactersIgnoringModifiers()
    } else {
        event.characters()
    }
    .expect("expected characters to be non-null")
    .to_string()
}

// As defined in: https://www.unicode.org/Public/MAPPINGS/VENDORS/APPLE/CORPCHAR.TXT
fn is_corporate_character(c: char) -> bool {
    matches!(c,
        '\u{F700}'..='\u{F747}'
        | '\u{F802}'..='\u{F84F}'
        | '\u{F850}'
        | '\u{F85C}'
        | '\u{F85D}'
        | '\u{F85F}'
        | '\u{F860}'..='\u{F86B}'
        | '\u{F870}'..='\u{F8FF}'
    )
}

// Retrieves a layout-independent keycode given an event.
fn retrieve_keycode(event: &NSEvent) -> Option<VirtualKeyCode> {
    #[inline]
    fn get_code(ev: &NSEvent, raw: bool) -> Option<VirtualKeyCode> {
        let characters = get_characters(ev, raw);
        characters.chars().next().and_then(char_to_keycode)
    }

    // Cmd switches Roman letters for Dvorak-QWERTY layout, so we try modified characters first.
    // If we don't get a match, then we fall back to unmodified characters.
    let code = get_code(event, false).or_else(|| get_code(event, true));

    // We've checked all layout related keys, so fall through to scancode.
    // Reaching this code means that the key is layout-independent (e.g. Backspace, Return).
    //
    // We're additionally checking here for F21-F24 keys, since their keycode
    // can vary, but we know that they are encoded
    // in characters property.
    code.or_else(|| {
        let scancode = event.scancode();
        scancode_to_keycode(scancode).or_else(|| check_function_keys(&get_characters(event, true)))
    })
}

declare_class!(
    #[derive(Debug)]
    #[allow(non_snake_case)]
    pub(super) struct WinitView {
        // Weak reference because the window keeps a strong reference to the view
        _ns_window: IvarDrop<Box<WeakId<WinitWindow>>>,
        pub(super) state: IvarDrop<Box<ViewState>>,
        marked_text: IvarDrop<Id<NSMutableAttributedString, Owned>>,
        accepts_first_mouse: bool,
    }

    unsafe impl ClassType for WinitView {
        #[inherits(NSResponder, NSObject)]
        type Super = NSView;
    }

    unsafe impl WinitView {
        #[sel(initWithId:acceptsFirstMouse:)]
        fn init_with_id(
            &mut self,
            window: &WinitWindow,
            accepts_first_mouse: bool,
        ) -> Option<&mut Self> {
            let this: Option<&mut Self> = unsafe { msg_send![super(self), init] };
            this.map(|this| {
                let state = ViewState {
                    cursor_state: Default::default(),
                    ime_position: LogicalPosition::new(0.0, 0.0),
                    modifiers: Default::default(),
                    tracking_rect: None,
                    ime_state: ImeState::Disabled,
                    input_source: String::new(),
                    ime_allowed: false,
                    forward_key_to_app: false,
                };

                Ivar::write(
                    &mut this._ns_window,
                    Box::new(WeakId::new(&window.retain())),
                );
                Ivar::write(&mut this.state, Box::new(state));
                Ivar::write(&mut this.marked_text, NSMutableAttributedString::new());
                Ivar::write(&mut this.accepts_first_mouse, accepts_first_mouse);

                this.setPostsFrameChangedNotifications(true);

                let notification_center: &Object =
                    unsafe { msg_send![class!(NSNotificationCenter), defaultCenter] };
                // About frame change
                let frame_did_change_notification_name =
                    NSString::from_str("NSViewFrameDidChangeNotification");
                #[allow(clippy::let_unit_value)]
                unsafe {
                    let _: () = msg_send![
                        notification_center,
                        addObserver: &*this,
                        selector: sel!(frameDidChange:),
                        name: &*frame_did_change_notification_name,
                        object: &*this,
                    ];
                }

                this.state.input_source = this.current_input_source();
                this
            })
        }
    }

    unsafe impl WinitView {
        #[sel(viewDidMoveToWindow)]
        fn view_did_move_to_window(&mut self) {
            trace_scope!("viewDidMoveToWindow");
            if let Some(tracking_rect) = self.state.tracking_rect.take() {
                self.removeTrackingRect(tracking_rect);
            }

            let rect = self.visibleRect();
            let tracking_rect = self.add_tracking_rect(rect, false);
            self.state.tracking_rect = Some(tracking_rect);
        }

        #[sel(frameDidChange:)]
        fn frame_did_change(&mut self, _event: &NSEvent) {
            trace_scope!("frameDidChange:");
            if let Some(tracking_rect) = self.state.tracking_rect.take() {
                self.removeTrackingRect(tracking_rect);
            }

            let rect = self.visibleRect();
            let tracking_rect = self.add_tracking_rect(rect, false);
            self.state.tracking_rect = Some(tracking_rect);

            // Emit resize event here rather than from windowDidResize because:
            // 1. When a new window is created as a tab, the frame size may change without a window resize occurring.
            // 2. Even when a window resize does occur on a new tabbed window, it contains the wrong size (includes tab height).
            let logical_size = LogicalSize::new(rect.size.width as f64, rect.size.height as f64);
            let size = logical_size.to_physical::<u32>(self.scale_factor());
            self.queue_event(WindowEvent::Resized(size));
        }

        #[sel(drawRect:)]
        fn draw_rect(&mut self, rect: NSRect) {
            trace_scope!("drawRect:");

            AppState::handle_redraw(self.window_id());

            #[allow(clippy::let_unit_value)]
            unsafe {
                let _: () = msg_send![super(self), drawRect: rect];
            }
        }

        #[sel(acceptsFirstResponder)]
        fn accepts_first_responder(&self) -> bool {
            trace_scope!("acceptsFirstResponder");
            true
        }

        // This is necessary to prevent a beefy terminal error on MacBook Pros:
        // IMKInputSession [0x7fc573576ff0 presentFunctionRowItemTextInputViewWithEndpoint:completionHandler:] : [self textInputContext]=0x7fc573558e10 *NO* NSRemoteViewController to client, NSError=Error Domain=NSCocoaErrorDomain Code=4099 "The connection from pid 0 was invalidated from this process." UserInfo={NSDebugDescription=The connection from pid 0 was invalidated from this process.}, com.apple.inputmethod.EmojiFunctionRowItem
        // TODO: Add an API extension for using `NSTouchBar`
        #[sel(touchBar)]
        fn touch_bar(&self) -> bool {
            trace_scope!("touchBar");
            false
        }

        #[sel(resetCursorRects)]
        fn reset_cursor_rects(&self) {
            trace_scope!("resetCursorRects");
            let bounds = self.bounds();
            let cursor_state = self.state.cursor_state.lock().unwrap();
            // We correctly invoke `addCursorRect` only from inside `resetCursorRects`
            if cursor_state.visible {
                self.addCursorRect(bounds, &cursor_state.cursor);
            } else {
                self.addCursorRect(bounds, &NSCursor::invisible());
            }
        }
    }

    unsafe impl Protocol<NSTextInputClient> for WinitView {
        #[sel(hasMarkedText)]
        fn has_marked_text(&self) -> bool {
            trace_scope!("hasMarkedText");
            self.marked_text.len_utf16() > 0
        }

        #[sel(markedRange)]
        fn marked_range(&self) -> NSRange {
            trace_scope!("markedRange");
            let length = self.marked_text.len_utf16();
            if length > 0 {
                NSRange::new(0, length)
            } else {
                util::EMPTY_RANGE
            }
        }

        #[sel(selectedRange)]
        fn selected_range(&self) -> NSRange {
            trace_scope!("selectedRange");
            util::EMPTY_RANGE
        }

        #[sel(setMarkedText:selectedRange:replacementRange:)]
        fn set_marked_text(
            &mut self,
            string: &NSObject,
            _selected_range: NSRange,
            _replacement_range: NSRange,
        ) {
            trace_scope!("setMarkedText:selectedRange:replacementRange:");

            // SAFETY: This method is guaranteed to get either a `NSString` or a `NSAttributedString`.
            let (marked_text, preedit_string) = if string.is_kind_of::<NSAttributedString>() {
                let string: *const NSObject = string;
                let string: *const NSAttributedString = string.cast();
                let string = unsafe { &*string };
                (
                    NSMutableAttributedString::from_attributed_nsstring(string),
                    string.string().to_string(),
                )
            } else {
                let string: *const NSObject = string;
                let string: *const NSString = string.cast();
                let string = unsafe { &*string };
                (
                    NSMutableAttributedString::from_nsstring(string),
                    string.to_string(),
                )
            };

            // Update marked text
            *self.marked_text = marked_text;

            // Notify IME is active if application still doesn't know it.
            if self.state.ime_state == ImeState::Disabled {
                self.state.input_source = self.current_input_source();
                self.queue_event(WindowEvent::Ime(Ime::Enabled));
            }

            // Don't update self.state to preedit when we've just commited a string, since the following
            // preedit string will be None anyway.
            if self.state.ime_state != ImeState::Commited {
                self.state.ime_state = ImeState::Preedit;
            }

            // Empty string basically means that there's no preedit, so indicate that by sending
            // `None` cursor range.
            let cursor_range = if preedit_string.is_empty() {
                None
            } else {
                Some((preedit_string.len(), preedit_string.len()))
            };

            // Send WindowEvent for updating marked text
            self.queue_event(WindowEvent::Ime(Ime::Preedit(preedit_string, cursor_range)));
        }

        #[sel(unmarkText)]
        fn unmark_text(&mut self) {
            trace_scope!("unmarkText");
            *self.marked_text = NSMutableAttributedString::new();

            let input_context = self.inputContext().expect("input context");
            input_context.discardMarkedText();

            self.queue_event(WindowEvent::Ime(Ime::Preedit(String::new(), None)));
            if self.is_ime_enabled() {
                // Leave the Preedit self.state
                self.state.ime_state = ImeState::Enabled;
            } else {
                warn!("Expected to have IME enabled when receiving unmarkText");
            }
        }

        #[sel(validAttributesForMarkedText)]
        fn valid_attributes_for_marked_text(&self) -> *const NSArray<NSAttributedStringKey> {
            trace_scope!("validAttributesForMarkedText");
            Id::autorelease_return(NSArray::new())
        }

        #[sel(attributedSubstringForProposedRange:actualRange:)]
        fn attributed_substring_for_proposed_range(
            &self,
            _range: NSRange,
            _actual_range: *mut c_void, // *mut NSRange
        ) -> *const NSAttributedString {
            trace_scope!("attributedSubstringForProposedRange:actualRange:");
            ptr::null()
        }

        #[sel(characterIndexForPoint:)]
        fn character_index_for_point(&self, _point: NSPoint) -> NSUInteger {
            trace_scope!("characterIndexForPoint:");
            0
        }

        #[sel(firstRectForCharacterRange:actualRange:)]
        fn first_rect_for_character_range(
            &self,
            _range: NSRange,
            _actual_range: *mut c_void, // *mut NSRange
        ) -> NSRect {
            trace_scope!("firstRectForCharacterRange:actualRange:");
            let window = self.window();
            let content_rect = window.contentRectForFrameRect(window.frame());
            let base_x = content_rect.origin.x as f64;
            let base_y = (content_rect.origin.y + content_rect.size.height) as f64;
            let x = base_x + self.state.ime_position.x;
            let y = base_y - self.state.ime_position.y;
            // This is not ideal: We _should_ return a different position based on
            // the currently selected character (which varies depending on the type
            // and size of the character), but in the current `winit` API there is
            // no way to express this. Same goes for the `NSSize`.
            NSRect::new(NSPoint::new(x as _, y as _), NSSize::new(0.0, 0.0))
        }

        #[sel(insertText:replacementRange:)]
        fn insert_text(&mut self, string: &NSObject, _replacement_range: NSRange) {
            trace_scope!("insertText:replacementRange:");

            // SAFETY: This method is guaranteed to get either a `NSString` or a `NSAttributedString`.
            let string = if string.is_kind_of::<NSAttributedString>() {
                let string: *const NSObject = string;
                let string: *const NSAttributedString = string.cast();
                unsafe { &*string }.string().to_string()
            } else {
                let string: *const NSObject = string;
                let string: *const NSString = string.cast();
                unsafe { &*string }.to_string()
            };

            let is_control = string.chars().next().map_or(false, |c| c.is_control());

            // Commit only if we have marked text.
            if self.hasMarkedText() && self.is_ime_enabled() && !is_control {
                self.queue_event(WindowEvent::Ime(Ime::Preedit(String::new(), None)));
                self.queue_event(WindowEvent::Ime(Ime::Commit(string)));
                self.state.ime_state = ImeState::Commited;
            }
        }

        // Basically, we're sent this message whenever a keyboard event that doesn't generate a "human
        // readable" character happens, i.e. newlines, tabs, and Ctrl+C.
        #[sel(doCommandBySelector:)]
        fn do_command_by_selector(&mut self, _command: Sel) {
            trace_scope!("doCommandBySelector:");
            // We shouldn't forward any character from just commited text, since we'll end up sending
            // it twice with some IMEs like Korean one. We'll also always send `Enter` in that case,
            // which is not desired given it was used to confirm IME input.
            if self.state.ime_state == ImeState::Commited {
                return;
            }

            self.state.forward_key_to_app = true;

            if self.hasMarkedText() && self.state.ime_state == ImeState::Preedit {
                // Leave preedit so that we also report the keyup for this key
                self.state.ime_state = ImeState::Enabled;
            }
        }
    }

    unsafe impl WinitView {
        #[sel(keyDown:)]
        fn key_down(&mut self, event: &NSEvent) {
            trace_scope!("keyDown:");
            let input_source = self.current_input_source();
            if self.state.input_source != input_source && self.is_ime_enabled() {
                self.state.ime_state = ImeState::Disabled;
                self.state.input_source = input_source;
                self.queue_event(WindowEvent::Ime(Ime::Disabled));
            }
            let was_in_preedit = self.state.ime_state == ImeState::Preedit;

            // Get the characters from the event.
            let ev_mods = event_mods(event);
            let ignore_alt_characters = match self.window().option_as_alt() {
                OptionAsAlt::OnlyLeft if event.lalt_pressed() => true,
                OptionAsAlt::OnlyRight if event.ralt_pressed() => true,
                OptionAsAlt::Both if ev_mods.alt() => true,
                _ => false,
            } && !ev_mods.ctrl()
                && !ev_mods.logo();

            let characters = get_characters(event, ignore_alt_characters);
            self.state.forward_key_to_app = false;

            // The `interpretKeyEvents` function might call
            // `setMarkedText`, `insertText`, and `doCommandBySelector`.
            // It's important that we call this before queuing the KeyboardInput, because
            // we must send the `KeyboardInput` event during IME if it triggered
            // `doCommandBySelector`. (doCommandBySelector means that the keyboard input
            // is not handled by IME and should be handled by the application)
            let mut text_commited = false;
            if self.state.ime_allowed {
                let new_event = if ignore_alt_characters {
                    replace_event_chars(event, &characters)
                } else {
                    event.copy()
                };

                let events_for_nsview = NSArray::from_slice(&[new_event]);
                unsafe { self.interpretKeyEvents(&events_for_nsview) };

                // If the text was commited we must treat the next keyboard event as IME related.
                if self.state.ime_state == ImeState::Commited {
                    // Remove any marked text, so normal input can continue.
                    *self.marked_text = NSMutableAttributedString::new();
                    self.state.ime_state = ImeState::Enabled;
                    text_commited = true;
                }
            }

            let now_in_preedit = self.state.ime_state == ImeState::Preedit;

            let scancode = event.scancode() as u32;
            let virtual_keycode = retrieve_keycode(event);

            self.update_potentially_stale_modifiers(event);

            let ime_related = was_in_preedit || now_in_preedit || text_commited;

            if !ime_related || self.state.forward_key_to_app || !self.state.ime_allowed {
                #[allow(deprecated)]
                self.queue_event(WindowEvent::KeyboardInput {
                    device_id: DEVICE_ID,
                    input: KeyboardInput {
                        state: ElementState::Pressed,
                        scancode,
                        virtual_keycode,
                        modifiers: ev_mods,
                    },
                    is_synthetic: false,
                });

                for character in characters.chars().filter(|c| !is_corporate_character(*c)) {
                    self.queue_event(WindowEvent::ReceivedCharacter(character));
                }
            }
        }

        #[sel(keyUp:)]
        fn key_up(&mut self, event: &NSEvent) {
            trace_scope!("keyUp:");
            let scancode = event.scancode() as u32;
            let virtual_keycode = retrieve_keycode(event);

            self.update_potentially_stale_modifiers(event);

            // We want to send keyboard input when we are not currently in preedit
            if self.state.ime_state != ImeState::Preedit {
                #[allow(deprecated)]
                self.queue_event(WindowEvent::KeyboardInput {
                    device_id: DEVICE_ID,
                    input: KeyboardInput {
                        state: ElementState::Released,
                        scancode,
                        virtual_keycode,
                        modifiers: event_mods(event),
                    },
                    is_synthetic: false,
                });
            }
        }

        #[sel(flagsChanged:)]
        fn flags_changed(&mut self, event: &NSEvent) {
            trace_scope!("flagsChanged:");

            if let Some(window_event) = modifier_event(
                event,
                NSEventModifierFlags::NSShiftKeyMask,
                self.state.modifiers.shift(),
            ) {
                self.state.modifiers.toggle(ModifiersState::SHIFT);
                self.queue_event(window_event);
            }

            if let Some(window_event) = modifier_event(
                event,
                NSEventModifierFlags::NSControlKeyMask,
                self.state.modifiers.ctrl(),
            ) {
                self.state.modifiers.toggle(ModifiersState::CTRL);
                self.queue_event(window_event);
            }

            if let Some(window_event) = modifier_event(
                event,
                NSEventModifierFlags::NSCommandKeyMask,
                self.state.modifiers.logo(),
            ) {
                self.state.modifiers.toggle(ModifiersState::LOGO);
                self.queue_event(window_event);
            }

            if let Some(window_event) = modifier_event(
                event,
                NSEventModifierFlags::NSAlternateKeyMask,
                self.state.modifiers.alt(),
            ) {
                self.state.modifiers.toggle(ModifiersState::ALT);
                self.queue_event(window_event);
            }

            self.queue_event(WindowEvent::ModifiersChanged(self.state.modifiers));
        }

        #[sel(insertTab:)]
        fn insert_tab(&self, _sender: *const Object) {
            trace_scope!("insertTab:");
            let window = self.window();
            if let Some(first_responder) = window.firstResponder() {
                if *first_responder == ***self {
                    window.selectNextKeyView(Some(self))
                }
            }
        }

        #[sel(insertBackTab:)]
        fn insert_back_tab(&self, _sender: *const Object) {
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
        #[sel(cancelOperation:)]
        fn cancel_operation(&mut self, _sender: *const Object) {
            trace_scope!("cancelOperation:");
            let scancode = 0x2f;
            let virtual_keycode = scancode_to_keycode(scancode);
            debug_assert_eq!(virtual_keycode, Some(VirtualKeyCode::Period));

            let event = NSApp()
                .currentEvent()
                .expect("could not find current event");

            self.update_potentially_stale_modifiers(&event);

            #[allow(deprecated)]
            self.queue_event(WindowEvent::KeyboardInput {
                device_id: DEVICE_ID,
                input: KeyboardInput {
                    state: ElementState::Pressed,
                    scancode: scancode as _,
                    virtual_keycode,
                    modifiers: event_mods(&event),
                },
                is_synthetic: false,
            });
        }

        #[sel(mouseDown:)]
        fn mouse_down(&mut self, event: &NSEvent) {
            trace_scope!("mouseDown:");
            self.mouse_motion(event);
            self.mouse_click(event, ElementState::Pressed);
        }

        #[sel(mouseUp:)]
        fn mouse_up(&mut self, event: &NSEvent) {
            trace_scope!("mouseUp:");
            self.mouse_motion(event);
            self.mouse_click(event, ElementState::Released);
        }

        #[sel(rightMouseDown:)]
        fn right_mouse_down(&mut self, event: &NSEvent) {
            trace_scope!("rightMouseDown:");
            self.mouse_motion(event);
            self.mouse_click(event, ElementState::Pressed);
        }

        #[sel(rightMouseUp:)]
        fn right_mouse_up(&mut self, event: &NSEvent) {
            trace_scope!("rightMouseUp:");
            self.mouse_motion(event);
            self.mouse_click(event, ElementState::Released);
        }

        #[sel(otherMouseDown:)]
        fn other_mouse_down(&mut self, event: &NSEvent) {
            trace_scope!("otherMouseDown:");
            self.mouse_motion(event);
            self.mouse_click(event, ElementState::Pressed);
        }

        #[sel(otherMouseUp:)]
        fn other_mouse_up(&mut self, event: &NSEvent) {
            trace_scope!("otherMouseUp:");
            self.mouse_motion(event);
            self.mouse_click(event, ElementState::Released);
        }

        // No tracing on these because that would be overly verbose

        #[sel(mouseMoved:)]
        fn mouse_moved(&mut self, event: &NSEvent) {
            self.mouse_motion(event);
        }

        #[sel(mouseDragged:)]
        fn mouse_dragged(&mut self, event: &NSEvent) {
            self.mouse_motion(event);
        }

        #[sel(rightMouseDragged:)]
        fn right_mouse_dragged(&mut self, event: &NSEvent) {
            self.mouse_motion(event);
        }

        #[sel(otherMouseDragged:)]
        fn other_mouse_dragged(&mut self, event: &NSEvent) {
            self.mouse_motion(event);
        }

        #[sel(mouseEntered:)]
        fn mouse_entered(&self, _event: &NSEvent) {
            trace_scope!("mouseEntered:");
            self.queue_event(WindowEvent::CursorEntered {
                device_id: DEVICE_ID,
            });
        }

        #[sel(mouseExited:)]
        fn mouse_exited(&self, _event: &NSEvent) {
            trace_scope!("mouseExited:");

            self.queue_event(WindowEvent::CursorLeft {
                device_id: DEVICE_ID,
            });
        }

        #[sel(scrollWheel:)]
        fn scroll_wheel(&mut self, event: &NSEvent) {
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
            // be mutually exclusive anyhow, which is why the API is rather incoherent). If no momentum
            // phase is recorded (or rather, the started/ended cases of the momentum phase) then we
            // report the touch phase.
            let phase = match event.momentumPhase() {
                NSEventPhase::NSEventPhaseMayBegin | NSEventPhase::NSEventPhaseBegan => {
                    TouchPhase::Started
                }
                NSEventPhase::NSEventPhaseEnded | NSEventPhase::NSEventPhaseCancelled => {
                    TouchPhase::Ended
                }
                _ => match event.phase() {
                    NSEventPhase::NSEventPhaseMayBegin | NSEventPhase::NSEventPhaseBegan => {
                        TouchPhase::Started
                    }
                    NSEventPhase::NSEventPhaseEnded | NSEventPhase::NSEventPhaseCancelled => {
                        TouchPhase::Ended
                    }
                    _ => TouchPhase::Moved,
                },
            };

            self.update_potentially_stale_modifiers(event);

            self.queue_device_event(DeviceEvent::MouseWheel { delta });
            self.queue_event(WindowEvent::MouseWheel {
                device_id: DEVICE_ID,
                delta,
                phase,
                modifiers: event_mods(event),
            });
        }

        #[sel(magnifyWithEvent:)]
        fn magnify_with_event(&self, event: &NSEvent) {
            trace_scope!("magnifyWithEvent:");

            let phase = match event.phase() {
                NSEventPhase::NSEventPhaseBegan => TouchPhase::Started,
                NSEventPhase::NSEventPhaseChanged => TouchPhase::Moved,
                NSEventPhase::NSEventPhaseCancelled => TouchPhase::Cancelled,
                NSEventPhase::NSEventPhaseEnded => TouchPhase::Ended,
                _ => return,
            };

            self.queue_event(WindowEvent::TouchpadMagnify {
                device_id: DEVICE_ID,
                delta: event.magnification(),
                phase,
            });
        }

        #[sel(smartMagnifyWithEvent:)]
        fn smart_magnify_with_event(&self, _event: &NSEvent) {
            trace_scope!("smartMagnifyWithEvent:");

            self.queue_event(WindowEvent::SmartMagnify {
                device_id: DEVICE_ID,
            });
        }

        #[sel(rotateWithEvent:)]
        fn rotate_with_event(&self, event: &NSEvent) {
            trace_scope!("rotateWithEvent:");

            let phase = match event.phase() {
                NSEventPhase::NSEventPhaseBegan => TouchPhase::Started,
                NSEventPhase::NSEventPhaseChanged => TouchPhase::Moved,
                NSEventPhase::NSEventPhaseCancelled => TouchPhase::Cancelled,
                NSEventPhase::NSEventPhaseEnded => TouchPhase::Ended,
                _ => return,
            };

            self.queue_event(WindowEvent::TouchpadRotate {
                device_id: DEVICE_ID,
                delta: event.rotation(),
                phase,
            });
        }

        #[sel(pressureChangeWithEvent:)]
        fn pressure_change_with_event(&mut self, event: &NSEvent) {
            trace_scope!("pressureChangeWithEvent:");

            self.mouse_motion(event);

            self.queue_event(WindowEvent::TouchpadPressure {
                device_id: DEVICE_ID,
                pressure: event.pressure(),
                stage: event.stage() as i64,
            });
        }

        // Allows us to receive Ctrl-Tab and Ctrl-Esc.
        // Note that this *doesn't* help with any missing Cmd inputs.
        // https://github.com/chromium/chromium/blob/a86a8a6bcfa438fa3ac2eba6f02b3ad1f8e0756f/ui/views/cocoa/bridged_content_view.mm#L816
        #[sel(_wantsKeyDownForEvent:)]
        fn wants_key_down_for_event(&self, _event: &NSEvent) -> bool {
            trace_scope!("_wantsKeyDownForEvent:");
            true
        }

        #[sel(acceptsFirstMouse:)]
        fn accepts_first_mouse(&self, _event: &NSEvent) -> bool {
            trace_scope!("acceptsFirstMouse:");
            *self.accepts_first_mouse
        }
    }
);

impl WinitView {
    pub(super) fn new(window: &WinitWindow, accepts_first_mouse: bool) -> Id<Self, Shared> {
        unsafe {
            msg_send_id![
                msg_send_id![Self::class(), alloc],
                initWithId: window,
                acceptsFirstMouse: accepts_first_mouse,
            ]
        }
    }

    fn window(&self) -> Id<WinitWindow, Shared> {
        // TODO: Simply use `window` property on `NSView`.
        // That only returns a window _after_ the view has been attached though!
        // (which is incompatible with `frameDidChange:`)
        //
        // unsafe { msg_send_id![self, window] }
        self._ns_window.load().expect("view to have a window")
    }

    fn window_id(&self) -> WindowId {
        WindowId(self.window().id())
    }

    fn queue_event(&self, event: WindowEvent<'static>) {
        let event = Event::WindowEvent {
            window_id: self.window_id(),
            event,
        };
        AppState::queue_event(EventWrapper::StaticEvent(event));
    }

    fn queue_device_event(&self, event: DeviceEvent) {
        let event = Event::DeviceEvent {
            device_id: DEVICE_ID,
            event,
        };
        AppState::queue_event(EventWrapper::StaticEvent(event));
    }

    fn scale_factor(&self) -> f64 {
        self.window().backingScaleFactor() as f64
    }

    fn is_ime_enabled(&self) -> bool {
        !matches!(self.state.ime_state, ImeState::Disabled)
    }

    fn current_input_source(&self) -> String {
        self.inputContext()
            .expect("input context")
            .selectedKeyboardInputSource()
            .map(|input_source| input_source.to_string())
            .unwrap_or_else(String::new)
    }

    pub(super) fn set_ime_allowed(&mut self, ime_allowed: bool) {
        if self.state.ime_allowed == ime_allowed {
            return;
        }
        self.state.ime_allowed = ime_allowed;
        if self.state.ime_allowed {
            return;
        }

        // Clear markedText
        *self.marked_text = NSMutableAttributedString::new();

        if self.state.ime_state != ImeState::Disabled {
            self.state.ime_state = ImeState::Disabled;
            self.queue_event(WindowEvent::Ime(Ime::Disabled));
        }
    }

    pub(super) fn set_ime_position(&mut self, position: LogicalPosition<f64>) {
        self.state.ime_position = position;
        let input_context = self.inputContext().expect("input context");
        input_context.invalidateCharacterCoordinates();
    }

    // Update `state.modifiers` if `event` has something different
    fn update_potentially_stale_modifiers(&mut self, event: &NSEvent) {
        let event_modifiers = event_mods(event);
        if self.state.modifiers != event_modifiers {
            self.state.modifiers = event_modifiers;

            self.queue_event(WindowEvent::ModifiersChanged(self.state.modifiers));
        }
    }

    fn mouse_click(&mut self, event: &NSEvent, button_state: ElementState) {
        let button = mouse_button(event);

        self.update_potentially_stale_modifiers(event);

        self.queue_event(WindowEvent::MouseInput {
            device_id: DEVICE_ID,
            state: button_state,
            button,
            modifiers: event_mods(event),
        });
    }

    fn mouse_motion(&mut self, event: &NSEvent) {
        let window_point = event.locationInWindow();
        let view_point = self.convertPoint_fromView(window_point, None);
        let view_rect = self.frame();

        if view_point.x.is_sign_negative()
            || view_point.y.is_sign_negative()
            || view_point.x > view_rect.size.width
            || view_point.y > view_rect.size.height
        {
            let mouse_buttons_down = NSEvent::pressedMouseButtons();
            if mouse_buttons_down == 0 {
                // Point is outside of the client area (view) and no buttons are pressed
                return;
            }
        }

        let x = view_point.x as f64;
        let y = view_rect.size.height as f64 - view_point.y as f64;
        let logical_position = LogicalPosition::new(x, y);

        self.update_potentially_stale_modifiers(event);

        self.queue_event(WindowEvent::CursorMoved {
            device_id: DEVICE_ID,
            position: logical_position.to_physical(self.scale_factor()),
            modifiers: event_mods(event),
        });
    }
}

/// Get the mouse button from the NSEvent.
fn mouse_button(event: &NSEvent) -> MouseButton {
    // The buttonNumber property only makes sense for the mouse events:
    // NSLeftMouse.../NSRightMouse.../NSOtherMouse...
    // For the other events, it's always set to 0.
    match event.buttonNumber() {
        0 => MouseButton::Left,
        1 => MouseButton::Right,
        2 => MouseButton::Middle,
        n => MouseButton::Other(n as u16),
    }
}

fn replace_event_chars(event: &NSEvent, characters: &str) -> Id<NSEvent, Shared> {
    let ns_chars = NSString::from_str(characters);
    let chars_ignoring_mods = event.charactersIgnoringModifiers().unwrap();

    NSEvent::keyEventWithType(
        event.type_(),
        event.locationInWindow(),
        event.modifierFlags(),
        event.timestamp(),
        event.window_number(),
        None,
        &ns_chars,
        &chars_ignoring_mods,
        event.is_a_repeat(),
        event.scancode(),
    )
}
