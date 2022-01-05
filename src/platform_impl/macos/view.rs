use std::{
    boxed::Box,
    collections::VecDeque,
    os::raw::*,
    slice, str,
    sync::{Arc, Mutex, Weak},
};

use cocoa::{
    appkit::{NSApp, NSEvent, NSEventModifierFlags, NSEventPhase, NSView, NSWindow},
    base::{id, nil},
    foundation::{NSInteger, NSPoint, NSRect, NSSize, NSString, NSUInteger},
};
use objc::{
    declare::ClassDecl,
    runtime::{Class, Object, Protocol, Sel, BOOL, NO, YES},
};

use crate::{
    dpi::LogicalPosition,
    event::{
        DeviceEvent, ElementState, Event, KeyboardInput, ModifiersState, MouseButton,
        MouseScrollDelta, TouchPhase, VirtualKeyCode, WindowEvent,
    },
    platform_impl::platform::{
        app_state::AppState,
        event::{
            char_to_keycode, check_function_keys, event_mods, get_scancode, modifier_event,
            scancode_to_keycode, EventWrapper,
        },
        ffi::*,
        util::{self, IdRef},
        window::get_window_id,
        DEVICE_ID,
    },
    window::WindowId,
};

pub struct CursorState {
    pub visible: bool,
    pub cursor: util::Cursor,
}

impl Default for CursorState {
    fn default() -> Self {
        Self {
            visible: true,
            cursor: Default::default(),
        }
    }
}

pub(super) struct ViewState {
    ns_window: id,
    pub cursor_state: Arc<Mutex<CursorState>>,
    ime_spot: Option<(f64, f64)>,
    raw_characters: Option<String>,
    pub(super) modifiers: ModifiersState,
    tracking_rect: Option<NSInteger>,
}

impl ViewState {
    fn get_scale_factor(&self) -> f64 {
        (unsafe { NSWindow::backingScaleFactor(self.ns_window) }) as f64
    }
}

pub fn new_view(ns_window: id) -> (IdRef, Weak<Mutex<CursorState>>) {
    let cursor_state = Default::default();
    let cursor_access = Arc::downgrade(&cursor_state);
    let state = ViewState {
        ns_window,
        cursor_state,
        ime_spot: None,
        raw_characters: None,
        modifiers: Default::default(),
        tracking_rect: None,
    };
    unsafe {
        // This is free'd in `dealloc`
        let state_ptr = Box::into_raw(Box::new(state)) as *mut c_void;
        let ns_view: id = msg_send![VIEW_CLASS.0, alloc];
        (
            IdRef::new(msg_send![ns_view, initWithWinit: state_ptr]),
            cursor_access,
        )
    }
}

pub unsafe fn set_ime_position(ns_view: id, input_context: id, x: f64, y: f64) {
    let state_ptr: *mut c_void = *(*ns_view).get_mut_ivar("winitState");
    let state = &mut *(state_ptr as *mut ViewState);
    let content_rect =
        NSWindow::contentRectForFrameRect_(state.ns_window, NSWindow::frame(state.ns_window));
    let base_x = content_rect.origin.x as f64;
    let base_y = (content_rect.origin.y + content_rect.size.height) as f64;
    state.ime_spot = Some((base_x + x, base_y - y));
    let _: () = msg_send![input_context, invalidateCharacterCoordinates];
}

struct ViewClass(*const Class);
unsafe impl Send for ViewClass {}
unsafe impl Sync for ViewClass {}

