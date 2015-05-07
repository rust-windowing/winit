#![cfg(all(target_os = "linux", feature = "window"))]

use {Event, BuilderAttribs, MouseCursor};
use CreationError;
use CreationError::OsError;
use libc;
use std::{mem, ptr};
use std::cell::Cell;
use std::ffi::CString;
use std::sync::atomic::AtomicBool;
use std::collections::VecDeque;
use std::sync::{Arc, Mutex, Once, ONCE_INIT};

use Api;
use CursorState;
use GlContext;
use GlRequest;
use PixelFormat;

use api::dlopen;
use api::glx::Context as GlxContext;
use api::egl::Context as EglContext;

pub use self::monitor::{MonitorID, get_available_monitors, get_primary_monitor};

mod events;
mod monitor;
pub mod ffi;

static THREAD_INIT: Once = ONCE_INIT;

// XOpenIM doesn't seem to be thread-safe
lazy_static! {      // TODO: use a static mutex when that's possible, and put me back in my function
    static ref GLOBAL_XOPENIM_LOCK: Mutex<()> = Mutex::new(());
}

unsafe extern "C" fn x_error_callback(_: *mut ffi::Display, event: *mut ffi::XErrorEvent) -> libc::c_int {
    println!("[glutin] x error code={} major={} minor={}!", (*event).error_code, (*event).request_code, (*event).minor_code);
    0
}

fn ensure_thread_init(xlib: &ffi::Xlib) {
    THREAD_INIT.call_once(|| {
        unsafe {
            (xlib.XInitThreads)();
            (xlib.XSetErrorHandler)(Some(x_error_callback));
        }
    });
}

// TODO: remove me
fn with_c_str<F, T>(s: &str, f: F) -> T where F: FnOnce(*const libc::c_char) -> T {
    use std::ffi::CString;
    let c_str = CString::new(s.as_bytes().to_vec()).unwrap();
    f(c_str.as_ptr())
}

pub struct XWindow {
    xlib: ffi::Xlib,
    xf86vmode: ffi::Xf86vmode,
    xcursor: ffi::Xcursor,
    display: *mut ffi::Display,
    window: ffi::Window,
    pub context: Context,
    is_fullscreen: bool,
    screen_id: libc::c_int,
    xf86_desk_mode: *mut ffi::XF86VidModeModeInfo,
    ic: ffi::XIC,
    im: ffi::XIM,
}

pub enum Context {
    Glx(GlxContext),
    Egl(EglContext),
    None,
}

unsafe impl Send for XWindow {}
unsafe impl Sync for XWindow {}

unsafe impl Send for Window {}
unsafe impl Sync for Window {}

impl Drop for XWindow {
    fn drop(&mut self) {
        unsafe {
            // we don't call MakeCurrent(0, 0) because we are not sure that the context
            // is still the current one
            self.context = Context::None;

            if self.is_fullscreen {
                (self.xf86vmode.XF86VidModeSwitchToMode)(self.display, self.screen_id, self.xf86_desk_mode);
                (self.xf86vmode.XF86VidModeSetViewPort)(self.display, self.screen_id, 0, 0);
            }

            (self.xlib.XDestroyIC)(self.ic);
            (self.xlib.XCloseIM)(self.im);
            (self.xlib.XDestroyWindow)(self.display, self.window);
            (self.xlib.XCloseDisplay)(self.display);
        }
    }
}

#[derive(Clone)]
pub struct WindowProxy {
    x: Arc<XWindow>,
}

impl WindowProxy {
    pub fn wakeup_event_loop(&self) {
        let mut xev = ffi::XClientMessageEvent {
            type_: ffi::ClientMessage,
            window: self.x.window,
            format: 32,
            message_type: 0,
            serial: 0,
            send_event: 0,
            display: self.x.display,
            data: unsafe { mem::zeroed() },
        };

        unsafe {
            (self.x.xlib.XSendEvent)(self.x.display, self.x.window, 0, 0, mem::transmute(&mut xev));
            (self.x.xlib.XFlush)(self.x.display);
        }
    }
}

