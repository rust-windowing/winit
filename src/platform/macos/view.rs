// This is a pretty close port of the implementation in GLFW:
// https://github.com/glfw/glfw/blob/7ef34eb06de54dd9186d3d21a401b2ef819b59e7/src/cocoa_window.m

use std::{slice, str};
use std::boxed::Box;
use std::collections::VecDeque;
use std::os::raw::*;
use std::sync::Weak;

use cocoa::base::{class, id, nil};
use cocoa::appkit::NSWindow;
use cocoa::foundation::{NSPoint, NSRect, NSSize, NSString, NSUInteger};
use objc::declare::ClassDecl;
use objc::runtime::{Class, Object, Protocol, Sel, BOOL};

use {ElementState, Event, KeyboardInput, WindowEvent, WindowId};
use platform::platform::events_loop::{DEVICE_ID, event_mods, Shared, to_virtual_key_code};
use platform::platform::util;
use platform::platform::ffi::*;
use platform::platform::window::{get_window_id, IdRef};

struct ViewState {
    window: id,
    shared: Weak<Shared>,
    ime_spot: Option<(i32, i32)>,
    raw_characters: Option<String>,
}

pub fn new_view(window: id, shared: Weak<Shared>) -> IdRef {
    let state = ViewState { window, shared, ime_spot: None, raw_characters: None };
    unsafe {
        // This is free'd in `dealloc`
        let state_ptr = Box::into_raw(Box::new(state)) as *mut c_void;
        let view: id = msg_send![VIEW_CLASS.0, alloc];
        IdRef::new(msg_send![view, initWithWinit:state_ptr])
    }
}

pub fn set_ime_spot(view: id, input_context: id, x: i32, y: i32) {
    unsafe {
        let state_ptr: *mut c_void = *(*view).get_mut_ivar("winitState");
        let state = &mut *(state_ptr as *mut ViewState);
        let content_rect = NSWindow::contentRectForFrameRect_(
            state.window,
            NSWindow::frame(state.window),
        );
        let base_x = content_rect.origin.x as i32;
        let base_y = (content_rect.origin.y + content_rect.size.height) as i32;
        state.ime_spot = Some((base_x + x, base_y - y));
        let _: () = msg_send![input_context, invalidateCharacterCoordinates];
    }
}

struct ViewClass(*const Class);
unsafe impl Send for ViewClass {}
unsafe impl Sync for ViewClass {}

lazy_static! {
    static ref VIEW_CLASS: ViewClass = unsafe {
        let superclass = Class::get("NSView").unwrap();
        let mut decl = ClassDecl::new("WinitView", superclass).unwrap();
        decl.add_method(sel!(dealloc), dealloc as extern fn(&Object, Sel));
        decl.add_method(
            sel!(initWithWinit:),
            init_with_winit as extern fn(&Object, Sel, *mut c_void) -> id,
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

extern fn has_marked_text(this: &Object, _sel: Sel) -> BOOL {
    unsafe {
        let marked_text: id = *this.get_ivar("markedText");
        (marked_text.length() > 0) as i8
    }
}

extern fn marked_range(this: &Object, _sel: Sel) -> NSRange {
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
    util::EMPTY_RANGE
}

extern fn set_marked_text(
    this: &mut Object,
    _sel: Sel,
    string: id,
    _selected_range: NSRange,
    _replacement_range: NSRange,
) {
    unsafe {
        let marked_text_ref: &mut id = this.get_mut_ivar("markedText");
        let _: () = msg_send![(*marked_text_ref), release];
        let marked_text = NSMutableAttributedString::alloc(nil);
        let has_attr = msg_send![string, isKindOfClass:class("NSAttributedString")];
        if has_attr {
            marked_text.initWithAttributedString(string);
        } else {
            marked_text.initWithString(string);
        };
        *marked_text_ref = marked_text;
    }
}

extern fn unmark_text(this: &Object, _sel: Sel) {
    unsafe {
        let marked_text: id = *this.get_ivar("markedText");
        let mutable_string = marked_text.mutableString();
        let _: () = msg_send![mutable_string, setString:""];
        let input_context: id = msg_send![this, inputContext];
        let _: () = msg_send![input_context, discardMarkedText];
    }
}

extern fn valid_attributes_for_marked_text(_this: &Object, _sel: Sel) -> id {
    unsafe { msg_send![class("NSArray"), array] }
}

extern fn attributed_substring_for_proposed_range(
    _this: &Object,
    _sel: Sel,
    _range: NSRange,
    _actual_range: *mut c_void, // *mut NSRange
) -> id {
    nil
}

extern fn character_index_for_point(_this: &Object, _sel: Sel, _point: NSPoint) -> NSUInteger {
    0
}

extern fn first_rect_for_character_range(
    this: &Object,
    _sel: Sel,
    _range: NSRange,
    _actual_range: *mut c_void, // *mut NSRange
) -> NSRect {
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
            (x as i32, y as i32)
        });
        
        NSRect::new(
            NSPoint::new(x as _, y as _),
            NSSize::new(0.0, 0.0),
        )
    }
}

extern fn insert_text(this: &Object, _sel: Sel, string: id, _replacement_range: NSRange) {
    unsafe {
        let state_ptr: *mut c_void = *this.get_ivar("winitState");
        let state = &mut *(state_ptr as *mut ViewState);

        let has_attr = msg_send![string, isKindOfClass:class("NSAttributedString")];
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

        // We don't need this now, but it's here if that changes.
        //let event: id = msg_send![class("NSApp"), currentEvent];

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
            // 2) to make the newline character explicit (...not that it matters)
            events.push_back(Event::WindowEvent {
                window_id: WindowId(get_window_id(state.window)),
                event: WindowEvent::ReceivedCharacter('\n'),
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

extern fn key_down(this: &Object, _sel: Sel, event: id) {
    unsafe {
        let state_ptr: *mut c_void = *this.get_ivar("winitState");
        let state = &mut *(state_ptr as *mut ViewState);

        let keycode: c_ushort = msg_send![event, keyCode];
        let virtual_keycode = to_virtual_key_code(keycode);
        let scancode = keycode as u32;

        let window_event = Event::WindowEvent {
            window_id: WindowId(get_window_id(state.window)),
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

        state.raw_characters = {
            let characters: id = msg_send![event, characters];
            let slice = slice::from_raw_parts(
                characters.UTF8String() as *const c_uchar,
                characters.len(),
            );
            let string = str::from_utf8_unchecked(slice);
            Some(string.to_owned())
        };

        if let Some(shared) = state.shared.upgrade() {
            shared.pending_events
                .lock()
                .unwrap()
                .push_back(window_event);
        }

        let array: id = msg_send![class("NSArray"), arrayWithObject:event];
        let (): _ = msg_send![this, interpretKeyEvents:array];
    }
}

extern fn key_up(this: &Object, _sel: Sel, event: id) {
    unsafe {
        let state_ptr: *mut c_void = *this.get_ivar("winitState");
        let state = &mut *(state_ptr as *mut ViewState);

        let keycode: c_ushort = msg_send![event, keyCode];
        let virtual_keycode = to_virtual_key_code(keycode);
        let scancode = keycode as u32;
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
