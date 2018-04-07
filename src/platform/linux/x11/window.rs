use MouseCursor;
use CreationError;
use CreationError::OsError;
use libc;
use std::borrow::Borrow;
use std::{mem, cmp, ptr};
use std::sync::{Arc, Mutex};
use std::os::raw::{c_int, c_long, c_uchar, c_uint, c_ulong, c_void};
use std::thread;
use std::time::Duration;

use CursorState;
use WindowAttributes;
use platform::PlatformSpecificWindowBuilderAttributes;

use platform::MonitorId as PlatformMonitorId;
use platform::x11::MonitorId as X11MonitorId;
use window::MonitorId as RootMonitorId;

use platform::x11::monitor::get_available_monitors;

use super::{ffi, util, XConnection, XError, WindowId, EventsLoop};

// TODO: remove me
fn with_c_str<F, T>(s: &str, f: F) -> T where F: FnOnce(*const libc::c_char) -> T {
    use std::ffi::CString;
    let c_str = CString::new(s.as_bytes().to_vec()).unwrap();
    f(c_str.as_ptr())
}

#[derive(Debug)]
enum StateOperation {
    Remove = 0, // _NET_WM_STATE_REMOVE
    Add = 1, // _NET_WM_STATE_ADD
    #[allow(dead_code)]
    Toggle = 2, // _NET_WM_STATE_TOGGLE
}

impl From<bool> for StateOperation {
    fn from(b: bool) -> Self {
        if b {
            StateOperation::Add
        } else {
            StateOperation::Remove
        }
    }
}

pub struct XWindow {
    display: Arc<XConnection>,
    window: ffi::Window,
    root: ffi::Window,
    screen_id: i32,
}

impl XWindow {
    /// Get parent window of `child`
    ///
    /// This method can return None if underlying xlib call fails.
    ///
    /// # Unsafety
    ///
    /// `child` must be a valid `Window`.
    unsafe fn get_parent_window(&self, child: ffi::Window) -> Option<ffi::Window> {
        let mut root: ffi::Window = mem::uninitialized();
        let mut parent: ffi::Window = mem::uninitialized();
        let mut children: *mut ffi::Window = ptr::null_mut();
        let mut nchildren: libc::c_uint = mem::uninitialized();

        let res = (self.display.xlib.XQueryTree)(
            self.display.display,
            child,
            &mut root,
            &mut parent,
            &mut children,
            &mut nchildren
        );

        if res == 0 {
            return None;
        }

        // The list of children isn't used
        if children != ptr::null_mut() {
            (self.display.xlib.XFree)(children as *mut _);
        }

        Some(parent)
    }
}

unsafe impl Send for XWindow {}
unsafe impl Sync for XWindow {}

unsafe impl Send for Window2 {}
unsafe impl Sync for Window2 {}

pub struct Window2 {
    pub x: Arc<XWindow>,
    cursor: Mutex<MouseCursor>,
    cursor_state: Mutex<CursorState>,
    supported_hints: Vec<ffi::Atom>,
    wm_name: Option<String>,
}

fn get_supported_hints(xwin: &Arc<XWindow>) -> Vec<ffi::Atom> {
    let supported_atom = unsafe { util::get_atom(&xwin.display, b"_NET_SUPPORTED\0") }
        .expect("Failed to call XInternAtom (_NET_SUPPORTED)");
    unsafe {
        util::get_property(
            &xwin.display,
            xwin.root,
            supported_atom,
            ffi::XA_ATOM,
        )
    }.unwrap_or_else(|_| Vec::with_capacity(0))
}

fn get_wm_name(xwin: &Arc<XWindow>, _supported_hints: &[ffi::Atom]) -> Option<String> {
    let check_atom = unsafe { util::get_atom(&xwin.display, b"_NET_SUPPORTING_WM_CHECK\0") }
        .expect("Failed to call XInternAtom (_NET_SUPPORTING_WM_CHECK)");
    let wm_name_atom = unsafe { util::get_atom(&xwin.display, b"_NET_WM_NAME\0") }
        .expect("Failed to call XInternAtom (_NET_WM_NAME)");

    // Mutter/Muffin/Budgie doesn't have _NET_SUPPORTING_WM_CHECK in its _NET_SUPPORTED, despite
    // it working and being supported. This has been reported upstream, but due to the
    // inavailability of time machines, we'll just try to get _NET_SUPPORTING_WM_CHECK
    // regardless of whether or not the WM claims to support it.
    //
    // Blackbox 0.70 also incorrectly reports not supporting this, though that appears to be fixed
    // in 0.72.
    /*if !supported_hints.contains(&check_atom) {
        return None;
    }*/

    // IceWM (1.3.x and earlier) doesn't report supporting _NET_WM_NAME, but will nonetheless
    // provide us with a value for it. Note that the unofficial 1.4 fork of IceWM works fine.
    /*if !supported_hints.contains(&wm_name_atom) {
        return None;
    }*/

    // Of the WMs tested, only xmonad and dwm fail to provide a WM name.

    // Querying this property on the root window will give us the ID of a child window created by
    // the WM.
    let root_window_wm_check = {
        let result = unsafe {
            util::get_property(
                &xwin.display,
                xwin.root,
                check_atom,
                ffi::XA_WINDOW,
            )
        };

        let wm_check = result
            .ok()
            .and_then(|wm_check| wm_check.get(0).cloned());

        if let Some(wm_check) = wm_check {
            wm_check
        } else {
            return None;
        }
    };

    // Querying the same property on the child window we were given, we should get this child
    // window's ID again.
    let child_window_wm_check = {
        let result = unsafe {
            util::get_property(
                &xwin.display,
                root_window_wm_check,
                check_atom,
                ffi::XA_WINDOW,
            )
        };

        let wm_check = result
            .ok()
            .and_then(|wm_check| wm_check.get(0).cloned());

        if let Some(wm_check) = wm_check {
            wm_check
        } else {
            return None;
        }
    };

    // These values should be the same.
    if root_window_wm_check != child_window_wm_check {
        return None;
    }

    // All of that work gives us a window ID that we can get the WM name from.
    let wm_name = {
        let utf8_string_atom = unsafe { util::get_atom(&xwin.display, b"UTF8_STRING\0") }
            .unwrap_or(ffi::XA_STRING);

        let result = unsafe {
            util::get_property(
                &xwin.display,
                root_window_wm_check,
                wm_name_atom,
                utf8_string_atom,
            )
        };

        // IceWM requires this. IceWM was also the only WM tested that returns a null-terminated
        // string. For more fun trivia, IceWM is also unique in including version and uname
        // information in this string (this means you'll have to be careful if you want to match
        // against it, though).
        // The unofficial 1.4 fork of IceWM still includes the extra details, but properly
        // returns a UTF8 string that isn't null-terminated.
        let no_utf8 = if let Err(ref err) = result {
            err.is_actual_property_type(ffi::XA_STRING)
        } else {
            false
        };

        if no_utf8 {
            unsafe {
                util::get_property(
                    &xwin.display,
                    root_window_wm_check,
                    wm_name_atom,
                    ffi::XA_STRING,
                )
            }
        } else {
            result
        }
    }.ok();

    wm_name.and_then(|wm_name| String::from_utf8(wm_name).ok())
}

