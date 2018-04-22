use MouseCursor;
use CreationError;
use CreationError::OsError;
use libc;
use std::borrow::Borrow;
use std::{mem, cmp};
use std::sync::Arc;
use std::os::raw::*;
use std::ffi::CString;

use parking_lot::Mutex;

use CursorState;
use WindowAttributes;
use platform::PlatformSpecificWindowBuilderAttributes;

use platform::MonitorId as PlatformMonitorId;
use platform::x11::MonitorId as X11MonitorId;
use window::MonitorId as RootMonitorId;

use platform::x11::monitor::get_available_monitors;

use super::{ffi, util, XConnection, XError, WindowId, EventsLoop};

unsafe extern "C" fn visibility_predicate(
    _display: *mut ffi::Display,
    event: *mut ffi::XEvent,
    arg: ffi::XPointer, // We populate this with the window ID (by value) when we call XIfEvent
) -> ffi::Bool {
    let event: &ffi::XAnyEvent = (*event).as_ref();
    let window = arg as ffi::Window;
    (event.window == window && event.type_ == ffi::VisibilityNotify) as _
}

pub struct XWindow {
    pub display: Arc<XConnection>,
    pub window: ffi::Window,
    pub root: ffi::Window,
    pub screen_id: i32,
}

unsafe impl Send for XWindow {}
unsafe impl Sync for XWindow {}

unsafe impl Send for Window2 {}
unsafe impl Sync for Window2 {}

#[derive(Debug, Default)]
pub struct SharedState {
    pub frame_extents: Option<util::FrameExtentsHeuristic>,
}

pub struct Window2 {
    pub x: Arc<XWindow>,
    cursor: Mutex<MouseCursor>,
    cursor_state: Mutex<CursorState>,
    pub shared_state: Arc<Mutex<SharedState>>,
}

impl Window2 {
    pub fn new(
        ctx: &EventsLoop,
        window_attrs: &WindowAttributes,
        pl_attribs: &PlatformSpecificWindowBuilderAttributes,
    ) -> Result<Window2, CreationError> {
        let xconn = &ctx.display;

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
            None => unsafe { (xconn.xlib.XDefaultScreen)(xconn.display) },
        };

        // getting the root window
        let root = ctx.root;

        // creating
        let mut set_win_attr = {
            let mut swa: ffi::XSetWindowAttributes = unsafe { mem::zeroed() };
            swa.colormap = if let Some(vi) = pl_attribs.visual_infos {
                unsafe {
                    let visual = vi.visual;
                    (xconn.xlib.XCreateColormap)(xconn.display, root, visual, ffi::AllocNone)
                }
            } else { 0 };
            swa.event_mask = ffi::ExposureMask
                | ffi::StructureNotifyMask
                | ffi::VisibilityChangeMask
                | ffi::KeyPressMask
                | ffi::KeyReleaseMask
                | ffi::KeymapStateMask
                | ffi::ButtonPressMask
                | ffi::ButtonReleaseMask
                | ffi::PointerMotionMask;
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
            (xconn.xlib.XCreateWindow)(
                xconn.display,
                root,
                0,
                0,
                dimensions.0 as c_uint,
                dimensions.1 as c_uint,
                0,
                match pl_attribs.visual_infos {
                    Some(vi) => vi.depth,
                    None => ffi::CopyFromParent
                },
                ffi::InputOutput as c_uint,
                match pl_attribs.visual_infos {
                    Some(vi) => vi.visual,
                    None => ffi::CopyFromParent as *mut _
                },
                window_attributes,
                &mut set_win_attr,
            )
        };

        let x_window = Arc::new(XWindow {
            display: Arc::clone(xconn),
            window,
            root,
            screen_id,
        });

        let window = Window2 {
            x: x_window,
            cursor: Mutex::new(MouseCursor::Default),
            cursor_state: Mutex::new(CursorState::Normal),
            shared_state: Arc::new(Mutex::new(SharedState::default())),
        };

        // Title must be set before mapping. Some tiling window managers (i.e. i3) use the window
        // title to determine placement/etc., so doing this after mapping would cause the WM to
        // act on the wrong title state.
        window.set_title_inner(&window_attrs.title).queue();
        window.set_decorations_inner(window_attrs.decorations).queue();

