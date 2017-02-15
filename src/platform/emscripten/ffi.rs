#![allow(dead_code)]
#![allow(non_snake_case)]
#![allow(non_camel_case_types)]

pub type EM_BOOL = ::libc::c_int;
pub type EM_UTF8 = ::libc::c_char;
pub type EMSCRIPTEN_RESULT = ::libc::c_int;

pub const EM_TRUE: EM_BOOL = 1;
pub const EM_FALSE: EM_BOOL = 0;

// values for EMSCRIPTEN_RESULT
pub const EMSCRIPTEN_RESULT_SUCCESS: ::libc::c_int = 0;
pub const EMSCRIPTEN_RESULT_DEFERRED: ::libc::c_int = 1;
pub const EMSCRIPTEN_RESULT_NOT_SUPPORTED: ::libc::c_int = -1;
pub const EMSCRIPTEN_RESULT_FAILED_NOT_DEFERRED: ::libc::c_int = -2;
pub const EMSCRIPTEN_RESULT_INVALID_TARGET: ::libc::c_int = -3;
pub const EMSCRIPTEN_RESULT_UNKNOWN_TARGET: ::libc::c_int = -4;
pub const EMSCRIPTEN_RESULT_INVALID_PARAM: ::libc::c_int = -5;
pub const EMSCRIPTEN_RESULT_FAILED: ::libc::c_int = -6;
pub const EMSCRIPTEN_RESULT_NO_DATA: ::libc::c_int = -7;

// values for EMSCRIPTEN EVENT
pub const EMSCRIPTEN_EVENT_KEYPRESS: ::libc::c_int = 1;
pub const EMSCRIPTEN_EVENT_KEYDOWN: ::libc::c_int = 2;
pub const EMSCRIPTEN_EVENT_KEYUP: ::libc::c_int = 3;
pub const EMSCRIPTEN_EVENT_CLICK: ::libc::c_int = 4;
pub const EMSCRIPTEN_EVENT_MOUSEDOWN: ::libc::c_int = 5;
pub const EMSCRIPTEN_EVENT_MOUSEUP: ::libc::c_int = 6;
pub const EMSCRIPTEN_EVENT_DBLCLICK: ::libc::c_int = 7;
pub const EMSCRIPTEN_EVENT_MOUSEMOVE: ::libc::c_int = 8;
pub const EMSCRIPTEN_EVENT_WHEEL: ::libc::c_int = 9;
pub const EMSCRIPTEN_EVENT_RESIZE: ::libc::c_int = 10;
pub const EMSCRIPTEN_EVENT_SCROLL: ::libc::c_int = 11;
pub const EMSCRIPTEN_EVENT_BLUR: ::libc::c_int = 12;
pub const EMSCRIPTEN_EVENT_FOCUS: ::libc::c_int = 13;
pub const EMSCRIPTEN_EVENT_FOCUSIN: ::libc::c_int = 14;
pub const EMSCRIPTEN_EVENT_FOCUSOUT: ::libc::c_int = 15;
pub const EMSCRIPTEN_EVENT_DEVICEORIENTATION: ::libc::c_int = 16;
pub const EMSCRIPTEN_EVENT_DEVICEMOTION: ::libc::c_int = 17;
pub const EMSCRIPTEN_EVENT_ORIENTATIONCHANGE: ::libc::c_int = 18;
pub const EMSCRIPTEN_EVENT_FULLSCREENCHANGE: ::libc::c_int = 19;
pub const EMSCRIPTEN_EVENT_POINTERLOCKCHANGE: ::libc::c_int = 20;
pub const EMSCRIPTEN_EVENT_VISIBILITYCHANGE: ::libc::c_int = 21;
pub const EMSCRIPTEN_EVENT_TOUCHSTART: ::libc::c_int = 22;
pub const EMSCRIPTEN_EVENT_TOUCHEND: ::libc::c_int = 23;
pub const EMSCRIPTEN_EVENT_TOUCHMOVE: ::libc::c_int = 24;
pub const EMSCRIPTEN_EVENT_TOUCHCANCEL: ::libc::c_int = 25;
pub const EMSCRIPTEN_EVENT_GAMEPADCONNECTED: ::libc::c_int = 26;
pub const EMSCRIPTEN_EVENT_GAMEPADDISCONNECTED: ::libc::c_int = 27;
pub const EMSCRIPTEN_EVENT_BEFOREUNLOAD: ::libc::c_int = 28;
pub const EMSCRIPTEN_EVENT_BATTERYCHARGINGCHANGE: ::libc::c_int = 29;
pub const EMSCRIPTEN_EVENT_BATTERYLEVELCHANGE: ::libc::c_int = 30;
pub const EMSCRIPTEN_EVENT_WEBGLCONTEXTLOST: ::libc::c_int = 31;
pub const EMSCRIPTEN_EVENT_WEBGLCONTEXTRESTORED: ::libc::c_int = 32;
pub const EMSCRIPTEN_EVENT_MOUSEENTER: ::libc::c_int = 33;
pub const EMSCRIPTEN_EVENT_MOUSELEAVE: ::libc::c_int = 34;
pub const EMSCRIPTEN_EVENT_MOUSEOVER: ::libc::c_int = 35;
pub const EMSCRIPTEN_EVENT_MOUSEOUT: ::libc::c_int = 36;
pub const EMSCRIPTEN_EVENT_CANVASRESIZED: ::libc::c_int = 37;
pub const EMSCRIPTEN_EVENT_POINTERLOCKERROR: ::libc::c_int = 38;

pub const EM_HTML5_SHORT_STRING_LEN_BYTES: usize = 32;

pub type em_callback_func = Option<unsafe extern "C" fn()>;

