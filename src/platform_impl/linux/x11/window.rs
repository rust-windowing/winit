use std::{
    cmp,
    collections::HashSet,
    env,
    ffi::CString,
    mem::{self, MaybeUninit},
    os::raw::*,
    path::Path,
    ptr, slice,
    sync::Arc,
};

use libc;
use parking_lot::Mutex;

use crate::{
    dpi::{LogicalPosition, LogicalSize},
    error::{ExternalError, NotSupportedError, OsError as RootOsError},
    monitor::{MonitorHandle as RootMonitorHandle, VideoMode as RootVideoMode},
    platform_impl::{
        x11::{ime::ImeContextCreationError, MonitorHandle as X11MonitorHandle},
        MonitorHandle as PlatformMonitorHandle, OsError, PlatformSpecificWindowBuilderAttributes,
        VideoMode as PlatformVideoMode,
    },
    window::{CursorIcon, Fullscreen, Icon, WindowAttributes},
};

use super::{ffi, util, EventLoopWindowTarget, ImeSender, WindowId, XConnection, XError};

unsafe extern "C" fn visibility_predicate(
    _display: *mut ffi::Display,
    event: *mut ffi::XEvent,
    arg: ffi::XPointer, // We populate this with the window ID (by value) when we call XIfEvent
) -> ffi::Bool {
    let event: &ffi::XAnyEvent = (*event).as_ref();
    let window = arg as ffi::Window;
    (event.window == window && event.type_ == ffi::VisibilityNotify) as _
}

#[derive(Debug, Default)]
pub struct SharedState {
    pub cursor_pos: Option<(f64, f64)>,
    pub size: Option<(u32, u32)>,
    pub position: Option<(i32, i32)>,
    pub inner_position: Option<(i32, i32)>,
    pub inner_position_rel_parent: Option<(i32, i32)>,
    pub guessed_dpi: Option<f64>,
    pub last_monitor: Option<X11MonitorHandle>,
    pub dpi_adjusted: Option<(f64, f64)>,
    pub fullscreen: Option<Fullscreen>,
    // Used to restore position after exiting fullscreen
    pub restore_position: Option<(i32, i32)>,
    // Used to restore video mode after exiting fullscreen
    pub desktop_video_mode: Option<(ffi::RRCrtc, ffi::RRMode)>,
    pub frame_extents: Option<util::FrameExtentsHeuristic>,
    pub min_inner_size: Option<LogicalSize>,
    pub max_inner_size: Option<LogicalSize>,
}

impl SharedState {
    fn new(dpi_factor: f64) -> Mutex<Self> {
        let mut shared_state = SharedState::default();
        shared_state.guessed_dpi = Some(dpi_factor);
        Mutex::new(shared_state)
    }
}

unsafe impl Send for UnownedWindow {}
unsafe impl Sync for UnownedWindow {}

pub struct UnownedWindow {
    pub xconn: Arc<XConnection>, // never changes
    xwindow: ffi::Window,        // never changes
    root: ffi::Window,           // never changes
    screen_id: i32,              // never changes
    cursor: Mutex<CursorIcon>,
    cursor_grabbed: Mutex<bool>,
    cursor_visible: Mutex<bool>,
    ime_sender: Mutex<ImeSender>,
    pub shared_state: Mutex<SharedState>,
    pending_redraws: Arc<::std::sync::Mutex<HashSet<WindowId>>>,
}

