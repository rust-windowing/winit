#![allow(dead_code, non_camel_case_types, non_snake_case)]

use std::os::raw::{c_int, c_char, c_void, c_ulong, c_double, c_long, c_ushort};
#[cfg(test)]
use std::mem;

pub type EM_BOOL = c_int;
pub type EM_UTF8 = c_char;
pub type EMSCRIPTEN_RESULT = c_int;

pub const EM_TRUE: EM_BOOL = 1;
pub const EM_FALSE: EM_BOOL = 0;

// values for EMSCRIPTEN_RESULT
pub const EMSCRIPTEN_RESULT_SUCCESS: c_int = 0;
pub const EMSCRIPTEN_RESULT_DEFERRED: c_int = 1;
pub const EMSCRIPTEN_RESULT_NOT_SUPPORTED: c_int = -1;
pub const EMSCRIPTEN_RESULT_FAILED_NOT_DEFERRED: c_int = -2;
pub const EMSCRIPTEN_RESULT_INVALID_TARGET: c_int = -3;
pub const EMSCRIPTEN_RESULT_UNKNOWN_TARGET: c_int = -4;
pub const EMSCRIPTEN_RESULT_INVALID_PARAM: c_int = -5;
pub const EMSCRIPTEN_RESULT_FAILED: c_int = -6;
pub const EMSCRIPTEN_RESULT_NO_DATA: c_int = -7;

// values for EMSCRIPTEN EVENT
pub const EMSCRIPTEN_EVENT_KEYPRESS: c_int = 1;
pub const EMSCRIPTEN_EVENT_KEYDOWN: c_int = 2;
pub const EMSCRIPTEN_EVENT_KEYUP: c_int = 3;
pub const EMSCRIPTEN_EVENT_CLICK: c_int = 4;
pub const EMSCRIPTEN_EVENT_MOUSEDOWN: c_int = 5;
pub const EMSCRIPTEN_EVENT_MOUSEUP: c_int = 6;
pub const EMSCRIPTEN_EVENT_DBLCLICK: c_int = 7;
pub const EMSCRIPTEN_EVENT_MOUSEMOVE: c_int = 8;
pub const EMSCRIPTEN_EVENT_WHEEL: c_int = 9;
pub const EMSCRIPTEN_EVENT_RESIZE: c_int = 10;
pub const EMSCRIPTEN_EVENT_SCROLL: c_int = 11;
pub const EMSCRIPTEN_EVENT_BLUR: c_int = 12;
pub const EMSCRIPTEN_EVENT_FOCUS: c_int = 13;
pub const EMSCRIPTEN_EVENT_FOCUSIN: c_int = 14;
pub const EMSCRIPTEN_EVENT_FOCUSOUT: c_int = 15;
pub const EMSCRIPTEN_EVENT_DEVICEORIENTATION: c_int = 16;
pub const EMSCRIPTEN_EVENT_DEVICEMOTION: c_int = 17;
pub const EMSCRIPTEN_EVENT_ORIENTATIONCHANGE: c_int = 18;
pub const EMSCRIPTEN_EVENT_FULLSCREENCHANGE: c_int = 19;
pub const EMSCRIPTEN_EVENT_POINTERLOCKCHANGE: c_int = 20;
pub const EMSCRIPTEN_EVENT_VISIBILITYCHANGE: c_int = 21;
pub const EMSCRIPTEN_EVENT_TOUCHSTART: c_int = 22;
pub const EMSCRIPTEN_EVENT_TOUCHEND: c_int = 23;
pub const EMSCRIPTEN_EVENT_TOUCHMOVE: c_int = 24;
pub const EMSCRIPTEN_EVENT_TOUCHCANCEL: c_int = 25;
pub const EMSCRIPTEN_EVENT_GAMEPADCONNECTED: c_int = 26;
pub const EMSCRIPTEN_EVENT_GAMEPADDISCONNECTED: c_int = 27;
pub const EMSCRIPTEN_EVENT_BEFOREUNLOAD: c_int = 28;
pub const EMSCRIPTEN_EVENT_BATTERYCHARGINGCHANGE: c_int = 29;
pub const EMSCRIPTEN_EVENT_BATTERYLEVELCHANGE: c_int = 30;
pub const EMSCRIPTEN_EVENT_WEBGLCONTEXTLOST: c_int = 31;
pub const EMSCRIPTEN_EVENT_WEBGLCONTEXTRESTORED: c_int = 32;
pub const EMSCRIPTEN_EVENT_MOUSEENTER: c_int = 33;
pub const EMSCRIPTEN_EVENT_MOUSELEAVE: c_int = 34;
pub const EMSCRIPTEN_EVENT_MOUSEOVER: c_int = 35;
pub const EMSCRIPTEN_EVENT_MOUSEOUT: c_int = 36;
pub const EMSCRIPTEN_EVENT_CANVASRESIZED: c_int = 37;
pub const EMSCRIPTEN_EVENT_POINTERLOCKERROR: c_int = 38;

pub const EM_HTML5_SHORT_STRING_LEN_BYTES: usize = 32;