lazy_static! {
    static ref VIEW_CLASS: ViewClass = unsafe {
        let superclass = class!(NSView);
        let mut decl = ClassDecl::new("WinitView", superclass).unwrap();
        decl.add_method(sel!(dealloc), dealloc as extern "C" fn(&Object, Sel));
        decl.add_method(
            sel!(initWithWinit:),
            init_with_winit as extern "C" fn(&Object, Sel, *mut c_void) -> id,
        );
        decl.add_method(
            sel!(viewDidMoveToWindow),
            view_did_move_to_window as extern "C" fn(&Object, Sel),
        );
        decl.add_method(
            sel!(drawRect:),
            draw_rect as extern "C" fn(&Object, Sel, NSRect),
        );
        decl.add_method(
            sel!(acceptsFirstResponder),
            accepts_first_responder as extern "C" fn(&Object, Sel) -> BOOL,
        );
        decl.add_method(
            sel!(touchBar),
            touch_bar as extern "C" fn(&Object, Sel) -> BOOL,
        );
        decl.add_method(
            sel!(resetCursorRects),
            reset_cursor_rects as extern "C" fn(&Object, Sel),
        );
        decl.add_method(
            sel!(hasMarkedText),
            has_marked_text as extern "C" fn(&Object, Sel) -> BOOL,
        );
        decl.add_method(
            sel!(markedRange),
            marked_range as extern "C" fn(&Object, Sel) -> NSRange,
        );
        decl.add_method(
            sel!(selectedRange),
            selected_range as extern "C" fn(&Object, Sel) -> NSRange,
        );
        decl.add_method(
            sel!(setMarkedText:selectedRange:replacementRange:),
            set_marked_text as extern "C" fn(&mut Object, Sel, id, NSRange, NSRange),
        );
        decl.add_method(sel!(unmarkText), unmark_text as extern "C" fn(&Object, Sel));
        decl.add_method(
            sel!(validAttributesForMarkedText),
            valid_attributes_for_marked_text as extern "C" fn(&Object, Sel) -> id,
        );
        decl.add_method(
            sel!(attributedSubstringForProposedRange:actualRange:),
            attributed_substring_for_proposed_range
                as extern "C" fn(&Object, Sel, NSRange, *mut c_void) -> id,
        );
        decl.add_method(
            sel!(insertText:replacementRange:),
            insert_text as extern "C" fn(&Object, Sel, id, NSRange),
        );
        decl.add_method(
            sel!(characterIndexForPoint:),
            character_index_for_point as extern "C" fn(&Object, Sel, NSPoint) -> NSUInteger,
        );
        decl.add_method(
            sel!(firstRectForCharacterRange:actualRange:),
            first_rect_for_character_range
                as extern "C" fn(&Object, Sel, NSRange, *mut c_void) -> NSRect,
        );
        decl.add_method(
            sel!(doCommandBySelector:),
            do_command_by_selector as extern "C" fn(&Object, Sel, Sel),
        );
        decl.add_method(sel!(keyDown:), key_down as extern "C" fn(&Object, Sel, id));
        decl.add_method(sel!(keyUp:), key_up as extern "C" fn(&Object, Sel, id));
        decl.add_method(
            sel!(flagsChanged:),
            flags_changed as extern "C" fn(&Object, Sel, id),
        );
        decl.add_method(
            sel!(insertTab:),
            insert_tab as extern "C" fn(&Object, Sel, id),
        );
        decl.add_method(
            sel!(insertBackTab:),
            insert_back_tab as extern "C" fn(&Object, Sel, id),
        );
        decl.add_method(
            sel!(mouseDown:),
            mouse_down as extern "C" fn(&Object, Sel, id),
        );
        decl.add_method(sel!(mouseUp:), mouse_up as extern "C" fn(&Object, Sel, id));
        decl.add_method(
            sel!(rightMouseDown:),
            right_mouse_down as extern "C" fn(&Object, Sel, id),
        );
        decl.add_method(
            sel!(rightMouseUp:),
            right_mouse_up as extern "C" fn(&Object, Sel, id),
        );
        decl.add_method(
            sel!(otherMouseDown:),
            other_mouse_down as extern "C" fn(&Object, Sel, id),
        );
        decl.add_method(
            sel!(otherMouseUp:),
            other_mouse_up as extern "C" fn(&Object, Sel, id),
        );
        decl.add_method(
            sel!(mouseMoved:),
            mouse_moved as extern "C" fn(&Object, Sel, id),
        );
        decl.add_method(
            sel!(mouseDragged:),
            mouse_dragged as extern "C" fn(&Object, Sel, id),
        );
        decl.add_method(
            sel!(rightMouseDragged:),
            right_mouse_dragged as extern "C" fn(&Object, Sel, id),
        );
        decl.add_method(
            sel!(otherMouseDragged:),
            other_mouse_dragged as extern "C" fn(&Object, Sel, id),
        );
        decl.add_method(
            sel!(mouseEntered:),
            mouse_entered as extern "C" fn(&Object, Sel, id),
        );
        decl.add_method(
            sel!(mouseExited:),
            mouse_exited as extern "C" fn(&Object, Sel, id),
        );
        decl.add_method(
            sel!(scrollWheel:),
            scroll_wheel as extern "C" fn(&Object, Sel, id),
        );
        decl.add_method(
            sel!(pressureChangeWithEvent:),
            pressure_change_with_event as extern "C" fn(&Object, Sel, id),
        );
        decl.add_method(
            sel!(_wantsKeyDownForEvent:),
            wants_key_down_for_event as extern "C" fn(&Object, Sel, id) -> BOOL,
        );
        decl.add_method(
            sel!(cancelOperation:),
            cancel_operation as extern "C" fn(&Object, Sel, id),
        );
        decl.add_method(
            sel!(frameDidChange:),
            frame_did_change as extern "C" fn(&Object, Sel, id),
        );
        decl.add_method(
            sel!(acceptsFirstMouse:),
            accepts_first_mouse as extern "C" fn(&Object, Sel, id) -> BOOL,
        );
        decl.add_ivar::<*mut c_void>("winitState");
        decl.add_ivar::<id>("markedText");
        let protocol = Protocol::get("NSTextInputClient").unwrap();
        decl.add_protocol(&protocol);
        ViewClass(decl.register())
    };
}

