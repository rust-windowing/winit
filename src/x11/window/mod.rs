use {Event, WindowBuilder};
use CreationError;
use CreationError::OsError;
use libc;
use std::{mem, ptr};
use std::cell::Cell;
use std::sync::atomic::AtomicBool;
use super::ffi;
use std::sync::{Arc, Once, ONCE_INIT};

pub use self::monitor::{MonitorID, get_available_monitors, get_primary_monitor};

mod events;
mod monitor;

static THREAD_INIT: Once = ONCE_INIT;

fn ensure_thread_init() {
    THREAD_INIT.doit(|| {
        unsafe {
            ffi::XInitThreads();
        }
    });
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

#[deriving(Clone)]
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
    pub fn new(builder: WindowBuilder) -> Result<Window, CreationError> {
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
                let mode: ffi::XF86VidModeModeInfo = ptr::read(*modes.offset(i as int) as *const _);
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
                ffi::XF86VidModeSwitchToMode(display, screen_id, *modes.offset(best_mode as int));
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
            use std::c_str::ToCStr;

            let delete_window = "WM_DELETE_WINDOW".to_c_str();
            let mut wm_delete_window = ffi::XInternAtom(display, delete_window.as_ptr(), 0);
            ffi::XSetWMProtocols(display, window, &mut wm_delete_window, 1);
            let c_title = builder.title.to_c_str();
            ffi::XStoreName(display, window, c_title.as_ptr());
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
            use std::c_str::ToCStr;

            let input_style = "inputStyle".to_c_str();
            let client_window = "clientWindow".to_c_str();
            let ic = ffi::XCreateIC(im, input_style.as_ptr(),
                ffi::XIMPreeditNothing | ffi::XIMStatusNothing, client_window.as_ptr(),
                window, ptr::null());
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
                addr.with_c_str(|s| {
                    use libc;
                    ffi::glx::GetProcAddress(s as *const u8) as *const libc::c_void
                })
            });

            let share = if let Some(win) = builder.sharing {
                win.window.x.context
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
        use std::sync::atomic::Relaxed;
        self.is_closed.load(Relaxed)
    }

    pub fn set_title(&self, title: &str) {
        let c_title = title.to_c_str();
        unsafe {
            ffi::XStoreName(self.x.display, self.x.window, c_title.as_ptr());
            ffi::XFlush(self.x.display);
        }
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

    fn get_geometry(&self) -> Option<(int, int, uint, uint)> {
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

            Some((x as int, y as int, width as uint, height as uint))
        }
    }

    pub fn get_position(&self) -> Option<(int, int)> {
        self.get_geometry().map(|(x, y, _, _)| (x, y))
    }

    pub fn set_position(&self, x: int, y: int) {
        unsafe { ffi::XMoveWindow(self.x.display, self.x.window, x as libc::c_int, y as libc::c_int) }
    }

    pub fn get_inner_size(&self) -> Option<(uint, uint)> {
        self.get_geometry().map(|(_, _, w, h)| (w, h))
    }

    pub fn get_outer_size(&self) -> Option<(uint, uint)> {
        unimplemented!()
    }

    pub fn set_inner_size(&self, _x: uint, _y: uint) {
        unimplemented!()
    }

    pub fn create_window_proxy(&self) -> WindowProxy {
        WindowProxy {
            x: self.x.clone()
        }
    }

    pub fn poll_events(&self) -> Vec<Event> {
        use std::mem;

        let mut events = Vec::new();

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
                    use std::sync::atomic::Relaxed;

                    let client_msg: &ffi::XClientMessageEvent = unsafe { mem::transmute(&xev) };

                    if client_msg.l[0] == self.wm_delete_window as libc::c_long {
                        self.is_closed.store(true, Relaxed);
                        events.push(Closed);
                    } else {
                        events.push(Awakened);
                    }
                },

                ffi::ConfigureNotify => {
                    use events::Event::Resized;
                    let cfg_event: &ffi::XConfigureEvent = unsafe { mem::transmute(&xev) };
                    let (current_width, current_height) = self.current_size.get();
                    if current_width != cfg_event.width || current_height != cfg_event.height {
                        self.current_size.set((cfg_event.width, cfg_event.height));
                        events.push(Resized(cfg_event.width as uint, cfg_event.height as uint));
                    }
                },

                ffi::MotionNotify => {
                    use events::Event::MouseMoved;
                    let event: &ffi::XMotionEvent = unsafe { mem::transmute(&xev) };
                    events.push(MouseMoved((event.x as int, event.y as int)));
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

                        let mut buffer: [u8, ..16] = [mem::uninitialized(), ..16];
                        let raw_ev: *mut ffi::XKeyEvent = event;
                        let count = ffi::Xutf8LookupString(self.x.ic, mem::transmute(raw_ev),
                            mem::transmute(buffer.as_mut_ptr()),
                            buffer.len() as libc::c_int, ptr::null_mut(), ptr::null_mut());

                        str::from_utf8(buffer.as_slice().slice_to(count as uint))
                            .unwrap_or("").to_string()
                    };

                    for chr in written.as_slice().chars() {
                        events.push(ReceivedCharacter(chr));
                    }

                    let keysym = unsafe {
                        ffi::XKeycodeToKeysym(self.x.display, event.keycode as ffi::KeyCode, 0)
                    };

                    let vkey =  events::keycode_to_element(keysym as libc::c_uint);

                    events.push(KeyboardInput(state, event.keycode as u8, vkey));
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
                            events.push(MouseWheel(1));
                            None
                        }
                        ffi::Button5 => {
                            events.push(MouseWheel(-1));
                            None
                        }
                        _ => None
                    };

                    match button {
                        Some(button) =>
                            events.push(MouseInput(state, button)),
                        None => ()
                    };
                },

                _ => ()
            }
        }

        events
    }

    pub fn wait_events(&self) -> Vec<Event> {
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
        use std::c_str::ToCStr;
        use std::mem;

        unsafe {
            addr.with_c_str(|s| {
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

    pub fn set_window_resize_callback(&mut self, _: Option<fn(uint, uint)>) {
    }
}