impl Window2 {
    pub fn new(ctx: &EventsLoop, window_attrs: &WindowAttributes,
               pl_attribs: &PlatformSpecificWindowBuilderAttributes)
               -> Result<Window2, CreationError>
    {
        let display = &ctx.display;
        let dimensions = {

            // x11 only applies constraints when the window is actively resized
            // by the user, so we have to manually apply the initial constraints
            let mut dimensions = window_attrs.dimensions.unwrap_or((800, 600));
            if let Some(max) = window_attrs.max_dimensions {
                dimensions.0 = cmp::min(dimensions.0, max.0);
                dimensions.1 = cmp::min(dimensions.1, max.1);
            }

            if let Some(min) = window_attrs.min_dimensions {
                dimensions.0 = cmp::max(dimensions.0, min.0);
                dimensions.1 = cmp::max(dimensions.1, min.1);
            }
            dimensions

        };

        let screen_id = match pl_attribs.screen_id {
            Some(id) => id,
            None => unsafe { (display.xlib.XDefaultScreen)(display.display) },
        };

        // getting the root window
        let root = ctx.root;

        // creating
        let mut set_win_attr = {
            let mut swa: ffi::XSetWindowAttributes = unsafe { mem::zeroed() };
            swa.colormap = if let Some(vi) = pl_attribs.visual_infos {
                unsafe {
                    let visual = vi.visual;
                    (display.xlib.XCreateColormap)(display.display, root, visual, ffi::AllocNone)
                }
            } else { 0 };
            swa.event_mask = ffi::ExposureMask | ffi::StructureNotifyMask |
                ffi::VisibilityChangeMask | ffi::KeyPressMask | ffi::PointerMotionMask |
                ffi::KeyReleaseMask | ffi::ButtonPressMask |
                ffi::ButtonReleaseMask | ffi::KeymapStateMask;
            swa.border_pixel = 0;
            if window_attrs.transparent {
                swa.background_pixel = 0;
            }
            swa.override_redirect = 0;
            swa
        };

        let mut window_attributes = ffi::CWBorderPixel | ffi::CWColormap | ffi::CWEventMask;

        if window_attrs.transparent {
            window_attributes |= ffi::CWBackPixel;
        }

        // finally creating the window
        let window = unsafe {
            let win = (display.xlib.XCreateWindow)(display.display, root, 0, 0, dimensions.0 as libc::c_uint,
                dimensions.1 as libc::c_uint, 0,
                match pl_attribs.visual_infos {
                    Some(vi) => vi.depth,
                    None => ffi::CopyFromParent
                },
                ffi::InputOutput as libc::c_uint,
                match pl_attribs.visual_infos {
                    Some(vi) => vi.visual,
                    None => ffi::CopyFromParent as *mut _
                },
                window_attributes,
                &mut set_win_attr);
            display.check_errors().expect("Failed to call XCreateWindow");
            win
        };

        let x_window = Arc::new(XWindow {
            display: display.clone(),
            window,
            root,
            screen_id,
        });

        // These values will cease to be correct if the user replaces the WM during the life of
        // the window, so hopefully they don't do that.
        let supported_hints = get_supported_hints(&x_window);
        let wm_name = get_wm_name(&x_window, &supported_hints);

        let window = Window2 {
            x: x_window,
            cursor: Mutex::new(MouseCursor::Default),
            cursor_state: Mutex::new(CursorState::Normal),
            supported_hints,
            wm_name,
        };

        // Title must be set before mapping, lest some tiling window managers briefly pick up on
        // the initial un-titled window state
        window.set_title(&window_attrs.title);
        window.set_decorations(window_attrs.decorations);

        {
            let ref x_window: &XWindow = window.x.borrow();

            // Enable drag and drop
            unsafe {
                let atom = util::get_atom(display, b"XdndAware\0")
                    .expect("Failed to call XInternAtom (XdndAware)");
                let version = &5; // Latest version; hasn't changed since 2002
                (display.xlib.XChangeProperty)(
                    display.display,
                    x_window.window,
                    atom,
                    ffi::XA_ATOM,
                    32,
                    ffi::PropModeReplace,
                    version,
                    1
                );
                display.check_errors().expect("Failed to set drag and drop properties");
            }

            // Set ICCCM WM_CLASS property based on initial window title
            // Must be done *before* mapping the window by ICCCM 4.1.2.5
            unsafe {
                with_c_str(&*window_attrs.title, |c_name| {
                    let hint = (display.xlib.XAllocClassHint)();
                    (*hint).res_name = c_name as *mut libc::c_char;
                    (*hint).res_class = c_name as *mut libc::c_char;
                    (display.xlib.XSetClassHint)(display.display, x_window.window, hint);
                    display.check_errors().expect("Failed to call XSetClassHint");
                    (display.xlib.XFree)(hint as *mut _);
                });
            }

            // set size hints
            {
                let mut size_hints = {
                    let size_hints = unsafe { (display.xlib.XAllocSizeHints)() };
                    util::XSmartPointer::new(&display, size_hints)
                        .expect("XAllocSizeHints returned null; out of memory")
                };
                (*size_hints).flags = ffi::PSize;
                (*size_hints).width = dimensions.0 as c_int;
                (*size_hints).height = dimensions.1 as c_int;
                if let Some(dimensions) = window_attrs.min_dimensions {
                    (*size_hints).flags |= ffi::PMinSize;
                    (*size_hints).min_width = dimensions.0 as c_int;
                    (*size_hints).min_height = dimensions.1 as c_int;
                }
                if let Some(dimensions) = window_attrs.max_dimensions {
                    (*size_hints).flags |= ffi::PMaxSize;
                    (*size_hints).max_width = dimensions.0 as c_int;
                    (*size_hints).max_height = dimensions.1 as c_int;
                }
                unsafe {
                    (display.xlib.XSetWMNormalHints)(
                        display.display,
                        x_window.window,
                        size_hints.ptr,
                    );
                }
                display.check_errors().expect("Failed to call XSetWMNormalHints");
            }

            // Opt into handling window close
            unsafe {
                (display.xlib.XSetWMProtocols)(display.display, x_window.window, &ctx.wm_delete_window as *const _ as *mut _, 1);
                display.check_errors().expect("Failed to call XSetWMProtocols");
                (display.xlib.XFlush)(display.display);
                display.check_errors().expect("Failed to call XFlush");
            }

            // Set visibility (map window)
            if window_attrs.visible {
                unsafe {
                    (display.xlib.XMapRaised)(display.display, x_window.window);
                    (display.xlib.XFlush)(display.display);
                }

                display.check_errors().expect("Failed to set window visibility");
            }

            // Attempt to make keyboard input repeat detectable
            unsafe {
                let mut supported_ptr = ffi::False;
                (display.xlib.XkbSetDetectableAutoRepeat)(display.display, ffi::True, &mut supported_ptr);
                if supported_ptr == ffi::False {
                    return Err(OsError(format!("XkbSetDetectableAutoRepeat failed")));
                }
            }

            // Select XInput2 events
            {
                let mask = ffi::XI_MotionMask
                    | ffi::XI_ButtonPressMask | ffi::XI_ButtonReleaseMask
                    // | ffi::XI_KeyPressMask | ffi::XI_KeyReleaseMask
                    | ffi::XI_EnterMask | ffi::XI_LeaveMask
                    | ffi::XI_FocusInMask | ffi::XI_FocusOutMask
                    | if window_attrs.multitouch { ffi::XI_TouchBeginMask | ffi::XI_TouchUpdateMask | ffi::XI_TouchEndMask } else { 0 };
                unsafe {
                    let mut event_mask = ffi::XIEventMask{
                        deviceid: ffi::XIAllMasterDevices,
                        mask: mem::transmute::<*const i32, *mut c_uchar>(&mask as *const i32),
                        mask_len: mem::size_of_val(&mask) as c_int,
                    };
                    (display.xinput2.XISelectEvents)(display.display, x_window.window,
                                                     &mut event_mask as *mut ffi::XIEventMask, 1);
                };
            }

            // These properties must be set after mapping
            window.set_maximized(window_attrs.maximized);
            window.set_fullscreen(window_attrs.fullscreen.clone());

            if window_attrs.visible {
                unsafe {
                    // XSetInputFocus generates an error if the window is not visible,
                    // therefore we wait until it's the case.
                    loop {
                        let mut window_attributes = mem::uninitialized();
                        (display.xlib.XGetWindowAttributes)(display.display, x_window.window, &mut window_attributes);
                        display.check_errors().expect("Failed to call XGetWindowAttributes");

                        if window_attributes.map_state == ffi::IsViewable {
                            (display.xlib.XSetInputFocus)(
                                display.display,
                                x_window.window,
                                ffi::RevertToParent,
                                ffi::CurrentTime
                            );
                            display.check_errors().expect("Failed to call XSetInputFocus");
                            break;
                        }

                        // Wait about a frame to avoid too-busy waiting
                        thread::sleep(Duration::from_millis(16));
                    }
                }
            }
        }

        // returning
        Ok(window)
    }

