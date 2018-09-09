#![cfg(target_os = "emscripten")]

mod ffi;

use std::{mem, ptr, str};
use std::cell::RefCell;
use std::collections::VecDeque;
use std::os::raw::{c_char, c_void, c_double, c_ulong, c_int};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Mutex, Arc};

use dpi::{LogicalPosition, LogicalSize, PhysicalPosition, PhysicalSize};
use window::MonitorId as RootMonitorId;

const DOCUMENT_NAME: &'static str = "#document\0";

fn get_hidpi_factor() -> f64 {
    unsafe { ffi::emscripten_get_device_pixel_ratio() as f64 }
}

#[derive(Clone, Default)]
pub struct PlatformSpecificWindowBuilderAttributes;

unsafe impl Send for PlatformSpecificWindowBuilderAttributes {}
unsafe impl Sync for PlatformSpecificWindowBuilderAttributes {}

#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct DeviceId;

#[derive(Clone, Default)]
pub struct PlatformSpecificHeadlessBuilderAttributes;

#[derive(Debug, Clone)]
pub struct MonitorId;

impl MonitorId {
    #[inline]
    pub fn get_name(&self) -> Option<String> {
        Some("Canvas".to_owned())
    }

    #[inline]
    pub fn get_position(&self) -> PhysicalPosition {
        unimplemented!()
    }

    #[inline]
    pub fn get_dimensions(&self) -> PhysicalSize {
        (0, 0).into()
    }

    #[inline]
    pub fn get_hidpi_factor(&self) -> f64 {
        get_hidpi_factor()
    }
}

// Used to assign a callback to emscripten main loop
thread_local!(static MAIN_LOOP_CALLBACK: RefCell<*mut c_void> = RefCell::new(ptr::null_mut()));

// Used to assign a callback to emscripten main loop
pub fn set_main_loop_callback<F>(callback : F) where F : FnMut() {
    MAIN_LOOP_CALLBACK.with(|log| {
        *log.borrow_mut() = &callback as *const _ as *mut c_void;
    });

    unsafe { ffi::emscripten_set_main_loop(Some(wrapper::<F>), 0, 1); }

    unsafe extern "C" fn wrapper<F>() where F : FnMut() {
        MAIN_LOOP_CALLBACK.with(|z| {
            let closure = *z.borrow_mut() as *mut F;
            (*closure)();
        });
    }
}

#[derive(Clone)]
pub struct EventsLoopProxy;

impl EventsLoopProxy {
    pub fn wakeup(&self) -> Result<(), ::EventsLoopClosed> {
        unimplemented!()
    }
}

pub struct EventsLoop {
    window: Mutex<Option<Arc<Window2>>>,
    interrupted: AtomicBool,
}

impl EventsLoop {
    pub fn new() -> EventsLoop {
        EventsLoop {
            window: Mutex::new(None),
            interrupted: AtomicBool::new(false),
        }
    }

    #[inline]
    pub fn interrupt(&self) {
        self.interrupted.store(true, Ordering::Relaxed);
    }

    #[inline]
    pub fn create_proxy(&self) -> EventsLoopProxy {
        unimplemented!()
    }

    #[inline]
    pub fn get_available_monitors(&self) -> VecDeque<MonitorId> {
        let mut list = VecDeque::with_capacity(1);
        list.push_back(MonitorId);
        list
    }

    #[inline]
    pub fn get_primary_monitor(&self) -> MonitorId {
        MonitorId
    }

    pub fn poll_events<F>(&self, mut callback: F)
        where F: FnMut(::Event)
    {
        let ref mut window = *self.window.lock().unwrap();
        if let &mut Some(ref mut window) = window {
            while let Some(event) = window.events.lock().unwrap().pop_front() {
                callback(event)
            }
        }
    }

