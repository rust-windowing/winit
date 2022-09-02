use std::{
    boxed::Box,
    collections::VecDeque,
    os::raw::*,
    ptr, slice, str,
    sync::{
        atomic::{compiler_fence, Ordering},
        Mutex,
    },
};

use cocoa::{
    appkit::{NSApp, NSEvent, NSEventModifierFlags, NSEventPhase, NSView, NSWindow},
    base::{id, nil},
    foundation::{NSPoint, NSRect, NSSize, NSString},
};
use objc2::foundation::{NSInteger, NSObject, NSRange, NSUInteger};
use objc2::rc::{Id, Shared};
use objc2::runtime::{Bool, Object, Sel};
use objc2::{declare_class, ClassType};

use super::appkit::{NSCursor, NSResponder, NSView as NSViewClass};
use crate::{
    dpi::{LogicalPosition, LogicalSize},
    event::{
        DeviceEvent, ElementState, Event, Ime, KeyboardInput, ModifiersState, MouseButton,
        MouseScrollDelta, TouchPhase, VirtualKeyCode, WindowEvent,
    },
    platform_impl::platform::{
        app_state::AppState,
        event::{
            char_to_keycode, check_function_keys, event_mods, get_scancode, modifier_event,
            scancode_to_keycode, EventWrapper,
        },
        ffi::*,
        util::{self, id_to_string_lossy, IdRef},
        window::get_window_id,
        DEVICE_ID,
    },
    window::WindowId,
};

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

pub(super) struct ViewState {
    ns_window: id,
    pub cursor_state: Mutex<CursorState>,
    ime_position: LogicalPosition<f64>,
    pub(super) modifiers: ModifiersState,
    tracking_rect: Option<NSInteger>,
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

impl ViewState {
    fn get_scale_factor(&self) -> f64 {
        (unsafe { NSWindow::backingScaleFactor(self.ns_window) }) as f64
    }

