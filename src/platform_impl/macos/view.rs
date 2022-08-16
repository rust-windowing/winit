use std::{
    boxed::Box,
    collections::VecDeque,
    os::raw::*,
    ptr, slice, str,
    sync::{
        atomic::{compiler_fence, Ordering},
        Arc, Mutex, Weak,
    },
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
use once_cell::sync::Lazy;

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
    pub cursor_state: Arc<Mutex<CursorState>>,
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

pub fn new_view(ns_window: id) -> (IdRef, Weak<Mutex<CursorState>>) {
    let cursor_state = Default::default();
    let cursor_access = Arc::downgrade(&cursor_state);
    let state = ViewState {
        ns_window,
        cursor_state,
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
        let ns_view: id = msg_send![VIEW_CLASS.0, alloc];
        (
            IdRef::new(msg_send![ns_view, initWithWinit: state_ptr]),
            cursor_access,
        )
    }
}

pub unsafe fn set_ime_position(ns_view: id, position: LogicalPosition<f64>) {
    let state_ptr: *mut c_void = *(*ns_view).get_mut_ivar("winitState");
    let state = &mut *(state_ptr as *mut ViewState);
    state.ime_position = position;
    let input_context: id = msg_send![ns_view, inputContext];
    let _: () = msg_send![input_context, invalidateCharacterCoordinates];
}

pub unsafe fn set_ime_allowed(ns_view: id, ime_allowed: bool) {
    let state_ptr: *mut c_void = *(*ns_view).get_mut_ivar("winitState");
    let state = &mut *(state_ptr as *mut ViewState);
    if state.ime_allowed == ime_allowed {
        return;
    }
    state.ime_allowed = ime_allowed;
    if state.ime_allowed {
        return;
    }
    let marked_text_ref: &mut id = (*ns_view).get_mut_ivar("markedText");

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

struct ViewClass(*const Class);
unsafe impl Send for ViewClass {}
unsafe impl Sync for ViewClass {}

static VIEW_CLASS: Lazy<ViewClass> = Lazy::new(|| unsafe {
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

    // ------------------------------------------------------------------
    // NSTextInputClient
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
    // ------------------------------------------------------------------

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
        sel!(magnifyWithEvent:),
        magnify_with_event as extern "C" fn(&Object, Sel, id),
    );
    decl.add_method(
        sel!(rotateWithEvent:),
        rotate_with_event as extern "C" fn(&Object, Sel, id),
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
    decl.add_protocol(protocol);
    ViewClass(decl.register())
});

extern "C" fn dealloc(this: &Object, _sel: Sel) {
    unsafe {
        let marked_text: id = *this.get_ivar("markedText");
        let _: () = msg_send![marked_text, release];
        let state: *mut c_void = *this.get_ivar("winitState");
        drop(Box::from_raw(state as *mut ViewState));
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
            // About frame change
            let frame_did_change_notification_name =
                IdRef::new(NSString::alloc(nil).init_str("NSViewFrameDidChangeNotification"));
            let _: () = msg_send![
                notification_center,
                addObserver: this
                selector: sel!(frameDidChange:)
                name: frame_did_change_notification_name
                object: this
            ];

            let winit_state = &mut *(state as *mut ViewState);
            winit_state.input_source = current_input_source(this);
        }
        this
    }
}

extern "C" fn view_did_move_to_window(this: &Object, _sel: Sel) {
    trace_scope!("viewDidMoveToWindow");
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
            userData:ptr::null_mut::<c_void>()
            assumeInside:NO
        ];
        state.tracking_rect = Some(tracking_rect);
    }
}

extern "C" fn frame_did_change(this: &Object, _sel: Sel, _event: id) {
    trace_scope!("frameDidChange:");
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
            userData:ptr::null_mut::<c_void>()
            assumeInside:NO
        ];
        state.tracking_rect = Some(tracking_rect);

        // Emit resize event here rather than from windowDidResize because:
        // 1. When a new window is created as a tab, the frame size may change without a window resize occurring.
        // 2. Even when a window resize does occur on a new tabbed window, it contains the wrong size (includes tab height).
        let logical_size = LogicalSize::new(rect.size.width as f64, rect.size.height as f64);
        let size = logical_size.to_physical::<u32>(state.get_scale_factor());
        AppState::queue_event(EventWrapper::StaticEvent(Event::WindowEvent {
            window_id: WindowId(get_window_id(state.ns_window)),
            event: WindowEvent::Resized(size),
        }));
    }
}

