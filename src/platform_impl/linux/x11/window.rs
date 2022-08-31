use std::{
    cmp, env,
    ffi::CString,
    mem::{self, replace, MaybeUninit},
    os::raw::*,
    path::Path,
    ptr, slice,
    sync::{Arc, Mutex, MutexGuard},
};

use libc;
use raw_window_handle::{RawDisplayHandle, RawWindowHandle, XlibDisplayHandle, XlibWindowHandle};
use x11_dl::xlib::TrueColor;

use crate::{
    dpi::{PhysicalPosition, PhysicalSize, Position, Size},
    error::{ExternalError, NotSupportedError, OsError as RootOsError},
    monitor::{MonitorHandle as RootMonitorHandle, VideoMode as RootVideoMode},
    platform_impl::{
        x11::{ime::ImeContextCreationError, MonitorHandle as X11MonitorHandle},
        MonitorHandle as PlatformMonitorHandle, OsError, PlatformSpecificWindowBuilderAttributes,
        VideoMode as PlatformVideoMode,
    },
    window::{CursorGrabMode, CursorIcon, Fullscreen, Icon, UserAttentionType, WindowAttributes},
};

use super::{
    ffi, util, EventLoopWindowTarget, ImeRequest, ImeSender, WakeSender, WindowId, XConnection,
    XError,
};

#[derive(Debug)]
pub struct SharedState {
    pub cursor_pos: Option<(f64, f64)>,
    pub size: Option<(u32, u32)>,
    pub position: Option<(i32, i32)>,
    pub inner_position: Option<(i32, i32)>,
    pub inner_position_rel_parent: Option<(i32, i32)>,
    pub is_resizable: bool,
    pub is_decorated: bool,
    pub last_monitor: X11MonitorHandle,
    pub dpi_adjusted: Option<(u32, u32)>,
    pub fullscreen: Option<Fullscreen>,
    // Set when application calls `set_fullscreen` when window is not visible
    pub desired_fullscreen: Option<Option<Fullscreen>>,
    // Used to restore position after exiting fullscreen
    pub restore_position: Option<(i32, i32)>,
    // Used to restore video mode after exiting fullscreen
    pub desktop_video_mode: Option<(ffi::RRCrtc, ffi::RRMode)>,
    pub frame_extents: Option<util::FrameExtentsHeuristic>,
    pub min_inner_size: Option<Size>,
    pub max_inner_size: Option<Size>,
    pub resize_increments: Option<Size>,
    pub base_size: Option<Size>,
    pub visibility: Visibility,
}

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum Visibility {
    No,
    Yes,
    // Waiting for VisibilityNotify
    YesWait,
}

