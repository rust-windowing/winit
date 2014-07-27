use {Event, Hints, MonitorID};
use libc;
use std::{mem, ptr};
use std::sync::atomics::AtomicBool;

mod ffi;

pub struct Window {
    display: *mut ffi::Display,
    window: ffi::Window,
    context: ffi::GLXContext,
    should_close: AtomicBool,
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
                ffi::VisibilityChangeMask | ffi::KeyPressMask;
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
            should_close: AtomicBool::new(false),
            wm_delete_window: wm_delete_window,
        })
    }

    pub fn should_close(&self) -> bool {
        use std::sync::atomics::Relaxed;
        self.should_close.load(Relaxed)
    }

    pub fn set_title(&self, title: &str) {
        unsafe {
            ffi::XStoreName(self.display, self.window,
                mem::transmute(title.as_slice().as_ptr()));
        }
    }

    pub fn get_position(&self) -> (uint, uint) {
        unimplemented!()
    }

    pub fn set_position(&self, x: uint, y: uint) {
        unimplemented!()
    }

    pub fn get_inner_size(&self) -> (uint, uint) {
        unimplemented!()
    }

    pub fn get_outer_size(&self) -> (uint, uint) {
        unimplemented!()
    }

    pub fn set_inner_size(&self, x: uint, y: uint) {
        unimplemented!()
    }

    pub fn poll_events(&self) -> Vec<Event> {
        unimplemented!()
    }

    pub fn wait_events(&self) -> Vec<Event> {
        use std::mem;

        let mut xev = unsafe { mem::uninitialized() };
        unsafe { ffi::XNextEvent(self.display, &mut xev) };

        let mut events = Vec::new();
        
        match xev.type_ {
            ffi::ClientMessage => {
                use Closed;
                use std::sync::atomics::Relaxed;

                let client_msg: &ffi::XClientMessageEvent = unsafe { mem::transmute(&xev) };

                if client_msg.l[0] == self.wm_delete_window as libc::c_long {
                    self.should_close.store(true, Relaxed);
                    events.push(Closed);
                }
            },

            _ => ()
        }

        events
    }

    pub fn make_current(&self) {
        let res = unsafe { ffi::glXMakeCurrent(self.display, self.window, self.context) };
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