    pub fn run_forever<F>(&self, mut callback: F)
        where F: FnMut(::Event) -> ::ControlFlow
    {
        self.interrupted.store(false, Ordering::Relaxed);

        // TODO: handle control flow

        set_main_loop_callback(|| {
            self.poll_events(|e| { callback(e); });
            ::std::thread::sleep(::std::time::Duration::from_millis(5));
            if self.interrupted.load(Ordering::Relaxed) {
                unsafe { ffi::emscripten_cancel_main_loop(); }
            }
        });
    }
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct WindowId(usize);

pub struct Window2 {
    cursor_grabbed: Mutex<bool>,
    cursor_hidden: Mutex<bool>,
    is_fullscreen: bool,
    events: Box<Mutex<VecDeque<::Event>>>,
}

pub struct Window {
    window: Arc<Window2>,
}

fn show_mouse() {
    // Hide mouse hasn't show mouse equivalent.
    // There is a pull request on emscripten that hasn't been merged #4616
    // that contains:
    //
    // var styleSheet = document.styleSheets[0];
    // var rules = styleSheet.cssRules;
    // for (var i = 0; i < rules.length; i++) {
    //   if (rules[i].cssText.substr(0, 6) == 'canvas') {
    //     styleSheet.deleteRule(i);
    //     i--;
    //   }
    // }
    // styleSheet.insertRule('canvas.emscripten { border: none; cursor: auto; }', 0);
    unsafe {
            ffi::emscripten_asm_const(b"var styleSheet = document.styleSheets[0]; var rules = styleSheet.cssRules; for (var i = 0; i < rules.length; i++) { if (rules[i].cssText.substr(0, 6) == 'canvas') { styleSheet.deleteRule(i); i--; } } styleSheet.insertRule('canvas.emscripten { border: none; cursor: auto; }', 0);\0".as_ptr() as *const c_char);
    }
}

extern "C" fn mouse_callback(
    event_type: c_int,
    event: *const ffi::EmscriptenMouseEvent,
    event_queue: *mut c_void) -> ffi::EM_BOOL
{
    unsafe {
        let queue: &Mutex<VecDeque<::Event>> = mem::transmute(event_queue);

        let modifiers = ::ModifiersState {
            shift: (*event).shiftKey == ffi::EM_TRUE,
            ctrl: (*event).ctrlKey == ffi::EM_TRUE,
            alt: (*event).altKey == ffi::EM_TRUE,
            logo: (*event).metaKey == ffi::EM_TRUE,
        };

        match event_type {
            ffi::EMSCRIPTEN_EVENT_MOUSEMOVE => {
                let dpi_factor = get_hidpi_factor();
                let position = LogicalPosition::from_physical(
                    ((*event).canvasX as f64, (*event).canvasY as f64),
                    dpi_factor,
                );
                queue.lock().unwrap().push_back(::Event::WindowEvent {
                    window_id: ::WindowId(WindowId(0)),
                    event: ::WindowEvent::CursorMoved {
                        device_id: ::DeviceId(DeviceId),
                        position,
                        modifiers: modifiers,
                    }
                });
                queue.lock().unwrap().push_back(::Event::DeviceEvent {
                    device_id: ::DeviceId(DeviceId),
                    event: ::DeviceEvent::MouseMotion {
                        delta: ((*event).movementX as f64, (*event).movementY as f64),
                    }
                });
            },
            mouse_input @ ffi::EMSCRIPTEN_EVENT_MOUSEDOWN |
            mouse_input @ ffi::EMSCRIPTEN_EVENT_MOUSEUP => {
                let button = match (*event).button {
                    0 => ::MouseButton::Left,
                    1 => ::MouseButton::Middle,
                    2 => ::MouseButton::Right,
                    other => ::MouseButton::Other(other as u8),
                };
                let state = match mouse_input {
                    ffi::EMSCRIPTEN_EVENT_MOUSEDOWN => ::ElementState::Pressed,
                    ffi::EMSCRIPTEN_EVENT_MOUSEUP => ::ElementState::Released,
                    _ => unreachable!(),
                };
                queue.lock().unwrap().push_back(::Event::WindowEvent {
                    window_id: ::WindowId(WindowId(0)),
                    event: ::WindowEvent::MouseInput {
                        device_id: ::DeviceId(DeviceId),
                        state: state,
                        button: button,
                        modifiers: modifiers,
                    }
                })
            },
            _ => {
            }
        }
    }
    ffi::EM_FALSE
}

extern "C" fn keyboard_callback(
    event_type: c_int,
    event: *const ffi::EmscriptenKeyboardEvent,
    event_queue: *mut c_void) -> ffi::EM_BOOL
{
    unsafe {
        let queue: &Mutex<VecDeque<::Event>> = mem::transmute(event_queue);

        let modifiers = ::ModifiersState {
            shift: (*event).shiftKey == ffi::EM_TRUE,
            ctrl: (*event).ctrlKey == ffi::EM_TRUE,
            alt: (*event).altKey == ffi::EM_TRUE,
            logo: (*event).metaKey == ffi::EM_TRUE,
        };

        match event_type {
            ffi::EMSCRIPTEN_EVENT_KEYDOWN => {
                queue.lock().unwrap().push_back(::Event::WindowEvent {
                    window_id: ::WindowId(WindowId(0)),
                    event: ::WindowEvent::KeyboardInput {
                        device_id: ::DeviceId(DeviceId),
                        input: ::KeyboardInput {
                            scancode: key_translate((*event).key) as u32,
                            state: ::ElementState::Pressed,
                            virtual_keycode: key_translate_virt((*event).key, (*event).location),
                            modifiers,
                        },
                    },
                });
            },
            ffi::EMSCRIPTEN_EVENT_KEYUP => {
                queue.lock().unwrap().push_back(::Event::WindowEvent {
                    window_id: ::WindowId(WindowId(0)),
                    event: ::WindowEvent::KeyboardInput {
                        device_id: ::DeviceId(DeviceId),
                        input: ::KeyboardInput {
                            scancode: key_translate((*event).key) as u32,
                            state: ::ElementState::Released,
                            virtual_keycode: key_translate_virt((*event).key, (*event).location),
                            modifiers,
                        },
                    },
                });
            },
            _ => {
            }
        }
    }
    ffi::EM_FALSE
}

extern fn touch_callback(
    event_type: c_int,
    event: *const ffi::EmscriptenTouchEvent,
    event_queue: *mut c_void) -> ffi::EM_BOOL
{
    unsafe {
        let queue: &Mutex<VecDeque<::Event>> = mem::transmute(event_queue);

        let phase = match event_type {
            ffi::EMSCRIPTEN_EVENT_TOUCHSTART => ::TouchPhase::Started,
            ffi::EMSCRIPTEN_EVENT_TOUCHEND => ::TouchPhase::Ended,
            ffi::EMSCRIPTEN_EVENT_TOUCHMOVE => ::TouchPhase::Moved,
            ffi::EMSCRIPTEN_EVENT_TOUCHCANCEL => ::TouchPhase::Cancelled,
            _ => return ffi::EM_FALSE,
        };

        for touch in 0..(*event).numTouches as usize {
            let touch = (*event).touches[touch];
            if touch.isChanged == ffi::EM_TRUE {
                let dpi_factor = get_hidpi_factor();
                let location = LogicalPosition::from_physical(
                    (touch.canvasX as f64, touch.canvasY as f64),
                    dpi_factor,
                );
                queue.lock().unwrap().push_back(::Event::WindowEvent {
                    window_id: ::WindowId(WindowId(0)),
                    event: ::WindowEvent::Touch(::Touch {
                        device_id: ::DeviceId(DeviceId),
                        phase,
                        id: touch.identifier as u64,
                        location,
                    }),
                });
            }
        }
    }
    ffi::EM_FALSE
}

// In case of fullscreen window this method will request fullscreen on change
#[allow(non_snake_case)]
unsafe extern "C" fn fullscreen_callback(
    _eventType: c_int,
    _fullscreenChangeEvent: *const ffi::EmscriptenFullscreenChangeEvent,
    _userData: *mut c_void) -> ffi::EM_BOOL
{
    ffi::emscripten_request_fullscreen(ptr::null(), ffi::EM_TRUE);
    ffi::EM_FALSE
}

// In case of pointer grabbed this method will request pointer lock on change
#[allow(non_snake_case)]
unsafe extern "C" fn pointerlockchange_callback(
    _eventType: c_int,
    _pointerlockChangeEvent: *const ffi::EmscriptenPointerlockChangeEvent,
    _userData: *mut c_void) -> ffi::EM_BOOL
{
    ffi::emscripten_request_pointerlock(ptr::null(), ffi::EM_TRUE);
    ffi::EM_FALSE
}

fn em_try(res: ffi::EMSCRIPTEN_RESULT) -> Result<(), String> {
    match res {
        ffi::EMSCRIPTEN_RESULT_SUCCESS | ffi::EMSCRIPTEN_RESULT_DEFERRED => Ok(()),
        r @ _ => Err(error_to_str(r).to_string()),
    }
}

impl Window {
    pub fn new(events_loop: &EventsLoop, attribs: ::WindowAttributes,
               _pl_attribs: PlatformSpecificWindowBuilderAttributes)
        -> Result<Window, ::CreationError>
    {
        if events_loop.window.lock().unwrap().is_some() {
            return Err(::CreationError::OsError("Cannot create another window".to_owned()));
        }

        let w = Window2 {
            cursor_grabbed: Default::default(),
            cursor_hidden: Default::default(),
            events: Default::default(),
            is_fullscreen: attribs.fullscreen.is_some(),
        };

        let window = Window {
            window: Arc::new(w),
        };


        // TODO: set up more event callbacks
        unsafe {
            em_try(ffi::emscripten_set_mousemove_callback(DOCUMENT_NAME.as_ptr() as *const c_char, mem::transmute(&*window.window.events), ffi::EM_FALSE, Some(mouse_callback)))
                .map_err(|e| ::CreationError::OsError(format!("emscripten error: {}", e)))?;
            em_try(ffi::emscripten_set_mousedown_callback(DOCUMENT_NAME.as_ptr() as *const c_char, mem::transmute(&*window.window.events), ffi::EM_FALSE, Some(mouse_callback)))
                .map_err(|e| ::CreationError::OsError(format!("emscripten error: {}", e)))?;
            em_try(ffi::emscripten_set_mouseup_callback(DOCUMENT_NAME.as_ptr() as *const c_char, mem::transmute(&*window.window.events), ffi::EM_FALSE, Some(mouse_callback)))
                .map_err(|e| ::CreationError::OsError(format!("emscripten error: {}", e)))?;
            em_try(ffi::emscripten_set_keydown_callback(DOCUMENT_NAME.as_ptr() as *const c_char, mem::transmute(&*window.window.events), ffi::EM_FALSE, Some(keyboard_callback)))
                .map_err(|e| ::CreationError::OsError(format!("emscripten error: {}", e)))?;
            em_try(ffi::emscripten_set_keyup_callback(DOCUMENT_NAME.as_ptr() as *const c_char, mem::transmute(&*window.window.events), ffi::EM_FALSE, Some(keyboard_callback)))
                .map_err(|e| ::CreationError::OsError(format!("emscripten error: {}", e)))?;
            em_try(ffi::emscripten_set_touchstart_callback(DOCUMENT_NAME.as_ptr() as *const c_char, mem::transmute(&*window.window.events), ffi::EM_FALSE, Some(touch_callback)))
                .map_err(|e| ::CreationError::OsError(format!("emscripten error: {}", e)))?;
            em_try(ffi::emscripten_set_touchend_callback(DOCUMENT_NAME.as_ptr() as *const c_char, mem::transmute(&*window.window.events), ffi::EM_FALSE, Some(touch_callback)))
                .map_err(|e| ::CreationError::OsError(format!("emscripten error: {}", e)))?;
            em_try(ffi::emscripten_set_touchmove_callback(DOCUMENT_NAME.as_ptr() as *const c_char, mem::transmute(&*window.window.events), ffi::EM_FALSE, Some(touch_callback)))
                .map_err(|e| ::CreationError::OsError(format!("emscripten error: {}", e)))?;
            em_try(ffi::emscripten_set_touchcancel_callback(DOCUMENT_NAME.as_ptr() as *const c_char, mem::transmute(&*window.window.events), ffi::EM_FALSE, Some(touch_callback)))
                .map_err(|e| ::CreationError::OsError(format!("emscripten error: {}", e)))?;
        }

        if attribs.fullscreen.is_some() {
            unsafe {
                em_try(ffi::emscripten_request_fullscreen(ptr::null(), ffi::EM_TRUE))
                    .map_err(|e| ::CreationError::OsError(e))?;
                em_try(ffi::emscripten_set_fullscreenchange_callback(ptr::null(), 0 as *mut c_void, ffi::EM_FALSE, Some(fullscreen_callback)))
                    .map_err(|e| ::CreationError::OsError(e))?;
            }
        } else if let Some(size) = attribs.dimensions {
            window.set_inner_size(size);
        }

        *events_loop.window.lock().unwrap() = Some(window.window.clone());
        Ok(window)
    }