    fn set_netwm(
        xconn: &Arc<XConnection>,
        window: ffi::Window,
        root: ffi::Window,
        properties: (c_long, c_long, c_long, c_long),
        operation: StateOperation
    ) {
        let state_atom = unsafe { util::get_atom(xconn, b"_NET_WM_STATE\0") }
            .expect("Failed to call XInternAtom (_NET_WM_STATE)");

        unsafe {
            util::send_client_msg(
                xconn,
                window,
                root,
                state_atom,
                Some(ffi::SubstructureRedirectMask | ffi::SubstructureNotifyMask),
                (
                    operation as c_long,
                    properties.0,
                    properties.1,
                    properties.2,
                    properties.3,
                )
            )
        }.expect("Failed to send NET_WM hint.");
    }

    pub fn set_fullscreen(&self, monitor: Option<RootMonitorId>) {
        match monitor {
            None => {
                self.set_fullscreen_hint(false);
            },
            Some(RootMonitorId { inner: PlatformMonitorId::X(monitor) }) => {
                let screenpos = monitor.get_position();
                self.set_position(screenpos.0 as i32, screenpos.1 as i32);
                self.set_fullscreen_hint(true);
            }
            _ => {
                eprintln!("[winit] Something's broken, got an unknown fullscreen state in X11");
            }
        }
    }

