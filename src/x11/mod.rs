use {Event, Hints, MonitorID};
use libc;
use std::{mem, ptr};
use std::sync::atomics::AtomicBool;

mod events;
mod ffi;

pub struct Window {
    display: *mut ffi::Display,
    window: ffi::Window,
    context: ffi::GLXContext,
    is_closed: AtomicBool,
    wm_delete_window: ffi::Atom,
}

impl Window {
    pub fn new(dimensions: Option<(uint, uint)>, title: &str, hints: &Hints, _: Option<MonitorID>)
        -> Result<Window, String>
    {
        // calling XOpenDisplay
        let display = unsafe {
            let display = ffi::XOpenDisplay(ptr::null());
            if display.is_null() {
                return Err(format!("XOpenDisplay failed"));
            }
            display
        };

        // TODO: set error handler

        static VISUAL_ATTRIBUTES: [libc::c_int, ..5] = [
            ffi::GLX_RGBA,
            ffi::GLX_DEPTH_SIZE,
            24,
            ffi::GLX_DOUBLEBUFFER,
            0
        ];

        // getting the visual infos
        let visual_infos = unsafe {
            let vi = ffi::glXChooseVisual(display, 0, VISUAL_ATTRIBUTES.as_ptr());
            if vi.is_null() {
                return Err(format!("glXChooseVisual failed"));
            }
            vi
        };

        // getting the root window
        let root = unsafe { ffi::XDefaultRootWindow(display) };

        // creating the color map
        let cmap = unsafe {
            let cmap = ffi::XCreateColormap(display, root,
                (*visual_infos).visual, ffi::AllocNone);
            // TODO: error checking?
            cmap
        };

        // creating
        let mut set_win_attr = {
            let mut swa: ffi::XSetWindowAttributes = unsafe { mem::zeroed() };
            swa.colormap = cmap;
            swa.event_mask = ffi::ExposureMask | ffi::ResizeRedirectMask |
                ffi::VisibilityChangeMask | ffi::KeyPressMask | ffi::PointerMotionMask |
                ffi::KeyPressMask | ffi::KeyReleaseMask | ffi::ButtonPressMask |
                ffi::ButtonReleaseMask;
            swa
        };

        // finally creating the window
        let window = unsafe {
            let dimensions = dimensions.unwrap_or((800, 600));

            let win = ffi::XCreateWindow(display, root, 50, 50, dimensions.val0() as libc::c_uint,
                dimensions.val1() as libc::c_uint, 0, (*visual_infos).depth, ffi::InputOutput,
                (*visual_infos).visual, ffi::CWColormap | ffi::CWEventMask,
                &mut set_win_attr);
            win
        };

        // creating window, step 2
        let wm_delete_window = unsafe {
            use std::c_str::ToCStr;

            ffi::XMapWindow(display, window);
            let mut wm_delete_window = ffi::XInternAtom(display,
                "WM_DELETE_WINDOW".to_c_str().as_ptr() as *const libc::c_char, 0);
            ffi::XSetWMProtocols(display, window, &mut wm_delete_window, 1);
            ffi::XStoreName(display, window, mem::transmute(title.as_slice().as_ptr()));
            ffi::XFlush(display);

            wm_delete_window
        };

        // creating GL context
        let context = unsafe {
            ffi::glXCreateContext(display, visual_infos, ptr::null(), 1)
        };

        // returning
        Ok(Window{
            display: display,
            window: window,
            context: context,
            is_closed: AtomicBool::new(false),
            wm_delete_window: wm_delete_window,
        })
    }

    pub fn is_closed(&self) -> bool {
        use std::sync::atomics::Relaxed;
        self.is_closed.load(Relaxed)
    }

    pub fn set_title(&self, title: &str) {
        unsafe {
            ffi::XStoreName(self.display, self.window,
                mem::transmute(title.as_slice().as_ptr()));
        }
    }

    pub fn get_position(&self) -> Option<(int, int)> {
        unimplemented!()
    }