pub const DOM_KEY_LOCATION_STANDARD: c_ulong = 0x00;
pub const DOM_KEY_LOCATION_LEFT: c_ulong = 0x01;
pub const DOM_KEY_LOCATION_RIGHT: c_ulong = 0x02;
pub const DOM_KEY_LOCATION_NUMPAD: c_ulong = 0x03;

pub type em_callback_func = Option<unsafe extern "C" fn()>;

pub type em_key_callback_func = Option<unsafe extern "C" fn(
    eventType: c_int,
    keyEvent: *const EmscriptenKeyboardEvent,
    userData: *mut c_void) -> EM_BOOL>;

pub type em_mouse_callback_func = Option<unsafe extern "C" fn(
    eventType: c_int,
    mouseEvent: *const EmscriptenMouseEvent,
    userData: *mut c_void) -> EM_BOOL>;

pub type em_pointerlockchange_callback_func = Option<unsafe extern "C" fn(
    eventType: c_int,
    pointerlockChangeEvent: *const EmscriptenPointerlockChangeEvent,
    userData: *mut c_void) -> EM_BOOL>;

pub type em_fullscreenchange_callback_func = Option<unsafe extern "C" fn(
    eventType: c_int,
    fullscreenChangeEvent: *const EmscriptenFullscreenChangeEvent,
    userData: *mut c_void) -> EM_BOOL>;

pub type em_touch_callback_func = Option<unsafe extern "C" fn(
    eventType: c_int,
    touchEvent: *const EmscriptenTouchEvent,
    userData: *mut c_void) -> EM_BOOL>;

#[repr(C)]
pub struct EmscriptenFullscreenChangeEvent {
    pub isFullscreen: c_int,
    pub fullscreenEnabled: c_int,
    pub nodeName: [c_char; 128usize],
    pub id: [c_char; 128usize],
    pub elementWidth: c_int,
    pub elementHeight: c_int,
    pub screenWidth: c_int,
    pub screenHeight: c_int,
}
#[test]
fn bindgen_test_layout_EmscriptenFullscreenChangeEvent() {
    assert_eq!(mem::size_of::<EmscriptenFullscreenChangeEvent>(), 280usize);
    assert_eq!(mem::align_of::<EmscriptenFullscreenChangeEvent>(), 4usize);
}

#[repr(C)]
#[derive(Debug, Copy)]
pub struct EmscriptenKeyboardEvent {
    pub key: [c_char; 32usize],
    pub code: [c_char; 32usize],
    pub location: c_ulong,
    pub ctrlKey: c_int,
    pub shiftKey: c_int,
    pub altKey: c_int,
    pub metaKey: c_int,
    pub repeat: c_int,
    pub locale: [c_char; 32usize],
    pub charValue: [c_char; 32usize],
    pub charCode: c_ulong,
    pub keyCode: c_ulong,
    pub which: c_ulong,
}
#[test]
fn bindgen_test_layout_EmscriptenKeyboardEvent() {
    assert_eq!(mem::size_of::<EmscriptenKeyboardEvent>(), 184usize);
    assert_eq!(mem::align_of::<EmscriptenKeyboardEvent>(), 8usize);
}
impl Clone for EmscriptenKeyboardEvent {
    fn clone(&self) -> Self { *self }
}

#[repr(C)]
#[derive(Debug, Copy, Clone)]
pub struct EmscriptenMouseEvent {
    pub timestamp: f64,
    pub screenX: c_long,
    pub screenY: c_long,
    pub clientX: c_long,
    pub clientY: c_long,
    pub ctrlKey: c_int,
    pub shiftKey: c_int,
    pub altKey: c_int,
    pub metaKey: c_int,
    pub button: c_ushort,
    pub buttons: c_ushort,
    pub movementX: c_long,
    pub movementY: c_long,
    pub targetX: c_long,
    pub targetY: c_long,
    pub canvasX: c_long,
    pub canvasY: c_long,
    pub padding: c_long,
}
#[test]
fn bindgen_test_layout_EmscriptenMouseEvent() {
    assert_eq!(mem::size_of::<EmscriptenMouseEvent>(), 120usize);
    assert_eq!(mem::align_of::<EmscriptenMouseEvent>(), 8usize);
}

#[repr(C)]
#[derive(Debug, Copy, Clone)]
pub struct EmscriptenTouchPoint {
    pub identifier: c_long,
    pub screenX: c_long,
    pub screenY: c_long,
    pub clientX: c_long,
    pub clientY: c_long,
    pub pageX: c_long,
    pub pageY: c_long,
    pub isChanged: c_int,
    pub onTarget: c_int,
    pub targetX: c_long,
    pub targetY: c_long,
    pub canvasX: c_long,
    pub canvasY: c_long,
}
#[test]
fn bindgen_test_layout_EmscriptenTouchPoint() {
    assert_eq!(mem::size_of::<EmscriptenTouchPoint>(), 96usize);
    assert_eq!(mem::align_of::<EmscriptenTouchPoint>(), 8usize);
}