    pub fn get_current_monitor(&self) -> X11MonitorId {
        let monitors = get_available_monitors(&self.x.display);
        let default = monitors[0].clone();

        let (wx,wy) = match self.get_position() {
            Some(val) => (cmp::max(0,val.0) as u32, cmp::max(0,val.1) as u32),
            None=> return default,
        };
        let (ww,wh) = match self.get_outer_size() {
            Some(val) => val,
            None=> return default,
        };
        // Opposite corner coordinates
        let (wxo, wyo) = (wx+ww-1, wy+wh-1);

        // Find the monitor with the biggest overlap with the window
        let mut overlap = 0;
        let mut find = default;
        for monitor in monitors {
            let (mx, my) = monitor.get_position();
            let mx = mx as u32;
            let my = my as u32;
            let (mw, mh) = monitor.get_dimensions();
            let (mxo, myo) = (mx+mw-1, my+mh-1);
            let (ox, oy) = (cmp::max(wx, mx), cmp::max(wy, my));
            let (oxo, oyo) = (cmp::min(wxo, mxo), cmp::min(wyo, myo));
            let osize = if ox <= oxo || oy <= oyo { 0 } else { (oxo-ox)*(oyo-oy) };

            if osize > overlap {
                overlap = osize;
                find = monitor;
            }
        }

        find
    }

    pub fn set_maximized(&self, maximized: bool) {
        let xconn = &self.x.display;

        let horz_atom = unsafe { util::get_atom(xconn, b"_NET_WM_STATE_MAXIMIZED_HORZ\0") }
            .expect("Failed to call XInternAtom (_NET_WM_STATE_MAXIMIZED_HORZ)");
        let vert_atom = unsafe { util::get_atom(xconn, b"_NET_WM_STATE_MAXIMIZED_VERT\0") }
            .expect("Failed to call XInternAtom (_NET_WM_STATE_MAXIMIZED_VERT)");

        Window2::set_netwm(
            xconn,
            self.x.window,
            self.x.root,
            (horz_atom as c_long, vert_atom as c_long, 0, 0),
            maximized.into()
        );
    }

    fn set_fullscreen_hint(&self, fullscreen: bool) {
        let xconn = &self.x.display;

        let fullscreen_atom = unsafe { util::get_atom(xconn, b"_NET_WM_STATE_FULLSCREEN\0") }
            .expect("Failed to call XInternAtom (_NET_WM_STATE_FULLSCREEN)");

        Window2::set_netwm(
            xconn,
            self.x.window,
            self.x.root,
            (fullscreen_atom as c_long, 0, 0, 0),
            fullscreen.into()
        );
    }

    pub fn set_title(&self, title: &str) {
        let wm_name = unsafe {
            (self.x.display.xlib.XInternAtom)(self.x.display.display, b"_NET_WM_NAME\0".as_ptr() as *const _, 0)
        };
        self.x.display.check_errors().expect("Failed to call XInternAtom");

        let wm_utf8_string = unsafe {
            (self.x.display.xlib.XInternAtom)(self.x.display.display, b"UTF8_STRING\0".as_ptr() as *const _, 0)
        };
        self.x.display.check_errors().expect("Failed to call XInternAtom");

        with_c_str(title, |c_title| unsafe {
            (self.x.display.xlib.XStoreName)(self.x.display.display, self.x.window, c_title);

            let len = title.as_bytes().len();
            (self.x.display.xlib.XChangeProperty)(self.x.display.display, self.x.window,
                                            wm_name, wm_utf8_string, 8, ffi::PropModeReplace,
                                            c_title as *const u8, len as libc::c_int);
            (self.x.display.xlib.XFlush)(self.x.display.display);
        });
        self.x.display.check_errors().expect("Failed to set window title");

    }

    pub fn set_decorations(&self, decorations: bool) {
        #[repr(C)]
        struct MotifWindowHints {
            flags: c_ulong,
            functions: c_ulong,
            decorations: c_ulong,
            input_mode: c_long,
            status: c_ulong,
        }

        let wm_hints = unsafe { util::get_atom(&self.x.display, b"_MOTIF_WM_HINTS\0") }
            .expect("Failed to call XInternAtom (_MOTIF_WM_HINTS)");

        let hints = MotifWindowHints {
            flags: 2, // MWM_HINTS_DECORATIONS
            functions: 0,
            decorations: decorations as _,
            input_mode: 0,
            status: 0,
        };

        unsafe {
            (self.x.display.xlib.XChangeProperty)(
                self.x.display.display,
                self.x.window,
                wm_hints,
                wm_hints,
                32, // struct members are longs
                ffi::PropModeReplace,
                &hints as *const _ as *const u8,
                5 // struct has 5 members
            );
            (self.x.display.xlib.XFlush)(self.x.display.display);
        }

        self.x.display.check_errors().expect("Failed to set decorations");
    }