impl SharedState {
    fn new(last_monitor: X11MonitorHandle, window_attributes: &WindowAttributes) -> Mutex<Self> {
        let visibility = if window_attributes.visible {
            Visibility::YesWait
        } else {
            Visibility::No
        };

        Mutex::new(SharedState {
            last_monitor,
            visibility,

            is_resizable: window_attributes.resizable,
            is_decorated: window_attributes.decorations,
            cursor_pos: None,
            size: None,
            position: None,
            inner_position: None,
            inner_position_rel_parent: None,
            dpi_adjusted: None,
            fullscreen: None,
            desired_fullscreen: None,
            restore_position: None,
            desktop_video_mode: None,
            frame_extents: None,
            min_inner_size: None,
            max_inner_size: None,
            resize_increments: None,
            base_size: None,
        })
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
    cursor_grabbed_mode: Mutex<CursorGrabMode>,
    #[allow(clippy::mutex_atomic)]
    cursor_visible: Mutex<bool>,
    ime_sender: Mutex<ImeSender>,
    pub shared_state: Mutex<SharedState>,
    redraw_sender: WakeSender<WindowId>,
}

impl UnownedWindow {
    pub(crate) fn new<T>(
        event_loop: &EventLoopWindowTarget<T>,
        window_attrs: WindowAttributes,
        pl_attribs: PlatformSpecificWindowBuilderAttributes,
    ) -> Result<UnownedWindow, RootOsError> {
        let xconn = &event_loop.xconn;
        let root = event_loop.root;

        let mut monitors = xconn.available_monitors();
        let guessed_monitor = if monitors.is_empty() {
            X11MonitorHandle::dummy()
        } else {
            xconn
                .query_pointer(root, util::VIRTUAL_CORE_POINTER)
                .ok()
                .and_then(|pointer_state| {
                    let (x, y) = (pointer_state.root_x as i64, pointer_state.root_y as i64);

                    for i in 0..monitors.len() {
                        if monitors[i].rect.contains_point(x, y) {
                            return Some(monitors.swap_remove(i));
                        }
                    }

                    None
                })
                .unwrap_or_else(|| monitors.swap_remove(0))
        };
        let scale_factor = guessed_monitor.scale_factor();

        info!("Guessed window scale factor: {}", scale_factor);

        let max_inner_size: Option<(u32, u32)> = window_attrs
            .max_inner_size
            .map(|size| size.to_physical::<u32>(scale_factor).into());
        let min_inner_size: Option<(u32, u32)> = window_attrs
            .min_inner_size
            .map(|size| size.to_physical::<u32>(scale_factor).into());

        let position = window_attrs
            .position
            .map(|position| position.to_physical::<i32>(scale_factor));

        let dimensions = {
            // x11 only applies constraints when the window is actively resized
            // by the user, so we have to manually apply the initial constraints
            let mut dimensions: (u32, u32) = window_attrs
                .inner_size
                .map(|size| size.to_physical::<u32>(scale_factor))
                .or_else(|| Some((800, 600).into()))
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
        let (visual, depth, require_colormap) = match pl_attribs.visual_infos {
            Some(vi) => (vi.visual, vi.depth, false),
            None if window_attrs.transparent => {
                // Find a suitable visual
                let mut vinfo = MaybeUninit::uninit();
                let vinfo_initialized = unsafe {
                    (xconn.xlib.XMatchVisualInfo)(
                        xconn.display,
                        screen_id,
                        32,
                        TrueColor,
                        vinfo.as_mut_ptr(),
                    ) != 0
                };
                if vinfo_initialized {
                    let vinfo = unsafe { vinfo.assume_init() };
                    (vinfo.visual, vinfo.depth, true)
                } else {
                    debug!("Could not set transparency, because XMatchVisualInfo returned zero for the required parameters");
                    (
                        ffi::CopyFromParent as *mut ffi::Visual,
                        ffi::CopyFromParent,
                        false,
                    )
                }
            }
            _ => (
                ffi::CopyFromParent as *mut ffi::Visual,
                ffi::CopyFromParent,
                false,
            ),
        };

        let mut set_win_attr = {
            let mut swa: ffi::XSetWindowAttributes = unsafe { mem::zeroed() };
            swa.colormap = if let Some(vi) = pl_attribs.visual_infos {
                unsafe {
                    let visual = vi.visual;
                    (xconn.xlib.XCreateColormap)(xconn.display, root, visual, ffi::AllocNone)
                }
            } else if require_colormap {
                unsafe { (xconn.xlib.XCreateColormap)(xconn.display, root, visual, ffi::AllocNone) }
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
                position.map_or(0, |p: PhysicalPosition<i32>| p.x as c_int),
                position.map_or(0, |p: PhysicalPosition<i32>| p.y as c_int),
                dimensions.0 as c_uint,
                dimensions.1 as c_uint,
                0,
                depth,
                ffi::InputOutput as c_uint,
                visual,
                window_attributes,
                &mut set_win_attr,
            )
        };

        #[allow(clippy::mutex_atomic)]
        let mut window = UnownedWindow {
            xconn: Arc::clone(xconn),
            xwindow,
            root,
            screen_id,
            cursor: Default::default(),
            cursor_grabbed_mode: Mutex::new(CursorGrabMode::None),
            cursor_visible: Mutex::new(true),
            ime_sender: Mutex::new(event_loop.ime_sender.clone()),
            shared_state: SharedState::new(guessed_monitor, &window_attrs),
            redraw_sender: WakeSender {
                waker: event_loop.redraw_sender.waker.clone(),
                sender: event_loop.redraw_sender.sender.clone(),
            },
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
                let (class, instance) = if let Some(name) = pl_attribs.name {
                    let instance = CString::new(name.instance.as_str())
                        .expect("`WM_CLASS` instance contained null byte");
                    let class = CString::new(name.general.as_str())
                        .expect("`WM_CLASS` class contained null byte");
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

            if let Some(flusher) = window.set_pid() {
                flusher.queue()
            }

            window.set_window_types(pl_attribs.x11_window_types).queue();

            if let Some(variant) = pl_attribs.gtk_theme_variant {
                window.set_gtk_theme_variant(variant).queue();
            }

            // set size hints
            {
                let mut min_inner_size = window_attrs
                    .min_inner_size
                    .map(|size| size.to_physical::<u32>(scale_factor));
                let mut max_inner_size = window_attrs
                    .max_inner_size
                    .map(|size| size.to_physical::<u32>(scale_factor));

                if !window_attrs.resizable {
                    if util::wm_name_is_one_of(&["Xfwm4"]) {
                        warn!("To avoid a WM bug, disabling resizing has no effect on Xfwm4");
                    } else {
                        max_inner_size = Some(dimensions.into());
                        min_inner_size = Some(dimensions.into());
                    }
                }

                let mut shared_state = window.shared_state.get_mut().unwrap();
                shared_state.min_inner_size = min_inner_size.map(Into::into);
                shared_state.max_inner_size = max_inner_size.map(Into::into);
                shared_state.resize_increments = pl_attribs.resize_increments;
                shared_state.base_size = pl_attribs.base_size;

                let mut normal_hints = util::NormalHints::new(xconn);
                normal_hints.set_position(position.map(|PhysicalPosition { x, y }| (x, y)));
                normal_hints.set_size(Some(dimensions));
                normal_hints.set_min_size(min_inner_size.map(Into::into));
                normal_hints.set_max_size(max_inner_size.map(Into::into));
                normal_hints.set_resize_increments(
                    pl_attribs
                        .resize_increments
                        .map(|size| size.to_physical::<u32>(scale_factor).into()),
                );
                normal_hints.set_base_size(
                    pl_attribs
                        .base_size
                        .map(|size| size.to_physical::<u32>(scale_factor).into()),
                );
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
            xconn
                .select_xinput_events(window.xwindow, ffi::XIAllMasterDevices, mask)
                .queue();

            {
                let result = event_loop
                    .ime
                    .borrow_mut()
                    .create_context(window.xwindow, false);
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
                if let Some(flusher) = window.set_fullscreen_inner(window_attrs.fullscreen.clone())
                {
                    flusher.queue()
                }

                if let Some(PhysicalPosition { x, y }) = position {
                    let shared_state = window.shared_state.get_mut().unwrap();

                    shared_state.restore_position = Some((x, y));
                }
            }
            if window_attrs.always_on_top {
                window
                    .set_always_on_top_inner(window_attrs.always_on_top)
                    .queue();
            }
        }

        // We never want to give the user a broken window, since by then, it's too late to handle.
        xconn
            .sync_with_server()
            .map(|_| window)
            .map_err(|x_err| os_error!(OsError::XError(x_err)))
    }

    pub(super) fn shared_state_lock(&self) -> MutexGuard<'_, SharedState> {
        self.shared_state.lock().unwrap()
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

    fn set_window_types(&self, window_types: Vec<util::WindowType>) -> util::Flusher<'_> {
        let hint_atom = unsafe { self.xconn.get_atom_unchecked(b"_NET_WM_WINDOW_TYPE\0") };
        let atoms: Vec<_> = window_types
            .iter()
            .map(|t| t.as_atom(&self.xconn))
            .collect();

        self.xconn.change_property(
            self.xwindow,
            hint_atom,
            ffi::XA_ATOM,
            util::PropMode::Replace,
            &atoms,
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
        let flusher = self.set_netwm(fullscreen.into(), (fullscreen_atom as c_long, 0, 0, 0));

        if fullscreen {
            // Ensure that the fullscreen window receives input focus to prevent
            // locking up the user's display.
            unsafe {
                (self.xconn.xlib.XSetInputFocus)(
                    self.xconn.display,
                    self.xwindow,
                    ffi::RevertToParent,
                    ffi::CurrentTime,
                );
            }
        }

        flusher
    }

    fn set_fullscreen_inner(&self, fullscreen: Option<Fullscreen>) -> Option<util::Flusher<'_>> {
        let mut shared_state_lock = self.shared_state_lock();

        match shared_state_lock.visibility {
            // Setting fullscreen on a window that is not visible will generate an error.
            Visibility::No | Visibility::YesWait => {
                shared_state_lock.desired_fullscreen = Some(fullscreen);
                return None;
            }
            Visibility::Yes => (),
        }

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
                let mut shared_state_lock = self.shared_state_lock();
                if let Some(position) = shared_state_lock.restore_position.take() {
                    drop(shared_state_lock);
                    self.set_position_inner(position.0, position.1).queue();
                }
                Some(flusher)
            }
            Some(fullscreen) => {
                let (video_mode, monitor) = match fullscreen {
                    Fullscreen::Exclusive(RootVideoMode {
                        video_mode: PlatformVideoMode::X(ref video_mode),
                    }) => (Some(video_mode), video_mode.monitor.clone().unwrap()),
                    Fullscreen::Borderless(Some(RootMonitorHandle {
                        inner: PlatformMonitorHandle::X(monitor),
                    })) => (None, monitor),
                    Fullscreen::Borderless(None) => (None, self.current_monitor()),
                    #[cfg(feature = "wayland")]
                    _ => unreachable!(),
                };

                // Don't set fullscreen on an invalid dummy monitor handle
                if monitor.is_dummy() {
                    return None;
                }

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
                self.shared_state_lock().restore_position = Some(window_position);
                let monitor_origin: (i32, i32) = monitor.position().into();
                self.set_position_inner(monitor_origin.0, monitor_origin.1)
                    .queue();
                Some(self.set_fullscreen_hint(true))
            }
        }
    }