extern "C" fn draw_rect(this: &Object, _sel: Sel, rect: NSRect) {
    trace_scope!("drawRect:");
    unsafe {
        let state_ptr: *mut c_void = *this.get_ivar("winitState");
        let state = &mut *(state_ptr as *mut ViewState);

        AppState::handle_redraw(WindowId(get_window_id(state.ns_window)));

        let superclass = util::superclass(this);
        let _: () = msg_send![super(this, superclass), drawRect: rect];
    }
}

extern "C" fn accepts_first_responder(_this: &Object, _sel: Sel) -> BOOL {
    trace_scope!("acceptsFirstResponder");
    YES
}

// This is necessary to prevent a beefy terminal error on MacBook Pros:
// IMKInputSession [0x7fc573576ff0 presentFunctionRowItemTextInputViewWithEndpoint:completionHandler:] : [self textInputContext]=0x7fc573558e10 *NO* NSRemoteViewController to client, NSError=Error Domain=NSCocoaErrorDomain Code=4099 "The connection from pid 0 was invalidated from this process." UserInfo={NSDebugDescription=The connection from pid 0 was invalidated from this process.}, com.apple.inputmethod.EmojiFunctionRowItem
// TODO: Add an API extension for using `NSTouchBar`
extern "C" fn touch_bar(_this: &Object, _sel: Sel) -> BOOL {
    trace_scope!("touchBar");
    NO
}

extern "C" fn reset_cursor_rects(this: &Object, _sel: Sel) {
    trace_scope!("resetCursorRects");
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
    trace_scope!("hasMarkedText");
    unsafe {
        let marked_text: id = *this.get_ivar("markedText");
        (marked_text.length() > 0) as BOOL
    }
}

extern "C" fn marked_range(this: &Object, _sel: Sel) -> NSRange {
    trace_scope!("markedRange");
    unsafe {
        let marked_text: id = *this.get_ivar("markedText");
        let length = marked_text.length();
        if length > 0 {
            NSRange::new(0, length)
        } else {
            util::EMPTY_RANGE
        }
    }
}

extern "C" fn selected_range(_this: &Object, _sel: Sel) -> NSRange {
    trace_scope!("selectedRange");
    util::EMPTY_RANGE
}

/// Safety: Assumes that `view` is an instance of `VIEW_CLASS` from winit.
unsafe fn current_input_source(view: *const Object) -> String {
    let input_context: id = msg_send![view, inputContext];
    let input_source: id = msg_send![input_context, selectedKeyboardInputSource];
    id_to_string_lossy(input_source)
}