    #[inline]
    pub fn id(&self) -> WindowId {
        WindowId(0)
    }

    #[inline]
    pub fn set_title(&self, _title: &str) {
    }

    #[inline]
    pub fn get_position(&self) -> Option<LogicalPosition> {
        Some((0, 0).into())
    }

    #[inline]
    pub fn get_inner_position(&self) -> Option<LogicalPosition> {
        Some((0, 0).into())
    }

    #[inline]
    pub fn set_position(&self, _: LogicalPosition) {
    }

    #[inline]
    pub fn get_inner_size(&self) -> Option<LogicalSize> {
        unsafe {
            let mut width = 0;
            let mut height = 0;
            let mut fullscreen = 0;

            if ffi::emscripten_get_canvas_size(&mut width, &mut height, &mut fullscreen)
                != ffi::EMSCRIPTEN_RESULT_SUCCESS
            {
                None
            } else {
                let dpi_factor = self.get_hidpi_factor();
                let logical = LogicalSize::from_physical((width as u32, height as u32), dpi_factor);
                Some(logical)
            }
        }
    }

    #[inline]
    pub fn get_outer_size(&self) -> Option<LogicalSize> {
        self.get_inner_size()
    }

    #[inline]
    pub fn set_inner_size(&self, size: LogicalSize) {
        unsafe {
            let dpi_factor = self.get_hidpi_factor();
            let physical = PhysicalSize::from_logical(size, dpi_factor);
            let (width, height): (u32, u32) = physical.into();
            ffi::emscripten_set_element_css_size(
                ptr::null(),
                width as c_double,
                height as c_double,
            );
        }
    }