    pub fn show(&self) {
        unsafe {
            (self.x.display.xlib.XMapRaised)(self.x.display.display, self.x.window);
            (self.x.display.xlib.XFlush)(self.x.display.display);
            self.x.display.check_errors().expect("Failed to call XMapRaised");
        }
    }

    pub fn hide(&self) {
        unsafe {
            (self.x.display.xlib.XUnmapWindow)(self.x.display.display, self.x.window);
            (self.x.display.xlib.XFlush)(self.x.display.display);
            self.x.display.check_errors().expect("Failed to call XUnmapWindow");
        }
    }

    fn get_frame_extents(&self) -> Option<util::FrameExtents> {
        let extents_atom = unsafe { util::get_atom(&self.x.display, b"_NET_FRAME_EXTENTS\0") }
            .expect("Failed to call XInternAtom (_NET_FRAME_EXTENTS)");

        if !self.supported_hints.contains(&extents_atom) {
            return None;
        }

        // Of the WMs tested, xmonad, i3, dwm, IceWM (1.3.x and earlier), and blackbox don't
        // support this. As this is part of EWMH (Extended Window Manager Hints), it's likely to
        // be unsupported by many smaller WMs.
        let extents: Option<Vec<c_ulong>> = unsafe {
            util::get_property(
                &self.x.display,
                self.x.window,
                extents_atom,
                ffi::XA_CARDINAL,
            )
        }.ok();

        extents.and_then(|extents| {
            if extents.len() >= 4 {
                Some(util::FrameExtents {
                    left: extents[0],
                    right: extents[1],
                    top: extents[2],
                    bottom: extents[3],
                })
            } else {
                None
            }
        })
    }

    fn is_top_level(&self, id: ffi::Window) -> Option<bool> {
        let client_list_atom = unsafe { util::get_atom(&self.x.display, b"_NET_CLIENT_LIST\0") }
            .expect("Failed to call XInternAtom (_NET_CLIENT_LIST)");

        if !self.supported_hints.contains(&client_list_atom) {
            return None;
        }

        let client_list: Option<Vec<ffi::Window>> = unsafe {
            util::get_property(
                &self.x.display,
                self.x.root,
                client_list_atom,
                ffi::XA_WINDOW,
            )
        }.ok();

        client_list.map(|client_list| {
            client_list.contains(&id)
        })
    }