pub struct PollEventsIterator<'a> {
    window: &'a Window,
}

impl<'a> Iterator for PollEventsIterator<'a> {
    type Item = Event;

    fn next(&mut self) -> Option<Event> {
        if let Some(ev) = self.window.pending_events.lock().unwrap().pop_front() {
            return Some(ev);
        }

        loop {
            let mut xev = unsafe { mem::uninitialized() };
            let res = unsafe { (self.window.x.xlib.XCheckMaskEvent)(self.window.x.display, -1, &mut xev) };

            if res == 0 {
                let res = unsafe { (self.window.x.xlib.XCheckTypedEvent)(self.window.x.display, ffi::ClientMessage, &mut xev) };

                if res == 0 {
                    return None;
                }
            }

            match xev.get_type() {
                ffi::KeymapNotify => {
                    unsafe { (self.window.x.xlib.XRefreshKeyboardMapping)(mem::transmute(&xev)); }
                },

                ffi::ClientMessage => {
                    use events::Event::{Closed, Awakened};
                    use std::sync::atomic::Ordering::Relaxed;

                    let client_msg: &ffi::XClientMessageEvent = unsafe { mem::transmute(&xev) };

                    if client_msg.data.get_long(0) == self.window.wm_delete_window as libc::c_long {
                        self.window.is_closed.store(true, Relaxed);
                        return Some(Closed);
                    } else {
                        return Some(Awakened);
                    }
                },

                ffi::ConfigureNotify => {
                    use events::Event::Resized;
                    let cfg_event: &ffi::XConfigureEvent = unsafe { mem::transmute(&xev) };
                    let (current_width, current_height) = self.window.current_size.get();
                    if current_width != cfg_event.width || current_height != cfg_event.height {
                        self.window.current_size.set((cfg_event.width, cfg_event.height));
                        return Some(Resized(cfg_event.width as u32, cfg_event.height as u32));
                    }
                },

                ffi::Expose => {
                    use events::Event::Refresh;
                    return Some(Refresh);
                },

                ffi::MotionNotify => {
                    use events::Event::MouseMoved;
                    let event: &ffi::XMotionEvent = unsafe { mem::transmute(&xev) };
                    return Some(MouseMoved((event.x as i32, event.y as i32)));
                },

                ffi::KeyPress | ffi::KeyRelease => {
                    use events::Event::{KeyboardInput, ReceivedCharacter};
                    use events::ElementState::{Pressed, Released};
                    let event: &mut ffi::XKeyEvent = unsafe { mem::transmute(&xev) };

                    if event.type_ == ffi::KeyPress {
                        let raw_ev: *mut ffi::XKeyEvent = event;
                        unsafe { (self.window.x.xlib.XFilterEvent)(mem::transmute(raw_ev), self.window.x.window) };
                    }

                    let state = if xev.get_type() == ffi::KeyPress { Pressed } else { Released };

                    let written = unsafe {
                        use std::str;

                        let mut buffer: [u8; 16] = [mem::uninitialized(); 16];
                        let raw_ev: *mut ffi::XKeyEvent = event;
                        let count = (self.window.x.xlib.Xutf8LookupString)(self.window.x.ic, mem::transmute(raw_ev),
                            mem::transmute(buffer.as_mut_ptr()),
                            buffer.len() as libc::c_int, ptr::null_mut(), ptr::null_mut());

                        str::from_utf8(&buffer[..count as usize]).unwrap_or("").to_string()
                    };

                    {
                        let mut pending = self.window.pending_events.lock().unwrap();
                        for chr in written.chars() {
                            pending.push_back(ReceivedCharacter(chr));
                        }
                    }

                    let keysym = unsafe {
                        (self.window.x.xlib.XKeycodeToKeysym)(self.window.x.display, event.keycode as ffi::KeyCode, 0)
                    };

                    let vkey =  events::keycode_to_element(keysym as libc::c_uint);

                    return Some(KeyboardInput(state, event.keycode as u8, vkey));
                },

                ffi::ButtonPress | ffi::ButtonRelease => {
                    use events::Event::{MouseInput, MouseWheel};
                    use events::ElementState::{Pressed, Released};
                    use events::MouseButton::{Left, Right, Middle};

                    let event: &ffi::XButtonEvent = unsafe { mem::transmute(&xev) };

                    let state = if xev.get_type() == ffi::ButtonPress { Pressed } else { Released };

                    let button = match event.button {
                        ffi::Button1 => Some(Left),
                        ffi::Button2 => Some(Middle),
                        ffi::Button3 => Some(Right),
                        ffi::Button4 => {
                            self.window.pending_events.lock().unwrap().push_back(MouseWheel(0.0, 1.0));
                            None
                        }
                        ffi::Button5 => {
                            self.window.pending_events.lock().unwrap().push_back(MouseWheel(0.0, -1.0));
                            None
                        }
                        _ => None
                    };

                    match button {
                        Some(button) =>
                            return Some(MouseInput(state, button)),
                        None => ()
                    };
                },

                _ => ()
            };
        }
    }
}