extern "C" fn dealloc(this: &Object, _sel: Sel) {
    unsafe {
        let state: *mut c_void = *this.get_ivar("winitState");
        let marked_text: id = *this.get_ivar("markedText");
        let _: () = msg_send![marked_text, release];
        Box::from_raw(state as *mut ViewState);
    }
}

extern "C" fn init_with_winit(this: &Object, _sel: Sel, state: *mut c_void) -> id {
    unsafe {
        let this: id = msg_send![this, init];
        if this != nil {
            (*this).set_ivar("winitState", state);
            let marked_text =
                <id as NSMutableAttributedString>::init(NSMutableAttributedString::alloc(nil));
            (*this).set_ivar("markedText", marked_text);
            let _: () = msg_send![this, setPostsFrameChangedNotifications: YES];

            let notification_center: &Object =
                msg_send![class!(NSNotificationCenter), defaultCenter];
            let notification_name =
                IdRef::new(NSString::alloc(nil).init_str("NSViewFrameDidChangeNotification"));
            let _: () = msg_send![
                notification_center,
                addObserver: this
                selector: sel!(frameDidChange:)
                name: notification_name
                object: this
            ];
        }
        this
    }
}

extern "C" fn view_did_move_to_window(this: &Object, _sel: Sel) {
    trace!("Triggered `viewDidMoveToWindow`");
    unsafe {
        let state_ptr: *mut c_void = *this.get_ivar("winitState");
        let state = &mut *(state_ptr as *mut ViewState);

        if let Some(tracking_rect) = state.tracking_rect.take() {
            let _: () = msg_send![this, removeTrackingRect: tracking_rect];
        }

        let rect: NSRect = msg_send![this, visibleRect];
        let tracking_rect: NSInteger = msg_send![this,
            addTrackingRect:rect
            owner:this
            userData:nil
            assumeInside:NO
        ];
        state.tracking_rect = Some(tracking_rect);
    }
    trace!("Completed `viewDidMoveToWindow`");
}

