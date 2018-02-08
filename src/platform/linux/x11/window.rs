use MouseCursor;
use CreationError;
use CreationError::OsError;
use libc;
use std::borrow::Borrow;
use std::{mem, cmp, ptr};
use std::sync::{Arc, Mutex};
use std::os::raw::{c_int, c_long, c_uchar, c_ulong, c_void};
use std::thread;
use std::time::Duration;

use CursorState;
use WindowAttributes;
use platform::PlatformSpecificWindowBuilderAttributes;

use platform::MonitorId as PlatformMonitorId;
use platform::x11::MonitorId as X11MonitorId;
use window::MonitorId as RootMonitorId;

use platform::x11::monitor::get_available_monitors;

use super::{ffi, util, XConnection, WindowId, EventsLoop};

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

        let window = Window2 {
            x: Arc::new(XWindow {
                display: display.clone(),
                window,
                root,
                screen_id,
            }),
            cursor: Mutex::new(MouseCursor::Default),
            cursor_state: Mutex::new(CursorState::Normal),
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
                (display.xlib.XSetNormalHints)(display.display, x_window.window, &mut size_hints);
                display.check_errors().expect("Failed to call XSetNormalHints");
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

            // Get non-positioning data from winit window
            if (self.x.display.xlib.XGetGeometry)(self.x.display.display, self.x.window,
                &mut root, &mut x, &mut y, &mut width, &mut height,
                &mut border, &mut depth) == 0
            {
                return None;
            }

            let width_out = width;
            let height_out = height;
            let border_out = border;

            // Some window managers like i3wm will actually nest application
            // windows (like those opened by winit) within other windows to, for
            // example, add decorations. Initially when debugging this method on
            // i3, the x and y positions were always returned as "2".
            //
            // The solution that other xlib abstractions use is to climb up the
            // window hierarchy until just below the root window, and that
            // window must be used to determine the appropriate position.
            let window = {
                let root = self.x.root;
                let mut window = self.x.window;
                loop {
                    let candidate = self.x.get_parent_window(window).unwrap();
                    if candidate == root {
                        break window;
                    }

                    window = candidate;
                }
            };

            if (self.x.display.xlib.XGetGeometry)(self.x.display.display, window,
                &mut root, &mut x, &mut y, &mut width, &mut height,
                &mut border, &mut depth) == 0
            {
                return None;
            }

            Some((x as i32, y as i32, width_out as u32, height_out as u32, border_out as u32))
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
        self.get_geometry().map(|(_, _, w, h, _)| (w, h))
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