impl UnownedWindow {
    pub fn new<T>(
        event_loop: &EventLoopWindowTarget<T>,
        window_attrs: WindowAttributes,
        pl_attribs: PlatformSpecificWindowBuilderAttributes,
    ) -> Result<UnownedWindow, RootOsError> {
        let xconn = &event_loop.xconn;
        let root = event_loop.root;

        let monitors = xconn.available_monitors();
        let dpi_factor = if !monitors.is_empty() {
            let mut dpi_factor = Some(monitors[0].hidpi_factor());
            for monitor in &monitors {
                if Some(monitor.hidpi_factor()) != dpi_factor {
                    dpi_factor = None;
                }
            }
            dpi_factor.unwrap_or_else(|| {
                xconn
                    .query_pointer(root, util::VIRTUAL_CORE_POINTER)
                    .ok()
                    .and_then(|pointer_state| {
                        let (x, y) = (pointer_state.root_x as i64, pointer_state.root_y as i64);
                        let mut dpi_factor = None;
                        for monitor in &monitors {
                            if monitor.rect.contains_point(x, y) {
                                dpi_factor = Some(monitor.hidpi_factor());
                                break;
                            }
                        }
                        dpi_factor
                    })
                    .unwrap_or(1.0)
            })
        } else {
            return Err(os_error!(OsError::XMisc("No monitors were detected.")));
        };

        info!("Guessed window DPI factor: {}", dpi_factor);

        let max_inner_size: Option<(u32, u32)> = window_attrs
            .max_inner_size
            .map(|size| size.to_physical(dpi_factor).into());
        let min_inner_size: Option<(u32, u32)> = window_attrs
            .min_inner_size
            .map(|size| size.to_physical(dpi_factor).into());

        let dimensions = {
            // x11 only applies constraints when the window is actively resized
            // by the user, so we have to manually apply the initial constraints
            let mut dimensions: (u32, u32) = window_attrs
                .inner_size
                .or_else(|| Some((800, 600).into()))
                .map(|size| size.to_physical(dpi_factor))
                .map(Into::into)
                .unwrap();
            if let Some(max) = max_inner_size {
                dimensions.0 = cmp::min(dimensions.0, max.0);
                dimensions.1 = cmp::min(dimensions.1, max.1);
            }
            if let Some(min) = min_inner_size {
                dimensions.0 = cmp::max(dimensions.0, min.0);
                dimensions.1 = cmp::max(dimensions.1, min.1);
            }
            debug!(
                "Calculated physical dimensions: {}x{}",
                dimensions.0, dimensions.1
            );
            dimensions
        };

        let screen_id = match pl_attribs.screen_id {
            Some(id) => id,
            None => unsafe { (xconn.xlib.XDefaultScreen)(xconn.display) },
        };

        // creating
        let mut set_win_attr = {
            let mut swa: ffi::XSetWindowAttributes = unsafe { mem::zeroed() };
            swa.colormap = if let Some(vi) = pl_attribs.visual_infos {
                unsafe {
                    let visual = vi.visual;
                    (xconn.xlib.XCreateColormap)(xconn.display, root, visual, ffi::AllocNone)
                }
            } else {
                0
            };
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
            swa.override_redirect = pl_attribs.override_redirect as c_int;
            swa
        };

        let mut window_attributes = ffi::CWBorderPixel | ffi::CWColormap | ffi::CWEventMask;

        if pl_attribs.override_redirect {
            window_attributes |= ffi::CWOverrideRedirect;
        }

        // finally creating the window
        let xwindow = unsafe {
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
                    None => ffi::CopyFromParent,
                },
                ffi::InputOutput as c_uint,
                // TODO: If window wants transparency and `visual_infos` is None,
                // we need to find our own visual which has an `alphaMask` which
                // is > 0, like we do in glutin.
                //
                // It is non obvious which masks, if any, we should pass to
                // `XGetVisualInfo`. winit doesn't recieve any info about what
                // properties the user wants. Users should consider choosing the
                // visual themselves as glutin does.
                match pl_attribs.visual_infos {
                    Some(vi) => vi.visual,
                    None => ffi::CopyFromParent as *mut ffi::Visual,
                },
                window_attributes,
                &mut set_win_attr,
            )
        };

        let window = UnownedWindow {
            xconn: Arc::clone(xconn),
            xwindow,
            root,
            screen_id,
            cursor: Default::default(),
            cursor_grabbed: Mutex::new(false),
            cursor_visible: Mutex::new(true),
            ime_sender: Mutex::new(event_loop.ime_sender.clone()),
            shared_state: SharedState::new(dpi_factor),
            pending_redraws: event_loop.pending_redraws.clone(),
        };

        // Title must be set before mapping. Some tiling window managers (i.e. i3) use the window
        // title to determine placement/etc., so doing this after mapping would cause the WM to
        // act on the wrong title state.
        window.set_title_inner(&window_attrs.title).queue();
        window
            .set_decorations_inner(window_attrs.decorations)
            .queue();

        {
            // Enable drag and drop (TODO: extend API to make this toggleable)
            unsafe {
                let dnd_aware_atom = xconn.get_atom_unchecked(b"XdndAware\0");
                let version = &[5 as c_ulong]; // Latest version; hasn't changed since 2002
                xconn.change_property(
                    window.xwindow,
                    dnd_aware_atom,
                    ffi::XA_ATOM,
                    util::PropMode::Replace,
                    version,
                )
            }
            .queue();

            // WM_CLASS must be set *before* mapping the window, as per ICCCM!
            {
                let (class, instance) = if let Some((instance, class)) = pl_attribs.class {
                    let instance = CString::new(instance.as_str())
                        .expect("`WM_CLASS` instance contained null byte");
                    let class =
                        CString::new(class.as_str()).expect("`WM_CLASS` class contained null byte");
                    (instance, class)
                } else {
                    let class = env::args()
                        .next()
                        .as_ref()
                        // Default to the name of the binary (via argv[0])
                        .and_then(|path| Path::new(path).file_name())
                        .and_then(|bin_name| bin_name.to_str())
                        .map(|bin_name| bin_name.to_owned())
                        .or_else(|| Some(window_attrs.title.clone()))
                        .and_then(|string| CString::new(string.as_str()).ok())
                        .expect("Default `WM_CLASS` class contained null byte");
                    // This environment variable is extraordinarily unlikely to actually be used...
                    let instance = env::var("RESOURCE_NAME")
                        .ok()
                        .and_then(|instance| CString::new(instance.as_str()).ok())
                        .or_else(|| Some(class.clone()))
                        .expect("Default `WM_CLASS` instance contained null byte");
                    (instance, class)
                };

                let mut class_hint = xconn.alloc_class_hint();
                (*class_hint).res_name = class.as_ptr() as *mut c_char;
                (*class_hint).res_class = instance.as_ptr() as *mut c_char;

                unsafe {
                    (xconn.xlib.XSetClassHint)(xconn.display, window.xwindow, class_hint.ptr);
                } //.queue();
            }

            window.set_pid().map(|flusher| flusher.queue());

            if pl_attribs.x11_window_type != Default::default() {
                window.set_window_type(pl_attribs.x11_window_type).queue();
            }

            if let Some(variant) = pl_attribs.gtk_theme_variant {
                window.set_gtk_theme_variant(variant).queue();
            }

            // set size hints
            {
                let mut min_inner_size = window_attrs
                    .min_inner_size
                    .map(|size| size.to_physical(dpi_factor));
                let mut max_inner_size = window_attrs
                    .max_inner_size
                    .map(|size| size.to_physical(dpi_factor));
                if !window_attrs.resizable {
                    if util::wm_name_is_one_of(&["Xfwm4"]) {
                        warn!("To avoid a WM bug, disabling resizing has no effect on Xfwm4");
                    } else {
                        max_inner_size = Some(dimensions.into());
                        min_inner_size = Some(dimensions.into());

                        let mut shared_state_lock = window.shared_state.lock();
                        shared_state_lock.min_inner_size = window_attrs.min_inner_size;
                        shared_state_lock.max_inner_size = window_attrs.max_inner_size;
                    }
                }

                let mut normal_hints = util::NormalHints::new(xconn);
                normal_hints.set_size(Some(dimensions));
                normal_hints.set_min_size(min_inner_size.map(Into::into));
                normal_hints.set_max_size(max_inner_size.map(Into::into));
                normal_hints.set_resize_increments(pl_attribs.resize_increments);
                normal_hints.set_base_size(pl_attribs.base_size);
                xconn.set_normal_hints(window.xwindow, normal_hints).queue();
            }

            // Set window icons
            if let Some(icon) = window_attrs.window_icon {
                window.set_icon_inner(icon).queue();
            }

            // Opt into handling window close
            unsafe {
                (xconn.xlib.XSetWMProtocols)(
                    xconn.display,
                    window.xwindow,
                    &[event_loop.wm_delete_window, event_loop.net_wm_ping] as *const ffi::Atom
                        as *mut ffi::Atom,
                    2,
                );
            } //.queue();

            // Set visibility (map window)
            if window_attrs.visible {
                unsafe {
                    (xconn.xlib.XMapRaised)(xconn.display, window.xwindow);
                } //.queue();
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
                    return Err(os_error!(OsError::XMisc(
                        "`XkbSetDetectableAutoRepeat` failed"
                    )));
                }
            }

            // Select XInput2 events
            let mask = {
                let mask = ffi::XI_MotionMask
                    | ffi::XI_ButtonPressMask
                    | ffi::XI_ButtonReleaseMask
                    //| ffi::XI_KeyPressMask
                    //| ffi::XI_KeyReleaseMask
                    | ffi::XI_EnterMask
                    | ffi::XI_LeaveMask
                    | ffi::XI_FocusInMask
                    | ffi::XI_FocusOutMask
                    | ffi::XI_TouchBeginMask
                    | ffi::XI_TouchUpdateMask
                    | ffi::XI_TouchEndMask;
                mask
            };
            xconn
                .select_xinput_events(window.xwindow, ffi::XIAllMasterDevices, mask)
                .queue();

            {
                let result = event_loop.ime.borrow_mut().create_context(window.xwindow);
                if let Err(err) = result {
                    let e = match err {
                        ImeContextCreationError::XError(err) => OsError::XError(err),
                        ImeContextCreationError::Null => {
                            OsError::XMisc("IME Context creation failed")
                        }
                    };
                    return Err(os_error!(e));
                }
            }

            // These properties must be set after mapping
            if window_attrs.maximized {
                window.set_maximized_inner(window_attrs.maximized).queue();
            }
            if window_attrs.fullscreen.is_some() {
                window
                    .set_fullscreen_inner(window_attrs.fullscreen.clone())
                    .unwrap()
                    .queue();
            }
            if window_attrs.always_on_top {
                window
                    .set_always_on_top_inner(window_attrs.always_on_top)
                    .queue();
            }

            if window_attrs.visible {
                unsafe {
                    // XSetInputFocus generates an error if the window is not visible, so we wait
                    // until we receive VisibilityNotify.
                    let mut event = MaybeUninit::uninit();
                    (xconn.xlib.XIfEvent)(
                        // This will flush the request buffer IF it blocks.
                        xconn.display,
                        event.as_mut_ptr(),
                        Some(visibility_predicate),
                        window.xwindow as _,
                    );
                    (xconn.xlib.XSetInputFocus)(
                        xconn.display,
                        window.xwindow,
                        ffi::RevertToParent,
                        ffi::CurrentTime,
                    );
                }
            }
        }

        // We never want to give the user a broken window, since by then, it's too late to handle.
        xconn
            .sync_with_server()
            .map(|_| window)
            .map_err(|x_err| os_error!(OsError::XError(x_err)))
    }

    fn logicalize_coords(&self, (x, y): (i32, i32)) -> LogicalPosition {
        let dpi = self.hidpi_factor();
        LogicalPosition::from_physical((x, y), dpi)
    }

    fn logicalize_size(&self, (width, height): (u32, u32)) -> LogicalSize {
        let dpi = self.hidpi_factor();
        LogicalSize::from_physical((width, height), dpi)
    }

    fn set_pid(&self) -> Option<util::Flusher<'_>> {
        let pid_atom = unsafe { self.xconn.get_atom_unchecked(b"_NET_WM_PID\0") };
        let client_machine_atom = unsafe { self.xconn.get_atom_unchecked(b"WM_CLIENT_MACHINE\0") };
        unsafe {
            // 64 would suffice for Linux, but 256 will be enough everywhere (as per SUSv2). For instance, this is
            // the limit defined by OpenBSD.
            const MAXHOSTNAMELEN: usize = 256;
            // `assume_init` is safe here because the array consists of `MaybeUninit` values,
            // which do not require initialization.
            let mut buffer: [MaybeUninit<c_char>; MAXHOSTNAMELEN] =
                MaybeUninit::uninit().assume_init();
            let status = libc::gethostname(buffer.as_mut_ptr() as *mut c_char, buffer.len());
            if status != 0 {
                return None;
            }
            ptr::write(buffer[MAXHOSTNAMELEN - 1].as_mut_ptr() as *mut u8, b'\0'); // a little extra safety
            let hostname_length = libc::strlen(buffer.as_ptr() as *const c_char);

            let hostname = slice::from_raw_parts(buffer.as_ptr() as *const c_char, hostname_length);

            self.xconn
                .change_property(
                    self.xwindow,
                    pid_atom,
                    ffi::XA_CARDINAL,
                    util::PropMode::Replace,
                    &[libc::getpid() as util::Cardinal],
                )
                .queue();
            let flusher = self.xconn.change_property(
                self.xwindow,
                client_machine_atom,
                ffi::XA_STRING,
                util::PropMode::Replace,
                &hostname[0..hostname_length],
            );
            Some(flusher)
        }
    }

    fn set_window_type(&self, window_type: util::WindowType) -> util::Flusher<'_> {
        let hint_atom = unsafe { self.xconn.get_atom_unchecked(b"_NET_WM_WINDOW_TYPE\0") };
        let window_type_atom = window_type.as_atom(&self.xconn);
        self.xconn.change_property(
            self.xwindow,
            hint_atom,
            ffi::XA_ATOM,
            util::PropMode::Replace,
            &[window_type_atom],
        )
    }

    fn set_gtk_theme_variant(&self, variant: String) -> util::Flusher<'_> {
        let hint_atom = unsafe { self.xconn.get_atom_unchecked(b"_GTK_THEME_VARIANT\0") };
        let utf8_atom = unsafe { self.xconn.get_atom_unchecked(b"UTF8_STRING\0") };
        let variant = CString::new(variant).expect("`_GTK_THEME_VARIANT` contained null byte");
        self.xconn.change_property(
            self.xwindow,
            hint_atom,
            utf8_atom,
            util::PropMode::Replace,
            variant.as_bytes(),
        )
    }

    #[inline]
    pub fn set_urgent(&self, is_urgent: bool) {
        let mut wm_hints = self
            .xconn
            .get_wm_hints(self.xwindow)
            .expect("`XGetWMHints` failed");
        if is_urgent {
            (*wm_hints).flags |= ffi::XUrgencyHint;
        } else {
            (*wm_hints).flags &= !ffi::XUrgencyHint;
        }
        self.xconn
            .set_wm_hints(self.xwindow, wm_hints)
            .flush()
            .expect("Failed to set urgency hint");
    }

    fn set_netwm(
        &self,
        operation: util::StateOperation,
        properties: (c_long, c_long, c_long, c_long),
    ) -> util::Flusher<'_> {
        let state_atom = unsafe { self.xconn.get_atom_unchecked(b"_NET_WM_STATE\0") };
        self.xconn.send_client_msg(
            self.xwindow,
            self.root,
            state_atom,
            Some(ffi::SubstructureRedirectMask | ffi::SubstructureNotifyMask),
            [
                operation as c_long,
                properties.0,
                properties.1,
                properties.2,
                properties.3,
            ],
        )
    }

    fn set_fullscreen_hint(&self, fullscreen: bool) -> util::Flusher<'_> {
        let fullscreen_atom =
            unsafe { self.xconn.get_atom_unchecked(b"_NET_WM_STATE_FULLSCREEN\0") };
        self.set_netwm(fullscreen.into(), (fullscreen_atom as c_long, 0, 0, 0))
    }

    fn set_fullscreen_inner(&self, fullscreen: Option<Fullscreen>) -> Option<util::Flusher<'_>> {
        let mut shared_state_lock = self.shared_state.lock();
        let old_fullscreen = shared_state_lock.fullscreen.clone();
        if old_fullscreen == fullscreen {
            return None;
        }
        shared_state_lock.fullscreen = fullscreen.clone();

        match (&old_fullscreen, &fullscreen) {
            // Store the desktop video mode before entering exclusive
            // fullscreen, so we can restore it upon exit, as XRandR does not
            // provide a mechanism to set this per app-session or restore this
            // to the desktop video mode as macOS and Windows do
            (
                &None,
                &Some(Fullscreen::Exclusive(RootVideoMode {
                    video_mode: PlatformVideoMode::X(ref video_mode),
                })),
            )
            | (
                &Some(Fullscreen::Borderless(_)),
                &Some(Fullscreen::Exclusive(RootVideoMode {
                    video_mode: PlatformVideoMode::X(ref video_mode),
                })),
            ) => {
                let monitor = video_mode.monitor.as_ref().unwrap();
                shared_state_lock.desktop_video_mode =
                    Some((monitor.id, self.xconn.get_crtc_mode(monitor.id)));
            }
            // Restore desktop video mode upon exiting exclusive fullscreen
            (&Some(Fullscreen::Exclusive(_)), &None)
            | (&Some(Fullscreen::Exclusive(_)), &Some(Fullscreen::Borderless(_))) => {
                let (monitor_id, mode_id) = shared_state_lock.desktop_video_mode.take().unwrap();
                self.xconn
                    .set_crtc_config(monitor_id, mode_id)
                    .expect("failed to restore desktop video mode");
            }
            _ => (),
        }

        drop(shared_state_lock);

        match fullscreen {
            None => {
                let flusher = self.set_fullscreen_hint(false);
                let mut shared_state_lock = self.shared_state.lock();
                if let Some(position) = shared_state_lock.restore_position.take() {
                    self.set_position_inner(position.0, position.1).queue();
                }
                Some(flusher)
            }
            Some(fullscreen) => {
                let (video_mode, monitor) = match fullscreen {
                    Fullscreen::Exclusive(RootVideoMode {
                        video_mode: PlatformVideoMode::X(ref video_mode),
                    }) => (Some(video_mode), video_mode.monitor.as_ref().unwrap()),
                    Fullscreen::Borderless(RootMonitorHandle {
                        inner: PlatformMonitorHandle::X(ref monitor),
                    }) => (None, monitor),
                    _ => unreachable!(),
                };

                if let Some(video_mode) = video_mode {
                    // FIXME: this is actually not correct if we're setting the
                    // video mode to a resolution higher than the current
                    // desktop resolution, because XRandR does not automatically
                    // reposition the monitors to the right and below this
                    // monitor.
                    //
                    // What ends up happening is we will get the fullscreen
                    // window showing up on those monitors as well, because
                    // their virtual position now overlaps with the monitor that
                    // we just made larger..
                    //
                    // It'd be quite a bit of work to handle this correctly (and
                    // nobody else seems to bother doing this correctly either),
                    // so we're just leaving this broken. Fixing this would
                    // involve storing all CRTCs upon entering fullscreen,
                    // restoring them upon exit, and after entering fullscreen,
                    // repositioning displays to the right and below this
                    // display. I think there would still be edge cases that are
                    // difficult or impossible to handle correctly, e.g. what if
                    // a new monitor was plugged in while in fullscreen?
                    //
                    // I think we might just want to disallow setting the video
                    // mode higher than the current desktop video mode (I'm sure
                    // this will make someone unhappy, but it's very unusual for
                    // games to want to do this anyway).
                    self.xconn
                        .set_crtc_config(monitor.id, video_mode.native_mode)
                        .expect("failed to set video mode");
                }

                let window_position = self.outer_position_physical();
                self.shared_state.lock().restore_position = Some(window_position);
                let monitor_origin: (i32, i32) = monitor.position().into();
                self.set_position_inner(monitor_origin.0, monitor_origin.1)
                    .queue();
                Some(self.set_fullscreen_hint(true))
            }
        }
    }

    #[inline]
    pub fn fullscreen(&self) -> Option<Fullscreen> {
        self.shared_state.lock().fullscreen.clone()
    }

    #[inline]
    pub fn set_fullscreen(&self, fullscreen: Option<Fullscreen>) {
        if let Some(flusher) = self.set_fullscreen_inner(fullscreen) {
            flusher
                .flush()
                .expect("Failed to change window fullscreen state");
            self.invalidate_cached_frame_extents();
        }
    }

    fn get_rect(&self) -> util::AaRect {
        // TODO: This might round-trip more times than needed.
        let position = self.outer_position_physical();
        let size = self.outer_size_physical();
        util::AaRect::new(position, size)
    }

    #[inline]
    pub fn current_monitor(&self) -> X11MonitorHandle {
        let monitor = self.shared_state.lock().last_monitor.as_ref().cloned();
        monitor.unwrap_or_else(|| {
            let monitor = self
                .xconn
                .get_monitor_for_window(Some(self.get_rect()))
                .to_owned();
            self.shared_state.lock().last_monitor = Some(monitor.clone());
            monitor
        })
    }

    pub fn available_monitors(&self) -> Vec<X11MonitorHandle> {
        self.xconn.available_monitors()
    }

    pub fn primary_monitor(&self) -> X11MonitorHandle {
        self.xconn.primary_monitor()
    }

    fn set_maximized_inner(&self, maximized: bool) -> util::Flusher<'_> {
        let horz_atom = unsafe {
            self.xconn
                .get_atom_unchecked(b"_NET_WM_STATE_MAXIMIZED_HORZ\0")
        };
        let vert_atom = unsafe {
            self.xconn
                .get_atom_unchecked(b"_NET_WM_STATE_MAXIMIZED_VERT\0")
        };
        self.set_netwm(
            maximized.into(),
            (horz_atom as c_long, vert_atom as c_long, 0, 0),
        )
    }

    #[inline]
    pub fn set_maximized(&self, maximized: bool) {
        self.set_maximized_inner(maximized)
            .flush()
            .expect("Failed to change window maximization");
        self.invalidate_cached_frame_extents();
    }

    fn set_title_inner(&self, title: &str) -> util::Flusher<'_> {
        let wm_name_atom = unsafe { self.xconn.get_atom_unchecked(b"_NET_WM_NAME\0") };
        let utf8_atom = unsafe { self.xconn.get_atom_unchecked(b"UTF8_STRING\0") };
        let title = CString::new(title).expect("Window title contained null byte");
        unsafe {
            (self.xconn.xlib.XStoreName)(
                self.xconn.display,
                self.xwindow,
                title.as_ptr() as *const c_char,
            );
            self.xconn.change_property(
                self.xwindow,
                wm_name_atom,
                utf8_atom,
                util::PropMode::Replace,
                title.as_bytes(),
            )
        }
    }

    #[inline]
    pub fn set_title(&self, title: &str) {
        self.set_title_inner(title)
            .flush()
            .expect("Failed to set window title");
    }

    fn set_decorations_inner(&self, decorations: bool) -> util::Flusher<'_> {
        let mut hints = self.xconn.get_motif_hints(self.xwindow);

        hints.set_decorations(decorations);

        self.xconn.set_motif_hints(self.xwindow, &hints)
    }

    #[inline]
    pub fn set_decorations(&self, decorations: bool) {
        self.set_decorations_inner(decorations)
            .flush()
            .expect("Failed to set decoration state");
        self.invalidate_cached_frame_extents();
    }

    fn set_maximizable_inner(&self, maximizable: bool) -> util::Flusher<'_> {
        let mut hints = self.xconn.get_motif_hints(self.xwindow);

        hints.set_maximizable(maximizable);

        self.xconn.set_motif_hints(self.xwindow, &hints)
    }

    fn set_always_on_top_inner(&self, always_on_top: bool) -> util::Flusher<'_> {
        let above_atom = unsafe { self.xconn.get_atom_unchecked(b"_NET_WM_STATE_ABOVE\0") };
        self.set_netwm(always_on_top.into(), (above_atom as c_long, 0, 0, 0))
    }

    #[inline]
    pub fn set_always_on_top(&self, always_on_top: bool) {
        self.set_always_on_top_inner(always_on_top)
            .flush()
            .expect("Failed to set always-on-top state");
    }

    fn set_icon_inner(&self, icon: Icon) -> util::Flusher<'_> {
        let icon_atom = unsafe { self.xconn.get_atom_unchecked(b"_NET_WM_ICON\0") };
        let data = icon.to_cardinals();
        self.xconn.change_property(
            self.xwindow,
            icon_atom,
            ffi::XA_CARDINAL,
            util::PropMode::Replace,
            data.as_slice(),
        )
    }

    fn unset_icon_inner(&self) -> util::Flusher<'_> {
        let icon_atom = unsafe { self.xconn.get_atom_unchecked(b"_NET_WM_ICON\0") };
        let empty_data: [util::Cardinal; 0] = [];
        self.xconn.change_property(
            self.xwindow,
            icon_atom,
            ffi::XA_CARDINAL,
            util::PropMode::Replace,
            &empty_data,
        )
    }

    #[inline]
    pub fn set_window_icon(&self, icon: Option<Icon>) {
        match icon {
            Some(icon) => self.set_icon_inner(icon),
            None => self.unset_icon_inner(),
        }
        .flush()
        .expect("Failed to set icons");
    }

    #[inline]
    pub fn set_visible(&self, visible: bool) {
        match visible {
            true => unsafe {
                (self.xconn.xlib.XMapRaised)(self.xconn.display, self.xwindow);
                self.xconn
                    .flush_requests()
                    .expect("Failed to call XMapRaised");
            },
            false => unsafe {
                (self.xconn.xlib.XUnmapWindow)(self.xconn.display, self.xwindow);
                self.xconn
                    .flush_requests()
                    .expect("Failed to call XUnmapWindow");
            },
        }
    }

    fn update_cached_frame_extents(&self) {
        let extents = self
            .xconn
            .get_frame_extents_heuristic(self.xwindow, self.root);
        (*self.shared_state.lock()).frame_extents = Some(extents);
    }

    pub(crate) fn invalidate_cached_frame_extents(&self) {
        (*self.shared_state.lock()).frame_extents.take();
    }

    pub(crate) fn outer_position_physical(&self) -> (i32, i32) {
        let extents = (*self.shared_state.lock()).frame_extents.clone();
        if let Some(extents) = extents {
            let (x, y) = self.inner_position_physical();
            extents.inner_pos_to_outer(x, y)
        } else {
            self.update_cached_frame_extents();
            self.outer_position_physical()
        }
    }

    #[inline]
    pub fn outer_position(&self) -> Result<LogicalPosition, NotSupportedError> {
        let extents = (*self.shared_state.lock()).frame_extents.clone();
        if let Some(extents) = extents {
            let logical = self.inner_position().unwrap();
            Ok(extents.inner_pos_to_outer_logical(logical, self.hidpi_factor()))
        } else {
            self.update_cached_frame_extents();
            self.outer_position()
        }
    }

    pub(crate) fn inner_position_physical(&self) -> (i32, i32) {
        // This should be okay to unwrap since the only error XTranslateCoordinates can return
        // is BadWindow, and if the window handle is bad we have bigger problems.
        self.xconn
            .translate_coords(self.xwindow, self.root)
            .map(|coords| (coords.x_rel_root, coords.y_rel_root))
            .unwrap()
    }

    #[inline]
    pub fn inner_position(&self) -> Result<LogicalPosition, NotSupportedError> {
        Ok(self.logicalize_coords(self.inner_position_physical()))
    }

    pub(crate) fn set_position_inner(&self, mut x: i32, mut y: i32) -> util::Flusher<'_> {
        // There are a few WMs that set client area position rather than window position, so
        // we'll translate for consistency.
        if util::wm_name_is_one_of(&["Enlightenment", "FVWM"]) {
            let extents = (*self.shared_state.lock()).frame_extents.clone();
            if let Some(extents) = extents {
                x += extents.frame_extents.left as i32;
                y += extents.frame_extents.top as i32;
            } else {
                self.update_cached_frame_extents();
                return self.set_position_inner(x, y);
            }
        }
        unsafe {
            (self.xconn.xlib.XMoveWindow)(self.xconn.display, self.xwindow, x as c_int, y as c_int);
        }
        util::Flusher::new(&self.xconn)
    }

    pub(crate) fn set_position_physical(&self, x: i32, y: i32) {
        self.set_position_inner(x, y)
            .flush()
            .expect("Failed to call `XMoveWindow`");
    }

    #[inline]
    pub fn set_outer_position(&self, logical_position: LogicalPosition) {
        let (x, y) = logical_position.to_physical(self.hidpi_factor()).into();
        self.set_position_physical(x, y);
    }

    pub(crate) fn inner_size_physical(&self) -> (u32, u32) {
        // This should be okay to unwrap since the only error XGetGeometry can return
        // is BadWindow, and if the window handle is bad we have bigger problems.
        self.xconn
            .get_geometry(self.xwindow)
            .map(|geo| (geo.width, geo.height))
            .unwrap()
    }

    #[inline]
    pub fn inner_size(&self) -> LogicalSize {
        self.logicalize_size(self.inner_size_physical())
    }

    pub(crate) fn outer_size_physical(&self) -> (u32, u32) {
        let extents = self.shared_state.lock().frame_extents.clone();
        if let Some(extents) = extents {
            let (w, h) = self.inner_size_physical();
            extents.inner_size_to_outer(w, h)
        } else {
            self.update_cached_frame_extents();
            self.outer_size_physical()
        }
    }

    #[inline]
    pub fn outer_size(&self) -> LogicalSize {
        let extents = self.shared_state.lock().frame_extents.clone();
        if let Some(extents) = extents {
            let logical = self.inner_size();
            extents.inner_size_to_outer_logical(logical, self.hidpi_factor())
        } else {
            self.update_cached_frame_extents();
            self.outer_size()
        }
    }

    pub(crate) fn set_inner_size_physical(&self, width: u32, height: u32) {
        unsafe {
            (self.xconn.xlib.XResizeWindow)(
                self.xconn.display,
                self.xwindow,
                width as c_uint,
                height as c_uint,
            );
            self.xconn.flush_requests()
        }
        .expect("Failed to call `XResizeWindow`");
    }

    #[inline]
    pub fn set_inner_size(&self, logical_size: LogicalSize) {
        let dpi_factor = self.hidpi_factor();
        let (width, height) = logical_size.to_physical(dpi_factor).into();
        self.set_inner_size_physical(width, height);
    }

    fn update_normal_hints<F>(&self, callback: F) -> Result<(), XError>
    where
        F: FnOnce(&mut util::NormalHints<'_>) -> (),
    {
        let mut normal_hints = self.xconn.get_normal_hints(self.xwindow)?;
        callback(&mut normal_hints);
        self.xconn
            .set_normal_hints(self.xwindow, normal_hints)
            .flush()
    }

    pub(crate) fn set_min_inner_size_physical(&self, dimensions: Option<(u32, u32)>) {
        self.update_normal_hints(|normal_hints| normal_hints.set_min_size(dimensions))
            .expect("Failed to call `XSetWMNormalHints`");
    }

    #[inline]
    pub fn set_min_inner_size(&self, logical_dimensions: Option<LogicalSize>) {
        self.shared_state.lock().min_inner_size = logical_dimensions;
        let physical_dimensions = logical_dimensions
            .map(|logical_dimensions| logical_dimensions.to_physical(self.hidpi_factor()).into());
        self.set_min_inner_size_physical(physical_dimensions);
    }

    pub(crate) fn set_max_inner_size_physical(&self, dimensions: Option<(u32, u32)>) {
        self.update_normal_hints(|normal_hints| normal_hints.set_max_size(dimensions))
            .expect("Failed to call `XSetWMNormalHints`");
    }

    #[inline]
    pub fn set_max_inner_size(&self, logical_dimensions: Option<LogicalSize>) {
        self.shared_state.lock().max_inner_size = logical_dimensions;
        let physical_dimensions = logical_dimensions
            .map(|logical_dimensions| logical_dimensions.to_physical(self.hidpi_factor()).into());
        self.set_max_inner_size_physical(physical_dimensions);
    }

    pub(crate) fn adjust_for_dpi(
        &self,
        old_dpi_factor: f64,
        new_dpi_factor: f64,
        width: f64,
        height: f64,
    ) -> (f64, f64, util::Flusher<'_>) {
        let scale_factor = new_dpi_factor / old_dpi_factor;
        let new_width = width * scale_factor;
        let new_height = height * scale_factor;
        self.update_normal_hints(|normal_hints| {
            let dpi_adjuster = |(width, height): (u32, u32)| -> (u32, u32) {
                let new_width = width as f64 * scale_factor;
                let new_height = height as f64 * scale_factor;
                (new_width.round() as u32, new_height.round() as u32)
            };
            let max_size = normal_hints.get_max_size().map(&dpi_adjuster);
            let min_size = normal_hints.get_min_size().map(&dpi_adjuster);
            let resize_increments = normal_hints.get_resize_increments().map(&dpi_adjuster);
            let base_size = normal_hints.get_base_size().map(&dpi_adjuster);
            normal_hints.set_max_size(max_size);
            normal_hints.set_min_size(min_size);
            normal_hints.set_resize_increments(resize_increments);
            normal_hints.set_base_size(base_size);
        })
        .expect("Failed to update normal hints");
        unsafe {
            (self.xconn.xlib.XResizeWindow)(
                self.xconn.display,
                self.xwindow,
                new_width.round() as c_uint,
                new_height.round() as c_uint,
            );
        }
        (new_width, new_height, util::Flusher::new(&self.xconn))
    }

    pub fn set_resizable(&self, resizable: bool) {
        if util::wm_name_is_one_of(&["Xfwm4"]) {
            // Making the window unresizable on Xfwm prevents further changes to `WM_NORMAL_HINTS` from being detected.
            // This makes it impossible for resizing to be re-enabled, and also breaks DPI scaling. As such, we choose
            // the lesser of two evils and do nothing.
            warn!("To avoid a WM bug, disabling resizing has no effect on Xfwm4");
            return;
        }

        let (logical_min, logical_max) = if resizable {
            let shared_state_lock = self.shared_state.lock();
            (
                shared_state_lock.min_inner_size,
                shared_state_lock.max_inner_size,
            )
        } else {
            let window_size = Some(self.inner_size());
            (window_size.clone(), window_size)
        };

        self.set_maximizable_inner(resizable).queue();

        let dpi_factor = self.hidpi_factor();
        let min_inner_size = logical_min
            .map(|logical_size| logical_size.to_physical(dpi_factor))
            .map(Into::into);
        let max_inner_size = logical_max
            .map(|logical_size| logical_size.to_physical(dpi_factor))
            .map(Into::into);
        self.update_normal_hints(|normal_hints| {
            normal_hints.set_min_size(min_inner_size);
            normal_hints.set_max_size(max_inner_size);
        })
        .expect("Failed to call `XSetWMNormalHints`");
    }

    #[inline]
    pub fn xlib_display(&self) -> *mut c_void {
        self.xconn.display as _
    }

    #[inline]
    pub fn xlib_screen_id(&self) -> c_int {
        self.screen_id
    }

    #[inline]
    pub fn xlib_xconnection(&self) -> Arc<XConnection> {
        Arc::clone(&self.xconn)
    }

    #[inline]
    pub fn xlib_window(&self) -> c_ulong {
        self.xwindow
    }

    #[inline]
    pub fn xcb_connection(&self) -> *mut c_void {
        unsafe { (self.xconn.xlib_xcb.XGetXCBConnection)(self.xconn.display) as *mut _ }
    }

    fn load_cursor(&self, name: &[u8]) -> ffi::Cursor {
        unsafe {
            (self.xconn.xcursor.XcursorLibraryLoadCursor)(
                self.xconn.display,
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

    fn get_cursor(&self, cursor: CursorIcon) -> ffi::Cursor {
        let load = |name: &[u8]| self.load_cursor(name);

        let loadn = |names: &[&[u8]]| self.load_first_existing_cursor(names);

        // Try multiple names in some cases where the name
        // differs on the desktop environments or themes.
        //
        // Try the better looking (or more suiting) names first.
        match cursor {
            CursorIcon::Alias => load(b"link\0"),
            CursorIcon::Arrow => load(b"arrow\0"),
            CursorIcon::Cell => load(b"plus\0"),
            CursorIcon::Copy => load(b"copy\0"),
            CursorIcon::Crosshair => load(b"crosshair\0"),
            CursorIcon::Default => load(b"left_ptr\0"),
            CursorIcon::Hand => loadn(&[b"hand2\0", b"hand1\0"]),
            CursorIcon::Help => load(b"question_arrow\0"),
            CursorIcon::Move => load(b"move\0"),
            CursorIcon::Grab => loadn(&[b"openhand\0", b"grab\0"]),
            CursorIcon::Grabbing => loadn(&[b"closedhand\0", b"grabbing\0"]),
            CursorIcon::Progress => load(b"left_ptr_watch\0"),
            CursorIcon::AllScroll => load(b"all-scroll\0"),
            CursorIcon::ContextMenu => load(b"context-menu\0"),

            CursorIcon::NoDrop => loadn(&[b"no-drop\0", b"circle\0"]),
            CursorIcon::NotAllowed => load(b"crossed_circle\0"),

            // Resize cursors
            CursorIcon::EResize => load(b"right_side\0"),
            CursorIcon::NResize => load(b"top_side\0"),
            CursorIcon::NeResize => load(b"top_right_corner\0"),
            CursorIcon::NwResize => load(b"top_left_corner\0"),
            CursorIcon::SResize => load(b"bottom_side\0"),
            CursorIcon::SeResize => load(b"bottom_right_corner\0"),
            CursorIcon::SwResize => load(b"bottom_left_corner\0"),
            CursorIcon::WResize => load(b"left_side\0"),
            CursorIcon::EwResize => load(b"h_double_arrow\0"),
            CursorIcon::NsResize => load(b"v_double_arrow\0"),
            CursorIcon::NwseResize => loadn(&[b"bd_double_arrow\0", b"size_bdiag\0"]),
            CursorIcon::NeswResize => loadn(&[b"fd_double_arrow\0", b"size_fdiag\0"]),
            CursorIcon::ColResize => loadn(&[b"split_h\0", b"h_double_arrow\0"]),
            CursorIcon::RowResize => loadn(&[b"split_v\0", b"v_double_arrow\0"]),

            CursorIcon::Text => loadn(&[b"text\0", b"xterm\0"]),
            CursorIcon::VerticalText => load(b"vertical-text\0"),

            CursorIcon::Wait => load(b"watch\0"),

            CursorIcon::ZoomIn => load(b"zoom-in\0"),
            CursorIcon::ZoomOut => load(b"zoom-out\0"),
        }
    }

    fn update_cursor(&self, cursor: ffi::Cursor) {
        unsafe {
            (self.xconn.xlib.XDefineCursor)(self.xconn.display, self.xwindow, cursor);
            if cursor != 0 {
                (self.xconn.xlib.XFreeCursor)(self.xconn.display, cursor);
            }
            self.xconn
                .flush_requests()
                .expect("Failed to set or free the cursor");
        }
    }

    #[inline]
    pub fn set_cursor_icon(&self, cursor: CursorIcon) {
        *self.cursor.lock() = cursor;
        if *self.cursor_visible.lock() {
            self.update_cursor(self.get_cursor(cursor));
        }
    }

    // TODO: This could maybe be cached. I don't think it's worth
    // the complexity, since cursor changes are not so common,
    // and this is just allocating a 1x1 pixmap...
    fn create_empty_cursor(&self) -> Option<ffi::Cursor> {
        let data = 0;
        let pixmap = unsafe {
            (self.xconn.xlib.XCreateBitmapFromData)(self.xconn.display, self.xwindow, &data, 1, 1)
        };
        if pixmap == 0 {
            // Failed to allocate
            return None;
        }

        let cursor = unsafe {
            // We don't care about this color, since it only fills bytes
            // in the pixmap which are not 0 in the mask.
            let mut dummy_color = MaybeUninit::uninit();
            let cursor = (self.xconn.xlib.XCreatePixmapCursor)(
                self.xconn.display,
                pixmap,
                pixmap,
                dummy_color.as_mut_ptr(),
                dummy_color.as_mut_ptr(),
                0,
                0,
            );
            (self.xconn.xlib.XFreePixmap)(self.xconn.display, pixmap);
            cursor
        };
        Some(cursor)
    }

    #[inline]
    pub fn set_cursor_grab(&self, grab: bool) -> Result<(), ExternalError> {
        let mut grabbed_lock = self.cursor_grabbed.lock();
        if grab == *grabbed_lock {
            return Ok(());
        }
        unsafe {
            // We ungrab before grabbing to prevent passive grabs from causing `AlreadyGrabbed`.
            // Therefore, this is common to both codepaths.
            (self.xconn.xlib.XUngrabPointer)(self.xconn.display, ffi::CurrentTime);
        }
        let result = if grab {
            let result = unsafe {
                (self.xconn.xlib.XGrabPointer)(
                    self.xconn.display,
                    self.xwindow,
                    ffi::True,
                    (ffi::ButtonPressMask
                        | ffi::ButtonReleaseMask
                        | ffi::EnterWindowMask
                        | ffi::LeaveWindowMask
                        | ffi::PointerMotionMask
                        | ffi::PointerMotionHintMask
                        | ffi::Button1MotionMask
                        | ffi::Button2MotionMask
                        | ffi::Button3MotionMask
                        | ffi::Button4MotionMask
                        | ffi::Button5MotionMask
                        | ffi::ButtonMotionMask
                        | ffi::KeymapStateMask) as c_uint,
                    ffi::GrabModeAsync,
                    ffi::GrabModeAsync,
                    self.xwindow,
                    0,
                    ffi::CurrentTime,
                )
            };

            match result {
                ffi::GrabSuccess => Ok(()),
                ffi::AlreadyGrabbed => {
                    Err("Cursor could not be grabbed: already grabbed by another client")
                }
                ffi::GrabInvalidTime => Err("Cursor could not be grabbed: invalid time"),
                ffi::GrabNotViewable => {
                    Err("Cursor could not be grabbed: grab location not viewable")
                }
                ffi::GrabFrozen => Err("Cursor could not be grabbed: frozen by another client"),
                _ => unreachable!(),
            }
            .map_err(|err| ExternalError::Os(os_error!(OsError::XMisc(err))))
        } else {
            self.xconn
                .flush_requests()
                .map_err(|err| ExternalError::Os(os_error!(OsError::XError(err))))
        };
        if result.is_ok() {
            *grabbed_lock = grab;
        }
        result
    }

    #[inline]
    pub fn set_cursor_visible(&self, visible: bool) {
        let mut visible_lock = self.cursor_visible.lock();
        if visible == *visible_lock {
            return;
        }
        let cursor = if visible {
            self.get_cursor(*self.cursor.lock())
        } else {
            self.create_empty_cursor()
                .expect("Failed to create empty cursor")
        };
        *visible_lock = visible;
        drop(visible_lock);
        self.update_cursor(cursor);
    }

    #[inline]
    pub fn hidpi_factor(&self) -> f64 {
        self.current_monitor().hidpi_factor
    }

    pub fn set_cursor_position_physical(&self, x: i32, y: i32) -> Result<(), ExternalError> {
        unsafe {
            (self.xconn.xlib.XWarpPointer)(self.xconn.display, 0, self.xwindow, 0, 0, 0, 0, x, y);
            self.xconn
                .flush_requests()
                .map_err(|e| ExternalError::Os(os_error!(OsError::XError(e))))
        }
    }

    #[inline]
    pub fn set_cursor_position(
        &self,
        logical_position: LogicalPosition,
    ) -> Result<(), ExternalError> {
        let (x, y) = logical_position.to_physical(self.hidpi_factor()).into();
        self.set_cursor_position_physical(x, y)
    }

    pub(crate) fn set_ime_position_physical(&self, x: i32, y: i32) {
        let _ = self
            .ime_sender
            .lock()
            .send((self.xwindow, x as i16, y as i16));
    }

    #[inline]
    pub fn set_ime_position(&self, logical_spot: LogicalPosition) {
        let (x, y) = logical_spot.to_physical(self.hidpi_factor()).into();
        self.set_ime_position_physical(x, y);
    }

    #[inline]
    pub fn id(&self) -> WindowId {
        WindowId(self.xwindow)
    }

    #[inline]
    pub fn request_redraw(&self) {
        self.pending_redraws
            .lock()
            .unwrap()
            .insert(WindowId(self.xwindow));
    }
}