extern "C" fn frame_did_change(this: &Object, _sel: Sel, _event: id) {
    unsafe {
        let state_ptr: *mut c_void = *this.get_ivar("winitState");
        let state = &mut *(state_ptr as *mut ViewState);

        if let Some(tracking_rect) = state.tracking_rect.take() {
            let _: () = msg_send![this, removeTrackingRect: tracking_rect];
        }

        let rect: NSRect = msg_send![this, visibleRect];
        let tracking_rect: NSInteger = msg_send![this,
            addTrackingRect:rect
            owner:this
            userData:nil
            assumeInside:NO
        ];

        state.tracking_rect = Some(tracking_rect);
    }
}

extern "C" fn draw_rect(this: &Object, _sel: Sel, rect: NSRect) {
    unsafe {
        let state_ptr: *mut c_void = *this.get_ivar("winitState");
        let state = &mut *(state_ptr as *mut ViewState);

        AppState::handle_redraw(WindowId(get_window_id(state.ns_window)));

        let superclass = util::superclass(this);
        let () = msg_send![super(this, superclass), drawRect: rect];
    }
}

extern "C" fn accepts_first_responder(_this: &Object, _sel: Sel) -> BOOL {
    YES
}

// This is necessary to prevent a beefy terminal error on MacBook Pros:
// IMKInputSession [0x7fc573576ff0 presentFunctionRowItemTextInputViewWithEndpoint:completionHandler:] : [self textInputContext]=0x7fc573558e10 *NO* NSRemoteViewController to client, NSError=Error Domain=NSCocoaErrorDomain Code=4099 "The connection from pid 0 was invalidated from this process." UserInfo={NSDebugDescription=The connection from pid 0 was invalidated from this process.}, com.apple.inputmethod.EmojiFunctionRowItem
// TODO: Add an API extension for using `NSTouchBar`
extern "C" fn touch_bar(_this: &Object, _sel: Sel) -> BOOL {
    NO
}

extern "C" fn reset_cursor_rects(this: &Object, _sel: Sel) {
    unsafe {
        let state_ptr: *mut c_void = *this.get_ivar("winitState");
        let state = &mut *(state_ptr as *mut ViewState);

        let bounds: NSRect = msg_send![this, bounds];
        let cursor_state = state.cursor_state.lock().unwrap();
        let cursor = if cursor_state.visible {
            cursor_state.cursor.load()
        } else {
            util::invisible_cursor()
        };
        let _: () = msg_send![this,
            addCursorRect:bounds
            cursor:cursor
        ];
    }
}

extern "C" fn has_marked_text(this: &Object, _sel: Sel) -> BOOL {
    unsafe {
        trace!("Triggered `hasMarkedText`");
        let marked_text: id = *this.get_ivar("markedText");
        trace!("Completed `hasMarkedText`");
        (marked_text.length() > 0) as BOOL
    }
}

extern "C" fn marked_range(this: &Object, _sel: Sel) -> NSRange {
    unsafe {
        trace!("Triggered `markedRange`");
        let marked_text: id = *this.get_ivar("markedText");
        let length = marked_text.length();
        trace!("Completed `markedRange`");
        if length > 0 {
            NSRange::new(0, length - 1)
        } else {
            util::EMPTY_RANGE
        }
    }
}

extern "C" fn selected_range(_this: &Object, _sel: Sel) -> NSRange {
    trace!("Triggered `selectedRange`");
    trace!("Completed `selectedRange`");
    util::EMPTY_RANGE
}

extern "C" fn set_marked_text(
    this: &mut Object,
    _sel: Sel,
    string: id,
    _selected_range: NSRange,
    _replacement_range: NSRange,
) {
    trace!("Triggered `setMarkedText`");
    unsafe {
        let marked_text_ref: &mut id = this.get_mut_ivar("markedText");
        let _: () = msg_send![(*marked_text_ref), release];
        let marked_text = NSMutableAttributedString::alloc(nil);
        let has_attr = msg_send![string, isKindOfClass: class!(NSAttributedString)];
        if has_attr {
            marked_text.initWithAttributedString(string);
        } else {
            marked_text.initWithString(string);
        };
        *marked_text_ref = marked_text;
    }
    trace!("Completed `setMarkedText`");
}