extern "C" fn set_marked_text(
    this: &mut Object,
    _sel: Sel,
    string: id,
    _selected_range: NSRange,
    _replacement_range: NSRange,
) {
    trace_scope!("setMarkedText:selectedRange:replacementRange:");
    unsafe {
        // Get pre-edit text
        let marked_text_ref: &mut id = this.get_mut_ivar("markedText");

        // Update markedText
        let _: () = msg_send![(*marked_text_ref), release];
        let marked_text = NSMutableAttributedString::alloc(nil);
        let has_attr: BOOL = msg_send![string, isKindOfClass: class!(NSAttributedString)];
        if has_attr != NO {
            marked_text.initWithAttributedString(string);
        } else {
            marked_text.initWithString(string);
        };
        *marked_text_ref = marked_text;

        // Update ViewState with new marked text
        let state_ptr: *mut c_void = *this.get_ivar("winitState");
        let state = &mut *(state_ptr as *mut ViewState);
        let preedit_string = id_to_string_lossy(string);

        // Notify IME is active if application still doesn't know it.
        if state.ime_state == ImeState::Disabled {
            state.input_source = current_input_source(this);
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

extern "C" fn unmark_text(this: &Object, _sel: Sel) {
    trace_scope!("unmarkText");
    unsafe {
        let marked_text: id = *this.get_ivar("markedText");
        let mutable_string = marked_text.mutableString();
        let s: id = msg_send![class!(NSString), new];
        let _: () = msg_send![mutable_string, setString: s];
        let _: () = msg_send![s, release];
        let input_context: id = msg_send![this, inputContext];
        let _: () = msg_send![input_context, discardMarkedText];

        let state_ptr: *mut c_void = *this.get_ivar("winitState");
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

extern "C" fn valid_attributes_for_marked_text(_this: &Object, _sel: Sel) -> id {
    trace_scope!("validAttributesForMarkedText");
    unsafe { msg_send![class!(NSArray), array] }
}

extern "C" fn attributed_substring_for_proposed_range(
    _this: &Object,
    _sel: Sel,
    _range: NSRange,
    _actual_range: *mut c_void, // *mut NSRange
) -> id {
    trace_scope!("attributedSubstringForProposedRange:actualRange:");
    nil
}

extern "C" fn character_index_for_point(_this: &Object, _sel: Sel, _point: NSPoint) -> NSUInteger {
    trace_scope!("characterIndexForPoint:");
    0
}

extern "C" fn first_rect_for_character_range(
    this: &Object,
    _sel: Sel,
    _range: NSRange,
    _actual_range: *mut c_void, // *mut NSRange
) -> NSRect {
    trace_scope!("firstRectForCharacterRange:actualRange:");
    unsafe {
        let state_ptr: *mut c_void = *this.get_ivar("winitState");
        let state = &mut *(state_ptr as *mut ViewState);
        let content_rect =
            NSWindow::contentRectForFrameRect_(state.ns_window, NSWindow::frame(state.ns_window));
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

extern "C" fn insert_text(this: &Object, _sel: Sel, string: id, _replacement_range: NSRange) {
    trace_scope!("insertText:replacementRange:");
    unsafe {
        let state_ptr: *mut c_void = *this.get_ivar("winitState");
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

extern "C" fn do_command_by_selector(this: &Object, _sel: Sel, _command: Sel) {
    trace_scope!("doCommandBySelector:");
    // Basically, we're sent this message whenever a keyboard event that doesn't generate a "human
    // readable" character happens, i.e. newlines, tabs, and Ctrl+C.
    unsafe {
        let state_ptr: *mut c_void = *this.get_ivar("winitState");
        let state = &mut *(state_ptr as *mut ViewState);

        // We shouldn't forward any character from just commited text, since we'll end up sending
        // it twice with some IMEs like Korean one. We'll also always send `Enter` in that case,
        // which is not desired given it was used to confirm IME input.
        if state.ime_state == ImeState::Commited {
            return;
        }

        state.forward_key_to_app = true;

        let has_marked_text: BOOL = msg_send![this, hasMarkedText];
        if has_marked_text == NO && state.ime_state == ImeState::Preedit {
            // Leave preedit so that we also report the keyup for this key
            state.ime_state = ImeState::Enabled;
        }
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

extern "C" fn key_down(this: &Object, _sel: Sel, event: id) {
    trace_scope!("keyDown:");
    unsafe {
        let state_ptr: *mut c_void = *this.get_ivar("winitState");
        let state = &mut *(state_ptr as *mut ViewState);
        let window_id = WindowId(get_window_id(state.ns_window));

        let input_source = current_input_source(this);
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
            let _: () = msg_send![this, interpretKeyEvents: events_for_nsview];

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

extern "C" fn key_up(this: &Object, _sel: Sel, event: id) {
    trace_scope!("keyUp:");
    unsafe {
        let state_ptr: *mut c_void = *this.get_ivar("winitState");
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

extern "C" fn flags_changed(this: &Object, _sel: Sel, event: id) {
    trace_scope!("flagsChanged:");
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
}

extern "C" fn insert_tab(this: &Object, _sel: Sel, _sender: id) {
    trace_scope!("insertTab:");
    unsafe {
        let window: id = msg_send![this, window];
        let first_responder: id = msg_send![window, firstResponder];
        let this_ptr = this as *const _ as *mut _;
        if first_responder == this_ptr {
            let _: () = msg_send![window, selectNextKeyView: this];
        }
    }
}

extern "C" fn insert_back_tab(this: &Object, _sel: Sel, _sender: id) {
    trace_scope!("insertBackTab:");
    unsafe {
        let window: id = msg_send![this, window];
        let first_responder: id = msg_send![window, firstResponder];
        let this_ptr = this as *const _ as *mut _;
        if first_responder == this_ptr {
            let _: () = msg_send![window, selectPreviousKeyView: this];
        }
    }
}

// Allows us to receive Cmd-. (the shortcut for closing a dialog)
// https://bugs.eclipse.org/bugs/show_bug.cgi?id=300620#c6
extern "C" fn cancel_operation(this: &Object, _sel: Sel, _sender: id) {
    trace_scope!("cancelOperation:");
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
    trace_scope!("mouseDown:");
    mouse_motion(this, event);
    mouse_click(this, event, MouseButton::Left, ElementState::Pressed);
}

extern "C" fn mouse_up(this: &Object, _sel: Sel, event: id) {
    trace_scope!("mouseUp:");
    mouse_motion(this, event);
    mouse_click(this, event, MouseButton::Left, ElementState::Released);
}

extern "C" fn right_mouse_down(this: &Object, _sel: Sel, event: id) {
    trace_scope!("rightMouseDown:");
    mouse_motion(this, event);
    mouse_click(this, event, MouseButton::Right, ElementState::Pressed);
}

extern "C" fn right_mouse_up(this: &Object, _sel: Sel, event: id) {
    trace_scope!("rightMouseUp:");
    mouse_motion(this, event);
    mouse_click(this, event, MouseButton::Right, ElementState::Released);
}

extern "C" fn other_mouse_down(this: &Object, _sel: Sel, event: id) {
    trace_scope!("otherMouseDown:");
    mouse_motion(this, event);
    mouse_click(this, event, MouseButton::Middle, ElementState::Pressed);
}

extern "C" fn other_mouse_up(this: &Object, _sel: Sel, event: id) {
    trace_scope!("otherMouseUp:");
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

// No tracing on these because that would be overly verbose

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
    trace_scope!("mouseEntered:");
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
}

extern "C" fn mouse_exited(this: &Object, _sel: Sel, _event: id) {
    trace_scope!("mouseExited:");
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
}

extern "C" fn scroll_wheel(this: &Object, _sel: Sel, event: id) {
    trace_scope!("scrollWheel:");

    mouse_motion(this, event);

    unsafe {
        let state_ptr: *mut c_void = *this.get_ivar("winitState");
        let state = &mut *(state_ptr as *mut ViewState);

        let delta = {
            let (x, y) = (event.scrollingDeltaX(), event.scrollingDeltaY());
            if event.hasPreciseScrollingDeltas() == YES {
                let delta = LogicalPosition::new(x, y).to_physical(state.get_scale_factor());
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
}

extern "C" fn magnify_with_event(this: &Object, _sel: Sel, event: id) {
    trace_scope!("magnifyWithEvent:");

    unsafe {
        let state_ptr: *mut c_void = *this.get_ivar("winitState");
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

extern "C" fn rotate_with_event(this: &Object, _sel: Sel, event: id) {
    trace_scope!("rotateWithEvent:");

    unsafe {
        let state_ptr: *mut c_void = *this.get_ivar("winitState");
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

extern "C" fn pressure_change_with_event(this: &Object, _sel: Sel, event: id) {
    trace_scope!("pressureChangeWithEvent:");

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
                stage: stage as i64,
            },
        };

        AppState::queue_event(EventWrapper::StaticEvent(window_event));
    }
}

// Allows us to receive Ctrl-Tab and Ctrl-Esc.
// Note that this *doesn't* help with any missing Cmd inputs.
// https://github.com/chromium/chromium/blob/a86a8a6bcfa438fa3ac2eba6f02b3ad1f8e0756f/ui/views/cocoa/bridged_content_view.mm#L816
extern "C" fn wants_key_down_for_event(_this: &Object, _sel: Sel, _event: id) -> BOOL {
    trace_scope!("_wantsKeyDownForEvent:");
    YES
}

extern "C" fn accepts_first_mouse(_this: &Object, _sel: Sel, _event: id) -> BOOL {
    trace_scope!("acceptsFirstMouse:");
    YES
}
