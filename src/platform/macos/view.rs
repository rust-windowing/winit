// This is a pretty close port of the implementation in GLFW:
// https://github.com/glfw/glfw/blob/7ef34eb06de54dd9186d3d21a401b2ef819b59e7/src/cocoa_window.m

use std::{slice, str};
use std::boxed::Box;
use std::collections::VecDeque;
use std::os::raw::*;
use std::sync::{Arc, Mutex, Weak};

use cocoa::base::{id, nil};
use cocoa::appkit::{NSEvent, NSView, NSWindow};
use cocoa::foundation::{NSPoint, NSRect, NSSize, NSString, NSUInteger};
use objc::declare::ClassDecl;
use objc::runtime::{Class, Object, Protocol, Sel, BOOL, YES};

use {ElementState, Event, KeyboardInput, MouseButton, WindowEvent, WindowId};
use platform::platform::events_loop::{DEVICE_ID, event_mods, Shared, scancode_to_keycode, char_to_keycode, check_function_keys, get_scancode};
use platform::platform::util;
use platform::platform::ffi::*;
use platform::platform::window::{get_window_id, IdRef};
use events;

struct ViewState {
    window: id,
    shared: Weak<Shared>,
    cursor: Arc<Mutex<util::Cursor>>,
    ime_spot: Option<(f64, f64)>,
    raw_characters: Option<String>,
    is_key_down: bool,
}

pub fn new_view(window: id, shared: Weak<Shared>) -> (IdRef, Weak<Mutex<util::Cursor>>) {
    let cursor = Default::default();
    let cursor_access = Arc::downgrade(&cursor);
    let state = ViewState {
        window,
        shared,
        cursor,
        ime_spot: None,
        raw_characters: None,
        is_key_down: false,
    };
    unsafe {
        // This is free'd in `dealloc`
        let state_ptr = Box::into_raw(Box::new(state)) as *mut c_void;
        let view: id = msg_send![VIEW_CLASS.0, alloc];
        (IdRef::new(msg_send![view, initWithWinit:state_ptr]), cursor_access)
    }
}

pub fn set_ime_spot(view: id, input_context: id, x: f64, y: f64) {
    unsafe {
        let state_ptr: *mut c_void = *(*view).get_mut_ivar("winitState");
        let state = &mut *(state_ptr as *mut ViewState);
        let content_rect = NSWindow::contentRectForFrameRect_(
            state.window,
            NSWindow::frame(state.window),
        );
        let base_x = content_rect.origin.x as f64;
        let base_y = (content_rect.origin.y + content_rect.size.height) as f64;
        state.ime_spot = Some((base_x + x, base_y - y));
        let _: () = msg_send![input_context, invalidateCharacterCoordinates];
    }
}

struct ViewClass(*const Class);
unsafe impl Send for ViewClass {}
unsafe impl Sync for ViewClass {}

