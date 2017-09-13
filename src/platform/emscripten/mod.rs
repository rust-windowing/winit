#![cfg(target_os = "emscripten")]

mod ffi;

use std::mem;
use std::os::raw::{c_char, c_void, c_double, c_ulong, c_int};
use std::ptr;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Mutex, Arc, Weak};
use std::cell::RefCell;
use std::collections::VecDeque;

const DOCUMENT_NAME: &'static str = "#document\0";

#[derive(Clone, Default)]
pub struct PlatformSpecificWindowBuilderAttributes;

unsafe impl Send for PlatformSpecificWindowBuilderAttributes {}
unsafe impl Sync for PlatformSpecificWindowBuilderAttributes {}

#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct DeviceId;

#[derive(Clone, Default)]
pub struct PlatformSpecificHeadlessBuilderAttributes;

#[derive(Clone)]
pub struct MonitorId;

impl MonitorId {
    #[inline]
    pub fn get_name(&self) -> Option<String> {
        Some("Canvas".to_owned())
    }

    #[inline]
    pub fn get_position(&self) -> (u32, u32) {
        unimplemented!()
    }

    #[inline]
    pub fn get_dimensions(&self) -> (u32, u32) {
        (0, 0)
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

    pub fn interrupt(&self) {
        self.interrupted.store(true, Ordering::Relaxed);
    }

    pub fn create_proxy(&self) -> EventsLoopProxy {
        unimplemented!()
    }

    #[inline]
    pub fn get_available_monitors(&self) -> VecDeque<MonitorId> {
        let mut list = VecDeque::new();
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
            while let Some(event) = window.events.borrow_mut().pop_front() {
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
    cursor_state: Mutex<::CursorState>,
    is_fullscreen: bool,
    events: Box<RefCell<VecDeque<::Event>>>,
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

extern "C" fn keyboard_callback(
    event_type: c_int,
    event: *const ffi::EmscriptenKeyboardEvent,
    event_queue: *mut c_void) -> ffi::EM_BOOL
{
    unsafe {
        let queue: &RefCell<VecDeque<::Event>> = mem::transmute(event_queue);
        match event_type {
            ffi::EMSCRIPTEN_EVENT_KEYDOWN => {
                queue.borrow_mut().push_back(::Event::WindowEvent {
                    window_id: ::WindowId(WindowId(0)),
                    event: ::WindowEvent::KeyboardInput {
                        device_id: ::DeviceId(DeviceId),
                        input: ::KeyboardInput {
                            scancode: key_translate((*event).key) as u32,
                            state: ::ElementState::Pressed,
                            virtual_keycode: key_translate_virt((*event).key, (*event).location),
                            modifiers: ::ModifiersState::default()        // TODO:
                        },   
                    },
                });
            },
            ffi::EMSCRIPTEN_EVENT_KEYUP => {
                queue.borrow_mut().push_back(::Event::WindowEvent {
                    window_id: ::WindowId(WindowId(0)),
                    event: ::WindowEvent::KeyboardInput {
                        device_id: ::DeviceId(DeviceId),
                        input: ::KeyboardInput {
                            scancode: key_translate((*event).key) as u32,
                            state: ::ElementState::Released,
                            virtual_keycode: key_translate_virt((*event).key, (*event).location),
                            modifiers: ::ModifiersState::default()        // TODO:
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
    pub fn new(events_loop: &EventsLoop, attribs: &::WindowAttributes,
               _pl_attribs: &PlatformSpecificWindowBuilderAttributes)
        -> Result<Window, ::CreationError>
    {
        if events_loop.window.lock().unwrap().is_some() {
            return Err(::CreationError::OsError("Cannot create another window".to_owned()));
        }

        let w = Window2 {
            cursor_state: Mutex::new(::CursorState::Normal),
            events: Box::new(RefCell::new(VecDeque::new())),
            is_fullscreen: attribs.fullscreen.is_some(),
        };

        let window = Window {
            window: Arc::new(w),
        };


        // TODO: set up more event callbacks
        unsafe {
            em_try(ffi::emscripten_set_keydown_callback(DOCUMENT_NAME.as_ptr() as *const c_char, mem::transmute(&*window.window.events), ffi::EM_FALSE, Some(keyboard_callback)))
                .map_err(|e| ::CreationError::OsError(format!("emscripten error: {}", e)))?;
            em_try(ffi::emscripten_set_keyup_callback(DOCUMENT_NAME.as_ptr() as *const c_char, mem::transmute(&*window.window.events), ffi::EM_FALSE, Some(keyboard_callback)))
                .map_err(|e| ::CreationError::OsError(format!("emscripten error: {}", e)))?;
        }

        if attribs.fullscreen.is_some() {
            unsafe {
                em_try(ffi::emscripten_request_fullscreen(ptr::null(), ffi::EM_TRUE))
                    .map_err(|e| ::CreationError::OsError(e))?;
                em_try(ffi::emscripten_set_fullscreenchange_callback(ptr::null(), 0 as *mut c_void, ffi::EM_FALSE, Some(fullscreen_callback)))
                    .map_err(|e| ::CreationError::OsError(e))?;
            }
        } else if let Some((w, h)) = attribs.dimensions {
            window.set_inner_size(w, h);
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
    pub fn get_position(&self) -> Option<(i32, i32)> {
        Some((0, 0))
    }

    #[inline]
    pub fn set_position(&self, _: i32, _: i32) {
    }

    pub fn get_inner_size(&self) -> Option<(u32, u32)> {
        unsafe {
            use std::{mem, ptr};
            let mut width = mem::uninitialized();
            let mut height = mem::uninitialized();

            if ffi::emscripten_get_element_css_size(ptr::null(), &mut width, &mut height)
                != ffi::EMSCRIPTEN_RESULT_SUCCESS
            {
                None
            } else {
                Some((width as u32, height as u32))
            }
        }
    }

    #[inline]
    pub fn get_outer_size(&self) -> Option<(u32, u32)> {
        self.get_inner_size()
    }

    #[inline]
    pub fn set_inner_size(&self, width: u32, height: u32) {
        unsafe {
            use std::ptr;
            ffi::emscripten_set_element_css_size(ptr::null(), width as c_double, height
                as c_double);
        }
    }

    #[inline]
    pub fn show(&self) {}
    #[inline]
    pub fn hide(&self) {}

    #[inline]
    pub fn platform_display(&self) -> *mut ::libc::c_void {
        unimplemented!()
    }

    #[inline]
    pub fn platform_window(&self) -> *mut ::libc::c_void {
        unimplemented!()
    }

    #[inline]
    pub fn set_cursor(&self, _cursor: ::MouseCursor) {}

    #[inline]
    pub fn set_cursor_state(&self, state: ::CursorState) -> Result<(), String> {
        unsafe {
            use ::CursorState::*;

            let mut old_state = self.window.cursor_state.lock().unwrap();
            if state == *old_state {
                return Ok(());
            }

            // Set or unset grab callback
            match state {
                Hide | Normal => em_try(ffi::emscripten_set_pointerlockchange_callback(ptr::null(), 0 as *mut c_void, ffi::EM_FALSE, None))?,
                Grab => em_try(ffi::emscripten_set_pointerlockchange_callback(ptr::null(), 0 as *mut c_void, ffi::EM_FALSE, Some(pointerlockchange_callback)))?,
            }

            // Go back to normal cursor state
            match *old_state {
                Hide => show_mouse(),
                Grab => em_try(ffi::emscripten_exit_pointerlock())?,
                Normal => (),
            }

            // Set cursor from normal cursor state
            match state {
                Hide => ffi::emscripten_hide_mouse(),
                Grab => em_try(ffi::emscripten_request_pointerlock(ptr::null(), ffi::EM_TRUE))?,
                Normal => (),
            }

            // Update
            *old_state = state;

            Ok(())
        }
    }

    #[inline]
    pub fn hidpi_factor(&self) -> f32 {
        unsafe { ffi::emscripten_get_device_pixel_ratio() as f32 }
    }

    #[inline]
    pub fn set_cursor_position(&self, _x: i32, _y: i32) -> Result<(), ()> {
        Err(())
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
    pub fn get_current_monitor(&self) -> ::MonitorId {
        ::MonitorId{inner: MonitorId}
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
            let _ = self.set_cursor_state(::CursorState::Normal);

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
    use std::str;
    let slice = &input[0..input.iter().take_while(|x| **x != 0).count()];
    let key = unsafe { str::from_utf8(mem::transmute::<&[i8], &[u8]>(slice)).unwrap() };
    if key.chars().count() == 1 {
        key.as_bytes()[0]
    } else {
        0
    }
}

fn key_translate_virt(_input: [ffi::EM_UTF8; ffi::EM_HTML5_SHORT_STRING_LEN_BYTES],
                      _location: c_ulong) -> Option<::VirtualKeyCode>
{
    // TODO
    None
}