    #[inline]
    pub fn set_min_dimensions(&self, _dimensions: Option<LogicalSize>) {
        // N/A
    }

    #[inline]
    pub fn set_max_dimensions(&self, _dimensions: Option<LogicalSize>) {
        // N/A
    }

    #[inline]
    pub fn set_resizable(&self, _resizable: bool) {
        // N/A
    }

    #[inline]
    pub fn show(&self) {
        // N/A
    }

    #[inline]
    pub fn hide(&self) {
        // N/A
    }

    #[inline]
    pub fn set_cursor(&self, _cursor: ::MouseCursor) {
        // N/A
    }

    #[inline]
    pub fn grab_cursor(&self, grab: bool) -> Result<(), String> {
        let mut grabbed_lock = self.window.cursor_grabbed.lock().unwrap();
        if grab == *grabbed_lock { return Ok(()); }
        unsafe {
            if grab {
                em_try(ffi::emscripten_set_pointerlockchange_callback(
                    ptr::null(),
                    0 as *mut c_void,
                    ffi::EM_FALSE,
                    Some(pointerlockchange_callback),
                ))?;
                em_try(ffi::emscripten_request_pointerlock(ptr::null(), ffi::EM_TRUE))?;
            } else {
                em_try(ffi::emscripten_set_pointerlockchange_callback(
                    ptr::null(),
                    0 as *mut c_void,
                    ffi::EM_FALSE,
                    None,
                ))?;
                em_try(ffi::emscripten_exit_pointerlock())?;
            }
        }
        *grabbed_lock = grab;
        Ok(())
    }

