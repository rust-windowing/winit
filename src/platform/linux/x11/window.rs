use MouseCursor;
use CreationError;
use CreationError::OsError;
use libc;
use std::borrow::Borrow;
use std::{mem, ptr, cmp};
use std::sync::{Arc, Mutex};
use std::os::raw::{c_int, c_long, c_uchar};
use std::thread;
use std::time::Duration;

use CursorState;
use WindowAttributes;
use platform::PlatformSpecificWindowBuilderAttributes;

use platform::MonitorId as PlatformMonitorId;

use super::{ffi};
use super::{MonitorId, XConnection, WindowId, EventsLoop};

// TODO: remove me
fn with_c_str<F, T>(s: &str, f: F) -> T where F: FnOnce(*const libc::c_char) -> T {
    use std::ffi::CString;
    let c_str = CString::new(s.as_bytes().to_vec()).unwrap();
    f(c_str.as_ptr())
}

struct WindowProxyData {
    display: Arc<XConnection>,
    window: ffi::Window,
}

unsafe impl Send for WindowProxyData {}

pub struct XWindow {
    display: Arc<XConnection>,
    window: ffi::Window,
    is_fullscreen: bool,
    screen_id: libc::c_int,
    xf86_desk_mode: Option<ffi::XF86VidModeModeInfo>,
    window_proxy_data: Arc<Mutex<Option<WindowProxyData>>>,
}

unsafe impl Send for XWindow {}
unsafe impl Sync for XWindow {}

unsafe impl Send for Window {}
unsafe impl Sync for Window {}

impl Drop for XWindow {
    fn drop(&mut self) {
        unsafe {
            // Clear out the window proxy data arc, so that any window proxy objects
            // are no longer able to send messages to this window.
            *self.window_proxy_data.lock().unwrap() = None;

            if self.is_fullscreen {
                if let Some(mut xf86_desk_mode) = self.xf86_desk_mode {
                    (self.display.xf86vmode.XF86VidModeSwitchToMode)(self.display.display, self.screen_id, &mut xf86_desk_mode);
                }
                (self.display.xf86vmode.XF86VidModeSetViewPort)(self.display.display, self.screen_id, 0, 0);
            }

            (self.display.xlib.XDestroyWindow)(self.display.display, self.window);
        }
    }
}

#[derive(Clone)]
pub struct WindowProxy {
    data: Arc<Mutex<Option<WindowProxyData>>>,
}

impl WindowProxy {
    pub fn wakeup_event_loop(&self) {
        let window_proxy_data = self.data.lock().unwrap();

        if let Some(ref data) = *window_proxy_data {
            let mut xev = ffi::XClientMessageEvent {
                type_: ffi::ClientMessage,
                window: data.window,
                format: 32,
                message_type: 0,
                serial: 0,
                send_event: 0,
                display: data.display.display,
                data: unsafe { mem::zeroed() },
            };

            unsafe {
                (data.display.xlib.XSendEvent)(data.display.display, data.window, 0, 0, mem::transmute(&mut xev));
                (data.display.xlib.XFlush)(data.display.display);
                data.display.check_errors().expect("Failed to call XSendEvent after wakeup");
            }
        }
    }
}

pub struct Window {
    pub x: Arc<XWindow>,
    cursor_state: Mutex<CursorState>,
}