        {
            let ref x_window: &XWindow = window.x.borrow();

            // Enable drag and drop (TODO: extend API to make this toggleable)
            unsafe {
                let dnd_aware_atom = util::get_atom(xconn, b"XdndAware\0")
                    .expect("Failed to call XInternAtom (XdndAware)");
                let version = &[5 as c_ulong]; // Latest version; hasn't changed since 2002
                util::change_property(
                    xconn,
                    x_window.window,
                    dnd_aware_atom,
                    ffi::XA_ATOM,
                    util::Format::Long,
                    util::PropMode::Replace,
                    version,
                )
            }.queue();

            // Set ICCCM WM_CLASS property based on initial window title
            // Must be done *before* mapping the window by ICCCM 4.1.2.5
            {
                let name = CString::new(window_attrs.title.as_str())
                    .expect("Window title contained null byte");
                let mut class_hints = {
                    let class_hints = unsafe { (xconn.xlib.XAllocClassHint)() };
                    util::XSmartPointer::new(xconn, class_hints)
                }.expect("XAllocClassHint returned null; out of memory");
                (*class_hints).res_name = name.as_ptr() as *mut c_char;
                (*class_hints).res_class = name.as_ptr() as *mut c_char;
                unsafe {
                    (xconn.xlib.XSetClassHint)(
                        xconn.display,
                        x_window.window,
                        class_hints.ptr,
                    );
                }//.queue();
            }

            // set size hints
            {
                let mut size_hints = {
                    let size_hints = unsafe { (xconn.xlib.XAllocSizeHints)() };
                    util::XSmartPointer::new(xconn, size_hints)
                }.expect("XAllocSizeHints returned null; out of memory");
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
                    (xconn.xlib.XSetWMNormalHints)(
                        xconn.display,
                        x_window.window,
                        size_hints.ptr,
                    );
                }//.queue();
            }

            // Opt into handling window close
            unsafe {
                (xconn.xlib.XSetWMProtocols)(
                    xconn.display,
                    x_window.window,
                    &ctx.wm_delete_window as *const _ as *mut _,
                    1,
                );
            }//.queue();

            // Set visibility (map window)
            if window_attrs.visible {
                unsafe {
                    (xconn.xlib.XMapRaised)(xconn.display, x_window.window);
                }//.queue();
            }

            // Attempt to make keyboard input repeat detectable
            unsafe {
                let mut supported_ptr = ffi::False;
                (xconn.xlib.XkbSetDetectableAutoRepeat)(
                    xconn.display,
                    ffi::True,
                    &mut supported_ptr,
                );
                if supported_ptr == ffi::False {
                    return Err(OsError(format!("XkbSetDetectableAutoRepeat failed")));
                }
            }

            // Select XInput2 events
            let mask = {
                let mut mask = ffi::XI_MotionMask
                    | ffi::XI_ButtonPressMask
                    | ffi::XI_ButtonReleaseMask
                    //| ffi::XI_KeyPressMask
                    //| ffi::XI_KeyReleaseMask
                    | ffi::XI_EnterMask
                    | ffi::XI_LeaveMask
                    | ffi::XI_FocusInMask
                    | ffi::XI_FocusOutMask;
                if window_attrs.multitouch {
                    mask |= ffi::XI_TouchBeginMask
                        | ffi::XI_TouchUpdateMask
                        | ffi::XI_TouchEndMask;
                }
                mask
            };
            unsafe {
                util::select_xinput_events(
                    xconn,
                    x_window.window,
                    ffi::XIAllMasterDevices,
                    mask,
                )
            }.queue();

            // These properties must be set after mapping
            window.set_maximized_inner(window_attrs.maximized).queue();
            window.set_fullscreen_inner(window_attrs.fullscreen.clone()).queue();