pub type em_key_callback_func = Option<unsafe extern "C" fn(
    eventType: ::libc::c_int,
    keyEvent: *const EmscriptenKeyboardEvent,
    userData: *mut ::libc::c_void) -> EM_BOOL>;

pub type em_pointerlockchange_callback_func = Option<unsafe extern "C" fn(
    eventType: ::libc::c_int,
    pointerlockChangeEvent: *const EmscriptenPointerlockChangeEvent,
    userData: *mut ::libc::c_void) -> EM_BOOL>;

pub type em_fullscreenchange_callback_func = Option<unsafe extern "C" fn(
    eventType: ::libc::c_int,
    fullscreenChangeEvent: *const EmscriptenFullscreenChangeEvent,
    userData: *mut ::libc::c_void) -> EM_BOOL>;

#[repr(C)]
pub struct EmscriptenFullscreenChangeEvent {
    pub isFullscreen: ::libc::c_int,
    pub fullscreenEnabled: ::libc::c_int,
    pub nodeName: [::libc::c_char; 128usize],
    pub id: [::libc::c_char; 128usize],
    pub elementWidth: ::libc::c_int,
    pub elementHeight: ::libc::c_int,
    pub screenWidth: ::libc::c_int,
    pub screenHeight: ::libc::c_int,
}
#[test]
fn bindgen_test_layout_EmscriptenFullscreenChangeEvent() {
    assert_eq!(::std::mem::size_of::<EmscriptenFullscreenChangeEvent>(), 280usize);
    assert_eq!(::std::mem::align_of::<EmscriptenFullscreenChangeEvent>(), 4usize);
}

#[repr(C)]
#[derive(Debug, Copy)]
pub struct EmscriptenKeyboardEvent {
    pub key: [::libc::c_char; 32usize],
    pub code: [::libc::c_char; 32usize],
    pub location: ::libc::c_ulong,
    pub ctrlKey: ::libc::c_int,
    pub shiftKey: ::libc::c_int,
    pub altKey: ::libc::c_int,
    pub metaKey: ::libc::c_int,
    pub repeat: ::libc::c_int,
    pub locale: [::libc::c_char; 32usize],
    pub charValue: [::libc::c_char; 32usize],
    pub charCode: ::libc::c_ulong,
    pub keyCode: ::libc::c_ulong,
    pub which: ::libc::c_ulong,
}
#[test]
fn bindgen_test_layout_EmscriptenKeyboardEvent() {
    assert_eq!(::std::mem::size_of::<EmscriptenKeyboardEvent>(), 184usize);
    assert_eq!(::std::mem::align_of::<EmscriptenKeyboardEvent>(), 8usize);
}
impl Clone for EmscriptenKeyboardEvent {
    fn clone(&self) -> Self { *self }
}

#[repr(C)]
pub struct EmscriptenPointerlockChangeEvent {
    pub isActive: ::libc::c_int,
    pub nodeName: [::libc::c_char; 128usize],
    pub id: [::libc::c_char; 128usize],
}
#[test]
fn bindgen_test_layout_EmscriptenPointerlockChangeEvent() {
    assert_eq!(::std::mem::size_of::<EmscriptenPointerlockChangeEvent>(), 260usize);
    assert_eq!(::std::mem::align_of::<EmscriptenPointerlockChangeEvent>(), 4usize);
}

extern "C" {
    pub fn emscripten_set_element_css_size(
        target: *const ::libc::c_char, width: ::libc::c_double,
        height: ::libc::c_double) -> EMSCRIPTEN_RESULT;

    pub fn emscripten_get_element_css_size(
        target: *const ::libc::c_char, width: *mut ::libc::c_double,
        height: *mut ::libc::c_double) -> EMSCRIPTEN_RESULT;

    pub fn emscripten_request_pointerlock(
        target: *const ::libc::c_char, deferUntilInEventHandler: EM_BOOL)
        -> EMSCRIPTEN_RESULT;

    pub fn emscripten_exit_pointerlock() -> EMSCRIPTEN_RESULT;

    pub fn emscripten_request_fullscreen(
        target: *const ::libc::c_char, deferUntilInEventHandler: EM_BOOL)
        -> EMSCRIPTEN_RESULT;

    pub fn emscripten_exit_fullscreen() -> EMSCRIPTEN_RESULT;

    pub fn emscripten_set_keydown_callback(
        target: *const ::libc::c_char, userData: *mut ::libc::c_void,
        useCapture: EM_BOOL, callback: em_key_callback_func)
        -> EMSCRIPTEN_RESULT;

    pub fn emscripten_set_keyup_callback(
        target: *const ::libc::c_char, userData: *mut ::libc::c_void,
        useCapture: EM_BOOL, callback: em_key_callback_func)
        -> EMSCRIPTEN_RESULT;

    pub fn emscripten_hide_mouse();

    pub fn emscripten_get_device_pixel_ratio() -> f64;

    pub fn emscripten_set_pointerlockchange_callback(
        target: *const ::libc::c_char, userData: *mut ::libc::c_void, useCapture: EM_BOOL,
        callback: em_pointerlockchange_callback_func) -> EMSCRIPTEN_RESULT;

    pub fn emscripten_set_fullscreenchange_callback(
        target: *const ::libc::c_char, userData: *mut ::libc::c_void, useCapture: EM_BOOL,
        callback: em_fullscreenchange_callback_func) -> EMSCRIPTEN_RESULT;

    pub fn emscripten_asm_const(code: *const ::libc::c_char);

    pub fn emscripten_set_main_loop(
        func: em_callback_func, fps: ::libc::c_int, simulate_infinite_loop: EM_BOOL);

    pub fn emscripten_cancel_main_loop();
}