extern "C" fn unmark_text(this: &Object, _sel: Sel) {
    trace!("Triggered `unmarkText`");
    unsafe {
        let marked_text: id = *this.get_ivar("markedText");
        let mutable_string = marked_text.mutableString();
        let _: () = msg_send![mutable_string, setString:""];
        let input_context: id = msg_send![this, inputContext];
        let _: () = msg_send![input_context, discardMarkedText];
    }
    trace!("Completed `unmarkText`");
}

extern "C" fn valid_attributes_for_marked_text(_this: &Object, _sel: Sel) -> id {
    trace!("Triggered `validAttributesForMarkedText`");
    trace!("Completed `validAttributesForMarkedText`");
    unsafe { msg_send![class!(NSArray), array] }
}

extern "C" fn attributed_substring_for_proposed_range(
    _this: &Object,
    _sel: Sel,
    _range: NSRange,
    _actual_range: *mut c_void, // *mut NSRange
) -> id {
    trace!("Triggered `attributedSubstringForProposedRange`");
    trace!("Completed `attributedSubstringForProposedRange`");
    nil
}

extern "C" fn character_index_for_point(_this: &Object, _sel: Sel, _point: NSPoint) -> NSUInteger {
    trace!("Triggered `characterIndexForPoint`");
    trace!("Completed `characterIndexForPoint`");
    0
}

extern "C" fn first_rect_for_character_range(
    this: &Object,
    _sel: Sel,
    _range: NSRange,
    _actual_range: *mut c_void, // *mut NSRange
) -> NSRect {
    unsafe {
        trace!("Triggered `firstRectForCharacterRange`");
        let state_ptr: *mut c_void = *this.get_ivar("winitState");
        let state = &mut *(state_ptr as *mut ViewState);
        let (x, y) = state.ime_spot.unwrap_or_else(|| {
            let content_rect = NSWindow::contentRectForFrameRect_(
                state.ns_window,
                NSWindow::frame(state.ns_window),
            );
            let x = content_rect.origin.x;
            let y = util::bottom_left_to_top_left(content_rect);
            (x, y)
        });
        trace!("Completed `firstRectForCharacterRange`");
        NSRect::new(NSPoint::new(x as _, y as _), NSSize::new(0.0, 0.0))
    }
}

extern "C" fn insert_text(this: &Object, _sel: Sel, string: id, _replacement_range: NSRange) {
    trace!("Triggered `insertText`");
    unsafe {
        let state_ptr: *mut c_void = *this.get_ivar("winitState");
        let state = &mut *(state_ptr as *mut ViewState);

        let has_attr = msg_send![string, isKindOfClass: class!(NSAttributedString)];
        let characters = if has_attr {
            // This is a *mut NSAttributedString
            msg_send![string, string]
        } else {
            // This is already a *mut NSString
            string
        };

        let slice =
            slice::from_raw_parts(characters.UTF8String() as *const c_uchar, characters.len());
        let string = str::from_utf8_unchecked(slice);

        // We don't need this now, but it's here if that changes.
        //let event: id = msg_send![NSApp(), currentEvent];

        let mut events = VecDeque::with_capacity(characters.len());
        for character in string.chars().filter(|c| !is_corporate_character(*c)) {
            events.push_back(EventWrapper::StaticEvent(Event::WindowEvent {
                window_id: WindowId(get_window_id(state.ns_window)),
                event: WindowEvent::ReceivedCharacter(character),
            }));
        }

        AppState::queue_events(events);
    }
    trace!("Completed `insertText`");
}