lazy_static! {
    static ref VIEW_CLASS: ViewClass = unsafe {
        let superclass = class!(NSView);
        let mut decl = ClassDecl::new("WinitView", superclass).unwrap();
        decl.add_method(sel!(dealloc), dealloc as extern fn(&Object, Sel));
        decl.add_method(
            sel!(initWithWinit:),
            init_with_winit as extern fn(&Object, Sel, *mut c_void) -> id,
        );
        decl.add_method(
            sel!(drawRect:),
            draw_rect as extern fn(&Object, Sel, NSRect),
        );
        decl.add_method(
            sel!(resetCursorRects),
            reset_cursor_rects as extern fn(&Object, Sel),
        );
        decl.add_method(sel!(hasMarkedText), has_marked_text as extern fn(&Object, Sel) -> BOOL);
        decl.add_method(
            sel!(markedRange),
            marked_range as extern fn(&Object, Sel) -> NSRange,
        );
        decl.add_method(sel!(selectedRange), selected_range as extern fn(&Object, Sel) -> NSRange);
        decl.add_method(
            sel!(setMarkedText:selectedRange:replacementRange:),
            set_marked_text as extern fn(&mut Object, Sel, id, NSRange, NSRange),
        );
        decl.add_method(sel!(unmarkText), unmark_text as extern fn(&Object, Sel));
        decl.add_method(
            sel!(validAttributesForMarkedText),
            valid_attributes_for_marked_text as extern fn(&Object, Sel) -> id,
        );
        decl.add_method(
            sel!(attributedSubstringForProposedRange:actualRange:),
            attributed_substring_for_proposed_range
                as extern fn(&Object, Sel, NSRange, *mut c_void) -> id,
        );
        decl.add_method(
            sel!(insertText:replacementRange:),
            insert_text as extern fn(&Object, Sel, id, NSRange),
        );
        decl.add_method(
            sel!(characterIndexForPoint:),
            character_index_for_point as extern fn(&Object, Sel, NSPoint) -> NSUInteger,
        );
        decl.add_method(
            sel!(firstRectForCharacterRange:actualRange:),
            first_rect_for_character_range
                as extern fn(&Object, Sel, NSRange, *mut c_void) -> NSRect,
        );
        decl.add_method(
            sel!(doCommandBySelector:),
            do_command_by_selector as extern fn(&Object, Sel, Sel),
        );
        decl.add_method(sel!(keyDown:), key_down as extern fn(&Object, Sel, id));
        decl.add_method(sel!(keyUp:), key_up as extern fn(&Object, Sel, id));
        decl.add_method(sel!(insertTab:), insert_tab as extern fn(&Object, Sel, id));
        decl.add_method(sel!(insertBackTab:), insert_back_tab as extern fn(&Object, Sel, id));
        decl.add_method(sel!(mouseDown:), mouse_down as extern fn(&Object, Sel, id));
        decl.add_method(sel!(mouseUp:), mouse_up as extern fn(&Object, Sel, id));
        decl.add_method(sel!(rightMouseDown:), right_mouse_down as extern fn(&Object, Sel, id));
        decl.add_method(sel!(rightMouseUp:), right_mouse_up as extern fn(&Object, Sel, id));
        decl.add_method(sel!(otherMouseDown:), other_mouse_down as extern fn(&Object, Sel, id));
        decl.add_method(sel!(otherMouseUp:), other_mouse_up as extern fn(&Object, Sel, id));
        decl.add_method(sel!(mouseMoved:), mouse_moved as extern fn(&Object, Sel, id));
        decl.add_method(sel!(mouseDragged:), mouse_dragged as extern fn(&Object, Sel, id));
        decl.add_method(sel!(rightMouseDragged:), right_mouse_dragged as extern fn(&Object, Sel, id));
        decl.add_method(sel!(otherMouseDragged:), other_mouse_dragged as extern fn(&Object, Sel, id));
        decl.add_method(sel!(_wantsKeyDownForEvent:), wants_key_down_for_event as extern fn(&Object, Sel, id) -> BOOL);
        decl.add_ivar::<*mut c_void>("winitState");
        decl.add_ivar::<id>("markedText");
        let protocol = Protocol::get("NSTextInputClient").unwrap();
        decl.add_protocol(&protocol);
        ViewClass(decl.register())
    };
}

extern fn dealloc(this: &Object, _sel: Sel) {
    unsafe {
        let state: *mut c_void = *this.get_ivar("winitState");
        let marked_text: id = *this.get_ivar("markedText");
        let _: () = msg_send![marked_text, release];
        Box::from_raw(state as *mut ViewState);
    }
}

extern fn init_with_winit(this: &Object, _sel: Sel, state: *mut c_void) -> id {
    unsafe {
        let this: id = msg_send![this, init];
        if this != nil {
            (*this).set_ivar("winitState", state);
            let marked_text = <id as NSMutableAttributedString>::init(
                NSMutableAttributedString::alloc(nil),
            );
            (*this).set_ivar("markedText", marked_text);
        }
        this
    }
}

extern fn draw_rect(this: &Object, _sel: Sel, rect: NSRect) {
    unsafe {
        let state_ptr: *mut c_void = *this.get_ivar("winitState");
        let state = &mut *(state_ptr as *mut ViewState);

        if let Some(shared) = state.shared.upgrade() {
            let window_event = Event::WindowEvent {
                window_id: WindowId(get_window_id(state.window)),
                event: WindowEvent::Refresh,
            };
            shared.pending_events
                .lock()
                .unwrap()
                .push_back(window_event);
        }

        let superclass = util::superclass(this);
        let () = msg_send![super(this, superclass), drawRect:rect];
    }
}

extern fn reset_cursor_rects(this: &Object, _sel: Sel) {
    unsafe {
        let state_ptr: *mut c_void = *this.get_ivar("winitState");
        let state = &mut *(state_ptr as *mut ViewState);

        let bounds: NSRect = msg_send![this, bounds];
        let cursor = state.cursor.lock().unwrap().load();
        let _: () = msg_send![this,
            addCursorRect:bounds
            cursor:cursor
        ];
    }
}