            if window_attrs.visible {
                unsafe {
                    // XSetInputFocus generates an error if the window is not visible, so we wait
                    // until we receive VisibilityNotify.
                    let mut event = mem::uninitialized();
                    (xconn.xlib.XIfEvent)( // This will flush the request buffer IF it blocks.
                        xconn.display,
                        &mut event as *mut ffi::XEvent,
                        Some(visibility_predicate),
                        x_window.window as _,
                    );
                    (xconn.xlib.XSetInputFocus)(
                        xconn.display,
                        x_window.window,
                        ffi::RevertToParent,
                        ffi::CurrentTime,
                    );
                }
            }
        }

        // We never want to give the user a broken window, since by then, it's too late to handle.
        unsafe { util::sync_with_server(xconn) }
            .map(|_| window)
            .map_err(|x_err| OsError(
                format!("X server returned error while building window: {:?}", x_err)
            ))
    }

    fn set_netwm(
        xconn: &Arc<XConnection>,
        window: ffi::Window,
        root: ffi::Window,
        properties: (c_long, c_long, c_long, c_long),
        operation: util::StateOperation
    ) -> util::Flusher {
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
        }
    }

    fn set_fullscreen_hint(&self, fullscreen: bool) -> util::Flusher {
        let xconn = &self.x.display;

        let fullscreen_atom = unsafe { util::get_atom(xconn, b"_NET_WM_STATE_FULLSCREEN\0") }
            .expect("Failed to call XInternAtom (_NET_WM_STATE_FULLSCREEN)");

        Window2::set_netwm(
            xconn,
            self.x.window,
            self.x.root,
            (fullscreen_atom as c_long, 0, 0, 0),
            fullscreen.into(),
        )
    }

    fn set_fullscreen_inner(&self, monitor: Option<RootMonitorId>) -> util::Flusher {
        match monitor {
            None => {
                self.set_fullscreen_hint(false)
            },
            Some(RootMonitorId { inner: PlatformMonitorId::X(monitor) }) => {
                let screenpos = monitor.get_position();
                self.set_position(screenpos.0 as i32, screenpos.1 as i32);
                self.set_fullscreen_hint(true)
            }
            _ => unreachable!(),
        }
    }

    pub fn set_fullscreen(&self, monitor: Option<RootMonitorId>) {
        self.set_fullscreen_inner(monitor)
            .flush()
            .expect("Failed to change window fullscreen state");
        self.invalidate_cached_frame_extents();
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

    fn set_maximized_inner(&self, maximized: bool) -> util::Flusher {
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
            maximized.into(),
        )
    }

    pub fn set_maximized(&self, maximized: bool) {
        self.set_maximized_inner(maximized)
            .flush()
            .expect("Failed to change window maximization");
        self.invalidate_cached_frame_extents();
    }

    fn set_title_inner(&self, title: &str) -> util::Flusher {
        let xconn = &self.x.display;

        let wm_name_atom = unsafe { util::get_atom(xconn, b"_NET_WM_NAME\0") }
            .expect("Failed to call XInternAtom (_NET_WM_NAME)");
        let utf8_atom = unsafe { util::get_atom(xconn, b"UTF8_STRING\0") }
            .expect("Failed to call XInternAtom (UTF8_STRING)");

        let title = CString::new(title).expect("Window title contained null byte");
        unsafe {
            (xconn.xlib.XStoreName)(
                xconn.display,
                self.x.window,
                title.as_ptr() as *const c_char,
            );

            util::change_property(
                xconn,
                self.x.window,
                wm_name_atom,
                utf8_atom,
                util::Format::Char,
                util::PropMode::Replace,
                title.as_bytes_with_nul(),
            )
        }
    }

    pub fn set_title(&self, title: &str) {
        self.set_title_inner(title)
            .flush()
            .expect("Failed to set window title");
    }

    fn set_decorations_inner(&self, decorations: bool) -> util::Flusher {
        let xconn = &self.x.display;

        let wm_hints = unsafe { util::get_atom(xconn, b"_MOTIF_WM_HINTS\0") }
            .expect("Failed to call XInternAtom (_MOTIF_WM_HINTS)");

        unsafe {
            util::change_property(
                xconn,
                self.x.window,
                wm_hints,
                wm_hints,
                util::Format::Long,
                util::PropMode::Replace,
                &[
                    util::MWM_HINTS_DECORATIONS, // flags
                    0, // functions
                    decorations as c_ulong, // decorations
                    0, // input mode
                    0, // status
                ],
            )
        }
    }

    pub fn set_decorations(&self, decorations: bool) {
        self.set_decorations_inner(decorations)
            .flush()
            .expect("Failed to set decoration state");
        self.invalidate_cached_frame_extents();
    }

    pub fn show(&self) {
        unsafe {
            (self.x.display.xlib.XMapRaised)(self.x.display.display, self.x.window);
            util::flush_requests(&self.x.display)
                .expect("Failed to call XMapRaised");
        }
    }

    pub fn hide(&self) {
        unsafe {
            (self.x.display.xlib.XUnmapWindow)(self.x.display.display, self.x.window);
            util::flush_requests(&self.x.display)
                .expect("Failed to call XUnmapWindow");
        }
    }

    fn update_cached_frame_extents(&self) {
        let extents = util::get_frame_extents_heuristic(
            &self.x.display,
            self.x.window,
            self.x.root,
        );
        (*self.shared_state.lock()).frame_extents = Some(extents);
    }

    fn invalidate_cached_frame_extents(&self) {
        (*self.shared_state.lock()).frame_extents.take();
    }

    #[inline]
    pub fn get_position(&self) -> Option<(i32, i32)> {
        let extents = (*self.shared_state.lock()).frame_extents.clone();
        if let Some(extents) = extents {
            self.get_inner_position().map(|(x, y)|
                extents.inner_pos_to_outer(x, y)
            )
        } else {
            self.update_cached_frame_extents();
            self.get_position()
        }
    }

    #[inline]
    pub fn get_inner_position(&self) -> Option<(i32, i32)> {
        unsafe { util::translate_coords(&self.x.display, self.x.window, self.x.root )}
            .ok()
            .map(|coords| (coords.x_rel_root, coords.y_rel_root))
    }

    pub fn set_position(&self, mut x: i32, mut y: i32) {
        // There are a few WMs that set client area position rather than window position, so
        // we'll translate for consistency.
        if util::wm_name_is_one_of(&["Enlightenment", "FVWM"]) {
            let extents = (*self.shared_state.lock()).frame_extents.clone();
            if let Some(extents) = extents {
                x += extents.frame_extents.left as i32;
                y += extents.frame_extents.top as i32;
            } else {
                self.update_cached_frame_extents();
                self.set_position(x, y)
            }
        }
        unsafe {
            (self.x.display.xlib.XMoveWindow)(
                self.x.display.display,
                self.x.window,
                x as c_int,
                y as c_int,
            );
            util::flush_requests(&self.x.display)
        }.expect("Failed to call XMoveWindow");
    }

    #[inline]
    pub fn get_inner_size(&self) -> Option<(u32, u32)> {
        unsafe { util::get_geometry(&self.x.display, self.x.window) }
            .ok()
            .map(|geo| (geo.width, geo.height))
    }

    #[inline]
    pub fn get_outer_size(&self) -> Option<(u32, u32)> {
        let extents = (*self.shared_state.lock()).frame_extents.clone();
        if let Some(extents) = extents {
            self.get_inner_size().map(|(w, h)|
                extents.inner_size_to_outer(w, h)
            )
        } else {
            self.update_cached_frame_extents();
            self.get_outer_size()
        }
    }

    #[inline]
    pub fn set_inner_size(&self, x: u32, y: u32) {
        unsafe {
            (self.x.display.xlib.XResizeWindow)(
                self.x.display.display,
                self.x.window,
                x as c_uint,
                y as c_uint,
            );
            util::flush_requests(&self.x.display)
        }.expect("Failed to call XResizeWindow");
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
        util::flush_requests(xconn)?;

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
            (self.x.display.xlib_xcb.XGetXCBConnection)(self.x.display.display) as *mut _
        }
    }

    fn load_cursor(&self, name: &[u8]) -> ffi::Cursor {
        unsafe {
            (self.x.display.xcursor.XcursorLibraryLoadCursor)(
                self.x.display.display,
                name.as_ptr() as *const c_char,
            )
        }
    }

    fn load_first_existing_cursor(&self, names: &[&[u8]]) -> ffi::Cursor {
        for name in names.iter() {
            let xcursor = self.load_cursor(name);
            if xcursor != 0 {
                return xcursor;
            }
        }
        0
    }

    fn get_cursor(&self, cursor: MouseCursor) -> ffi::Cursor {
        let load = |name: &[u8]| {
            self.load_cursor(name)
        };

        let loadn = |names: &[&[u8]]| {
            self.load_first_existing_cursor(names)
        };

        // Try multiple names in some cases where the name
        // differs on the desktop environments or themes.
        //
        // Try the better looking (or more suiting) names first.
        match cursor {
            MouseCursor::Alias => load(b"link\0"),
            MouseCursor::Arrow => load(b"arrow\0"),
            MouseCursor::Cell => load(b"plus\0"),
            MouseCursor::Copy => load(b"copy\0"),
            MouseCursor::Crosshair => load(b"crosshair\0"),
            MouseCursor::Default => load(b"left_ptr\0"),
            MouseCursor::Hand => loadn(&[b"hand2\0", b"hand1\0"]),
            MouseCursor::Help => load(b"question_arrow\0"),
            MouseCursor::Move => load(b"move\0"),
            MouseCursor::Grab => loadn(&[b"openhand\0", b"grab\0"]),
            MouseCursor::Grabbing => loadn(&[b"closedhand\0", b"grabbing\0"]),
            MouseCursor::Progress => load(b"left_ptr_watch\0"),
            MouseCursor::AllScroll => load(b"all-scroll\0"),
            MouseCursor::ContextMenu => load(b"context-menu\0"),

            MouseCursor::NoDrop => loadn(&[b"no-drop\0", b"circle\0"]),
            MouseCursor::NotAllowed => load(b"crossed_circle\0"),


            // Resize cursors
            MouseCursor::EResize => load(b"right_side\0"),
            MouseCursor::NResize => load(b"top_side\0"),
            MouseCursor::NeResize => load(b"top_right_corner\0"),
            MouseCursor::NwResize => load(b"top_left_corner\0"),
            MouseCursor::SResize => load(b"bottom_side\0"),
            MouseCursor::SeResize => load(b"bottom_right_corner\0"),
            MouseCursor::SwResize => load(b"bottom_left_corner\0"),
            MouseCursor::WResize => load(b"left_side\0"),
            MouseCursor::EwResize => load(b"h_double_arrow\0"),
            MouseCursor::NsResize => load(b"v_double_arrow\0"),
            MouseCursor::NwseResize => loadn(&[b"bd_double_arrow\0", b"size_bdiag\0"]),
            MouseCursor::NeswResize => loadn(&[b"fd_double_arrow\0", b"size_fdiag\0"]),
            MouseCursor::ColResize => loadn(&[b"split_h\0", b"h_double_arrow\0"]),
            MouseCursor::RowResize => loadn(&[b"split_v\0", b"v_double_arrow\0"]),

            MouseCursor::Text => loadn(&[b"text\0", b"xterm\0"]),
            MouseCursor::VerticalText => load(b"vertical-text\0"),

            MouseCursor::Wait => load(b"watch\0"),

            MouseCursor::ZoomIn => load(b"zoom-in\0"),
            MouseCursor::ZoomOut => load(b"zoom-out\0"),

            MouseCursor::NoneCursor => self.create_empty_cursor()
                .expect("Failed to create empty cursor"),
        }
    }

    fn update_cursor(&self, cursor: ffi::Cursor) {
        unsafe {
            (self.x.display.xlib.XDefineCursor)(self.x.display.display, self.x.window, cursor);
            if cursor != 0 {
                (self.x.display.xlib.XFreeCursor)(self.x.display.display, cursor);
            }
            util::flush_requests(&self.x.display).expect("Failed to set or free the cursor");
        }
    }

    pub fn set_cursor(&self, cursor: MouseCursor) {
        let mut current_cursor = self.cursor.lock();
        *current_cursor = cursor;
        if *self.cursor_state.lock() != CursorState::Hide {
            self.update_cursor(self.get_cursor(*current_cursor));
        }
    }

    // TODO: This could maybe be cached. I don't think it's worth
    // the complexity, since cursor changes are not so common,
    // and this is just allocating a 1x1 pixmap...
    fn create_empty_cursor(&self) -> Option<ffi::Cursor> {
        let data = 0;
        let pixmap = unsafe {
            (self.x.display.xlib.XCreateBitmapFromData)(
                self.x.display.display,
                self.x.window,
                &data,
                1,
                1,
            )
        };
        if pixmap == 0 {
            // Failed to allocate
            return None;
        }

        let cursor = unsafe {
            // We don't care about this color, since it only fills bytes
            // in the pixmap which are not 0 in the mask.
            let dummy_color: ffi::XColor = mem::uninitialized();
            let cursor = (self.x.display.xlib.XCreatePixmapCursor)(
                self.x.display.display,
                pixmap,
                pixmap,
                &dummy_color as *const _ as *mut _,
                &dummy_color as *const _ as *mut _,
                0,
                0,
            );
            (self.x.display.xlib.XFreePixmap)(self.x.display.display, pixmap);
            cursor
        };
        Some(cursor)
    }

    pub fn set_cursor_state(&self, state: CursorState) -> Result<(), String> {
        use CursorState::{ Grab, Normal, Hide };

        let mut cursor_state = self.cursor_state.lock();
        match (state, *cursor_state) {
            (Normal, Normal) | (Hide, Hide) | (Grab, Grab) => return Ok(()),
            _ => {},
        }

        match *cursor_state {
            Grab => {
                unsafe {
                    (self.x.display.xlib.XUngrabPointer)(self.x.display.display, ffi::CurrentTime);
                    util::flush_requests(&self.x.display).expect("Failed to call XUngrabPointer");
                }
            },
            Normal => {},
            Hide => self.update_cursor(self.get_cursor(*self.cursor.lock())),
        }

        match state {
            Normal => {
                *cursor_state = state;
                Ok(())
            },
            Hide => {
                *cursor_state = state;
                self.update_cursor(
                    self.create_empty_cursor().expect("Failed to create empty cursor")
                );
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
                        ffi::KeymapStateMask) as c_uint,
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
            (self.x.display.xlib.XWarpPointer)(
                self.x.display.display,
                0,
                self.x.window,
                0,
                0,
                0,
                0,
                x,
                y,
            );
            util::flush_requests(&self.x.display).map_err(|_| ())
        }
    }

    #[inline]
    pub fn id(&self) -> WindowId { WindowId(self.x.window) }
}