pub struct WaitEventsIterator<'a> {
    window: &'a Window,
}

impl<'a> Iterator for WaitEventsIterator<'a> {
    type Item = Event;

    fn next(&mut self) -> Option<Event> {
        use std::mem;

        while !self.window.is_closed() {
            if let Some(ev) = self.window.pending_events.lock().unwrap().pop_front() {
                return Some(ev);
            }

            // this will block until an event arrives, but doesn't remove
            // it from the queue
            let mut xev = unsafe { mem::uninitialized() };
            unsafe { (self.window.x.xlib.XPeekEvent)(self.window.x.display, &mut xev) };

            // calling poll_events()
            if let Some(ev) = self.window.poll_events().next() {
                return Some(ev);
            }
        }

        None
    }
}

pub struct Window {
    pub x: Arc<XWindow>,
    is_closed: AtomicBool,
    wm_delete_window: ffi::Atom,
    current_size: Cell<(libc::c_int, libc::c_int)>,
    pixel_format: PixelFormat,
    /// Events that have been retreived with XLib but not dispatched with iterators yet
    pending_events: Mutex<VecDeque<Event>>,
    cursor_state: Mutex<CursorState>,
}

impl Window {
    pub fn new(builder: BuilderAttribs) -> Result<Window, CreationError> {
        let xlib = ffi::Xlib::open().unwrap();        // FIXME: gracious handling
        let xcursor = ffi::Xcursor::open().unwrap();        // FIXME: gracious handling
        let xf86vmode = ffi::Xf86vmode::open().unwrap();        // FIXME: gracious handling

        let glx = {
            let libglx = unsafe { dlopen::dlopen(b"libGL.so\0".as_ptr() as *const _, dlopen::RTLD_NOW) };
            if libglx.is_null() {
                return Err(CreationError::NotSupported);
            }
            ffi::glx::Glx::load_with(|sym| {
                let sym = CString::new(sym).unwrap();
                unsafe { dlopen::dlsym(libglx, sym.as_ptr()) }
            })
        };

        ensure_thread_init(&xlib);
        let dimensions = builder.dimensions.unwrap_or((800, 600));

        // calling XOpenDisplay
        let display = unsafe {
            let display = (xlib.XOpenDisplay)(ptr::null());
            if display.is_null() {
                return Err(OsError(format!("XOpenDisplay failed")));
            }
            display
        };

        let screen_id = match builder.monitor {
            Some(MonitorID(monitor)) => monitor as i32,
            None => unsafe { (xlib.XDefaultScreen)(display) },
        };

        // getting the FBConfig
        let fb_config = unsafe {
            let mut visual_attributes = vec![
                ffi::glx::X_RENDERABLE as libc::c_int,  1,
                ffi::glx::DRAWABLE_TYPE as libc::c_int, ffi::glx::WINDOW_BIT as libc::c_int,
                ffi::glx::RENDER_TYPE as libc::c_int,   ffi::glx::RGBA_BIT as libc::c_int,
                ffi::glx::X_VISUAL_TYPE as libc::c_int, ffi::glx::TRUE_COLOR as libc::c_int,
                ffi::glx::RED_SIZE as libc::c_int,      8,
                ffi::glx::GREEN_SIZE as libc::c_int,    8,
                ffi::glx::BLUE_SIZE as libc::c_int,     8,
                ffi::glx::ALPHA_SIZE as libc::c_int,    8,
                ffi::glx::DEPTH_SIZE as libc::c_int,    24,
                ffi::glx::STENCIL_SIZE as libc::c_int,  8,
                ffi::glx::DOUBLEBUFFER as libc::c_int,  1,
            ];

            if let Some(val) = builder.multisampling {
                visual_attributes.push(ffi::glx::SAMPLE_BUFFERS as libc::c_int);
                visual_attributes.push(1);
                visual_attributes.push(ffi::glx::SAMPLES as libc::c_int);
                visual_attributes.push(val as libc::c_int);
            }

            if let Some(val) = builder.srgb {
                visual_attributes.push(ffi::glx_extra::FRAMEBUFFER_SRGB_CAPABLE_ARB as libc::c_int);
                visual_attributes.push(if val {1} else {0});
            }

            visual_attributes.push(0);

            let mut num_fb: libc::c_int = mem::uninitialized();

            let fb = glx.ChooseFBConfig(display as *mut _, (xlib.XDefaultScreen)(display),
                visual_attributes.as_ptr(), &mut num_fb);
            if fb.is_null() {
                return Err(OsError(format!("glx::ChooseFBConfig failed")));
            }
            let preferred_fb = *fb;     // TODO: choose more wisely
            (xlib.XFree)(fb as *mut _);
            preferred_fb
        };

        let mut best_mode = -1;
        let modes = unsafe {
            let mut mode_num: libc::c_int = mem::uninitialized();
            let mut modes: *mut *mut ffi::XF86VidModeModeInfo = mem::uninitialized();
            if (xf86vmode.XF86VidModeGetAllModeLines)(display, screen_id, &mut mode_num, &mut modes) == 0 {
                return Err(OsError(format!("Could not query the video modes")));
            }

            for i in 0..mode_num {
                let mode: ffi::XF86VidModeModeInfo = ptr::read(*modes.offset(i as isize) as *const _);
                if mode.hdisplay == dimensions.0 as u16 && mode.vdisplay == dimensions.1 as u16 {
                    best_mode = i;
                }
            };
            if best_mode == -1 && builder.monitor.is_some() {
                return Err(OsError(format!("Could not find a suitable graphics mode")));
            }

            modes
        };

        let xf86_desk_mode = unsafe {
            *modes.offset(0)
        };

        // getting the visual infos
        let mut visual_infos: ffi::glx::types::XVisualInfo = unsafe {
            let vi = glx.GetVisualFromFBConfig(display as *mut _, fb_config);
            if vi.is_null() {
                return Err(OsError(format!("glx::ChooseVisual failed")));
            }
            let vi_copy = ptr::read(vi as *const _);
            (xlib.XFree)(vi as *mut _);
            vi_copy
        };

        // querying the chosen pixel format
        let pixel_format = {
            let get_attrib = |attrib: libc::c_int| -> i32 {
                let mut value = 0;
                unsafe { glx.GetFBConfigAttrib(display as *mut _, fb_config, attrib, &mut value); }
                value
            };

            PixelFormat {
                hardware_accelerated: true,
                color_bits: get_attrib(ffi::glx::RED_SIZE as libc::c_int) as u8 +
                            get_attrib(ffi::glx::GREEN_SIZE as libc::c_int) as u8 +
                            get_attrib(ffi::glx::BLUE_SIZE as libc::c_int) as u8,
                alpha_bits: get_attrib(ffi::glx::ALPHA_SIZE as libc::c_int) as u8,
                depth_bits: get_attrib(ffi::glx::DEPTH_SIZE as libc::c_int) as u8,
                stencil_bits: get_attrib(ffi::glx::STENCIL_SIZE as libc::c_int) as u8,
                stereoscopy: get_attrib(ffi::glx::STEREO as libc::c_int) != 0,
                double_buffer: get_attrib(ffi::glx::DOUBLEBUFFER as libc::c_int) != 0,
                multisampling: if get_attrib(ffi::glx::SAMPLE_BUFFERS as libc::c_int) != 0 {
                    Some(get_attrib(ffi::glx::SAMPLES as libc::c_int) as u16)
                }else { None },
                srgb: get_attrib(ffi::glx_extra::FRAMEBUFFER_SRGB_CAPABLE_ARB as libc::c_int) != 0,
            }
        };

        // getting the root window
        let root = unsafe { (xlib.XDefaultRootWindow)(display) };

        // creating the color map
        let cmap = unsafe {
            let cmap = (xlib.XCreateColormap)(display, root,
                visual_infos.visual as *mut _, ffi::AllocNone);
            // TODO: error checking?
            cmap
        };

        // creating
        let mut set_win_attr = {
            let mut swa: ffi::XSetWindowAttributes = unsafe { mem::zeroed() };
            swa.colormap = cmap;
            swa.event_mask = ffi::ExposureMask | ffi::StructureNotifyMask |
                ffi::VisibilityChangeMask | ffi::KeyPressMask | ffi::PointerMotionMask |
                ffi::KeyReleaseMask | ffi::ButtonPressMask |
                ffi::ButtonReleaseMask | ffi::KeymapStateMask;
            swa.border_pixel = 0;
            swa.override_redirect = 0;
            swa
        };

        let mut window_attributes = ffi::CWBorderPixel | ffi::CWColormap | ffi:: CWEventMask;
        if builder.monitor.is_some() {
            window_attributes |= ffi::CWOverrideRedirect;
            unsafe {
                (xf86vmode.XF86VidModeSwitchToMode)(display, screen_id, *modes.offset(best_mode as isize));
                (xf86vmode.XF86VidModeSetViewPort)(display, screen_id, 0, 0);
                set_win_attr.override_redirect = 1;
            }
        }

        // finally creating the window
        let window = unsafe {
            let win = (xlib.XCreateWindow)(display, root, 0, 0, dimensions.0 as libc::c_uint,
                dimensions.1 as libc::c_uint, 0, visual_infos.depth, ffi::InputOutput as libc::c_uint,
                visual_infos.visual as *mut _, window_attributes,
                &mut set_win_attr);
            win
        };

        // set visibility
        if builder.visible {
            unsafe {
                (xlib.XMapRaised)(display, window);
                (xlib.XFlush)(display);
            }
        }

        // creating window, step 2
        let wm_delete_window = unsafe {
            let mut wm_delete_window = with_c_str("WM_DELETE_WINDOW", |delete_window|
                (xlib.XInternAtom)(display, delete_window, 0)
            );
            (xlib.XSetWMProtocols)(display, window, &mut wm_delete_window, 1);
            with_c_str(&*builder.title, |title| {;
                (xlib.XStoreName)(display, window, title);
            });
            (xlib.XFlush)(display);

            wm_delete_window
        };

        // creating IM
        let im = unsafe {
            let _lock = GLOBAL_XOPENIM_LOCK.lock().unwrap();

            let im = (xlib.XOpenIM)(display, ptr::null_mut(), ptr::null_mut(), ptr::null_mut());
            if im.is_null() {
                return Err(OsError(format!("XOpenIM failed")));
            }
            im
        };

        // creating input context
        let ic = unsafe {
            let ic = with_c_str("inputStyle", |input_style|
                with_c_str("clientWindow", |client_window|
                    (xlib.XCreateIC)(
                        im, input_style,
                        ffi::XIMPreeditNothing | ffi::XIMStatusNothing, client_window,
                        window, ptr::null::<()>()
                    )
                )
            );
            if ic.is_null() {
                return Err(OsError(format!("XCreateIC failed")));
            }
            (xlib.XSetICFocus)(ic);
            ic
        };

        // Attempt to make keyboard input repeat detectable
        unsafe {
            let mut supported_ptr = ffi::False;
            (xlib.XkbSetDetectableAutoRepeat)(display, ffi::True, &mut supported_ptr);
            if supported_ptr == ffi::False {
                return Err(OsError(format!("XkbSetDetectableAutoRepeat failed")));
            }
        }

        // Set ICCCM WM_CLASS property based on initial window title
        unsafe {
            with_c_str(&*builder.title, |c_name| {
                let hint = (xlib.XAllocClassHint)();
                (*hint).res_name = c_name as *mut i8;
                (*hint).res_class = c_name as *mut i8;
                (xlib.XSetClassHint)(display, window, hint);
                (xlib.XFree)(hint as *mut libc::c_void);
            });
        }

        let is_fullscreen = builder.monitor.is_some();
        // creating the context
        let context = match builder.gl_version {
            GlRequest::Latest | GlRequest::Specific(Api::OpenGl, _) | GlRequest::GlThenGles { .. } => {
                Context::Glx(try!(GlxContext::new(glx, builder, display, window,
                                                  fb_config, visual_infos)))
            },
            GlRequest::Specific(Api::OpenGlEs, _) => {
                let libegl = unsafe { dlopen::dlopen(b"libEGL.so\0".as_ptr() as *const _, dlopen::RTLD_NOW) };
                if libegl.is_null() {
                    return Err(CreationError::NotSupported);
                }
                let egl = ::api::egl::ffi::egl::Egl::load_with(|sym| {
                    let sym = CString::new(sym).unwrap();
                    unsafe { dlopen::dlsym(libegl, sym.as_ptr()) }
                });
                Context::Egl(try!(EglContext::new(egl, builder, Some(display as *const _), unsafe { mem::transmute(window) })))
            },
            GlRequest::Specific(_, _) => {
                return Err(CreationError::NotSupported);
            },
        };

        // creating the window object
        let window = Window {
            x: Arc::new(XWindow {
                xlib: xlib,
                xcursor: xcursor,
                xf86vmode: xf86vmode,
                display: display,
                window: window,
                im: im,
                ic: ic,
                context: context,
                screen_id: screen_id,
                is_fullscreen: is_fullscreen,
                xf86_desk_mode: xf86_desk_mode,
            }),
            is_closed: AtomicBool::new(false),
            wm_delete_window: wm_delete_window,
            current_size: Cell::new((0, 0)),
            pixel_format: pixel_format,
            pending_events: Mutex::new(VecDeque::new()),
            cursor_state: Mutex::new(CursorState::Normal),
        };

        // returning
        Ok(window)
    }

