use {Event, BuilderAttribs, MouseCursor};
use CreationError;
use CreationError::OsError;
use libc;
use std::{mem, ptr};
use std::cell::Cell;
use std::sync::atomic::AtomicBool;
use std::collections::RingBuf;
use super::ffi;
use std::sync::{Arc, Once, ONCE_INIT};

pub use self::monitor::{MonitorID, get_available_monitors, get_primary_monitor};

mod events;
mod monitor;

static THREAD_INIT: Once = ONCE_INIT;

fn ensure_thread_init() {
    THREAD_INIT.call_once(|| {
        unsafe {
            ffi::XInitThreads();
        }
    });
}

fn with_c_str<F, T>(s: &str, f: F) -> T where F: FnOnce(*const i8) -> T {
    use std::ffi::CString;
    let c_str = CString::from_slice(s.as_bytes());
    f(c_str.as_slice_with_nul().as_ptr())    
}

struct XWindow {
    display: *mut ffi::Display,
    window: ffi::Window,
    context: ffi::GLXContext,
    is_fullscreen: bool,
    screen_id: libc::c_int,
    xf86_desk_mode: *mut ffi::XF86VidModeModeInfo,
    ic: ffi::XIC,
    im: ffi::XIM,
}

unsafe impl Send for Window {}
unsafe impl Sync for Window {}

impl Drop for XWindow {
    fn drop(&mut self) {
        unsafe {
            ffi::glx::MakeCurrent(self.display, 0, ptr::null());
            ffi::glx::DestroyContext(self.display, self.context);

            if self.is_fullscreen {
                ffi::XF86VidModeSwitchToMode(self.display, self.screen_id, self.xf86_desk_mode);
                ffi::XF86VidModeSetViewPort(self.display, self.screen_id, 0, 0);
            }

            ffi::XDestroyIC(self.ic);
            ffi::XCloseIM(self.im);
            ffi::XDestroyWindow(self.display, self.window);
            ffi::XCloseDisplay(self.display);
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
            l: [0, 0, 0, 0, 0],
        };

        unsafe {
            ffi::XSendEvent(self.x.display, self.x.window, 0, 0, mem::transmute(&mut xev));
            ffi::XFlush(self.x.display);
        }
    }
}

pub struct Window {
    x: Arc<XWindow>,
    is_closed: AtomicBool,
    wm_delete_window: ffi::Atom,
    current_size: Cell<(libc::c_int, libc::c_int)>,
}