extern fn has_marked_text(this: &Object, _sel: Sel) -> BOOL {
    //println!("hasMarkedText");
    unsafe {
        let marked_text: id = *this.get_ivar("markedText");
        (marked_text.length() > 0) as i8
    }
}

extern fn marked_range(this: &Object, _sel: Sel) -> NSRange {
    //println!("markedRange");
    unsafe {
        let marked_text: id = *this.get_ivar("markedText");
        let length = marked_text.length();
        if length > 0 {
            NSRange::new(0, length - 1)
        } else {
            util::EMPTY_RANGE
        }
    }
}

extern fn selected_range(_this: &Object, _sel: Sel) -> NSRange {
    //println!("selectedRange");
    util::EMPTY_RANGE
}

extern fn set_marked_text(
    this: &mut Object,
    _sel: Sel,
    string: id,
    _selected_range: NSRange,
    _replacement_range: NSRange,
) {
    //println!("setMarkedText");
    unsafe {
        let marked_text_ref: &mut id = this.get_mut_ivar("markedText");
        let _: () = msg_send![(*marked_text_ref), release];
        let marked_text = NSMutableAttributedString::alloc(nil);
        let has_attr = msg_send![string, isKindOfClass:class!(NSAttributedString)];
        if has_attr {
            marked_text.initWithAttributedString(string);
        } else {
            marked_text.initWithString(string);
        };
        *marked_text_ref = marked_text;
    }
}

extern fn unmark_text(this: &Object, _sel: Sel) {
    //println!("unmarkText");
    unsafe {
        let marked_text: id = *this.get_ivar("markedText");
        let mutable_string = marked_text.mutableString();
        let _: () = msg_send![mutable_string, setString:""];
        let input_context: id = msg_send![this, inputContext];
        let _: () = msg_send![input_context, discardMarkedText];
    }
}

extern fn valid_attributes_for_marked_text(_this: &Object, _sel: Sel) -> id {
    //println!("validAttributesForMarkedText");
    unsafe { msg_send![class!(NSArray), array] }
}

extern fn attributed_substring_for_proposed_range(
    _this: &Object,
    _sel: Sel,
    _range: NSRange,
    _actual_range: *mut c_void, // *mut NSRange
) -> id {
    //println!("attributedSubstringForProposedRange");
    nil
}

extern fn character_index_for_point(_this: &Object, _sel: Sel, _point: NSPoint) -> NSUInteger {
    //println!("characterIndexForPoint");
    0
}

extern fn first_rect_for_character_range(
    this: &Object,
    _sel: Sel,
    _range: NSRange,
    _actual_range: *mut c_void, // *mut NSRange
) -> NSRect {
    //println!("firstRectForCharacterRange");
    unsafe {
        let state_ptr: *mut c_void = *this.get_ivar("winitState");
        let state = &mut *(state_ptr as *mut ViewState);
        let (x, y) = state.ime_spot.unwrap_or_else(|| {
            let content_rect = NSWindow::contentRectForFrameRect_(
                state.window,
                NSWindow::frame(state.window),
            );
            let x = content_rect.origin.x;
            let y = util::bottom_left_to_top_left(content_rect);
            (x, y)
        });

        NSRect::new(
            NSPoint::new(x as _, y as _),
            NSSize::new(0.0, 0.0),
        )
    }
}

extern fn insert_text(this: &Object, _sel: Sel, string: id, _replacement_range: NSRange) {
    //println!("insertText");
    unsafe {
        let state_ptr: *mut c_void = *this.get_ivar("winitState");
        let state = &mut *(state_ptr as *mut ViewState);

        let has_attr = msg_send![string, isKindOfClass:class!(NSAttributedString)];
        let characters = if has_attr {
            // This is a *mut NSAttributedString
            msg_send![string, string]
        } else {
            // This is already a *mut NSString
            string
        };

        let slice = slice::from_raw_parts(
            characters.UTF8String() as *const c_uchar,
            characters.len(),
        );
        let string = str::from_utf8_unchecked(slice);
        state.is_key_down = true;

        // We don't need this now, but it's here if that changes.
        //let event: id = msg_send![class!(NSApp), currentEvent];

        let mut events = VecDeque::with_capacity(characters.len());
        for character in string.chars() {
            events.push_back(Event::WindowEvent {
                window_id: WindowId(get_window_id(state.window)),
                event: WindowEvent::ReceivedCharacter(character),
            });
        }

        if let Some(shared) = state.shared.upgrade() {
            shared.pending_events
                .lock()
                .unwrap()
                .append(&mut events);
        }
    }
}