    pub fn is_closed(&self) -> bool {
        use std::sync::atomic::Ordering::Relaxed;
        self.is_closed.load(Relaxed)
    }

    pub fn set_title(&self, title: &str) {
        with_c_str(title, |title| unsafe {
            (self.x.xlib.XStoreName)(self.x.display, self.x.window, title);
            (self.x.xlib.XFlush)(self.x.display);
        })
    }

    pub fn show(&self) {
        unsafe {
            (self.x.xlib.XMapRaised)(self.x.display, self.x.window);
            (self.x.xlib.XFlush)(self.x.display);
        }
    }

    pub fn hide(&self) {
        unsafe {
            (self.x.xlib.XUnmapWindow)(self.x.display, self.x.window);
            (self.x.xlib.XFlush)(self.x.display);
        }
    }

    fn get_geometry(&self) -> Option<(i32, i32, u32, u32, u32)> {
        unsafe {
            use std::mem;

            let mut root: ffi::Window = mem::uninitialized();
            let mut x: libc::c_int = mem::uninitialized();
            let mut y: libc::c_int = mem::uninitialized();
            let mut width: libc::c_uint = mem::uninitialized();
            let mut height: libc::c_uint = mem::uninitialized();
            let mut border: libc::c_uint = mem::uninitialized();
            let mut depth: libc::c_uint = mem::uninitialized();

            if (self.x.xlib.XGetGeometry)(self.x.display, self.x.window,
                &mut root, &mut x, &mut y, &mut width, &mut height,
                &mut border, &mut depth) == 0
            {
                return None;
            }

            Some((x as i32, y as i32, width as u32, height as u32, border as u32))
        }
    }