    #[inline]
    pub fn fullscreen(&self) -> Option<Fullscreen> {
        let shared_state = self.shared_state_lock();

        shared_state
            .desired_fullscreen
            .clone()
            .unwrap_or_else(|| shared_state.fullscreen.clone())
    }

    #[inline]
    pub fn set_fullscreen(&self, fullscreen: Option<Fullscreen>) {
        if let Some(flusher) = self.set_fullscreen_inner(fullscreen) {
            flusher
                .sync()
                .expect("Failed to change window fullscreen state");
            self.invalidate_cached_frame_extents();
        }
    }

    // Called by EventProcessor when a VisibilityNotify event is received
    pub(crate) fn visibility_notify(&self) {
        let mut shared_state = self.shared_state_lock();

        match shared_state.visibility {
            Visibility::No => unsafe {
                (self.xconn.xlib.XUnmapWindow)(self.xconn.display, self.xwindow);
            },
            Visibility::Yes => (),
            Visibility::YesWait => {
                shared_state.visibility = Visibility::Yes;

                if let Some(fullscreen) = shared_state.desired_fullscreen.take() {
                    drop(shared_state);
                    self.set_fullscreen(fullscreen);
                }
            }
        }
    }

    #[inline]
    pub fn current_monitor(&self) -> X11MonitorHandle {
        self.shared_state_lock().last_monitor.clone()
    }