    #[inline]
    pub fn hide_cursor(&self, hide: bool) {
        let mut hidden_lock = self.window.cursor_hidden.lock().unwrap();
        if hide == *hidden_lock { return; }
        if hide {
            unsafe { ffi::emscripten_hide_mouse() };
        } else {
            show_mouse();
        }
        *hidden_lock = hide;
    }

    #[inline]
    pub fn get_hidpi_factor(&self) -> f64 {
        get_hidpi_factor()
    }

    #[inline]
    pub fn set_cursor_position(&self, _position: LogicalPosition) -> Result<(), String> {
        Err("Setting cursor position is not possible on Emscripten.".to_owned())
    }

    #[inline]
    pub fn set_maximized(&self, _maximized: bool) {
        // iOS has single screen maximized apps so nothing to do
    }

    #[inline]
    pub fn set_fullscreen(&self, _monitor: Option<::MonitorId>) {
        // iOS has single screen maximized apps so nothing to do
    }

    #[inline]
    pub fn set_decorations(&self, _decorations: bool) {
        // N/A
    }

    #[inline]
    pub fn set_always_on_top(&self, _always_on_top: bool) {
        // N/A
    }

    #[inline]
    pub fn set_window_icon(&self, _icon: Option<::Icon>) {
        // N/A
    }

    #[inline]
    pub fn set_ime_spot(&self, _logical_spot: LogicalPosition) {
        // N/A
    }

    #[inline]
    pub fn get_current_monitor(&self) -> RootMonitorId {
        RootMonitorId { inner: MonitorId }
    }

    #[inline]
    pub fn get_available_monitors(&self) -> VecDeque<MonitorId> {
        let mut list = VecDeque::with_capacity(1);
        list.push_back(MonitorId);
        list
    }

    #[inline]
    pub fn get_primary_monitor(&self) -> MonitorId {
        MonitorId
    }
}

impl Drop for Window {
    fn drop(&mut self) {
        // Delete window from events_loop
        // TODO: ?
        /*if let Some(ev) = self.events_loop.upgrade() {
            let _ = ev.window.lock().unwrap().take().unwrap();
        }*/

        unsafe {
            // Return back to normal cursor state
            self.hide_cursor(false);
            self.grab_cursor(false);

            // Exit fullscreen if on
            if self.window.is_fullscreen {
                ffi::emscripten_set_fullscreenchange_callback(ptr::null(), 0 as *mut c_void, ffi::EM_FALSE, None);
                ffi::emscripten_exit_fullscreen();
            }

            // Delete callbacks
            ffi::emscripten_set_keydown_callback(DOCUMENT_NAME.as_ptr() as *const c_char, 0 as *mut c_void, ffi::EM_FALSE,None);
            ffi::emscripten_set_keyup_callback(DOCUMENT_NAME.as_ptr() as *const c_char, 0 as *mut c_void, ffi::EM_FALSE,None);
        }
    }
}

fn error_to_str(code: ffi::EMSCRIPTEN_RESULT) -> &'static str {
    match code {
        ffi::EMSCRIPTEN_RESULT_SUCCESS | ffi::EMSCRIPTEN_RESULT_DEFERRED
            => "Internal error in the library (success detected as failure)",

        ffi::EMSCRIPTEN_RESULT_NOT_SUPPORTED => "Not supported",
        ffi::EMSCRIPTEN_RESULT_FAILED_NOT_DEFERRED => "Failed not deferred",
        ffi::EMSCRIPTEN_RESULT_INVALID_TARGET => "Invalid target",
        ffi::EMSCRIPTEN_RESULT_UNKNOWN_TARGET => "Unknown target",
        ffi::EMSCRIPTEN_RESULT_INVALID_PARAM => "Invalid parameter",
        ffi::EMSCRIPTEN_RESULT_FAILED => "Failed",
        ffi::EMSCRIPTEN_RESULT_NO_DATA => "No data",

        _ => "Undocumented error"
    }
}

fn key_translate(input: [ffi::EM_UTF8; ffi::EM_HTML5_SHORT_STRING_LEN_BYTES]) -> u8 {
    let slice = &input[0..input.iter().take_while(|x| **x != 0).count()];
    let maybe_key = unsafe { str::from_utf8(mem::transmute::<_, &[u8]>(slice)) };
    let key = match maybe_key {
        Ok(key) => key,
        Err(_) => { return 0; },
    };
    if key.chars().count() == 1 {
        key.as_bytes()[0]
    } else {
        0
    }
}