    pub fn get_position(&self) -> Option<(i32, i32)> {
        self.get_geometry().map(|(x, y, _, _, _)| (x, y))
    }

    pub fn set_position(&self, x: i32, y: i32) {
        unsafe { (self.x.xlib.XMoveWindow)(self.x.display, self.x.window, x as libc::c_int, y as libc::c_int); }
    }

    pub fn get_inner_size(&self) -> Option<(u32, u32)> {
        self.get_geometry().map(|(_, _, w, h, _)| (w, h))
    }

    pub fn get_outer_size(&self) -> Option<(u32, u32)> {
        self.get_geometry().map(|(_, _, w, h, b)| (w + b, h + b))       // TODO: is this really outside?
    }

    pub fn set_inner_size(&self, _x: u32, _y: u32) {
        unimplemented!()
    }

    pub fn create_window_proxy(&self) -> WindowProxy {
        WindowProxy {
            x: self.x.clone()
        }
    }

    pub fn poll_events(&self) -> PollEventsIterator {
        PollEventsIterator {
            window: self
        }
    }

    pub fn wait_events(&self) -> WaitEventsIterator {
        WaitEventsIterator {
            window: self
        }
    }

    pub fn platform_display(&self) -> *mut libc::c_void {
        self.x.display as *mut libc::c_void
    }