impl Window {
    pub fn new(ctx: &EventsLoop, window_attrs: &WindowAttributes,
               pl_attribs: &PlatformSpecificWindowBuilderAttributes)
               -> Result<Window, CreationError>
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
            None => match window_attrs.monitor {
                Some(PlatformMonitorId::X(MonitorId(_, monitor))) => monitor as i32,
                _ => unsafe { (display.xlib.XDefaultScreen)(display.display) },
            }
        };

        // finding the mode to switch to if necessary
        let (mode_to_switch_to, xf86_desk_mode) = unsafe {
            let mut mode_num: libc::c_int = mem::uninitialized();
            let mut modes: *mut *mut ffi::XF86VidModeModeInfo = mem::uninitialized();
            if (display.xf86vmode.XF86VidModeGetAllModeLines)(display.display, screen_id, &mut mode_num, &mut modes) == 0 {
                (None, None)
            } else {
                let xf86_desk_mode: ffi::XF86VidModeModeInfo = ptr::read(*modes.offset(0));
                let mode_to_switch_to = if window_attrs.monitor.is_some() {
                    let matching_mode = (0 .. mode_num).map(|i| {
                        let m: ffi::XF86VidModeModeInfo = ptr::read(*modes.offset(i as isize) as *const _); m
                    }).find(|m| m.hdisplay == dimensions.0 as u16 && m.vdisplay == dimensions.1 as u16);
                    if let Some(matching_mode) = matching_mode {
                        Some(matching_mode)
                    } else {
                        let m = (0 .. mode_num).map(|i| {
                            let m: ffi::XF86VidModeModeInfo = ptr::read(*modes.offset(i as isize) as *const _); m
                        }).find(|m| m.hdisplay >= dimensions.0 as u16 && m.vdisplay >= dimensions.1 as u16);

                        match m {
                            Some(m) => Some(m),
                            None => return Err(OsError(format!("Could not find a suitable graphics mode")))
                        }
                    }
                } else {
                    None
                };
                (display.xlib.XFree)(modes as *mut _);
                (mode_to_switch_to, Some(xf86_desk_mode))
            }
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

        // Set ICCCM WM_CLASS property based on initial window title
        // Must be done *before* mapping the window by ICCCM 4.1.2.5
        unsafe {
            with_c_str(&*window_attrs.title, |c_name| {
                let hint = (display.xlib.XAllocClassHint)();
                (*hint).res_name = c_name as *mut libc::c_char;
                (*hint).res_class = c_name as *mut libc::c_char;
                (display.xlib.XSetClassHint)(display.display, window, hint);
                display.check_errors().expect("Failed to call XSetClassHint");
                (display.xlib.XFree)(hint as *mut _);
            });
        }

        // set visibility
        if window_attrs.visible {
            unsafe {
                (display.xlib.XMapRaised)(display.display, window);
                (display.xlib.XFlush)(display.display);
            }

            display.check_errors().expect("Failed to set window visibility");
        }

        // Opt into handling window close
        unsafe {
            (display.xlib.XSetWMProtocols)(display.display, window, &ctx.wm_delete_window as *const _ as *mut _, 1);
            display.check_errors().expect("Failed to call XSetWMProtocols");
            (display.xlib.XFlush)(display.display);
            display.check_errors().expect("Failed to call XFlush");
        }

        // Attempt to make keyboard input repeat detectable
        unsafe {
            let mut supported_ptr = ffi::False;
            (display.xlib.XkbSetDetectableAutoRepeat)(display.display, ffi::True, &mut supported_ptr);
            if supported_ptr == ffi::False {
                return Err(OsError(format!("XkbSetDetectableAutoRepeat failed")));
            }
        }

        let is_fullscreen = window_attrs.monitor.is_some();

        if is_fullscreen {
            let state_atom = unsafe {
                with_c_str("_NET_WM_STATE", |state|
                    (display.xlib.XInternAtom)(display.display, state, 0)
                )
            };
            display.check_errors().expect("Failed to call XInternAtom");
            let fullscreen_atom = unsafe {
                with_c_str("_NET_WM_STATE_FULLSCREEN", |state_fullscreen|
                    (display.xlib.XInternAtom)(display.display, state_fullscreen, 0)
                )
            };
            display.check_errors().expect("Failed to call XInternAtom");

            let client_message_event = ffi::XClientMessageEvent {
                type_: ffi::ClientMessage,
                serial: 0,
                send_event: 1,            // true because we are sending this through `XSendEvent`
                display: display.display,
                window: window,
                message_type: state_atom, // the _NET_WM_STATE atom is sent to change the state of a window
                format: 32,               // view `data` as `c_long`s
                data: {
                    let mut data = ffi::ClientMessageData::new();
                    // This first `long` is the action; `1` means add/set following property.
                    data.set_long(0, 1);
                    // This second `long` is the property to set (fullscreen)
                    data.set_long(1, fullscreen_atom as c_long);
                    data
                }
            };
            let mut x_event = ffi::XEvent::from(client_message_event);

            unsafe {
                (display.xlib.XSendEvent)(
                    display.display,
                    root,
                    0,
                    ffi::SubstructureRedirectMask | ffi::SubstructureNotifyMask,
                    &mut x_event as *mut _
                );
                display.check_errors().expect("Failed to call XSendEvent");
            }

            if let Some(mut mode_to_switch_to) = mode_to_switch_to {
                unsafe {
                    (display.xf86vmode.XF86VidModeSwitchToMode)(
                        display.display,
                        screen_id,
                        &mut mode_to_switch_to
                    );
                    display.check_errors().expect("Failed to call XF86VidModeSwitchToMode");
                }
            }
            else {
                println!("[glutin] Unexpected state: `mode` is None creating fullscreen window");
            }
            unsafe {
                (display.xf86vmode.XF86VidModeSetViewPort)(display.display, screen_id, 0, 0);
                display.check_errors().expect("Failed to call XF86VidModeSetViewPort");
            }

        } else {

            // set size hints
            let mut size_hints: ffi::XSizeHints = unsafe { mem::zeroed() };
            size_hints.flags = ffi::PSize;
            size_hints.width = dimensions.0 as i32;
            size_hints.height = dimensions.1 as i32;

            if let Some(dimensions) = window_attrs.min_dimensions {
                size_hints.flags |= ffi::PMinSize;
                size_hints.min_width = dimensions.0 as i32;
                size_hints.min_height = dimensions.1 as i32;
            }

            if let Some(dimensions) = window_attrs.max_dimensions {
                size_hints.flags |= ffi::PMaxSize;
                size_hints.max_width = dimensions.0 as i32;
                size_hints.max_height = dimensions.1 as i32;
            }

            unsafe {
                (display.xlib.XSetNormalHints)(display.display, window, &mut size_hints);
                display.check_errors().expect("Failed to call XSetNormalHints");
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
                (display.xinput2.XISelectEvents)(display.display, window,
                                                 &mut event_mask as *mut ffi::XIEventMask, 1);
            };
        }

        // creating the window object
        let window_proxy_data = WindowProxyData {
            display: display.clone(),
            window: window,
        };
        let window_proxy_data = Arc::new(Mutex::new(Some(window_proxy_data)));

        let window = Window {
            x: Arc::new(XWindow {
                display: display.clone(),
                window: window,
                screen_id: screen_id,
                is_fullscreen: is_fullscreen,
                xf86_desk_mode: xf86_desk_mode,
                window_proxy_data: window_proxy_data,
            }),
            cursor_state: Mutex::new(CursorState::Normal),
        };

        window.set_title(&window_attrs.title);
        window.set_decorations(window_attrs.decorations);

        if window_attrs.visible {
            unsafe {
                let ref x_window: &XWindow = window.x.borrow();

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

        // returning
        Ok(window)
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
            flags: u32,
            functions: u32,
            decorations: u32,
            input_mode: i32,
            status: u32,
        }

        let wm_hints = unsafe {
            (self.x.display.xlib.XInternAtom)(self.x.display.display, b"_MOTIF_WM_HINTS\0".as_ptr() as *const _, 0)
        };
        self.x.display.check_errors().expect("Failed to call XInternAtom");

        if !decorations {
            let hints = MotifWindowHints {
                flags: 2, // MWM_HINTS_DECORATIONS
                functions: 0,
                decorations: 0,
                input_mode: 0,
                status: 0,
            };

            unsafe {
                (self.x.display.xlib.XChangeProperty)(
                    self.x.display.display, self.x.window,
                    wm_hints, wm_hints, 32 /* Size of elements in struct */,
                    ffi::PropModeReplace, &hints as *const MotifWindowHints as *const u8,
                    5 /* Number of elements in struct */);
                (self.x.display.xlib.XFlush)(self.x.display.display);
            }
        } else {
            unsafe {
                (self.x.display.xlib.XDeleteProperty)(self.x.display.display, self.x.window, wm_hints);
                (self.x.display.xlib.XFlush)(self.x.display.display);
            }
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

            if (self.x.display.xlib.XGetGeometry)(self.x.display.display, self.x.window,
                &mut root, &mut x, &mut y, &mut width, &mut height,
                &mut border, &mut depth) == 0
            {
                return None;
            }

            Some((x as i32, y as i32, width as u32, height as u32, border as u32))
        }
    }

    #[inline]
    pub fn get_position(&self) -> Option<(i32, i32)> {
        self.get_geometry().map(|(x, y, _, _, _)| (x, y))
    }

    pub fn set_position(&self, x: i32, y: i32) {
        unsafe { (self.x.display.xlib.XMoveWindow)(self.x.display.display, self.x.window, x as libc::c_int, y as libc::c_int); }
        self.x.display.check_errors().expect("Failed to call XMoveWindow");
    }

    #[inline]
    pub fn get_inner_size(&self) -> Option<(u32, u32)> {
        self.get_geometry().map(|(_, _, w, h, _)| ((w as f32 / self.hidpi_factor()) as u32, (h as f32 / self.hidpi_factor()) as u32))
    }

    #[inline]
    pub fn get_outer_size(&self) -> Option<(u32, u32)> {
        self.get_geometry().map(|(_, _, w, h, b)| (w + b, h + b))       // TODO: is this really outside?
    }

    #[inline]
    pub fn set_inner_size(&self, x: u32, y: u32) {
        unsafe { (self.x.display.xlib.XResizeWindow)(self.x.display.display, self.x.window, x as libc::c_uint, y as libc::c_uint); }
        self.x.display.check_errors().expect("Failed to call XResizeWindow");
    }

    #[inline]
    pub fn create_window_proxy(&self) -> WindowProxy {
        WindowProxy {
            data: self.x.window_proxy_data.clone()
        }
    }

    #[inline]
    pub fn get_xlib_display(&self) -> *mut libc::c_void {
        self.x.display.display as *mut libc::c_void
    }

    #[inline]
    pub fn get_xlib_screen_id(&self) -> *mut libc::c_void {
        self.x.screen_id as *mut libc::c_void
    }

    #[inline]
    pub fn get_xlib_xconnection(&self) -> Arc<XConnection> {
        self.x.display.clone()
    }

    #[inline]
    pub fn platform_display(&self) -> *mut libc::c_void {
        self.x.display.display as *mut libc::c_void
    }

    #[inline]
    pub fn get_xlib_window(&self) -> *mut libc::c_void {
        self.x.window as *mut libc::c_void
    }

    #[inline]
    pub fn platform_window(&self) -> *mut libc::c_void {
        self.x.window as *mut libc::c_void
    }

    pub fn get_xcb_connection(&self) -> *mut libc::c_void {
        unsafe {
            (self.x.display.xlib_xcb.XGetXCBConnection)(self.get_xlib_display() as *mut _) as *mut _
        }
    }

    #[inline]
    pub fn set_window_resize_callback(&mut self, _: Option<fn(u32, u32)>) {
    }

    pub fn set_cursor(&self, cursor: MouseCursor) {
        unsafe {
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
            let xcursor = match cursor {
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


                /// Resize cursors
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
            };

            (self.x.display.xlib.XDefineCursor)(self.x.display.display, self.x.window, xcursor);
            if xcursor != 0 {
                (self.x.display.xlib.XFreeCursor)(self.x.display.display, xcursor);
            }
            self.x.display.check_errors().expect("Failed to set or free the cursor");
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
            Hide => {
                // NB: Calling XDefineCursor with None (aka 0)
                // as a value resets the cursor to the default.
                unsafe {
                    (self.x.display.xlib.XDefineCursor)(self.x.display.display, self.x.window, 0);
                }
            },
        }

        *cursor_state = state;
        match state {
            Normal => Ok(()),
            Hide => {
                unsafe {
                    let cursor = self.create_empty_cursor();
                    (self.x.display.xlib.XDefineCursor)(self.x.display.display, self.x.window, cursor);
                    if cursor != 0 {
                        (self.x.display.xlib.XFreeCursor)(self.x.display.display, cursor);
                    }
                    self.x.display.check_errors().expect("Failed to call XDefineCursor or free the empty cursor");
                }
                Ok(())
            },
            Grab => {
                unsafe {
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
                        ffi::GrabSuccess => Ok(()),
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