impl Window {
    pub fn new(builder: BuilderAttribs) -> Result<Window, CreationError> {
        ensure_thread_init();
        let dimensions = builder.dimensions.unwrap_or((800, 600));

        // calling XOpenDisplay
        let display = unsafe {
            let display = ffi::XOpenDisplay(ptr::null());
            if display.is_null() {
                return Err(OsError(format!("XOpenDisplay failed")));
            }
            display
        };

        let screen_id = match builder.monitor {
            Some(MonitorID(monitor)) => monitor as i32,
            None => unsafe { ffi::XDefaultScreen(display) },
        };

        // getting the FBConfig
        let fb_config = unsafe {
            let mut visual_attributes = vec![
                ffi::GLX_X_RENDERABLE,  1,
                ffi::GLX_DRAWABLE_TYPE, ffi::GLX_WINDOW_BIT,
                ffi::GLX_RENDER_TYPE,   ffi::GLX_RGBA_BIT,
                ffi::GLX_X_VISUAL_TYPE, ffi::GLX_TRUE_COLOR,
                ffi::GLX_RED_SIZE,      8,
                ffi::GLX_GREEN_SIZE,    8,
                ffi::GLX_BLUE_SIZE,     8,
                ffi::GLX_ALPHA_SIZE,    8,
                ffi::GLX_DEPTH_SIZE,    24,
                ffi::GLX_STENCIL_SIZE,  8,
                ffi::GLX_DOUBLEBUFFER,  1,
            ];

            if let Some(val) = builder.multisampling {
                visual_attributes.push(ffi::glx::SAMPLE_BUFFERS as libc::c_int);
                visual_attributes.push(1);
                visual_attributes.push(ffi::glx::SAMPLES as libc::c_int);
                visual_attributes.push(val as libc::c_int);
            }

            visual_attributes.push(0);

            let mut num_fb: libc::c_int = mem::uninitialized();

            let fb = ffi::glx::ChooseFBConfig(display, ffi::XDefaultScreen(display),
                visual_attributes.as_ptr(), &mut num_fb);
            if fb.is_null() {
                return Err(OsError(format!("glx::ChooseFBConfig failed")));
            }
            let preferred_fb = *fb;     // TODO: choose more wisely
            ffi::XFree(fb as *const libc::c_void);
            preferred_fb
        };

        let mut best_mode = -1;
        let modes = unsafe {
            let mut mode_num: libc::c_int = mem::uninitialized();
            let mut modes: *mut *mut ffi::XF86VidModeModeInfo = mem::uninitialized();
            if ffi::XF86VidModeGetAllModeLines(display, screen_id, &mut mode_num, &mut modes) == 0 {
                return Err(OsError(format!("Could not query the video modes")));
            }

            for i in range(0, mode_num) {
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
            let vi = ffi::glx::GetVisualFromFBConfig(display, fb_config);
            if vi.is_null() {
                return Err(OsError(format!("glx::ChooseVisual failed")));
            }
            let vi_copy = ptr::read(vi as *const _);
            ffi::XFree(vi as *const libc::c_void);
            vi_copy
        };

        // getting the root window
        let root = unsafe { ffi::XDefaultRootWindow(display) };

        // creating the color map
        let cmap = unsafe {
            let cmap = ffi::XCreateColormap(display, root,
                visual_infos.visual, ffi::AllocNone);
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
                ffi::XF86VidModeSwitchToMode(display, screen_id, *modes.offset(best_mode as isize));
                ffi::XF86VidModeSetViewPort(display, screen_id, 0, 0);
                set_win_attr.override_redirect = 1;
            }
        }

        // finally creating the window
        let window = unsafe {
            let win = ffi::XCreateWindow(display, root, 0, 0, dimensions.0 as libc::c_uint,
                dimensions.1 as libc::c_uint, 0, visual_infos.depth, ffi::InputOutput,
                visual_infos.visual, window_attributes,
                &mut set_win_attr);
            win
        };

        // set visibility
        if builder.visible {
            unsafe {
                ffi::XMapRaised(display, window);
                ffi::XFlush(display);
            }
        }

        // creating window, step 2
        let wm_delete_window = unsafe {
            let mut wm_delete_window = with_c_str("WM_DELETE_WINDOW", |delete_window| 
                ffi::XInternAtom(display, delete_window, 0)
            );
            ffi::XSetWMProtocols(display, window, &mut wm_delete_window, 1);
            with_c_str(&*builder.title, |title| {;
                ffi::XStoreName(display, window, title);
            });
            ffi::XFlush(display);

            wm_delete_window
        };

        // creating IM
        let im = unsafe {
            let im = ffi::XOpenIM(display, ptr::null(), ptr::null_mut(), ptr::null_mut());
            if im.is_null() {
                return Err(OsError(format!("XOpenIM failed")));
            }
            im
        };

        // creating input context
        let ic = unsafe {
            let ic = with_c_str("inputStyle", |input_style|
                with_c_str("clientWindow", |client_window|
                    ffi::XCreateIC(
                        im, input_style,
                        ffi::XIMPreeditNothing | ffi::XIMStatusNothing, client_window,
                        window, ptr::null()
                    )
                )
            );
            if ic.is_null() {
                return Err(OsError(format!("XCreateIC failed")));
            }
            ffi::XSetICFocus(ic);
            ic
        };

        // Attempt to make keyboard input repeat detectable
        unsafe {
            let mut supported_ptr = false;
            ffi::XkbSetDetectableAutoRepeat(display, true, &mut supported_ptr);
            if !supported_ptr {
                return Err(OsError(format!("XkbSetDetectableAutoRepeat failed")));
            }
        }


        // creating GL context
        let context = unsafe {
            let mut attributes = Vec::new();

            if builder.gl_version.is_some() {
                let version = builder.gl_version.as_ref().unwrap();
                attributes.push(ffi::GLX_CONTEXT_MAJOR_VERSION);
                attributes.push(version.0 as libc::c_int);
                attributes.push(ffi::GLX_CONTEXT_MINOR_VERSION);
                attributes.push(version.1 as libc::c_int);
            }

            if builder.gl_debug {
                attributes.push(ffi::glx_extra::CONTEXT_FLAGS_ARB as libc::c_int);
                attributes.push(ffi::glx_extra::CONTEXT_DEBUG_BIT_ARB as libc::c_int);
            }

            attributes.push(0);

            // loading the extra GLX functions
            let extra_functions = ffi::glx_extra::Glx::load_with(|addr| {
                with_c_str(addr, |s| {
                    use libc;
                    ffi::glx::GetProcAddress(s as *const u8) as *const libc::c_void
                })
            });

            let share = if let Some(win) = builder.sharing {
                win.x.context
            } else {
                ptr::null()
            };

            let context = if extra_functions.CreateContextAttribsARB.is_loaded() {
                extra_functions.CreateContextAttribsARB(display as *mut ffi::glx_extra::types::Display,
                    fb_config, share, 1, attributes.as_ptr())
            } else {
                ffi::glx::CreateContext(display, &mut visual_infos, share, 1)
            };

            if context.is_null() {
                return Err(OsError(format!("GL context creation failed")));
            }

            context
        };

        // creating the window object
        let window = Window {
            x: Arc::new(XWindow {
                display: display,
                window: window,
                im: im,
                ic: ic,
                context: context,
                screen_id: screen_id,
                is_fullscreen: builder.monitor.is_some(),
                xf86_desk_mode: xf86_desk_mode,
            }),
            is_closed: AtomicBool::new(false),
            wm_delete_window: wm_delete_window,
            current_size: Cell::new((0, 0)),
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
            ffi::XStoreName(self.x.display, self.x.window, title);
            ffi::XFlush(self.x.display);
        })
    }

    pub fn show(&self) {
        unsafe {
            ffi::XMapRaised(self.x.display, self.x.window);
            ffi::XFlush(self.x.display);
        }
    }

    pub fn hide(&self) {
        unsafe {
            ffi::XUnmapWindow(self.x.display, self.x.window);
            ffi::XFlush(self.x.display);
        }
    }

    fn get_geometry(&self) -> Option<(i32, i32, u32, u32)> {
        unsafe {
            use std::mem;

            let mut root: ffi::Window = mem::uninitialized();
            let mut x: libc::c_int = mem::uninitialized();
            let mut y: libc::c_int = mem::uninitialized();
            let mut width: libc::c_uint = mem::uninitialized();
            let mut height: libc::c_uint = mem::uninitialized();
            let mut border: libc::c_uint = mem::uninitialized();
            let mut depth: libc::c_uint = mem::uninitialized();

            if ffi::XGetGeometry(self.x.display, self.x.window,
                &mut root, &mut x, &mut y, &mut width, &mut height,
                &mut border, &mut depth) == 0
            {
                return None;
            }

            Some((x as i32, y as i32, width as u32, height as u32))
        }
    }

    pub fn get_position(&self) -> Option<(i32, i32)> {
        self.get_geometry().map(|(x, y, _, _)| (x, y))
    }

    pub fn set_position(&self, x: i32, y: i32) {
        unsafe { ffi::XMoveWindow(self.x.display, self.x.window, x as libc::c_int, y as libc::c_int) }
    }

    pub fn get_inner_size(&self) -> Option<(u32, u32)> {
        self.get_geometry().map(|(_, _, w, h)| (w, h))
    }

    pub fn get_outer_size(&self) -> Option<(u32, u32)> {
        unimplemented!()
    }

    pub fn set_inner_size(&self, _x: u32, _y: u32) {
        unimplemented!()
    }

    pub fn create_window_proxy(&self) -> WindowProxy {
        WindowProxy {
            x: self.x.clone()
        }
    }

    pub fn poll_events(&self) -> RingBuf<Event> {
        use std::mem;

        let mut events = RingBuf::new();

        loop {
            use std::num::Int;

            let mut xev = unsafe { mem::uninitialized() };
            let res = unsafe { ffi::XCheckMaskEvent(self.x.display, Int::max_value(), &mut xev) };

            if res == 0 {
                let res = unsafe { ffi::XCheckTypedEvent(self.x.display, ffi::ClientMessage, &mut xev) };

                if res == 0 {
                    break
                }
            }

            match xev.type_ {
                ffi::KeymapNotify => {
                    unsafe { ffi::XRefreshKeyboardMapping(&xev) }
                },

                ffi::ClientMessage => {
                    use events::Event::{Closed, Awakened};
                    use std::sync::atomic::Ordering::Relaxed;

                    let client_msg: &ffi::XClientMessageEvent = unsafe { mem::transmute(&xev) };

                    if client_msg.l[0] == self.wm_delete_window as libc::c_long {
                        self.is_closed.store(true, Relaxed);
                        events.push_back(Closed);
                    } else {
                        events.push_back(Awakened);
                    }
                },

                ffi::ConfigureNotify => {
                    use events::Event::Resized;
                    let cfg_event: &ffi::XConfigureEvent = unsafe { mem::transmute(&xev) };
                    let (current_width, current_height) = self.current_size.get();
                    if current_width != cfg_event.width || current_height != cfg_event.height {
                        self.current_size.set((cfg_event.width, cfg_event.height));
                        events.push_back(Resized(cfg_event.width as u32, cfg_event.height as u32));
                    }
                },

                ffi::MotionNotify => {
                    use events::Event::MouseMoved;
                    let event: &ffi::XMotionEvent = unsafe { mem::transmute(&xev) };
                    events.push_back(MouseMoved((event.x as i32, event.y as i32)));
                },

                ffi::KeyPress | ffi::KeyRelease => {
                    use events::Event::{KeyboardInput, ReceivedCharacter};
                    use events::ElementState::{Pressed, Released};
                    let event: &mut ffi::XKeyEvent = unsafe { mem::transmute(&xev) };

                    if event.type_ == ffi::KeyPress {
                        let raw_ev: *mut ffi::XKeyEvent = event;
                        unsafe { ffi::XFilterEvent(mem::transmute(raw_ev), self.x.window) };
                    }

                    let state = if xev.type_ == ffi::KeyPress { Pressed } else { Released };

                    let written = unsafe {
                        use std::str;

                        let mut buffer: [u8; 16] = [mem::uninitialized(); 16];
                        let raw_ev: *mut ffi::XKeyEvent = event;
                        let count = ffi::Xutf8LookupString(self.x.ic, mem::transmute(raw_ev),
                            mem::transmute(buffer.as_mut_ptr()),
                            buffer.len() as libc::c_int, ptr::null_mut(), ptr::null_mut());

                        str::from_utf8(buffer.as_slice().slice_to(count as usize))
                            .unwrap_or("").to_string()
                    };

                    for chr in written.as_slice().chars() {
                        events.push_back(ReceivedCharacter(chr));
                    }

                    let keysym = unsafe {
                        ffi::XKeycodeToKeysym(self.x.display, event.keycode as ffi::KeyCode, 0)
                    };

                    let vkey =  events::keycode_to_element(keysym as libc::c_uint);

                    events.push_back(KeyboardInput(state, event.keycode as u8, vkey));
                },

                ffi::ButtonPress | ffi::ButtonRelease => {
                    use events::Event::{MouseInput, MouseWheel};
                    use events::ElementState::{Pressed, Released};
                    use events::MouseButton::{LeftMouseButton, RightMouseButton, MiddleMouseButton};

                    let event: &ffi::XButtonEvent = unsafe { mem::transmute(&xev) };

                    let state = if xev.type_ == ffi::ButtonPress { Pressed } else { Released };

                    let button = match event.button {
                        ffi::Button1 => Some(LeftMouseButton),
                        ffi::Button2 => Some(MiddleMouseButton),
                        ffi::Button3 => Some(RightMouseButton),
                        ffi::Button4 => {
                            events.push_back(MouseWheel(1));
                            None
                        }
                        ffi::Button5 => {
                            events.push_back(MouseWheel(-1));
                            None
                        }
                        _ => None
                    };

                    match button {
                        Some(button) =>
                            events.push_back(MouseInput(state, button)),
                        None => ()
                    };
                },

                _ => ()
            }
        }

        events
    }

    pub fn wait_events(&self) -> RingBuf<Event> {
        use std::mem;

        loop {
            // this will block until an event arrives, but doesn't remove
            //  it from the queue
            let mut xev = unsafe { mem::uninitialized() };
            unsafe { ffi::XPeekEvent(self.x.display, &mut xev) };

            // calling poll_events()
            let ev = self.poll_events();
            if ev.len() >= 1 {
                return ev;
            }
        }
    }

    pub unsafe fn make_current(&self) {
        let res = ffi::glx::MakeCurrent(self.x.display, self.x.window, self.x.context);
        if res == 0 {
            panic!("glx::MakeCurrent failed");
        }
    }

    pub fn get_proc_address(&self, addr: &str) -> *const () {
        use std::mem;

        unsafe {
            with_c_str(addr, |s| {
                ffi::glx::GetProcAddress(mem::transmute(s)) as *const ()
            })
        }
    }

    pub fn swap_buffers(&self) {
        unsafe { ffi::glx::SwapBuffers(self.x.display, self.x.window) }
    }

    pub fn platform_display(&self) -> *mut libc::c_void {
        self.x.display as *mut libc::c_void
    }

    /// See the docs in the crate root file.
    pub fn get_api(&self) -> ::Api {
        ::Api::OpenGl
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
            let c_string = CString::from_slice(cursor_name.as_bytes());
            let xcursor = ffi::XcursorLibraryLoadCursor(self.x.display, c_string.as_ptr());
            ffi::XDefineCursor (self.x.display, self.x.window, xcursor);
            ffi::XFlush(self.x.display);
        }
    }
}