    pub fn set_position(&self, x: uint, y: uint) {
        unimplemented!()
    }

    pub fn get_inner_size(&self) -> Option<(uint, uint)> {
        unimplemented!()
    }

    pub fn get_outer_size(&self) -> Option<(uint, uint)> {
        unimplemented!()
    }

    pub fn set_inner_size(&self, x: uint, y: uint) {
        unimplemented!()
    }

    pub fn poll_events(&self) -> Vec<Event> {
        use std::mem;
        
        let mut events = Vec::new();
        
        loop {
            use std::num::Bounded;

            let mut xev = unsafe { mem::uninitialized() };
            let res = unsafe { ffi::XCheckMaskEvent(self.display, Bounded::max_value(), &mut xev) };

            if res == 0 {
                let res = unsafe { ffi::XCheckTypedEvent(self.display, ffi::ClientMessage, &mut xev) };

                if res == 0 {
                    break
                }
            }

            match xev.type_ {
                ffi::ClientMessage => {
                    use Closed;
                    use std::sync::atomics::Relaxed;

                    let client_msg: &ffi::XClientMessageEvent = unsafe { mem::transmute(&xev) };

                    if client_msg.l[0] == self.wm_delete_window as libc::c_long {
                        self.is_closed.store(true, Relaxed);
                        events.push(Closed);
                    }
                },

                ffi::ResizeRequest => {
                    use Resized;
                    let rs_event: &ffi::XResizeRequestEvent = unsafe { mem::transmute(&xev) };
                    events.push(Resized(rs_event.width as uint, rs_event.height as uint));
                },

                ffi::MotionNotify => {
                    use CursorPositionChanged;
                    let event: &ffi::XMotionEvent = unsafe { mem::transmute(&xev) };
                    events.push(CursorPositionChanged(event.x as uint, event.y as uint));
                },

                ffi::KeyPress | ffi::KeyRelease => {
                    use {Pressed, Released};
                    let event: &ffi::XKeyEvent = unsafe { mem::transmute(&xev) };

                    let keysym = unsafe { ffi::XKeycodeToKeysym(self.display, event.keycode as ffi::KeyCode, 0) };

                    match events::keycode_to_element(keysym as libc::c_uint) {
                        Some(elem) if xev.type_ == ffi::KeyPress => {
                            events.push(Pressed(elem));
                        },
                        Some(elem) if xev.type_ == ffi::KeyRelease => {
                            events.push(Released(elem));
                        },
                        _ => ()
                    }
                    //
                },

                ffi::ButtonPress | ffi::ButtonRelease => {
                    use {Pressed, Released};
                    let event: &ffi::XButtonEvent = unsafe { mem::transmute(&xev) };
                    //events.push(CursorPositionChanged(event.x as uint, event.y as uint));
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
            unsafe { ffi::XPeekEvent(self.display, &mut xev) };

            // calling poll_events()
            let ev = self.poll_events();
            if ev.len() >= 1 {
                return ev;
            }
        }
    }

    pub unsafe fn make_current(&self) {
        let res = ffi::glXMakeCurrent(self.display, self.window, self.context);
        if res == 0 {
            fail!("glXMakeCurrent failed");
        }
    }

    pub fn get_proc_address(&self, addr: &str) -> *const () {
        use std::c_str::ToCStr;
        use std::mem;

        unsafe {
            addr.with_c_str(|s| {
                let p = ffi::glXGetProcAddress(mem::transmute(s)) as *const ();
                if !p.is_null() { return p; }
                println!("{}", p);
                p
            })
        }
    }

    pub fn swap_buffers(&self) {
        unsafe { ffi::glXSwapBuffers(self.display, self.window) }
    }
}

impl Drop for Window {
    fn drop(&mut self) {
        unsafe { ffi::glXDestroyContext(self.display, self.context) }
        unsafe { ffi::XDestroyWindow(self.display, self.window) }
        unsafe { ffi::XCloseDisplay(self.display) }
    }
}