    fn get_geometry(&self) -> Option<util::WindowGeometry> {
        // Position relative to root window.
        // With rare exceptions, this is the position of a nested window. Cases where the window
        // isn't nested are outlined in the comments throghout this function, but in addition to
        // that, fullscreen windows sometimes aren't nested.
        let (inner_x_rel_root, inner_y_rel_root, child) = unsafe {
            let mut inner_x_rel_root: c_int = mem::uninitialized();
            let mut inner_y_rel_root: c_int = mem::uninitialized();
            let mut child: ffi::Window = mem::uninitialized();

            (self.x.display.xlib.XTranslateCoordinates)(
                self.x.display.display,
                self.x.window,
                self.x.root,
                0,
                0,
                &mut inner_x_rel_root,
                &mut inner_y_rel_root,
                &mut child,
            );

            (inner_x_rel_root, inner_y_rel_root, child)
        };

        let (inner_x, inner_y, width, height, border) = unsafe {
            let mut root: ffi::Window = mem::uninitialized();
            // The same caveat outlined in the comment above for XTranslateCoordinates applies
            // here as well. The only difference is that this position is relative to the parent
            // window, rather than the root window.
            let mut inner_x: c_int = mem::uninitialized();
            let mut inner_y: c_int = mem::uninitialized();
            // The width and height here are for the client area.
            let mut width: c_uint = mem::uninitialized();
            let mut height: c_uint = mem::uninitialized();
            // xmonad and dwm were the only WMs tested that use the border return at all.
            // The majority of WMs seem to simply fill it with 0 unconditionally.
            let mut border: c_uint = mem::uninitialized();
            let mut depth: c_uint = mem::uninitialized();

            let status = (self.x.display.xlib.XGetGeometry)(
                self.x.display.display,
                self.x.window,
                &mut root,
                &mut inner_x,
                &mut inner_y,
                &mut width,
                &mut height,
                &mut border,
                &mut depth,
            );

            if status == 0 {
                return None;
            }

            (inner_x, inner_y, width, height, border)
        };

        // The first condition is only false for un-nested windows, but isn't always false for
        // un-nested windows. Mutter/Muffin/Budgie and Marco present a mysterious discrepancy:
        // when y is on the range [0, 2] and if the window has been unfocused since being
        // undecorated (or was undecorated upon construction), the first condition is true,
        // requiring us to rely on the second condition.
        let nested = !(self.x.window == child || self.is_top_level(child) == Some(true));

        // Hopefully the WM supports EWMH, allowing us to get exact info on the window frames.
        if let Some(mut extents) = self.get_frame_extents() {
            // Mutter/Muffin/Budgie and Marco preserve their decorated frame extents when
            // decorations are disabled, but since the window becomes un-nested, it's easy to
            // catch.
            if !nested {
                extents = util::FrameExtents::new(0, 0, 0, 0);
            }

            // The difference between the nested window's position and the outermost window's
            // position is equivalent to the frame size. In most scenarios, this is equivalent to
            // manually climbing the hierarchy as is done in the case below. Here's a list of
            // known discrepancies:
            // * Mutter/Muffin/Budgie gives decorated windows a margin of 9px (only 7px on top) in
            //   addition to a 1px semi-transparent border. The margin can be easily observed by
            //   using a screenshot tool to get a screenshot of a selected window, and is
            //   presumably used for drawing drop shadows. Getting window geometry information
            //   via hierarchy-climbing results in this margin being included in both the
            //   position and outer size, so a window positioned at (0, 0) would be reported as
            //   having a position (-10, -8).
            // * Compiz has a drop shadow margin just like Mutter/Muffin/Budgie, though it's 10px
            //   on all sides, and there's no additional border.
            // * Enlightenment otherwise gets a y position equivalent to inner_y_rel_root.
            //   Without decorations, there's no difference. This is presumably related to
            //   Enlightenment's fairly unique concept of window position; it interprets
            //   positions given to XMoveWindow as a client area position rather than a position
            //   of the overall window.
            let abs_x = inner_x_rel_root - extents.left as c_int;
            let abs_y = inner_y_rel_root - extents.top as c_int;

            Some(util::WindowGeometry {
                x: abs_x,
                y: abs_y,
                width,
                height,
                frame: extents,
            })
        } else if nested {
            // If the position value we have is for a nested window used as the client area, we'll
            // just climb up the hierarchy and get the geometry of the outermost window we're
            // nested in.
            let window = {
                let root = self.x.root;
                let mut window = self.x.window;
                loop {
                    let candidate = unsafe {
                        self.x.get_parent_window(window).unwrap()
                    };
                    if candidate == root {
                        break window;
                    }
                    window = candidate;
                }
            };

            let (outer_x, outer_y, outer_width, outer_height) = unsafe {
                let mut root: ffi::Window = mem::uninitialized();
                let mut outer_x: c_int = mem::uninitialized();
                let mut outer_y: c_int = mem::uninitialized();
                let mut outer_width: c_uint = mem::uninitialized();
                let mut outer_height: c_uint = mem::uninitialized();
                let mut border: c_uint = mem::uninitialized();
                let mut depth: c_uint = mem::uninitialized();

                let status = (self.x.display.xlib.XGetGeometry)(
                    self.x.display.display,
                    window,
                    &mut root,
                    &mut outer_x,
                    &mut outer_y,
                    &mut outer_width,
                    &mut outer_height,
                    &mut border,
                    &mut depth,
                );

                if status == 0 {
                    return None;
                }

                (outer_x, outer_y, outer_width, outer_height)
            };

            // Since we have the geometry of the outermost window and the geometry of the client
            // area, we can figure out what's in between.
            let frame = {
                let diff_x = outer_width.saturating_sub(width);
                let diff_y = outer_height.saturating_sub(height);
                let offset_y = inner_y_rel_root.saturating_sub(outer_y) as c_uint;

                let left = diff_x / 2;
                let right = left;
                let top = offset_y;
                let bottom = diff_y.saturating_sub(offset_y);

                util::FrameExtents::new(left.into(), right.into(), top.into(), bottom.into())
            };

            Some(util::WindowGeometry {
                x: outer_x,
                y: outer_y,
                width,
                height,
                frame,
            })
        } else {
            // This is the case for xmonad and dwm, AKA the only WMs tested that supplied a
            // border value. This is convenient, since we can use it to get an accurate frame.
            let frame = util::FrameExtents::from_border(border.into());
            Some(util::WindowGeometry {
                x: inner_x,
                y: inner_y,
                width,
                height,
                frame,
            })
        }
    }

    #[inline]
    pub fn get_position(&self) -> Option<(i32, i32)> {
        self.get_geometry().map(|geo| geo.get_position())
    }

    pub fn set_position(&self, mut x: i32, mut y: i32) {
        if let Some(ref wm_name) = self.wm_name {
            // There are a few WMs that set client area position rather than window position, so
            // we'll translate for consistency.
            if ["Enlightenment", "FVWM"].contains(&wm_name.as_str()) {
                if let Some(extents) = self.get_frame_extents() {
                    x += extents.left as i32;
                    y += extents.top as i32;
                }
            }
        }
        unsafe {
            (self.x.display.xlib.XMoveWindow)(
                self.x.display.display,
                self.x.window,
                x as c_int,
                y as c_int,
            );
        }
        self.x.display.check_errors().expect("Failed to call XMoveWindow");
    }

    #[inline]
    pub fn get_inner_size(&self) -> Option<(u32, u32)> {
        self.get_geometry().map(|geo| geo.get_inner_size())
    }

    #[inline]
    pub fn get_outer_size(&self) -> Option<(u32, u32)> {
        self.get_geometry().map(|geo| geo.get_outer_size())
    }

    #[inline]
    pub fn set_inner_size(&self, x: u32, y: u32) {
        unsafe { (self.x.display.xlib.XResizeWindow)(self.x.display.display, self.x.window, x as libc::c_uint, y as libc::c_uint); }
        self.x.display.check_errors().expect("Failed to call XResizeWindow");
    }