    pub fn platform_window(&self) -> *mut libc::c_void {
        unimplemented!()
    }


    pub fn set_window_resize_callback(&mut self, _: Option<fn(u32, u32)>) {
    }

    pub fn set_cursor(&self, cursor: MouseCursor) {
        unsafe {
            use std::ffi::CString;
            let cursor_name = match cursor {
                MouseCursor::Alias => "link",
                MouseCursor::Arrow => "arrow",
                MouseCursor::Cell => "plus",
                MouseCursor::Copy => "copy",
                MouseCursor::Crosshair => "crosshair",
                MouseCursor::Default => "left_ptr",
                MouseCursor::Grabbing => "grabbing",
                MouseCursor::Hand | MouseCursor::Grab => "hand",
                MouseCursor::Help => "question_arrow",
                MouseCursor::Move => "move",
                MouseCursor::NoDrop => "circle",
                MouseCursor::NotAllowed => "crossed_circle",
                MouseCursor::Progress => "left_ptr_watch",

                /// Resize cursors
                MouseCursor::EResize => "right_side",
                MouseCursor::NResize => "top_side",
                MouseCursor::NeResize => "top_right_corner",
                MouseCursor::NwResize => "top_left_corner",
                MouseCursor::SResize => "bottom_side",
                MouseCursor::SeResize => "bottom_right_corner",
                MouseCursor::SwResize => "bottom_left_corner",
                MouseCursor::WResize => "left_side",
                MouseCursor::EwResize | MouseCursor::ColResize => "h_double_arrow",
                MouseCursor::NsResize | MouseCursor::RowResize => "v_double_arrow",
                MouseCursor::NwseResize => "bd_double_arrow",
                MouseCursor::NeswResize => "fd_double_arrow",

                MouseCursor::Text | MouseCursor::VerticalText => "xterm",
                MouseCursor::Wait => "watch",

                /// TODO: Find matching X11 cursors
                MouseCursor::ContextMenu | MouseCursor::NoneCursor |
                MouseCursor::AllScroll | MouseCursor::ZoomIn |
                MouseCursor::ZoomOut => "left_ptr",
            };
            let c_string = CString::new(cursor_name.as_bytes().to_vec()).unwrap();
            let xcursor = (self.x.xcursor.XcursorLibraryLoadCursor)(self.x.display, c_string.as_ptr());
            (self.x.xlib.XDefineCursor)(self.x.display, self.x.window, xcursor);
            (self.x.xlib.XFlush)(self.x.display);
        }
    }