    fn is_ime_enabled(&self) -> bool {
        !matches!(self.ime_state, ImeState::Disabled)
    }
}

pub fn new_view(ns_window: id) -> IdRef {
    let state = ViewState {
        ns_window,
        cursor_state: Default::default(),
        ime_position: LogicalPosition::new(0.0, 0.0),
        modifiers: Default::default(),
        tracking_rect: None,
        ime_state: ImeState::Disabled,
        input_source: String::new(),
        ime_allowed: false,
        forward_key_to_app: false,
    };
    unsafe {
        // This is free'd in `dealloc`
        let state_ptr = Box::into_raw(Box::new(state)) as *mut c_void;
        let ns_view: id = msg_send![WinitView::class(), alloc];
        IdRef::new(msg_send![ns_view, initWithWinit: state_ptr])
    }
}

pub unsafe fn set_ime_position(ns_view: id, position: LogicalPosition<f64>) {
    let state_ptr: *mut c_void = *(*ns_view).ivar_mut("winitState");
    let state = &mut *(state_ptr as *mut ViewState);
    state.ime_position = position;
    let input_context: id = msg_send![ns_view, inputContext];
    let _: () = msg_send![input_context, invalidateCharacterCoordinates];
}

pub unsafe fn set_ime_allowed(ns_view: id, ime_allowed: bool) {
    let state_ptr: *mut c_void = *(*ns_view).ivar_mut("winitState");
    let state = &mut *(state_ptr as *mut ViewState);
    if state.ime_allowed == ime_allowed {
        return;
    }
    state.ime_allowed = ime_allowed;
    if state.ime_allowed {
        return;
    }
    let marked_text_ref: &mut id = (*ns_view).ivar_mut("markedText");

    // Clear markedText
    let _: () = msg_send![*marked_text_ref, release];
    let marked_text =
        <id as NSMutableAttributedString>::init(NSMutableAttributedString::alloc(nil));
    *marked_text_ref = marked_text;

    if state.ime_state != ImeState::Disabled {
        state.ime_state = ImeState::Disabled;
        AppState::queue_event(EventWrapper::StaticEvent(Event::WindowEvent {
            window_id: WindowId(get_window_id(state.ns_window)),
            event: WindowEvent::Ime(Ime::Disabled),
        }));
    }
}

fn get_characters(event: id, ignore_modifiers: bool) -> String {
    unsafe {
        let characters: id = if ignore_modifiers {
            msg_send![event, charactersIgnoringModifiers]
        } else {
            msg_send![event, characters]
        };

        assert_ne!(characters, nil);
        let slice =
            slice::from_raw_parts(characters.UTF8String() as *const c_uchar, characters.len());

        let string = str::from_utf8_unchecked(slice);
        string.to_owned()
    }
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
fn retrieve_keycode(event: id) -> Option<VirtualKeyCode> {
    #[inline]
    fn get_code(ev: id, raw: bool) -> Option<VirtualKeyCode> {
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
        let scancode = get_scancode(event);
        scancode_to_keycode(scancode).or_else(|| check_function_keys(&get_characters(event, true)))
    })
}

// Update `state.modifiers` if `event` has something different
fn update_potentially_stale_modifiers(state: &mut ViewState, event: id) {
    let event_modifiers = event_mods(event);
    if state.modifiers != event_modifiers {
        state.modifiers = event_modifiers;

        AppState::queue_event(EventWrapper::StaticEvent(Event::WindowEvent {
            window_id: WindowId(get_window_id(state.ns_window)),
            event: WindowEvent::ModifiersChanged(state.modifiers),
        }));
    }
}

fn mouse_click(this: &Object, event: id, button: MouseButton, button_state: ElementState) {
    unsafe {
        let state_ptr: *mut c_void = *this.ivar("winitState");
        let state = &mut *(state_ptr as *mut ViewState);

        update_potentially_stale_modifiers(state, event);

        let window_event = Event::WindowEvent {
            window_id: WindowId(get_window_id(state.ns_window)),
            event: WindowEvent::MouseInput {
                device_id: DEVICE_ID,
                state: button_state,
                button,
                modifiers: event_mods(event),
            },
        };

        AppState::queue_event(EventWrapper::StaticEvent(window_event));
    }
}

fn mouse_motion(this: &Object, event: id) {
    unsafe {
        let state_ptr: *mut c_void = *this.ivar("winitState");
        let state = &mut *(state_ptr as *mut ViewState);

        // We have to do this to have access to the `NSView` trait...
        let view: id = this as *const _ as *mut _;

        let window_point = event.locationInWindow();
        let view_point = view.convertPoint_fromView_(window_point, nil);
        let view_rect = NSView::frame(view);

        if view_point.x.is_sign_negative()
            || view_point.y.is_sign_negative()
            || view_point.x > view_rect.size.width
            || view_point.y > view_rect.size.height
        {
            let mouse_buttons_down: NSUInteger = msg_send![class!(NSEvent), pressedMouseButtons];
            if mouse_buttons_down == 0 {
                // Point is outside of the client area (view) and no buttons are pressed
                return;
            }
        }

        let x = view_point.x as f64;
        let y = view_rect.size.height as f64 - view_point.y as f64;
        let logical_position = LogicalPosition::new(x, y);

        update_potentially_stale_modifiers(state, event);

        let window_event = Event::WindowEvent {
            window_id: WindowId(get_window_id(state.ns_window)),
            event: WindowEvent::CursorMoved {
                device_id: DEVICE_ID,
                position: logical_position.to_physical(state.get_scale_factor()),
                modifiers: event_mods(event),
            },
        };

        AppState::queue_event(EventWrapper::StaticEvent(window_event));
    }
}

declare_class!(
    #[derive(Debug)]
    #[allow(non_snake_case)]
    struct WinitView {
        winitState: *mut c_void,
        markedText: id,
    }

    unsafe impl ClassType for WinitView {
        #[inherits(NSResponder, NSObject)]
        type Super = NSViewClass;
    }

    unsafe impl WinitView {
        #[sel(dealloc)]
        fn dealloc(&mut self) {
            unsafe {
                let marked_text: id = *self.ivar("markedText");
                let _: () = msg_send![marked_text, release];
                let state: *mut c_void = *self.ivar("winitState");
                drop(Box::from_raw(state as *mut ViewState));
            }
        }

        #[sel(initWithWinit:)]
        fn init_with_winit(&mut self, state: *mut c_void) -> Option<&mut Self> {
            let this: Option<&mut Self> = unsafe { msg_send![self, init] };
            this.map(|this| unsafe {
                (*this).set_ivar("winitState", state);
                let marked_text =
                    <id as NSMutableAttributedString>::init(NSMutableAttributedString::alloc(nil));
                (*this).set_ivar("markedText", marked_text);
                let _: () = msg_send![&mut *this, setPostsFrameChangedNotifications: true];

                let notification_center: &Object =
                    msg_send![class!(NSNotificationCenter), defaultCenter];
                // About frame change
                let frame_did_change_notification_name =
                    IdRef::new(NSString::alloc(nil).init_str("NSViewFrameDidChangeNotification"));
                let _: () = msg_send![
                    notification_center,
                    addObserver: &*this
                    selector: sel!(frameDidChange:)
                    name: *frame_did_change_notification_name
                    object: &*this
                ];

                let winit_state = &mut *(state as *mut ViewState);
                winit_state.input_source = this.current_input_source();
                this
            })
        }
    }

    unsafe impl WinitView {
        #[sel(viewDidMoveToWindow)]
        fn view_did_move_to_window(&self) {
            trace_scope!("viewDidMoveToWindow");
            unsafe {
                let state_ptr: *mut c_void = *self.ivar("winitState");
                let state = &mut *(state_ptr as *mut ViewState);

                if let Some(tracking_rect) = state.tracking_rect.take() {
                    let _: () = msg_send![self, removeTrackingRect: tracking_rect];
                }

                let rect: NSRect = msg_send![self, visibleRect];
                let tracking_rect: NSInteger = msg_send![
                    self,
                    addTrackingRect: rect,
                    owner: self,
                    userData: ptr::null_mut::<c_void>(),
                    assumeInside: false,
                ];
                state.tracking_rect = Some(tracking_rect);
            }
        }

        #[sel(frameDidChange:)]
        fn frame_did_change(&self, _event: id) {
            trace_scope!("frameDidChange:");
            unsafe {
                let state_ptr: *mut c_void = *self.ivar("winitState");
                let state = &mut *(state_ptr as *mut ViewState);

                if let Some(tracking_rect) = state.tracking_rect.take() {
                    let _: () = msg_send![self, removeTrackingRect: tracking_rect];
                }

                let rect: NSRect = msg_send![self, visibleRect];
                let tracking_rect: NSInteger = msg_send![
                    self,
                    addTrackingRect: rect,
                    owner: self,
                    userData: ptr::null_mut::<c_void>(),
                    assumeInside: false,
                ];
                state.tracking_rect = Some(tracking_rect);

                // Emit resize event here rather than from windowDidResize because:
                // 1. When a new window is created as a tab, the frame size may change without a window resize occurring.
                // 2. Even when a window resize does occur on a new tabbed window, it contains the wrong size (includes tab height).
                let logical_size =
                    LogicalSize::new(rect.size.width as f64, rect.size.height as f64);
                let size = logical_size.to_physical::<u32>(state.get_scale_factor());
                AppState::queue_event(EventWrapper::StaticEvent(Event::WindowEvent {
                    window_id: WindowId(get_window_id(state.ns_window)),
                    event: WindowEvent::Resized(size),
                }));
            }
        }

        #[sel(drawRect:)]
        fn draw_rect(&self, rect: NSRect) {
            trace_scope!("drawRect:");
            unsafe {
                let state_ptr: *mut c_void = *self.ivar("winitState");
                let state = &mut *(state_ptr as *mut ViewState);

                AppState::handle_redraw(WindowId(get_window_id(state.ns_window)));

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
            let state = unsafe {
                let state_ptr: *mut c_void = *self.ivar("winitState");
                &mut *(state_ptr as *mut ViewState)
            };

            let bounds = self.bounds();
            let cursor_state = state.cursor_state.lock().unwrap();
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
            unsafe {
                let marked_text: id = *self.ivar("markedText");
                marked_text.length() > 0
            }
        }

        #[sel(markedRange)]
        fn marked_range(&self) -> NSRange {
            trace_scope!("markedRange");
            unsafe {
                let marked_text: id = *self.ivar("markedText");
                let length = marked_text.length();
                if length > 0 {
                    NSRange::new(0, length)
                } else {
                    util::EMPTY_RANGE
                }
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
            string: id,
            _selected_range: NSRange,
            _replacement_range: NSRange,
        ) {
            trace_scope!("setMarkedText:selectedRange:replacementRange:");
            unsafe {
                // Get pre-edit text
                let marked_text_ref: &mut id = self.ivar_mut("markedText");

                // Update markedText
                let _: () = msg_send![*marked_text_ref, release];
                let marked_text = NSMutableAttributedString::alloc(nil);
                let has_attr = msg_send![string, isKindOfClass: class!(NSAttributedString)];
                if has_attr {
                    marked_text.initWithAttributedString(string);
                } else {
                    marked_text.initWithString(string);
                };
                *marked_text_ref = marked_text;

                // Update ViewState with new marked text
                let state_ptr: *mut c_void = *self.ivar("winitState");
                let state = &mut *(state_ptr as *mut ViewState);
                let preedit_string = id_to_string_lossy(string);

                // Notify IME is active if application still doesn't know it.
                if state.ime_state == ImeState::Disabled {
                    state.input_source = self.current_input_source();
                    AppState::queue_event(EventWrapper::StaticEvent(Event::WindowEvent {
                        window_id: WindowId(get_window_id(state.ns_window)),
                        event: WindowEvent::Ime(Ime::Enabled),
                    }));
                }

                // Don't update state to preedit when we've just commited a string, since the following
                // preedit string will be None anyway.
                if state.ime_state != ImeState::Commited {
                    state.ime_state = ImeState::Preedit;
                }

                // Empty string basically means that there's no preedit, so indicate that by sending
                // `None` cursor range.
                let cursor_range = if preedit_string.is_empty() {
                    None
                } else {
                    Some((preedit_string.len(), preedit_string.len()))
                };

                // Send WindowEvent for updating marked text
                AppState::queue_event(EventWrapper::StaticEvent(Event::WindowEvent {
                    window_id: WindowId(get_window_id(state.ns_window)),
                    event: WindowEvent::Ime(Ime::Preedit(preedit_string, cursor_range)),
                }));
            }
        }

        #[sel(unmarkText)]
        fn unmark_text(&self) {
            trace_scope!("unmarkText");
            unsafe {
                let marked_text: id = *self.ivar("markedText");
                let mutable_string = marked_text.mutableString();
                let s: id = msg_send![class!(NSString), new];
                let _: () = msg_send![mutable_string, setString: s];
                let _: () = msg_send![s, release];
                let input_context: &Object = msg_send![self, inputContext];
                let _: () = msg_send![input_context, discardMarkedText];

                let state_ptr: *mut c_void = *self.ivar("winitState");
                let state = &mut *(state_ptr as *mut ViewState);
                AppState::queue_event(EventWrapper::StaticEvent(Event::WindowEvent {
                    window_id: WindowId(get_window_id(state.ns_window)),
                    event: WindowEvent::Ime(Ime::Preedit(String::new(), None)),
                }));
                if state.is_ime_enabled() {
                    // Leave the Preedit state
                    state.ime_state = ImeState::Enabled;
                } else {
                    warn!("Expected to have IME enabled when receiving unmarkText");
                }
            }
        }

        #[sel(validAttributesForMarkedText)]
        fn valid_attributes_for_marked_text(&self) -> id {
            trace_scope!("validAttributesForMarkedText");
            unsafe { msg_send![class!(NSArray), array] }
        }

        #[sel(attributedSubstringForProposedRange:actualRange:)]
        fn attributed_substring_for_proposed_range(
            &self,
            _range: NSRange,
            _actual_range: *mut c_void, // *mut NSRange
        ) -> id {
            trace_scope!("attributedSubstringForProposedRange:actualRange:");
            nil
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
            unsafe {
                let state_ptr: *mut c_void = *self.ivar("winitState");
                let state = &mut *(state_ptr as *mut ViewState);
                let content_rect = NSWindow::contentRectForFrameRect_(
                    state.ns_window,
                    NSWindow::frame(state.ns_window),
                );
                let base_x = content_rect.origin.x as f64;
                let base_y = (content_rect.origin.y + content_rect.size.height) as f64;
                let x = base_x + state.ime_position.x;
                let y = base_y - state.ime_position.y;
                // This is not ideal: We _should_ return a different position based on
                // the currently selected character (which varies depending on the type
                // and size of the character), but in the current `winit` API there is
                // no way to express this. Same goes for the `NSSize`.
                NSRect::new(NSPoint::new(x as _, y as _), NSSize::new(0.0, 0.0))
            }
        }

        #[sel(insertText:replacementRange:)]
        fn insert_text(&self, string: id, _replacement_range: NSRange) {
            trace_scope!("insertText:replacementRange:");
            unsafe {
                let state_ptr: *mut c_void = *self.ivar("winitState");
                let state = &mut *(state_ptr as *mut ViewState);

                let string = id_to_string_lossy(string);

                let is_control = string.chars().next().map_or(false, |c| c.is_control());

                // We don't need this now, but it's here if that changes.
                //let event: id = msg_send![NSApp(), currentEvent];

                if state.is_ime_enabled() && !is_control {
                    AppState::queue_event(EventWrapper::StaticEvent(Event::WindowEvent {
                        window_id: WindowId(get_window_id(state.ns_window)),
                        event: WindowEvent::Ime(Ime::Commit(string)),
                    }));
                    state.ime_state = ImeState::Commited;
                }
            }
        }

        #[sel(doCommandBySelector:)]
        fn do_command_by_selector(&self, _command: Sel) {
            trace_scope!("doCommandBySelector:");
            // Basically, we're sent this message whenever a keyboard event that doesn't generate a "human
            // readable" character happens, i.e. newlines, tabs, and Ctrl+C.
            unsafe {
                let state_ptr: *mut c_void = *self.ivar("winitState");
                let state = &mut *(state_ptr as *mut ViewState);

                // We shouldn't forward any character from just commited text, since we'll end up sending
                // it twice with some IMEs like Korean one. We'll also always send `Enter` in that case,
                // which is not desired given it was used to confirm IME input.
                if state.ime_state == ImeState::Commited {
                    return;
                }

                state.forward_key_to_app = true;

                let has_marked_text = msg_send![self, hasMarkedText];
                if has_marked_text && state.ime_state == ImeState::Preedit {
                    // Leave preedit so that we also report the keyup for this key
                    state.ime_state = ImeState::Enabled;
                }
            }
        }
    }

    unsafe impl WinitView {
        #[sel(keyDown:)]
        fn key_down(&self, event: id) {
            trace_scope!("keyDown:");
            unsafe {
                let state_ptr: *mut c_void = *self.ivar("winitState");
                let state = &mut *(state_ptr as *mut ViewState);
                let window_id = WindowId(get_window_id(state.ns_window));

                let input_source = self.current_input_source();
                if state.input_source != input_source && state.is_ime_enabled() {
                    state.ime_state = ImeState::Disabled;
                    state.input_source = input_source;
                    AppState::queue_event(EventWrapper::StaticEvent(Event::WindowEvent {
                        window_id: WindowId(get_window_id(state.ns_window)),
                        event: WindowEvent::Ime(Ime::Disabled),
                    }));
                }
                let was_in_preedit = state.ime_state == ImeState::Preedit;

                let characters = get_characters(event, false);
                state.forward_key_to_app = false;

                // The `interpretKeyEvents` function might call
                // `setMarkedText`, `insertText`, and `doCommandBySelector`.
                // It's important that we call this before queuing the KeyboardInput, because
                // we must send the `KeyboardInput` event during IME if it triggered
                // `doCommandBySelector`. (doCommandBySelector means that the keyboard input
                // is not handled by IME and should be handled by the application)
                let mut text_commited = false;
                if state.ime_allowed {
                    let events_for_nsview: id = msg_send![class!(NSArray), arrayWithObject: event];
                    let _: () = msg_send![self, interpretKeyEvents: events_for_nsview];

                    // Using a compiler fence because `interpretKeyEvents` might call
                    // into functions that modify the `ViewState`, but the compiler
                    // doesn't know this. Without the fence, the compiler may think that
                    // some of the reads (eg `state.ime_state`) that happen after this
                    // point are not needed.
                    compiler_fence(Ordering::SeqCst);

                    // If the text was commited we must treat the next keyboard event as IME related.
                    if state.ime_state == ImeState::Commited {
                        state.ime_state = ImeState::Enabled;
                        text_commited = true;
                    }
                }

                let now_in_preedit = state.ime_state == ImeState::Preedit;

                let scancode = get_scancode(event) as u32;
                let virtual_keycode = retrieve_keycode(event);

                update_potentially_stale_modifiers(state, event);

                let ime_related = was_in_preedit || now_in_preedit || text_commited;

                if !ime_related || state.forward_key_to_app || !state.ime_allowed {
                    #[allow(deprecated)]
                    let window_event = Event::WindowEvent {
                        window_id,
                        event: WindowEvent::KeyboardInput {
                            device_id: DEVICE_ID,
                            input: KeyboardInput {
                                state: ElementState::Pressed,
                                scancode,
                                virtual_keycode,
                                modifiers: event_mods(event),
                            },
                            is_synthetic: false,
                        },
                    };

                    AppState::queue_event(EventWrapper::StaticEvent(window_event));

                    for character in characters.chars().filter(|c| !is_corporate_character(*c)) {
                        AppState::queue_event(EventWrapper::StaticEvent(Event::WindowEvent {
                            window_id,
                            event: WindowEvent::ReceivedCharacter(character),
                        }));
                    }
                }
            }
        }

        #[sel(keyUp:)]
        fn key_up(&self, event: id) {
            trace_scope!("keyUp:");
            unsafe {
                let state_ptr: *mut c_void = *self.ivar("winitState");
                let state = &mut *(state_ptr as *mut ViewState);

                let scancode = get_scancode(event) as u32;
                let virtual_keycode = retrieve_keycode(event);

                update_potentially_stale_modifiers(state, event);

                // We want to send keyboard input when we are not currently in preedit
                if state.ime_state != ImeState::Preedit {
                    #[allow(deprecated)]
                    let window_event = Event::WindowEvent {
                        window_id: WindowId(get_window_id(state.ns_window)),
                        event: WindowEvent::KeyboardInput {
                            device_id: DEVICE_ID,
                            input: KeyboardInput {
                                state: ElementState::Released,
                                scancode,
                                virtual_keycode,
                                modifiers: event_mods(event),
                            },
                            is_synthetic: false,
                        },
                    };

                    AppState::queue_event(EventWrapper::StaticEvent(window_event));
                }
            }
        }

        #[sel(flagsChanged:)]
        fn flags_changed(&self, event: id) {
            trace_scope!("flagsChanged:");
            unsafe {
                let state_ptr: *mut c_void = *self.ivar("winitState");
                let state = &mut *(state_ptr as *mut ViewState);

                let mut events = VecDeque::with_capacity(4);

                if let Some(window_event) = modifier_event(
                    event,
                    NSEventModifierFlags::NSShiftKeyMask,
                    state.modifiers.shift(),
                ) {
                    state.modifiers.toggle(ModifiersState::SHIFT);
                    events.push_back(window_event);
                }

                if let Some(window_event) = modifier_event(
                    event,
                    NSEventModifierFlags::NSControlKeyMask,
                    state.modifiers.ctrl(),
                ) {
                    state.modifiers.toggle(ModifiersState::CTRL);
                    events.push_back(window_event);
                }

                if let Some(window_event) = modifier_event(
                    event,
                    NSEventModifierFlags::NSCommandKeyMask,
                    state.modifiers.logo(),
                ) {
                    state.modifiers.toggle(ModifiersState::LOGO);
                    events.push_back(window_event);
                }

                if let Some(window_event) = modifier_event(
                    event,
                    NSEventModifierFlags::NSAlternateKeyMask,
                    state.modifiers.alt(),
                ) {
                    state.modifiers.toggle(ModifiersState::ALT);
                    events.push_back(window_event);
                }

                let window_id = WindowId(get_window_id(state.ns_window));

                for event in events {
                    AppState::queue_event(EventWrapper::StaticEvent(Event::WindowEvent {
                        window_id,
                        event,
                    }));
                }

                AppState::queue_event(EventWrapper::StaticEvent(Event::WindowEvent {
                    window_id,
                    event: WindowEvent::ModifiersChanged(state.modifiers),
                }));
            }
        }

        #[sel(insertTab:)]
        fn insert_tab(&self, _sender: id) {
            trace_scope!("insertTab:");
            unsafe {
                let window: id = msg_send![self, window];
                let first_responder: id = msg_send![window, firstResponder];
                let self_ptr = self as *const _ as *mut _;
                if first_responder == self_ptr {
                    let _: () = msg_send![window, selectNextKeyView: self];
                }
            }
        }

        #[sel(insertBackTab:)]
        fn insert_back_tab(&self, _sender: id) {
            trace_scope!("insertBackTab:");
            unsafe {
                let window: id = msg_send![self, window];
                let first_responder: id = msg_send![window, firstResponder];
                let self_ptr = self as *const _ as *mut _;
                if first_responder == self_ptr {
                    let _: () = msg_send![window, selectPreviousKeyView: self];
                }
            }
        }

        // Allows us to receive Cmd-. (the shortcut for closing a dialog)
        // https://bugs.eclipse.org/bugs/show_bug.cgi?id=300620#c6
        #[sel(cancelOperation:)]
        fn cancel_operation(&self, _sender: id) {
            trace_scope!("cancelOperation:");
            unsafe {
                let state_ptr: *mut c_void = *self.ivar("winitState");
                let state = &mut *(state_ptr as *mut ViewState);

                let scancode = 0x2f;
                let virtual_keycode = scancode_to_keycode(scancode);
                debug_assert_eq!(virtual_keycode, Some(VirtualKeyCode::Period));

                let event: id = msg_send![NSApp(), currentEvent];

                update_potentially_stale_modifiers(state, event);

                #[allow(deprecated)]
                let window_event = Event::WindowEvent {
                    window_id: WindowId(get_window_id(state.ns_window)),
                    event: WindowEvent::KeyboardInput {
                        device_id: DEVICE_ID,
                        input: KeyboardInput {
                            state: ElementState::Pressed,
                            scancode: scancode as _,
                            virtual_keycode,
                            modifiers: event_mods(event),
                        },
                        is_synthetic: false,
                    },
                };

                AppState::queue_event(EventWrapper::StaticEvent(window_event));
            }
        }

        #[sel(mouseDown:)]
        fn mouse_down(&self, event: id) {
            trace_scope!("mouseDown:");
            mouse_motion(self, event);
            mouse_click(self, event, MouseButton::Left, ElementState::Pressed);
        }

        #[sel(mouseUp:)]
        fn mouse_up(&self, event: id) {
            trace_scope!("mouseUp:");
            mouse_motion(self, event);
            mouse_click(self, event, MouseButton::Left, ElementState::Released);
        }

        #[sel(rightMouseDown:)]
        fn right_mouse_down(&self, event: id) {
            trace_scope!("rightMouseDown:");
            mouse_motion(self, event);
            mouse_click(self, event, MouseButton::Right, ElementState::Pressed);
        }

        #[sel(rightMouseUp:)]
        fn right_mouse_up(&self, event: id) {
            trace_scope!("rightMouseUp:");
            mouse_motion(self, event);
            mouse_click(self, event, MouseButton::Right, ElementState::Released);
        }

        #[sel(otherMouseDown:)]
        fn other_mouse_down(&self, event: id) {
            trace_scope!("otherMouseDown:");
            mouse_motion(self, event);
            mouse_click(self, event, MouseButton::Middle, ElementState::Pressed);
        }

        #[sel(otherMouseUp:)]
        fn other_mouse_up(&self, event: id) {
            trace_scope!("otherMouseUp:");
            mouse_motion(self, event);
            mouse_click(self, event, MouseButton::Middle, ElementState::Released);
        }

        // No tracing on these because that would be overly verbose

        #[sel(mouseMoved:)]
        fn mouse_moved(&self, event: id) {
            mouse_motion(self, event);
        }

        #[sel(mouseDragged:)]
        fn mouse_dragged(&self, event: id) {
            mouse_motion(self, event);
        }

        #[sel(rightMouseDragged:)]
        fn right_mouse_dragged(&self, event: id) {
            mouse_motion(self, event);
        }

        #[sel(otherMouseDragged:)]
        fn other_mouse_dragged(&self, event: id) {
            mouse_motion(self, event);
        }

        #[sel(mouseEntered:)]
        fn mouse_entered(&self, _event: id) {
            trace_scope!("mouseEntered:");
            unsafe {
                let state_ptr: *mut c_void = *self.ivar("winitState");
                let state = &mut *(state_ptr as *mut ViewState);

                let enter_event = Event::WindowEvent {
                    window_id: WindowId(get_window_id(state.ns_window)),
                    event: WindowEvent::CursorEntered {
                        device_id: DEVICE_ID,
                    },
                };

                AppState::queue_event(EventWrapper::StaticEvent(enter_event));
            }
        }

        #[sel(mouseExited:)]
        fn mouse_exited(&self, _event: id) {
            trace_scope!("mouseExited:");
            unsafe {
                let state_ptr: *mut c_void = *self.ivar("winitState");
                let state = &mut *(state_ptr as *mut ViewState);

                let window_event = Event::WindowEvent {
                    window_id: WindowId(get_window_id(state.ns_window)),
                    event: WindowEvent::CursorLeft {
                        device_id: DEVICE_ID,
                    },
                };

                AppState::queue_event(EventWrapper::StaticEvent(window_event));
            }
        }

        #[sel(scrollWheel:)]
        fn scroll_wheel(&self, event: id) {
            trace_scope!("scrollWheel:");

            mouse_motion(self, event);

            unsafe {
                let state_ptr: *mut c_void = *self.ivar("winitState");
                let state = &mut *(state_ptr as *mut ViewState);

                let delta = {
                    let (x, y) = (event.scrollingDeltaX(), event.scrollingDeltaY());
                    if Bool::from_raw(event.hasPreciseScrollingDeltas()).as_bool() {
                        let delta =
                            LogicalPosition::new(x, y).to_physical(state.get_scale_factor());
                        MouseScrollDelta::PixelDelta(delta)
                    } else {
                        MouseScrollDelta::LineDelta(x as f32, y as f32)
                    }
                };

                // The "momentum phase," if any, has higher priority than touch phase (the two should
                // be mutually exclusive anyhow, which is why the API is rather incoherent). If no momentum
                // phase is recorded (or rather, the started/ended cases of the momentum phase) then we
                // report the touch phase.
                let phase =
                    match event.momentumPhase() {
                        NSEventPhase::NSEventPhaseMayBegin | NSEventPhase::NSEventPhaseBegan => {
                            TouchPhase::Started
                        }
                        NSEventPhase::NSEventPhaseEnded | NSEventPhase::NSEventPhaseCancelled => {
                            TouchPhase::Ended
                        }
                        _ => match event.phase() {
                            NSEventPhase::NSEventPhaseMayBegin
                            | NSEventPhase::NSEventPhaseBegan => TouchPhase::Started,
                            NSEventPhase::NSEventPhaseEnded
                            | NSEventPhase::NSEventPhaseCancelled => TouchPhase::Ended,
                            _ => TouchPhase::Moved,
                        },
                    };

                let device_event = Event::DeviceEvent {
                    device_id: DEVICE_ID,
                    event: DeviceEvent::MouseWheel { delta },
                };

                let state_ptr: *mut c_void = *self.ivar("winitState");
                let state = &mut *(state_ptr as *mut ViewState);

                update_potentially_stale_modifiers(state, event);

                let window_event = Event::WindowEvent {
                    window_id: WindowId(get_window_id(state.ns_window)),
                    event: WindowEvent::MouseWheel {
                        device_id: DEVICE_ID,
                        delta,
                        phase,
                        modifiers: event_mods(event),
                    },
                };

                AppState::queue_event(EventWrapper::StaticEvent(device_event));
                AppState::queue_event(EventWrapper::StaticEvent(window_event));
            }
        }

        #[sel(magnifyWithEvent:)]
        fn magnify_with_event(&self, event: id) {
            trace_scope!("magnifyWithEvent:");

            unsafe {
                let state_ptr: *mut c_void = *self.ivar("winitState");
                let state = &mut *(state_ptr as *mut ViewState);

                let delta = event.magnification();
                let phase = match event.phase() {
                    NSEventPhase::NSEventPhaseBegan => TouchPhase::Started,
                    NSEventPhase::NSEventPhaseChanged => TouchPhase::Moved,
                    NSEventPhase::NSEventPhaseCancelled => TouchPhase::Cancelled,
                    NSEventPhase::NSEventPhaseEnded => TouchPhase::Ended,
                    _ => return,
                };

                let window_event = Event::WindowEvent {
                    window_id: WindowId(get_window_id(state.ns_window)),
                    event: WindowEvent::TouchpadMagnify {
                        device_id: DEVICE_ID,
                        delta,
                        phase,
                    },
                };

                AppState::queue_event(EventWrapper::StaticEvent(window_event));
            }
        }

        #[sel(rotateWithEvent:)]
        fn rotate_with_event(&self, event: id) {
            trace_scope!("rotateWithEvent:");

            unsafe {
                let state_ptr: *mut c_void = *self.ivar("winitState");
                let state = &mut *(state_ptr as *mut ViewState);

                let delta = event.rotation();
                let phase = match event.phase() {
                    NSEventPhase::NSEventPhaseBegan => TouchPhase::Started,
                    NSEventPhase::NSEventPhaseChanged => TouchPhase::Moved,
                    NSEventPhase::NSEventPhaseCancelled => TouchPhase::Cancelled,
                    NSEventPhase::NSEventPhaseEnded => TouchPhase::Ended,
                    _ => return,
                };

                let window_event = Event::WindowEvent {
                    window_id: WindowId(get_window_id(state.ns_window)),
                    event: WindowEvent::TouchpadRotate {
                        device_id: DEVICE_ID,
                        delta,
                        phase,
                    },
                };

                AppState::queue_event(EventWrapper::StaticEvent(window_event));
            }
        }

        #[sel(pressureChangeWithEvent:)]
        fn pressure_change_with_event(&self, event: id) {
            trace_scope!("pressureChangeWithEvent:");

            mouse_motion(self, event);

            unsafe {
                let state_ptr: *mut c_void = *self.ivar("winitState");
                let state = &mut *(state_ptr as *mut ViewState);

                let pressure = event.pressure();
                let stage = event.stage();

                let window_event = Event::WindowEvent {
                    window_id: WindowId(get_window_id(state.ns_window)),
                    event: WindowEvent::TouchpadPressure {
                        device_id: DEVICE_ID,
                        pressure,
                        stage: stage as i64,
                    },
                };

                AppState::queue_event(EventWrapper::StaticEvent(window_event));
            }
        }

        // Allows us to receive Ctrl-Tab and Ctrl-Esc.
        // Note that this *doesn't* help with any missing Cmd inputs.
        // https://github.com/chromium/chromium/blob/a86a8a6bcfa438fa3ac2eba6f02b3ad1f8e0756f/ui/views/cocoa/bridged_content_view.mm#L816
        #[sel(_wantsKeyDownForEvent:)]
        fn wants_key_down_for_event(&self, _event: id) -> bool {
            trace_scope!("_wantsKeyDownForEvent:");
            true
        }

        #[sel(acceptsFirstMouse:)]
        fn accepts_first_mouse(&self, _event: id) -> bool {
            trace_scope!("acceptsFirstMouse:");
            true
        }
    }
);

impl WinitView {
    fn current_input_source(&self) -> String {
        let input_context: id = unsafe { msg_send![self, inputContext] };
        let input_source: id = unsafe { msg_send![input_context, selectedKeyboardInputSource] };
        unsafe { id_to_string_lossy(input_source) }
    }
}