    pub fn available_monitors(&self) -> Vec<X11MonitorHandle> {
        self.xconn.available_monitors()
    }

    pub fn primary_monitor(&self) -> X11MonitorHandle {
        self.xconn.primary_monitor()
    }

    fn set_minimized_inner(&self, minimized: bool) -> util::Flusher<'_> {
        unsafe {
            if minimized {
                let screen = (self.xconn.xlib.XDefaultScreen)(self.xconn.display);

                (self.xconn.xlib.XIconifyWindow)(self.xconn.display, self.xwindow, screen);

                util::Flusher::new(&self.xconn)
            } else {
                let atom = self.xconn.get_atom_unchecked(b"_NET_ACTIVE_WINDOW\0");

                self.xconn.send_client_msg(
                    self.xwindow,
                    self.root,
                    atom,
                    Some(ffi::SubstructureRedirectMask | ffi::SubstructureNotifyMask),
                    [1, ffi::CurrentTime as c_long, 0, 0, 0],
                )
            }
        }
    }

    #[inline]
    pub fn set_minimized(&self, minimized: bool) {
        self.set_minimized_inner(minimized)
            .flush()
            .expect("Failed to change window minimization");
    }

    #[inline]
    pub fn is_maximized(&self) -> bool {
        let state_atom = unsafe { self.xconn.get_atom_unchecked(b"_NET_WM_STATE\0") };
        let state = self
            .xconn
            .get_property(self.xwindow, state_atom, ffi::XA_ATOM);
        let horz_atom = unsafe {
            self.xconn
                .get_atom_unchecked(b"_NET_WM_STATE_MAXIMIZED_HORZ\0")
        };
        let vert_atom = unsafe {
            self.xconn
                .get_atom_unchecked(b"_NET_WM_STATE_MAXIMIZED_VERT\0")
        };
        match state {
            Ok(atoms) => {
                let horz_maximized = atoms.iter().any(|atom: &ffi::Atom| *atom == horz_atom);
                let vert_maximized = atoms.iter().any(|atom: &ffi::Atom| *atom == vert_atom);
                horz_maximized && vert_maximized
            }
            _ => false,
        }
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
        self.shared_state_lock().is_decorated = decorations;
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

    #[inline]
    pub fn is_decorated(&self) -> bool {
        self.shared_state_lock().is_decorated
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
        let mut shared_state = self.shared_state_lock();

        match (visible, shared_state.visibility) {
            (true, Visibility::Yes) | (true, Visibility::YesWait) | (false, Visibility::No) => {
                return
            }
            _ => (),
        }

        if visible {
            unsafe {
                (self.xconn.xlib.XMapRaised)(self.xconn.display, self.xwindow);
            }
            self.xconn
                .flush_requests()
                .expect("Failed to call XMapRaised");
            shared_state.visibility = Visibility::YesWait;
        } else {
            unsafe {
                (self.xconn.xlib.XUnmapWindow)(self.xconn.display, self.xwindow);
            }
            self.xconn
                .flush_requests()
                .expect("Failed to call XUnmapWindow");
            shared_state.visibility = Visibility::No;
        }
    }

    #[inline]
    pub fn is_visible(&self) -> Option<bool> {
        Some(self.shared_state_lock().visibility == Visibility::Yes)
    }

    fn update_cached_frame_extents(&self) {
        let extents = self
            .xconn
            .get_frame_extents_heuristic(self.xwindow, self.root);
        (*self.shared_state_lock()).frame_extents = Some(extents);
    }

    pub(crate) fn invalidate_cached_frame_extents(&self) {
        (*self.shared_state_lock()).frame_extents.take();
    }

    pub(crate) fn outer_position_physical(&self) -> (i32, i32) {
        let extents = (*self.shared_state_lock()).frame_extents.clone();
        if let Some(extents) = extents {
            let (x, y) = self.inner_position_physical();
            extents.inner_pos_to_outer(x, y)
        } else {
            self.update_cached_frame_extents();
            self.outer_position_physical()
        }
    }

    #[inline]
    pub fn outer_position(&self) -> Result<PhysicalPosition<i32>, NotSupportedError> {
        let extents = (*self.shared_state_lock()).frame_extents.clone();
        if let Some(extents) = extents {
            let (x, y) = self.inner_position_physical();
            Ok(extents.inner_pos_to_outer(x, y).into())
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
    pub fn inner_position(&self) -> Result<PhysicalPosition<i32>, NotSupportedError> {
        Ok(self.inner_position_physical().into())
    }

    pub(crate) fn set_position_inner(&self, mut x: i32, mut y: i32) -> util::Flusher<'_> {
        // There are a few WMs that set client area position rather than window position, so
        // we'll translate for consistency.
        if util::wm_name_is_one_of(&["Enlightenment", "FVWM"]) {
            let extents = (*self.shared_state_lock()).frame_extents.clone();
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
    pub fn set_outer_position(&self, position: Position) {
        let (x, y) = position.to_physical::<i32>(self.scale_factor()).into();
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
    pub fn inner_size(&self) -> PhysicalSize<u32> {
        self.inner_size_physical().into()
    }

    #[inline]
    pub fn outer_size(&self) -> PhysicalSize<u32> {
        let extents = self.shared_state_lock().frame_extents.clone();
        if let Some(extents) = extents {
            let (width, height) = self.inner_size_physical();
            extents.inner_size_to_outer(width, height).into()
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
    pub fn set_inner_size(&self, size: Size) {
        let scale_factor = self.scale_factor();
        let size = size.to_physical::<u32>(scale_factor).into();
        if !self.shared_state_lock().is_resizable {
            self.update_normal_hints(|normal_hints| {
                normal_hints.set_min_size(Some(size));
                normal_hints.set_max_size(Some(size));
            })
            .expect("Failed to call `XSetWMNormalHints`");
        }
        self.set_inner_size_physical(size.0, size.1);
    }

    fn update_normal_hints<F>(&self, callback: F) -> Result<(), XError>
    where
        F: FnOnce(&mut util::NormalHints<'_>),
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
    pub fn set_min_inner_size(&self, dimensions: Option<Size>) {
        self.shared_state_lock().min_inner_size = dimensions;
        let physical_dimensions =
            dimensions.map(|dimensions| dimensions.to_physical::<u32>(self.scale_factor()).into());
        self.set_min_inner_size_physical(physical_dimensions);
    }

    pub(crate) fn set_max_inner_size_physical(&self, dimensions: Option<(u32, u32)>) {
        self.update_normal_hints(|normal_hints| normal_hints.set_max_size(dimensions))
            .expect("Failed to call `XSetWMNormalHints`");
    }

    #[inline]
    pub fn set_max_inner_size(&self, dimensions: Option<Size>) {
        self.shared_state_lock().max_inner_size = dimensions;
        let physical_dimensions =
            dimensions.map(|dimensions| dimensions.to_physical::<u32>(self.scale_factor()).into());
        self.set_max_inner_size_physical(physical_dimensions);
    }

    pub(crate) fn adjust_for_dpi(
        &self,
        old_scale_factor: f64,
        new_scale_factor: f64,
        width: u32,
        height: u32,
        shared_state: &SharedState,
    ) -> (u32, u32) {
        let scale_factor = new_scale_factor / old_scale_factor;
        self.update_normal_hints(|normal_hints| {
            let dpi_adjuster =
                |size: Size| -> (u32, u32) { size.to_physical::<u32>(new_scale_factor).into() };
            let max_size = shared_state.max_inner_size.map(&dpi_adjuster);
            let min_size = shared_state.min_inner_size.map(&dpi_adjuster);
            let resize_increments = shared_state.resize_increments.map(&dpi_adjuster);
            let base_size = shared_state.base_size.map(&dpi_adjuster);
            normal_hints.set_max_size(max_size);
            normal_hints.set_min_size(min_size);
            normal_hints.set_resize_increments(resize_increments);
            normal_hints.set_base_size(base_size);
        })
        .expect("Failed to update normal hints");

        let new_width = (width as f64 * scale_factor).round() as u32;
        let new_height = (height as f64 * scale_factor).round() as u32;

        (new_width, new_height)
    }

    pub fn set_resizable(&self, resizable: bool) {
        if util::wm_name_is_one_of(&["Xfwm4"]) {
            // Making the window unresizable on Xfwm prevents further changes to `WM_NORMAL_HINTS` from being detected.
            // This makes it impossible for resizing to be re-enabled, and also breaks DPI scaling. As such, we choose
            // the lesser of two evils and do nothing.
            warn!("To avoid a WM bug, disabling resizing has no effect on Xfwm4");
            return;
        }

        let (min_size, max_size) = if resizable {
            let shared_state_lock = self.shared_state_lock();
            (
                shared_state_lock.min_inner_size,
                shared_state_lock.max_inner_size,
            )
        } else {
            let window_size = Some(Size::from(self.inner_size()));
            (window_size, window_size)
        };
        self.shared_state_lock().is_resizable = resizable;

        self.set_maximizable_inner(resizable).queue();

        let scale_factor = self.scale_factor();
        let min_inner_size = min_size
            .map(|size| size.to_physical::<u32>(scale_factor))
            .map(Into::into);
        let max_inner_size = max_size
            .map(|size| size.to_physical::<u32>(scale_factor))
            .map(Into::into);
        self.update_normal_hints(|normal_hints| {
            normal_hints.set_min_size(min_inner_size);
            normal_hints.set_max_size(max_inner_size);
        })
        .expect("Failed to call `XSetWMNormalHints`");
    }

    #[inline]
    pub fn is_resizable(&self) -> bool {
        self.shared_state_lock().is_resizable
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

    #[inline]
    pub fn set_cursor_icon(&self, cursor: CursorIcon) {
        let old_cursor = replace(&mut *self.cursor.lock().unwrap(), cursor);
        #[allow(clippy::mutex_atomic)]
        if cursor != old_cursor && *self.cursor_visible.lock().unwrap() {
            self.xconn.set_cursor_icon(self.xwindow, Some(cursor));
        }
    }

    #[inline]
    pub fn set_cursor_grab(&self, mode: CursorGrabMode) -> Result<(), ExternalError> {
        let mut grabbed_lock = self.cursor_grabbed_mode.lock().unwrap();
        if mode == *grabbed_lock {
            return Ok(());
        }

        unsafe {
            // We ungrab before grabbing to prevent passive grabs from causing `AlreadyGrabbed`.
            // Therefore, this is common to both codepaths.
            (self.xconn.xlib.XUngrabPointer)(self.xconn.display, ffi::CurrentTime);
        }

        let result = match mode {
            CursorGrabMode::None => self
                .xconn
                .flush_requests()
                .map_err(|err| ExternalError::Os(os_error!(OsError::XError(err)))),
            CursorGrabMode::Confined => {
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
                        Err("Cursor could not be confined: already confined by another client")
                    }
                    ffi::GrabInvalidTime => Err("Cursor could not be confined: invalid time"),
                    ffi::GrabNotViewable => {
                        Err("Cursor could not be confined: confine location not viewable")
                    }
                    ffi::GrabFrozen => {
                        Err("Cursor could not be confined: frozen by another client")
                    }
                    _ => unreachable!(),
                }
                .map_err(|err| ExternalError::Os(os_error!(OsError::XMisc(err))))
            }
            CursorGrabMode::Locked => {
                return Err(ExternalError::NotSupported(NotSupportedError::new()));
            }
        };

        if result.is_ok() {
            *grabbed_lock = mode;
        }

        result
    }

    #[inline]
    pub fn set_cursor_visible(&self, visible: bool) {
        #[allow(clippy::mutex_atomic)]
        let mut visible_lock = self.cursor_visible.lock().unwrap();
        if visible == *visible_lock {
            return;
        }
        let cursor = if visible {
            Some(*self.cursor.lock().unwrap())
        } else {
            None
        };
        *visible_lock = visible;
        drop(visible_lock);
        self.xconn.set_cursor_icon(self.xwindow, cursor);
    }

    #[inline]
    pub fn scale_factor(&self) -> f64 {
        self.current_monitor().scale_factor
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
    pub fn set_cursor_position(&self, position: Position) -> Result<(), ExternalError> {
        let (x, y) = position.to_physical::<i32>(self.scale_factor()).into();
        self.set_cursor_position_physical(x, y)
    }

    #[inline]
    pub fn set_cursor_hittest(&self, _hittest: bool) -> Result<(), ExternalError> {
        Err(ExternalError::NotSupported(NotSupportedError::new()))
    }

    pub fn drag_window(&self) -> Result<(), ExternalError> {
        let pointer = self
            .xconn
            .query_pointer(self.xwindow, util::VIRTUAL_CORE_POINTER)
            .map_err(|err| ExternalError::Os(os_error!(OsError::XError(err))))?;

        let window = self.inner_position().map_err(ExternalError::NotSupported)?;

        let message = unsafe { self.xconn.get_atom_unchecked(b"_NET_WM_MOVERESIZE\0") };

        // we can't use `set_cursor_grab(false)` here because it doesn't run `XUngrabPointer`
        // if the cursor isn't currently grabbed
        let mut grabbed_lock = self.cursor_grabbed_mode.lock().unwrap();
        unsafe {
            (self.xconn.xlib.XUngrabPointer)(self.xconn.display, ffi::CurrentTime);
        }
        self.xconn
            .flush_requests()
            .map_err(|err| ExternalError::Os(os_error!(OsError::XError(err))))?;
        *grabbed_lock = CursorGrabMode::None;

        // we keep the lock until we are done
        self.xconn
            .send_client_msg(
                self.xwindow,
                self.root,
                message,
                Some(ffi::SubstructureRedirectMask | ffi::SubstructureNotifyMask),
                [
                    (window.x as c_long + pointer.win_x as c_long),
                    (window.y as c_long + pointer.win_y as c_long),
                    8, // _NET_WM_MOVERESIZE_MOVE
                    ffi::Button1 as c_long,
                    1,
                ],
            )
            .flush()
            .map_err(|err| ExternalError::Os(os_error!(OsError::XError(err))))
    }

    #[inline]
    pub fn set_ime_position(&self, spot: Position) {
        let (x, y) = spot.to_physical::<i32>(self.scale_factor()).into();
        let _ = self
            .ime_sender
            .lock()
            .unwrap()
            .send(ImeRequest::Position(self.xwindow, x, y));
    }

    #[inline]
    pub fn set_ime_allowed(&self, allowed: bool) {
        let _ = self
            .ime_sender
            .lock()
            .unwrap()
            .send(ImeRequest::Allow(self.xwindow, allowed));
    }

    #[inline]
    pub fn focus_window(&self) {
        let state_atom = unsafe { self.xconn.get_atom_unchecked(b"WM_STATE\0") };
        let state_type_atom = unsafe { self.xconn.get_atom_unchecked(b"CARD32\0") };
        let is_minimized = if let Ok(state) =
            self.xconn
                .get_property(self.xwindow, state_atom, state_type_atom)
        {
            state.contains(&(ffi::IconicState as c_ulong))
        } else {
            false
        };
        let is_visible = match self.shared_state_lock().visibility {
            Visibility::Yes => true,
            Visibility::YesWait | Visibility::No => false,
        };

        if is_visible && !is_minimized {
            let atom = unsafe { self.xconn.get_atom_unchecked(b"_NET_ACTIVE_WINDOW\0") };
            let flusher = self.xconn.send_client_msg(
                self.xwindow,
                self.root,
                atom,
                Some(ffi::SubstructureRedirectMask | ffi::SubstructureNotifyMask),
                [1, ffi::CurrentTime as c_long, 0, 0, 0],
            );
            if let Err(e) = flusher.flush() {
                log::error!(
                    "`flush` returned an error when focusing the window. Error was: {}",
                    e
                );
            }
        }
    }

    #[inline]
    pub fn request_user_attention(&self, request_type: Option<UserAttentionType>) {
        let mut wm_hints = self
            .xconn
            .get_wm_hints(self.xwindow)
            .expect("`XGetWMHints` failed");
        if request_type.is_some() {
            (*wm_hints).flags |= ffi::XUrgencyHint;
        } else {
            (*wm_hints).flags &= !ffi::XUrgencyHint;
        }
        self.xconn
            .set_wm_hints(self.xwindow, wm_hints)
            .flush()
            .expect("Failed to set urgency hint");
    }

    #[inline]
    pub fn id(&self) -> WindowId {
        WindowId(self.xwindow as u64)
    }

    #[inline]
    pub fn request_redraw(&self) {
        self.redraw_sender
            .sender
            .send(WindowId(self.xwindow as u64))
            .unwrap();
        self.redraw_sender.waker.wake().unwrap();
    }

    #[inline]
    pub fn raw_window_handle(&self) -> RawWindowHandle {
        let mut window_handle = XlibWindowHandle::empty();
        window_handle.window = self.xlib_window();
        RawWindowHandle::Xlib(window_handle)
    }

    #[inline]
    pub fn raw_display_handle(&self) -> RawDisplayHandle {
        let mut display_handle = XlibDisplayHandle::empty();
        display_handle.display = self.xlib_display();
        display_handle.screen = self.screen_id;
        RawDisplayHandle::Xlib(display_handle)
    }
}