extern fn do_command_by_selector(this: &Object, _sel: Sel, command: Sel) {
    //println!("doCommandBySelector");
    // Basically, we're sent this message whenever a keyboard event that doesn't generate a "human readable" character
    // happens, i.e. newlines, tabs, and Ctrl+C.
    unsafe {
        let state_ptr: *mut c_void = *this.get_ivar("winitState");
        let state = &mut *(state_ptr as *mut ViewState);

        let shared = if let Some(shared) = state.shared.upgrade() {
            shared
        } else {
            return;
        };

        let mut events = VecDeque::with_capacity(1);
        if command == sel!(insertNewline:) {
            // The `else` condition would emit the same character, but I'm keeping this here both...
            // 1) as a reminder for how `doCommandBySelector` works
            // 2) to make our use of carriage return explicit
            events.push_back(Event::WindowEvent {
                window_id: WindowId(get_window_id(state.window)),
                event: WindowEvent::ReceivedCharacter('\r'),
            });
        } else {
            let raw_characters = state.raw_characters.take();
            if let Some(raw_characters) = raw_characters {
                for character in raw_characters.chars() {
                    events.push_back(Event::WindowEvent {
                        window_id: WindowId(get_window_id(state.window)),
                        event: WindowEvent::ReceivedCharacter(character),
                    });
                }
            }
        };

        shared.pending_events
            .lock()
            .unwrap()
            .append(&mut events);
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
        let slice = slice::from_raw_parts(
            characters.UTF8String() as *const c_uchar,
            characters.len(),
        );

        let string = str::from_utf8_unchecked(slice);

        string.to_owned()
    }
}

// Retrieves a layout-independent keycode given an event.
fn retrieve_keycode(event: id) -> Option<events::VirtualKeyCode> {
    #[inline]
    fn get_code(ev: id, raw: bool) -> Option<events::VirtualKeyCode> {
        let characters = get_characters(ev, raw);
        characters.chars().next().map_or(None, |c| char_to_keycode(c))
    }

    // Cmd switches Roman letters for Dvorak-QWERTY layout, so we try modified characters first.
    // If we don't get a match, then we fall back to unmodified characters.
    let code = get_code(event, false)
        .or_else(|| {
            get_code(event, true)
        });

    // We've checked all layout related keys, so fall through to scancode.
    // Reaching this code means that the key is layout-independent (e.g. Backspace, Return).
    //
    // We're additionally checking here for F21-F24 keys, since their keycode
    // can vary, but we know that they are encoded
    // in characters property.
    code.or_else(|| {
        let scancode = get_scancode(event);
        scancode_to_keycode(scancode)
            .or_else(|| {
                check_function_keys(&get_characters(event, true))
            })
    })
}

extern fn key_down(this: &Object, _sel: Sel, event: id) {
    //println!("keyDown");
    unsafe {
        let state_ptr: *mut c_void = *this.get_ivar("winitState");
        let state = &mut *(state_ptr as *mut ViewState);
        let window_id = WindowId(get_window_id(state.window));
        let characters = get_characters(event, false);

        state.raw_characters = Some(characters.clone());

        let scancode = get_scancode(event) as u32;
        let virtual_keycode = retrieve_keycode(event);
        let is_repeat = msg_send![event, isARepeat];

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
            },
        };

        if let Some(shared) = state.shared.upgrade() {
            shared.pending_events
                .lock()
                .unwrap()
                .push_back(window_event);
            // Emit `ReceivedCharacter` for key repeats
            if is_repeat && state.is_key_down{
                for character in characters.chars() {
                    let window_event = Event::WindowEvent {
                        window_id,
                        event: WindowEvent::ReceivedCharacter(character),
                    };
                    shared.pending_events
                        .lock()
                        .unwrap()
                        .push_back(window_event);
                }
            } else {
                // Some keys (and only *some*, with no known reason) don't trigger `insertText`, while others do...
                // So, we don't give repeats the opportunity to trigger that, since otherwise our hack will cause some
                // keys to generate twice as many characters.
                let array: id = msg_send![class!(NSArray), arrayWithObject:event];
                let (): _ = msg_send![this, interpretKeyEvents:array];
            }
        }
    }
}