extern "C" fn do_command_by_selector(this: &Object, _sel: Sel, command: Sel) {
    trace!("Triggered `doCommandBySelector`");
    // Basically, we're sent this message whenever a keyboard event that doesn't generate a "human readable" character
    // happens, i.e. newlines, tabs, and Ctrl+C.
    unsafe {
        let state_ptr: *mut c_void = *this.get_ivar("winitState");
        let state = &mut *(state_ptr as *mut ViewState);

        let mut events = VecDeque::with_capacity(1);
        if command == sel!(insertNewline:) {
            // The `else` condition would emit the same character, but I'm keeping this here both...
            // 1) as a reminder for how `doCommandBySelector` works
            // 2) to make our use of carriage return explicit
            events.push_back(EventWrapper::StaticEvent(Event::WindowEvent {
                window_id: WindowId(get_window_id(state.ns_window)),
                event: WindowEvent::ReceivedCharacter('\r'),
            }));
        } else {
            let raw_characters = state.raw_characters.take();
            if let Some(raw_characters) = raw_characters {
                for character in raw_characters
                    .chars()
                    .filter(|c| !is_corporate_character(*c))
                {
                    events.push_back(EventWrapper::StaticEvent(Event::WindowEvent {
                        window_id: WindowId(get_window_id(state.ns_window)),
                        event: WindowEvent::ReceivedCharacter(character),
                    }));
                }
            }
        };

        AppState::queue_events(events);
    }
    trace!("Completed `doCommandBySelector`");
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
    match c {
        '\u{F700}'..='\u{F747}'
        | '\u{F802}'..='\u{F84F}'
        | '\u{F850}'
        | '\u{F85C}'
        | '\u{F85D}'
        | '\u{F85F}'
        | '\u{F860}'..='\u{F86B}'
        | '\u{F870}'..='\u{F8FF}' => true,
        _ => false,
    }
}