fn key_translate_virt(input: [ffi::EM_UTF8; ffi::EM_HTML5_SHORT_STRING_LEN_BYTES],
                      location: c_ulong) -> Option<::VirtualKeyCode>
{
    let slice = &input[0..input.iter().take_while(|x| **x != 0).count()];
    let maybe_key = unsafe { str::from_utf8(mem::transmute::<_, &[u8]>(slice)) };
    let key = match maybe_key {
        Ok(key) => key,
        Err(_) => { return None; },
    };
    use VirtualKeyCode::*;
    match key {
        "Alt" => match location {
            ffi::DOM_KEY_LOCATION_LEFT => Some(LAlt),
            ffi::DOM_KEY_LOCATION_RIGHT => Some(RAlt),
            _ => None,
        },
        "AltGraph" => None,
        "CapsLock" => None,
        "Control" => match location {
            ffi::DOM_KEY_LOCATION_LEFT => Some(LControl),
            ffi::DOM_KEY_LOCATION_RIGHT => Some(RControl),
            _ => None,
        },
        "Fn" => None,
        "FnLock" => None,
        "Hyper" => None,
        "Meta" => None,
        "NumLock" => Some(Numlock),
        "ScrollLock" => Some(Scroll),
        "Shift" => match location {
            ffi::DOM_KEY_LOCATION_LEFT => Some(LShift),
            ffi::DOM_KEY_LOCATION_RIGHT => Some(RShift),
            _ => None,
        },
        "Super" => None,
        "Symbol" => None,
        "SymbolLock" => None,

        "Enter" => match location {
            ffi::DOM_KEY_LOCATION_NUMPAD => Some(NumpadEnter),
            _ => Some(Return),
        },
        "Tab" => Some(Tab),
        " " => Some(Space),

        "ArrowDown" => Some(Down),
        "ArrowLeft" => Some(Left),
        "ArrowRight" => Some(Right),
        "ArrowUp" => Some(Up),
        "End" => None,
        "Home" => None,
        "PageDown" => None,
        "PageUp" => None,

        "Backspace" => Some(Back),
        "Clear" => None,
        "Copy" => None,
        "CrSel" => None,
        "Cut" => None,
        "Delete" => None,
        "EraseEof" => None,
        "ExSel" => None,
        "Insert" => Some(Insert),
        "Paste" => None,
        "Redo" => None,
        "Undo" => None,

        "Accept" => None,
        "Again" => None,
        "Attn" => None,
        "Cancel" => None,
        "ContextMenu" => None,
        "Escape" => Some(Escape),
        "Execute" => None,
        "Find" => None,
        "Finish" => None,
        "Help" => None,
        "Pause" => Some(Pause),
        "Play" => None,
        "Props" => None,
        "Select" => None,
        "ZoomIn" => None,
        "ZoomOut" => None,

        "BrightnessDown" => None,
        "BrightnessUp" => None,
        "Eject" => None,
        "LogOff" => None,
        "Power" => Some(Power),
        "PowerOff" => None,
        "PrintScreen" => Some(Snapshot),
        "Hibernate" => None,
        "Standby" => Some(Sleep),
        "WakeUp" => Some(Wake),

        "AllCandidates" => None,
        "Alphanumeric" => None,
        "CodeInput" => None,
        "Compose" => Some(Compose),
        "Convert" => Some(Convert),
        "Dead" => None,
        "FinalMode" => None,
        "GroupFirst" => None,
        "GroupLast" => None,
        "GroupNext" => None,
        "GroupPrevious" => None,
        "ModeChange" => None,
        "NextCandidate" => None,
        "NonConvert" => None,
        "PreviousCandidate" => None,
        "Process" => None,
        "SingleCandidate" => None,

        "HangulMode" => None,
        "HanjaMode" => None,
        "JunjaMode" => None,

        "Eisu" => None,
        "Hankaku" => None,
        "Hiragana" => None,
        "HiraganaKatakana" => None,
        "KanaMode" => Some(Kana),
        "KanjiMode" => Some(Kanji),
        "Romaji" => None,
        "Zenkaku" => None,
        "ZenkakuHanaku" => None,

        "F1" => Some(F1),
        "F2" => Some(F2),
        "F3" => Some(F3),
        "F4" => Some(F4),
        "F5" => Some(F5),
        "F6" => Some(F6),
        "F7" => Some(F7),
        "F8" => Some(F8),
        "F9" => Some(F9),
        "F10" => Some(F10),
        "F11" => Some(F11),
        "F12" => Some(F12),
        "F13" => Some(F13),
        "F14" => Some(F14),
        "F15" => Some(F15),
        "F16" => Some(F16),
        "F17" => Some(F17),
        "F18" => Some(F18),
        "F19" => Some(F19),
        "F20" => Some(F20),
        "F21" => Some(F21),
        "F22" => Some(F22),
        "F23" => Some(F23),
        "F24" => Some(F24),
        "Soft1" => None,
        "Soft2" => None,
        "Soft3" => None,
        "Soft4" => None,

        "AppSwitch" => None,
        "Call" => None,
        "Camera" => None,
        "CameraFocus" => None,
        "EndCall" => None,
        "GoBack" => None,
        "GoHome" => None,
        "HeadsetHook" => None,
        "LastNumberRedial" => None,
        "Notification" => None,
        "MannerMode" => None,
        "VoiceDial" => None,

        "ChannelDown" => None,
        "ChannelUp" => None,
        "MediaFastForward" => None,
        "MediaPause" => None,
        "MediaPlay" => None,
        "MediaPlayPause" => Some(PlayPause),
        "MediaRecord" => None,
        "MediaRewind" => None,
        "MediaStop" => Some(MediaStop),
        "MediaTrackNext" => Some(NextTrack),
        "MediaTrackPrevious" => Some(PrevTrack),

        "AudioBalanceLeft" => None,
        "AudioBalanceRight" => None,
        "AudioBassDown" => None,
        "AudioBassBoostDown" => None,
        "AudioBassBoostToggle" => None,
        "AudioBassBoostUp" => None,
        "AudioBassUp" => None,
        "AudioFaderFront" => None,
        "AudioFaderRear" => None,
        "AudioSurroundModeNext" => None,
        "AudioTrebleDown" => None,
        "AudioTrebleUp" => None,
        "AudioVolumeDown" => Some(VolumeDown),
        "AudioVolumeMute" => Some(Mute),
        "AudioVolumeUp" => Some(VolumeUp),
        "MicrophoneToggle" => None,
        "MicrophoneVolumeDown" => None,
        "MicrophoneVolumeMute" => None,
        "MicrophoneVolumeUp" => None,

        "TV" => None,
        "TV3DMode" => None,
        "TVAntennaCable" => None,
        "TVAudioDescription" => None,
        "TVAudioDescriptionMixDown" => None,
        "TVAudioDescriptionMixUp" => None,
        "TVContentsMenu" => None,
        "TVDataService" => None,
        "TVInput" => None,
        "TVInputComponent1" => None,
        "TVInputComponent2" => None,
        "TVInputComposite1" => None,
        "TVInputComposite2" => None,
        "TVInputHDM1" => None,
        "TVInputHDM2" => None,
        "TVInputHDM3" => None,
        "TVInputHDM4" => None,
        "TVInputVGA1" => None,
        "TVMediaContext" => None,
        "TVNetwork" => None,
        "TVNumberEntry" => None,
        "TVPower" => None,
        "TVRadioService" => None,
        "TVSatellite" => None,
        "TVSatelliteBS" => None,
        "TVSatelliteCS" => None,
        "TVSatelliteToggle" => None,
        "TVTerrestrialAnalog" => None,
        "TVTerrestrialDigital" => None,
        "TVTimer" => None,

        "AVRInput" => None,
        "AVRPower" => None,
        "ColorF0Red" => None,
        "ColorF1Green" => None,
        "ColorF2Yellow" => None,
        "ColorF3Blue" => None,
        "ColorF4Grey" => None,
        "ColorF5Brown" => None,
        "ClosedCaptionToggle" => None,
        "Dimmer" => None,
        "DisplaySwap" => None,
        "DVR" => None,
        "Exit" => None,
        "FavoriteClear0" => None,
        "FavoriteClear1" => None,
        "FavoriteClear2" => None,
        "FavoriteClear3" => None,
        "FavoriteRecall0" => None,
        "FavoriteRecall1" => None,
        "FavoriteRecall2" => None,
        "FavoriteRecall3" => None,
        "FavoriteStore0" => None,
        "FavoriteStore1" => None,
        "FavoriteStore2" => None,
        "FavoriteStore3" => None,
        "FavoriteStore4" => None,
        "Guide" => None,
        "GuideNextDay" => None,
        "GuidePreviousDay" => None,
        "Info" => None,
        "InstantReplay" => None,
        "Link" => None,
        "ListProgram" => None,
        "LiveContent" => None,
        "Lock" => None,
        "MediaApps" => None,
        "MediaAudioTrack" => None,
        "MediaLast" => None,
        "MediaSkipBackward" => None,
        "MediaSkipForward" => None,
        "MediaStepBackward" => None,
        "MediaStepForward" => None,
        "MediaTopMenu" => None,
        "NavigateIn" => None,
        "NavigateNext" => None,
        "NavigateOut" => None,
        "NavigatePrevious" => None,
        "NextFavoriteChannel" => None,
        "NextUserProfile" => None,
        "OnDemand" => None,
        "Pairing" => None,
        "PinPDown" => None,
        "PinPMove" => None,
        "PinPToggle" => None,
        "PinPUp" => None,
        "PlaySpeedDown" => None,
        "PlaySpeedReset" => None,
        "PlaySpeedUp" => None,
        "RandomToggle" => None,
        "RcLowBattery" => None,
        "RecordSpeedNext" => None,
        "RfBypass" => None,
        "ScanChannelsToggle" => None,
        "ScreenModeNext" => None,
        "Settings" => None,
        "SplitScreenToggle" => None,
        "STBInput" => None,
        "STBPower" => None,
        "Subtitle" => None,
        "Teletext" => None,
        "VideoModeNext" => None,
        "Wink" => None,
        "ZoomToggle" => None,

        "SpeechCorrectionList" => None,
        "SpeechInputToggle" => None,

        "Close" => None,
        "New" => None,
        "Open" => None,
        "Print" => None,
        "Save" => None,
        "SpellCheck" => None,
        "MailForward" => None,
        "MailReply" => None,
        "MailSend" => None,

        "LaunchCalculator" => Some(Calculator),
        "LaunchCalendar" => None,
        "LaunchContacts" => None,
        "LaunchMail" => Some(Mail),
        "LaunchMediaPlayer" => None,
        "LaunchMusicPlayer" => None,
        "LaunchMyComputer" => Some(MyComputer),
        "LaunchPhone" => None,
        "LaunchScreenSaver" => None,
        "LaunchSpreadsheet" => None,
        "LaunchWebCam" => None,
        "LaunchWordProcessor" => None,
        "LaunchApplication1" => None,
        "LaunchApplication2" => None,
        "LaunchApplication3" => None,
        "LaunchApplication4" => None,
        "LaunchApplication5" => None,
        "LaunchApplication6" => None,
        "LaunchApplication7" => None,
        "LaunchApplication8" => None,
        "LaunchApplication9" => None,
        "LaunchApplication10" => None,
        "LaunchApplication11" => None,
        "LaunchApplication12" => None,
        "LaunchApplication13" => None,
        "LaunchApplication14" => None,
        "LaunchApplication15" => None,
        "LaunchApplication16" => None,

        "BrowserBack" => Some(WebBack),
        "BrowserFavorites" => Some(WebFavorites),
        "BrowserForward" => Some(WebForward),
        "BrowserHome" => Some(WebHome),
        "BrowserRefresh" => Some(WebRefresh),
        "BrowserSearch" => Some(WebSearch),
        "BrowserStop" => Some(WebStop),

        "Decimal" => Some(Decimal),
        "Key11" => None,
        "Key12" => None,
        "Multiply" | "*" => Some(Multiply),
        "Add" | "+" => Some(Add),
        // "Clear" => None,
        "Divide" => Some(Divide),
        "Subtract" | "-" => Some(Subtract),
        "Separator" => None,
        "0" => match location {
            ffi::DOM_KEY_LOCATION_NUMPAD => Some(Numpad0),
            _ => Some(Key0),
        },
        "1" => match location {
            ffi::DOM_KEY_LOCATION_NUMPAD => Some(Numpad1),
            _ => Some(Key1),
        },
        "2" => match location {
            ffi::DOM_KEY_LOCATION_NUMPAD => Some(Numpad2),
            _ => Some(Key2),
        },
        "3" => match location {
            ffi::DOM_KEY_LOCATION_NUMPAD => Some(Numpad3),
            _ => Some(Key3),
        },
        "4" => match location {
            ffi::DOM_KEY_LOCATION_NUMPAD => Some(Numpad4),
            _ => Some(Key4),
        },
        "5" => match location {
            ffi::DOM_KEY_LOCATION_NUMPAD => Some(Numpad5),
            _ => Some(Key5),
        },
        "6" => match location {
            ffi::DOM_KEY_LOCATION_NUMPAD => Some(Numpad6),
            _ => Some(Key6),
        },
        "7" => match location {
            ffi::DOM_KEY_LOCATION_NUMPAD => Some(Numpad7),
            _ => Some(Key7),
        },
        "8" => match location {
            ffi::DOM_KEY_LOCATION_NUMPAD => Some(Numpad8),
            _ => Some(Key8),
        },
        "9" => match location {
            ffi::DOM_KEY_LOCATION_NUMPAD => Some(Numpad9),
            _ => Some(Key9),
        },

        "A" | "a" => Some(A),
        "B" | "b" => Some(B),
        "C" | "c" => Some(C),
        "D" | "d" => Some(D),
        "E" | "e" => Some(E),
        "F" | "f" => Some(F),
        "G" | "g" => Some(G),
        "H" | "h" => Some(H),
        "I" | "i" => Some(I),
        "J" | "j" => Some(J),
        "K" | "k" => Some(K),
        "L" | "l" => Some(L),
        "M" | "m" => Some(M),
        "N" | "n" => Some(N),
        "O" | "o" => Some(O),
        "P" | "p" => Some(P),
        "Q" | "q" => Some(Q),
        "R" | "r" => Some(R),
        "S" | "s" => Some(S),
        "T" | "t" => Some(T),
        "U" | "u" => Some(U),
        "V" | "v" => Some(V),
        "W" | "w" => Some(W),
        "X" | "x" => Some(X),
        "Y" | "y" => Some(Y),
        "Z" | "z" => Some(Z),

        "'" => Some(Apostrophe),
        "\\" => Some(Backslash),
        ":" => Some(Colon),
        "," => match location {
            ffi::DOM_KEY_LOCATION_NUMPAD => Some(NumpadComma),
            _ => Some(Comma),
        },
        "=" => match location {
            ffi::DOM_KEY_LOCATION_NUMPAD => Some(NumpadEquals),
            _ => Some(Equals),
        },
        "{" => Some(LBracket),
        "." => Some(Period),
        "}" => Some(RBracket),
        ";" => Some(Semicolon),
        "/" => Some(Slash),

        _ => None,
    }
}