extern fn key_up(this: &Object, _sel: Sel, event: id) {
    //println!("keyUp");
    unsafe {
        let state_ptr: *mut c_void = *this.get_ivar("winitState");
        let state = &mut *(state_ptr as *mut ViewState);

        state.is_key_down = false;

        let scancode = get_scancode(event) as u32;
        let virtual_keycode = retrieve_keycode(event);

        let window_event = Event::WindowEvent {
            window_id: WindowId(get_window_id(state.window)),
            event: WindowEvent::KeyboardInput {
                device_id: DEVICE_ID,
                input: KeyboardInput {
                    state: ElementState::Released,
                    scancode,
                    virtual_keycode,
                    modifiers: event_mods(event),
                },
            },
        };

        if let Some(shared) = state.shared.upgrade() {
            shared.pending_events
                .lock()
                .unwrap()
                .push_back(window_event);
        }
    }
}

extern fn insert_tab(this: &Object, _sel: Sel, _sender: id) {
    unsafe {
        let window: id = msg_send![this, window];
        let first_responder: id = msg_send![window, firstResponder];
        let this_ptr = this as *const _ as *mut _;
        if first_responder == this_ptr {
            let (): _ = msg_send![window, selectNextKeyView:this];
        }
    }
}

extern fn insert_back_tab(this: &Object, _sel: Sel, _sender: id) {
    unsafe {
        let window: id = msg_send![this, window];
        let first_responder: id = msg_send![window, firstResponder];
        let this_ptr = this as *const _ as *mut _;
        if first_responder == this_ptr {
            let (): _ = msg_send![window, selectPreviousKeyView:this];
        }
    }
}

fn mouse_click(this: &Object, event: id, button: MouseButton, button_state: ElementState) {
    unsafe {
        let state_ptr: *mut c_void = *this.get_ivar("winitState");
        let state = &mut *(state_ptr as *mut ViewState);

        let window_event = Event::WindowEvent {
            window_id: WindowId(get_window_id(state.window)),
            event: WindowEvent::MouseInput {
                device_id: DEVICE_ID,
                state: button_state,
                button,
                modifiers: event_mods(event),
            },
        };

        if let Some(shared) = state.shared.upgrade() {
            shared.pending_events
                .lock()
                .unwrap()
                .push_back(window_event);
        }
    }
}

extern fn mouse_down(this: &Object, _sel: Sel, event: id) {
    mouse_click(this, event, MouseButton::Left, ElementState::Pressed);
}

extern fn mouse_up(this: &Object, _sel: Sel, event: id) {
    mouse_click(this, event, MouseButton::Left, ElementState::Released);
}

extern fn right_mouse_down(this: &Object, _sel: Sel, event: id) {
    mouse_click(this, event, MouseButton::Right, ElementState::Pressed);
}

extern fn right_mouse_up(this: &Object, _sel: Sel, event: id) {
    mouse_click(this, event, MouseButton::Right, ElementState::Released);
}

extern fn other_mouse_down(this: &Object, _sel: Sel, event: id) {
    mouse_click(this, event, MouseButton::Middle, ElementState::Pressed);
}

extern fn other_mouse_up(this: &Object, _sel: Sel, event: id) {
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
        || view_point.y > view_rect.size.height {
            // Point is outside of the client area (view)
            return;
        }

        let x = view_point.x as f64;
        let y = view_rect.size.height as f64 - view_point.y as f64;

        let window_event = Event::WindowEvent {
            window_id: WindowId(get_window_id(state.window)),
            event: WindowEvent::CursorMoved {
                device_id: DEVICE_ID,
                position: (x, y).into(),
                modifiers: event_mods(event),
            },
        };

        if let Some(shared) = state.shared.upgrade() {
            shared.pending_events
                .lock()
                .unwrap()
                .push_back(window_event);
        }
    }
}

extern fn mouse_moved(this: &Object, _sel: Sel, event: id) {
    mouse_motion(this, event);
}

extern fn mouse_dragged(this: &Object, _sel: Sel, event: id) {
    mouse_motion(this, event);
}

extern fn right_mouse_dragged(this: &Object, _sel: Sel, event: id) {
    mouse_motion(this, event);
}

extern fn other_mouse_dragged(this: &Object, _sel: Sel, event: id) {
    mouse_motion(this, event);
}

// https://github.com/chromium/chromium/blob/a86a8a6bcfa438fa3ac2eba6f02b3ad1f8e0756f/ui/views/cocoa/bridged_content_view.mm#L816
extern fn wants_key_down_for_event(_this: &Object, _se: Sel, _event: id) -> BOOL {
    YES
}