#[repr(C)]
#[derive(Debug, Copy, Clone)]
pub struct EmscriptenTouchEvent {
    pub numTouches: c_int,
    pub ctrlKey: c_int,
    pub shiftKey: c_int,
    pub altKey: c_int,
    pub metaKey: c_int,
    pub touches: [EmscriptenTouchPoint; 32usize],
}
#[test]
fn bindgen_test_layout_EmscriptenTouchEvent() {
    assert_eq!(mem::size_of::<EmscriptenTouchEvent>(), 3096usize);
    assert_eq!(mem::align_of::<EmscriptenTouchEvent>(), 8usize);
}

#[repr(C)]
pub struct EmscriptenPointerlockChangeEvent {
    pub isActive: c_int,
    pub nodeName: [c_char; 128usize],
    pub id: [c_char; 128usize],
}
#[test]
fn bindgen_test_layout_EmscriptenPointerlockChangeEvent() {
    assert_eq!(mem::size_of::<EmscriptenPointerlockChangeEvent>(), 260usize);
    assert_eq!(mem::align_of::<EmscriptenPointerlockChangeEvent>(), 4usize);
}

extern "C" {
    pub fn emscripten_set_canvas_size(
        width: c_int, height: c_int)
        -> EMSCRIPTEN_RESULT;

    pub fn emscripten_get_canvas_size(
        width: *mut c_int, height: *mut c_int,
        is_fullscreen: *mut c_int)
        -> EMSCRIPTEN_RESULT;

    pub fn emscripten_set_element_css_size(
        target: *const c_char, width: c_double,
        height: c_double) -> EMSCRIPTEN_RESULT;

    pub fn emscripten_get_element_css_size(
        target: *const c_char, width: *mut c_double,
        height: *mut c_double) -> EMSCRIPTEN_RESULT;

    pub fn emscripten_request_pointerlock(
        target: *const c_char, deferUntilInEventHandler: EM_BOOL)
        -> EMSCRIPTEN_RESULT;

    pub fn emscripten_exit_pointerlock() -> EMSCRIPTEN_RESULT;

    pub fn emscripten_request_fullscreen(
        target: *const c_char, deferUntilInEventHandler: EM_BOOL)
        -> EMSCRIPTEN_RESULT;

    pub fn emscripten_exit_fullscreen() -> EMSCRIPTEN_RESULT;

    pub fn emscripten_set_keydown_callback(
        target: *const c_char, userData: *mut c_void,
        useCapture: EM_BOOL, callback: em_key_callback_func)
        -> EMSCRIPTEN_RESULT;

    pub fn emscripten_set_keyup_callback(
        target: *const c_char, userData: *mut c_void,
        useCapture: EM_BOOL, callback: em_key_callback_func)
        -> EMSCRIPTEN_RESULT;

    pub fn emscripten_set_mousemove_callback(
        target: *const c_char, user_data: *mut c_void,
        use_capture: EM_BOOL, callback: em_mouse_callback_func)
        -> EMSCRIPTEN_RESULT;

    pub fn emscripten_set_mousedown_callback(
        target: *const c_char, user_data: *mut c_void,
        use_capture: EM_BOOL, callback: em_mouse_callback_func)
        -> EMSCRIPTEN_RESULT;

    pub fn emscripten_set_mouseup_callback(
        target: *const c_char, user_data: *mut c_void,
        use_capture: EM_BOOL, callback: em_mouse_callback_func)
        -> EMSCRIPTEN_RESULT;

    pub fn emscripten_hide_mouse();

    pub fn emscripten_get_device_pixel_ratio() -> f64;

    pub fn emscripten_set_pointerlockchange_callback(
        target: *const c_char, userData: *mut c_void, useCapture: EM_BOOL,
        callback: em_pointerlockchange_callback_func) -> EMSCRIPTEN_RESULT;

    pub fn emscripten_set_fullscreenchange_callback(
        target: *const c_char, userData: *mut c_void, useCapture: EM_BOOL,
        callback: em_fullscreenchange_callback_func) -> EMSCRIPTEN_RESULT;

    pub fn emscripten_asm_const(code: *const c_char);

    pub fn emscripten_set_main_loop(
        func: em_callback_func, fps: c_int, simulate_infinite_loop: EM_BOOL);

    pub fn emscripten_cancel_main_loop();

    pub fn emscripten_set_touchstart_callback(
        target: *const c_char, userData: *mut c_void,
        useCapture: c_int, callback: em_touch_callback_func)
        -> EMSCRIPTEN_RESULT;

    pub fn emscripten_set_touchend_callback(
        target: *const c_char, userData: *mut c_void,
        useCapture: c_int, callback: em_touch_callback_func)
        -> EMSCRIPTEN_RESULT;

    pub fn emscripten_set_touchmove_callback(
        target: *const c_char, userData: *mut c_void,
        useCapture: c_int, callback: em_touch_callback_func)
        -> EMSCRIPTEN_RESULT;

    pub fn emscripten_set_touchcancel_callback(
        target: *const c_char, userData: *mut c_void,
        useCapture: c_int, callback: em_touch_callback_func)
        -> EMSCRIPTEN_RESULT;
}