    pub fn set_cursor_state(&self, state: CursorState) -> Result<(), String> {
        let mut cursor_state = self.cursor_state.lock().unwrap();

        match (state, *cursor_state) {
            (CursorState::Normal, CursorState::Grab) => {
                unsafe {
                    (self.x.xlib.XUngrabPointer)(self.x.display, ffi::CurrentTime);
                    *cursor_state = CursorState::Normal;
                    Ok(())
                }
            },

            (CursorState::Grab, CursorState::Normal) => {
                unsafe {
                    *cursor_state = CursorState::Grab;

                    match (self.x.xlib.XGrabPointer)(
                        self.x.display, self.x.window, ffi::False,
                        (ffi::ButtonPressMask | ffi::ButtonReleaseMask | ffi::EnterWindowMask |
                        ffi::LeaveWindowMask | ffi::PointerMotionMask | ffi::PointerMotionHintMask |
                        ffi::Button1MotionMask | ffi::Button2MotionMask | ffi::Button3MotionMask |
                        ffi::Button4MotionMask | ffi::Button5MotionMask | ffi::ButtonMotionMask |
                        ffi::KeymapStateMask) as libc::c_uint,
                        ffi::GrabModeAsync, ffi::GrabModeAsync,
                        self.x.window, 0, ffi::CurrentTime
                    ) {
                        ffi::GrabSuccess => Ok(()),
                        ffi::AlreadyGrabbed | ffi::GrabInvalidTime |
                        ffi::GrabNotViewable | ffi::GrabFrozen
                            => Err("cursor could not be grabbed".to_string()),
                        _ => unreachable!(),
                    }
                }
            },

            _ => unimplemented!(),
        }
    }