    unsafe fn update_normal_hints<F>(&self, callback: F) -> Result<(), XError>
        where F: FnOnce(*mut ffi::XSizeHints) -> ()
    {
        let xconn = &self.x.display;

        let size_hints = {
            let size_hints = (xconn.xlib.XAllocSizeHints)();
            util::XSmartPointer::new(&xconn, size_hints)
                .expect("XAllocSizeHints returned null; out of memory")
        };

        let mut flags: c_long = mem::uninitialized();

        (xconn.xlib.XGetWMNormalHints)(
            xconn.display,
            self.x.window,
            size_hints.ptr,
            &mut flags,
        );
        xconn.check_errors()?;

        callback(size_hints.ptr);

        (xconn.xlib.XSetWMNormalHints)(
            xconn.display,
            self.x.window,
            size_hints.ptr,
        );
        xconn.check_errors()?;

        Ok(())
    }

    pub fn set_min_dimensions(&self, dimensions: Option<(u32, u32)>) {
        unsafe {
            self.update_normal_hints(|size_hints| {
                if let Some((width, height)) = dimensions {
                    (*size_hints).flags |= ffi::PMinSize;
                    (*size_hints).min_width = width as c_int;
                    (*size_hints).min_height = height as c_int;
                } else {
                    (*size_hints).flags &= !ffi::PMinSize;
                }
            })
        }.expect("Failed to call XSetWMNormalHints");
    }

    pub fn set_max_dimensions(&self, dimensions: Option<(u32, u32)>) {
        unsafe {
            self.update_normal_hints(|size_hints| {
                if let Some((width, height)) = dimensions {
                    (*size_hints).flags |= ffi::PMaxSize;
                    (*size_hints).max_width = width as c_int;
                    (*size_hints).max_height = height as c_int;
                } else {
                    (*size_hints).flags &= !ffi::PMaxSize;
                }
            })
        }.expect("Failed to call XSetWMNormalHints");
    }

    #[inline]
    pub fn get_xlib_display(&self) -> *mut c_void {
        self.x.display.display as _
    }

    #[inline]
    pub fn get_xlib_screen_id(&self) -> c_int {
        self.x.screen_id
    }

    #[inline]
    pub fn get_xlib_xconnection(&self) -> Arc<XConnection> {
        self.x.display.clone()
    }

    #[inline]
    pub fn platform_display(&self) -> *mut libc::c_void {
        self.x.display.display as _
    }

    #[inline]
    pub fn get_xlib_window(&self) -> c_ulong {
        self.x.window
    }

    #[inline]
    pub fn platform_window(&self) -> *mut libc::c_void {
        self.x.window as _
    }

    pub fn get_xcb_connection(&self) -> *mut c_void {
        unsafe {
            (self.x.display.xlib_xcb.XGetXCBConnection)(self.get_xlib_display() as *mut _) as *mut _
        }
    }

    fn load_cursor(&self, name: &str) -> ffi::Cursor {
        use std::ffi::CString;
        unsafe {
            let c_string = CString::new(name.as_bytes()).unwrap();
            (self.x.display.xcursor.XcursorLibraryLoadCursor)(self.x.display.display, c_string.as_ptr())
        }
    }

    fn load_first_existing_cursor(&self, names :&[&str]) -> ffi::Cursor {
        for name in names.iter() {
            let xcursor = self.load_cursor(name);
            if xcursor != 0 {
                return xcursor;
            }
        }
        0
    }

    fn get_cursor(&self, cursor: MouseCursor) -> ffi::Cursor {
        let load = |name: &str| {
            self.load_cursor(name)
        };

        let loadn = |names: &[&str]| {
            self.load_first_existing_cursor(names)
        };

        // Try multiple names in some cases where the name
        // differs on the desktop environments or themes.
        //
        // Try the better looking (or more suiting) names first.
        match cursor {
            MouseCursor::Alias => load("link"),
            MouseCursor::Arrow => load("arrow"),
            MouseCursor::Cell => load("plus"),
            MouseCursor::Copy => load("copy"),
            MouseCursor::Crosshair => load("crosshair"),
            MouseCursor::Default => load("left_ptr"),
            MouseCursor::Hand => loadn(&["hand2", "hand1"]),
            MouseCursor::Help => load("question_arrow"),
            MouseCursor::Move => load("move"),
            MouseCursor::Grab => loadn(&["openhand", "grab"]),
            MouseCursor::Grabbing => loadn(&["closedhand", "grabbing"]),
            MouseCursor::Progress => load("left_ptr_watch"),
            MouseCursor::AllScroll => load("all-scroll"),
            MouseCursor::ContextMenu => load("context-menu"),

            MouseCursor::NoDrop => loadn(&["no-drop", "circle"]),
            MouseCursor::NotAllowed => load("crossed_circle"),


            // Resize cursors
            MouseCursor::EResize => load("right_side"),
            MouseCursor::NResize => load("top_side"),
            MouseCursor::NeResize => load("top_right_corner"),
            MouseCursor::NwResize => load("top_left_corner"),
            MouseCursor::SResize => load("bottom_side"),
            MouseCursor::SeResize => load("bottom_right_corner"),
            MouseCursor::SwResize => load("bottom_left_corner"),
            MouseCursor::WResize => load("left_side"),
            MouseCursor::EwResize => load("h_double_arrow"),
            MouseCursor::NsResize => load("v_double_arrow"),
            MouseCursor::NwseResize => loadn(&["bd_double_arrow", "size_bdiag"]),
            MouseCursor::NeswResize => loadn(&["fd_double_arrow", "size_fdiag"]),
            MouseCursor::ColResize => loadn(&["split_h", "h_double_arrow"]),
            MouseCursor::RowResize => loadn(&["split_v", "v_double_arrow"]),

            MouseCursor::Text => loadn(&["text", "xterm"]),
            MouseCursor::VerticalText => load("vertical-text"),

            MouseCursor::Wait => load("watch"),

            MouseCursor::ZoomIn => load("zoom-in"),
            MouseCursor::ZoomOut => load("zoom-out"),

            MouseCursor::NoneCursor => self.create_empty_cursor(),
        }
    }