// Retrieves a layout-independent keycode given an event.
fn retrieve_keycode(event: id) -> Option<VirtualKeyCode> {
    #[inline]
    fn get_code(ev: id, raw: bool) -> Option<VirtualKeyCode> {
        let characters = get_characters(ev, raw);
        characters.chars().next().and_then(|c| char_to_keycode(c))
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

extern "C" fn key_down(this: &Object, _sel: Sel, event: id) {
    trace!("Triggered `keyDown`");
    unsafe {
        let state_ptr: *mut c_void = *this.get_ivar("winitState");
        let state = &mut *(state_ptr as *mut ViewState);
        let window_id = WindowId(get_window_id(state.ns_window));
        let characters = get_characters(event, false);

        state.raw_characters = Some(characters.clone());

        let scancode = get_scancode(event) as u32;
        let virtual_keycode = retrieve_keycode(event);

        let is_repeat = msg_send![event, isARepeat];

        update_potentially_stale_modifiers(state, event);

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

        let pass_along = {
            AppState::queue_event(EventWrapper::StaticEvent(window_event));
            // Emit `ReceivedCharacter` for key repeats
            if is_repeat {
                for character in characters.chars().filter(|c| !is_corporate_character(*c)) {
                    AppState::queue_event(EventWrapper::StaticEvent(Event::WindowEvent {
                        window_id,
                        event: WindowEvent::ReceivedCharacter(character),
                    }));
                }
                false
            } else {
                true
            }
        };

        if pass_along {
            // Some keys (and only *some*, with no known reason) don't trigger `insertText`, while others do...
            // So, we don't give repeats the opportunity to trigger that, since otherwise our hack will cause some
            // keys to generate twice as many characters.
            let array: id = msg_send![class!(NSArray), arrayWithObject: event];
            let _: () = msg_send![this, interpretKeyEvents: array];
        }
    }
    trace!("Completed `keyDown`");
}

extern "C" fn key_up(this: &Object, _sel: Sel, event: id) {
    trace!("Triggered `keyUp`");
    unsafe {
        let state_ptr: *mut c_void = *this.get_ivar("winitState");
        let state = &mut *(state_ptr as *mut ViewState);

        let scancode = get_scancode(event) as u32;
        let virtual_keycode = retrieve_keycode(event);

        update_potentially_stale_modifiers(state, event);

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
    trace!("Completed `keyUp`");
}

extern "C" fn flags_changed(this: &Object, _sel: Sel, event: id) {
    trace!("Triggered `flagsChanged`");
    unsafe {
        let state_ptr: *mut c_void = *this.get_ivar("winitState");
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
    trace!("Completed `flagsChanged`");
}

extern "C" fn insert_tab(this: &Object, _sel: Sel, _sender: id) {
    unsafe {
        let window: id = msg_send![this, window];
        let first_responder: id = msg_send![window, firstResponder];
        let this_ptr = this as *const _ as *mut _;
        if first_responder == this_ptr {
            let (): _ = msg_send![window, selectNextKeyView: this];
        }
    }
}

extern "C" fn insert_back_tab(this: &Object, _sel: Sel, _sender: id) {
    unsafe {
        let window: id = msg_send![this, window];
        let first_responder: id = msg_send![window, firstResponder];
        let this_ptr = this as *const _ as *mut _;
        if first_responder == this_ptr {
            let (): _ = msg_send![window, selectPreviousKeyView: this];
        }
    }
}

// Allows us to receive Cmd-. (the shortcut for closing a dialog)
// https://bugs.eclipse.org/bugs/show_bug.cgi?id=300620#c6
extern "C" fn cancel_operation(this: &Object, _sel: Sel, _sender: id) {
    trace!("Triggered `cancelOperation`");
    unsafe {
        let state_ptr: *mut c_void = *this.get_ivar("winitState");
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
    trace!("Completed `cancelOperation`");
}

fn mouse_click(this: &Object, event: id, button: MouseButton, button_state: ElementState) {
    unsafe {
        let state_ptr: *mut c_void = *this.get_ivar("winitState");
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

extern "C" fn mouse_down(this: &Object, _sel: Sel, event: id) {
    mouse_motion(this, event);
    mouse_click(this, event, MouseButton::Left, ElementState::Pressed);
}

extern "C" fn mouse_up(this: &Object, _sel: Sel, event: id) {
    mouse_motion(this, event);
    mouse_click(this, event, MouseButton::Left, ElementState::Released);
}

extern "C" fn right_mouse_down(this: &Object, _sel: Sel, event: id) {
    mouse_motion(this, event);
    mouse_click(this, event, MouseButton::Right, ElementState::Pressed);
}

extern "C" fn right_mouse_up(this: &Object, _sel: Sel, event: id) {
    mouse_motion(this, event);
    mouse_click(this, event, MouseButton::Right, ElementState::Released);
}

extern "C" fn other_mouse_down(this: &Object, _sel: Sel, event: id) {
    mouse_motion(this, event);
    mouse_click(this, event, MouseButton::Middle, ElementState::Pressed);
}

extern "C" fn other_mouse_up(this: &Object, _sel: Sel, event: id) {
    mouse_motion(this, event);
    mouse_click(this, event, MouseButton::Middle, ElementState::Released);
}

fn mouse_motion(this: &Object, event: id) {
    unsafe {
        let state_ptr: *mut c_void = *this.get_ivar("winitState");
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
            let mouse_buttons_down: NSInteger = msg_send![class!(NSEvent), pressedMouseButtons];
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

extern "C" fn mouse_moved(this: &Object, _sel: Sel, event: id) {
    mouse_motion(this, event);
}

extern "C" fn mouse_dragged(this: &Object, _sel: Sel, event: id) {
    mouse_motion(this, event);
}

extern "C" fn right_mouse_dragged(this: &Object, _sel: Sel, event: id) {
    mouse_motion(this, event);
}

extern "C" fn other_mouse_dragged(this: &Object, _sel: Sel, event: id) {
    mouse_motion(this, event);
}

extern "C" fn mouse_entered(this: &Object, _sel: Sel, _event: id) {
    trace!("Triggered `mouseEntered`");
    unsafe {
        let state_ptr: *mut c_void = *this.get_ivar("winitState");
        let state = &mut *(state_ptr as *mut ViewState);

        let enter_event = Event::WindowEvent {
            window_id: WindowId(get_window_id(state.ns_window)),
            event: WindowEvent::CursorEntered {
                device_id: DEVICE_ID,
            },
        };

        AppState::queue_event(EventWrapper::StaticEvent(enter_event));
    }
    trace!("Completed `mouseEntered`");
}

extern "C" fn mouse_exited(this: &Object, _sel: Sel, _event: id) {
    trace!("Triggered `mouseExited`");
    unsafe {
        let state_ptr: *mut c_void = *this.get_ivar("winitState");
        let state = &mut *(state_ptr as *mut ViewState);

        let window_event = Event::WindowEvent {
            window_id: WindowId(get_window_id(state.ns_window)),
            event: WindowEvent::CursorLeft {
                device_id: DEVICE_ID,
            },
        };

        AppState::queue_event(EventWrapper::StaticEvent(window_event));
    }
    trace!("Completed `mouseExited`");
}

extern "C" fn scroll_wheel(this: &Object, _sel: Sel, event: id) {
    trace!("Triggered `scrollWheel`");

    mouse_motion(this, event);

    unsafe {
        let state_ptr: *mut c_void = *this.get_ivar("winitState");
        let state = &mut *(state_ptr as *mut ViewState);

        let delta = {
            // macOS horizontal sign convention is the inverse of winit.
            let (x, y) = (event.scrollingDeltaX() * -1.0, event.scrollingDeltaY());
            if event.hasPreciseScrollingDeltas() == YES {
                let delta = LogicalPosition::new(x, y).to_physical(state.get_scale_factor());
                MouseScrollDelta::PixelDelta(delta)
            } else {
                MouseScrollDelta::LineDelta(x as f32, y as f32)
            }
        };
        let phase = match event.phase() {
            NSEventPhase::NSEventPhaseMayBegin | NSEventPhase::NSEventPhaseBegan => {
                TouchPhase::Started
            }
            NSEventPhase::NSEventPhaseEnded => TouchPhase::Ended,
            _ => TouchPhase::Moved,
        };

        let device_event = Event::DeviceEvent {
            device_id: DEVICE_ID,
            event: DeviceEvent::MouseWheel { delta },
        };

        let state_ptr: *mut c_void = *this.get_ivar("winitState");
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
    trace!("Completed `scrollWheel`");
}

extern "C" fn pressure_change_with_event(this: &Object, _sel: Sel, event: id) {
    trace!("Triggered `pressureChangeWithEvent`");

    mouse_motion(this, event);

    unsafe {
        let state_ptr: *mut c_void = *this.get_ivar("winitState");
        let state = &mut *(state_ptr as *mut ViewState);

        let pressure = event.pressure();
        let stage = event.stage();

        let window_event = Event::WindowEvent {
            window_id: WindowId(get_window_id(state.ns_window)),
            event: WindowEvent::TouchpadPressure {
                device_id: DEVICE_ID,
                pressure,
                stage,
            },
        };

        AppState::queue_event(EventWrapper::StaticEvent(window_event));
    }
    trace!("Completed `pressureChangeWithEvent`");
}

// Allows us to receive Ctrl-Tab and Ctrl-Esc.
// Note that this *doesn't* help with any missing Cmd inputs.
// https://github.com/chromium/chromium/blob/a86a8a6bcfa438fa3ac2eba6f02b3ad1f8e0756f/ui/views/cocoa/bridged_content_view.mm#L816
extern "C" fn wants_key_down_for_event(_this: &Object, _sel: Sel, _event: id) -> BOOL {
    YES
}

extern "C" fn accepts_first_mouse(_this: &Object, _sel: Sel, _event: id) -> BOOL {
    YES
}