    pub fn hidpi_factor(&self) -> f32 {
        1.0
    }

    pub fn set_cursor_position(&self, x: i32, y: i32) -> Result<(), ()> {
        unsafe {
            (self.x.xlib.XWarpPointer)(self.x.display, 0, self.x.window, 0, 0, 0, 0, x, y);
        }

        Ok(())
    }
}

impl GlContext for Window {
    unsafe fn make_current(&self) {
        match self.x.context {
            Context::Glx(ref ctxt) => ctxt.make_current(),
            Context::Egl(ref ctxt) => ctxt.make_current(),
            Context::None => {}
        }
    }

    fn is_current(&self) -> bool {
        match self.x.context {
            Context::Glx(ref ctxt) => ctxt.is_current(),
            Context::Egl(ref ctxt) => ctxt.is_current(),
            Context::None => panic!()
        }
    }

    fn get_proc_address(&self, addr: &str) -> *const libc::c_void {
        match self.x.context {
            Context::Glx(ref ctxt) => ctxt.get_proc_address(addr),
            Context::Egl(ref ctxt) => ctxt.get_proc_address(addr),
            Context::None => ptr::null()
        }
    }

    fn swap_buffers(&self) {
        match self.x.context {
            Context::Glx(ref ctxt) => ctxt.swap_buffers(),
            Context::Egl(ref ctxt) => ctxt.swap_buffers(),
            Context::None => {}
        }
    }

    fn get_api(&self) -> Api {
        match self.x.context {
            Context::Glx(ref ctxt) => ctxt.get_api(),
            Context::Egl(ref ctxt) => ctxt.get_api(),
            Context::None => panic!()
        }
    }

    fn get_pixel_format(&self) -> PixelFormat {
        self.pixel_format.clone()
    }
}