    fn update_cursor(&self, cursor: ffi::Cursor) {
        unsafe {
            (self.x.display.xlib.XDefineCursor)(self.x.display.display, self.x.window, cursor);
            if cursor != 0 {
                (self.x.display.xlib.XFreeCursor)(self.x.display.display, cursor);
            }
            self.x.display.check_errors().expect("Failed to set or free the cursor");
        }
    }

    pub fn set_cursor(&self, cursor: MouseCursor) {
        let mut current_cursor = self.cursor.lock().unwrap();
        *current_cursor = cursor;
        if *self.cursor_state.lock().unwrap() != CursorState::Hide {
            self.update_cursor(self.get_cursor(*current_cursor));
        }
    }

    // TODO: This could maybe be cached. I don't think it's worth
    // the complexity, since cursor changes are not so common,
    // and this is just allocating a 1x1 pixmap...
    fn create_empty_cursor(&self) -> ffi::Cursor {
        use std::mem;

        let data = 0;
        unsafe {
            let pixmap = (self.x.display.xlib.XCreateBitmapFromData)(self.x.display.display, self.x.window, &data, 1, 1);
            if pixmap == 0 {
                // Failed to allocate
                return 0;
            }

            // We don't care about this color, since it only fills bytes
            // in the pixmap which are not 0 in the mask.
            let dummy_color: ffi::XColor = mem::uninitialized();
            let cursor = (self.x.display.xlib.XCreatePixmapCursor)(self.x.display.display,
                                                                   pixmap,
                                                                   pixmap,
                                                                   &dummy_color as *const _ as *mut _,
                                                                   &dummy_color as *const _ as *mut _, 0, 0);
            (self.x.display.xlib.XFreePixmap)(self.x.display.display, pixmap);
            cursor
        }
    }

    pub fn set_cursor_state(&self, state: CursorState) -> Result<(), String> {
        use CursorState::{ Grab, Normal, Hide };

        let mut cursor_state = self.cursor_state.lock().unwrap();
        match (state, *cursor_state) {
            (Normal, Normal) | (Hide, Hide) | (Grab, Grab) => return Ok(()),
            _ => {},
        }

        match *cursor_state {
            Grab => {
                unsafe {
                    (self.x.display.xlib.XUngrabPointer)(self.x.display.display, ffi::CurrentTime);
                    self.x.display.check_errors().expect("Failed to call XUngrabPointer");
                }
            },
            Normal => {},
            Hide => self.update_cursor(self.get_cursor(*self.cursor.lock().unwrap())),
        }

        match state {
            Normal => {
                *cursor_state = state;
                Ok(())
            },
            Hide => {
                *cursor_state = state;
                self.update_cursor(self.create_empty_cursor());
                Ok(())
            },
            Grab => {
                unsafe {
                    // Ungrab before grabbing to prevent passive grabs
                    // from causing AlreadyGrabbed
                    (self.x.display.xlib.XUngrabPointer)(self.x.display.display, ffi::CurrentTime);

                    match (self.x.display.xlib.XGrabPointer)(
                        self.x.display.display, self.x.window, ffi::True,
                        (ffi::ButtonPressMask | ffi::ButtonReleaseMask | ffi::EnterWindowMask |
                        ffi::LeaveWindowMask | ffi::PointerMotionMask | ffi::PointerMotionHintMask |
                        ffi::Button1MotionMask | ffi::Button2MotionMask | ffi::Button3MotionMask |
                        ffi::Button4MotionMask | ffi::Button5MotionMask | ffi::ButtonMotionMask |
                        ffi::KeymapStateMask) as libc::c_uint,
                        ffi::GrabModeAsync, ffi::GrabModeAsync,
                        self.x.window, 0, ffi::CurrentTime
                    ) {
                        ffi::GrabSuccess => {
                            *cursor_state = state;
                            Ok(())
                        },
                        ffi::AlreadyGrabbed | ffi::GrabInvalidTime |
                        ffi::GrabNotViewable | ffi::GrabFrozen
                            => Err("cursor could not be grabbed".to_string()),
                        _ => unreachable!(),
                    }
                }
            },
        }
    }

    pub fn hidpi_factor(&self) -> f32 {
        unsafe {
            let x_px = (self.x.display.xlib.XDisplayWidth)(self.x.display.display, self.x.screen_id);
            let y_px = (self.x.display.xlib.XDisplayHeight)(self.x.display.display, self.x.screen_id);
            let x_mm = (self.x.display.xlib.XDisplayWidthMM)(self.x.display.display, self.x.screen_id);
            let y_mm = (self.x.display.xlib.XDisplayHeightMM)(self.x.display.display, self.x.screen_id);
            let ppmm = ((x_px as f32 * y_px as f32) / (x_mm as f32 * y_mm as f32)).sqrt();
            ((ppmm * (12.0 * 25.4 / 96.0)).round() / 12.0).max(1.0) // quantize with 1/12 step size.
        }
    }

    pub fn set_cursor_position(&self, x: i32, y: i32) -> Result<(), ()> {
        unsafe {
            (self.x.display.xlib.XWarpPointer)(self.x.display.display, 0, self.x.window, 0, 0, 0, 0, x, y);
            self.x.display.check_errors().map_err(|_| ())
        }
    }

    #[inline]
    pub fn id(&self) -> WindowId { WindowId(self.x.window) }
}
