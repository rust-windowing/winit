use raw_window_handle::unix::XcbHandle;
use std::sync::atomic::AtomicUsize;
use std::sync::atomic::Ordering::Relaxed;
use std::{
    cmp, env,
    ffi::CString,
    mem::{replace, MaybeUninit},
    os::raw::*,
    path::Path,
    ptr, slice,
    sync::Arc,
};

use libc;
use mio_misc::channel::Sender;
use parking_lot::Mutex;

use crate::{
    dpi::{PhysicalPosition, PhysicalSize, Position, Size},
    error::{ExternalError, NotSupportedError, OsError as RootOsError},
    monitor::{MonitorHandle as RootMonitorHandle, VideoMode as RootVideoMode},
    platform_impl::{
        x11::MonitorHandle as X11MonitorHandle, MonitorHandle as PlatformMonitorHandle, OsError,
        PlatformSpecificWindowBuilderAttributes, VideoMode as PlatformVideoMode,
    },
    window::{CursorIcon, Fullscreen, Icon, UserAttentionType, WindowAttributes},
};

use super::{ffi, util, EventLoopWindowTarget, WindowId, XConnection};
use crate::platform_impl::x11::util::HintsError;
use crate::platform_impl::x11::util::PropMode;
use crate::platform_impl::x11::xdisplay::Screen;
use xcb_dl_util::hint::XcbSizeHints;
use xcb_dl_util::void::{XcbPendingCommand, XcbPendingCommands};

#[derive(Debug)]
pub struct SharedState {
    pub cursor_pos: Option<(f64, f64)>,
    pub size: Option<(u32, u32)>,
    pub position: Option<(i32, i32)>,
    pub inner_position: Option<(i32, i32)>,
    pub inner_position_rel_parent: Option<(i32, i32)>,
    pub last_monitor: X11MonitorHandle,
    pub dpi_adjusted: Option<(u32, u32)>,
    pub fullscreen: Option<Fullscreen>,
    // Set when application calls `set_fullscreen` when window is not visible
    pub desired_fullscreen: Option<Option<Fullscreen>>,
    // Used to restore position after exiting fullscreen
    pub restore_position: Option<(i32, i32)>,
    // Used to restore video mode after exiting fullscreen
    pub desktop_video_mode: Option<(ffi::xcb_randr_crtc_t, ffi::xcb_randr_mode_t)>,
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
    fn new(last_monitor: X11MonitorHandle, is_visible: bool) -> Mutex<Self> {
        let visibility = if is_visible {
            Visibility::YesWait
        } else {
            Visibility::No
        };

        Mutex::new(SharedState {
            last_monitor,
            visibility,

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
    pub xconn: Arc<XConnection>,    // never changes
    pub xwindow: ffi::xcb_window_t, // never changes
    pub screen: Arc<Screen>,
    cursor: Mutex<CursorIcon>,
    cursor_grabbed: Mutex<bool>,
    cursor_visible: Mutex<bool>,
    pub shared_state: Mutex<SharedState>,
    redraw_sender: Sender<WindowId>,
    reset_dead_keys: Arc<AtomicUsize>,
}

impl UnownedWindow {
    pub fn new<T>(
        event_loop: &EventLoopWindowTarget<T>,
        window_attrs: WindowAttributes,
        pl_attribs: PlatformSpecificWindowBuilderAttributes,
    ) -> Result<UnownedWindow, RootOsError> {
        let xconn = &event_loop.xconn;

        let screen_id = pl_attribs
            .screen_id
            .map(|s| s as usize)
            .unwrap_or(xconn.default_screen_id);
        let screen = match xconn.screens.iter().skip(screen_id).next() {
            Some(s) => s,
            _ => return Err(os_error!(OsError::XMisc("Screen id out of bounds"))),
        };

        let root = screen.root;

        let mut monitors = xconn.available_monitors();
        let guessed_monitor = if monitors.is_empty() {
            X11MonitorHandle::dummy()
        } else {
            xconn
                .query_pointer(root as _, util::VIRTUAL_CORE_POINTER)
                .ok()
                .and_then(|pointer_state| {
                    let root_x = util::fp1616_to_f64(pointer_state.root_x);
                    let root_y = util::fp1616_to_f64(pointer_state.root_y);
                    let (x, y) = (root_x as i64, root_y as i64);

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
            .map(|position| position.to_physical::<i32>(scale_factor).into());

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

        let mut commands = XcbPendingCommands::new();

        // creating
        let set_win_attr = {
            let mut swa = ffi::xcb_create_window_value_list_t::default();
            if let Some(visual_id) = pl_attribs.visual_infos.visual_id {
                unsafe {
                    swa.colormap = xconn.generate_id();
                    commands.push(
                        xconn
                            .xcb
                            .xcb_create_colormap_checked(xconn.c, 0, swa.colormap, root, visual_id)
                            .into(),
                    );
                }
            }
            swa.event_mask = ffi::XCB_EVENT_MASK_EXPOSURE
                | ffi::XCB_EVENT_MASK_STRUCTURE_NOTIFY
                | ffi::XCB_EVENT_MASK_VISIBILITY_CHANGE
                | ffi::XCB_EVENT_MASK_KEY_PRESS
                | ffi::XCB_EVENT_MASK_KEYMAP_STATE
                | ffi::XCB_EVENT_MASK_BUTTON_PRESS
                | ffi::XCB_EVENT_MASK_BUTTON_RELEASE
                | ffi::XCB_EVENT_MASK_POINTER_MOTION;
            swa.border_pixel = 0;
            swa.override_redirect = pl_attribs.override_redirect as u32;
            swa
        };

        let mut window_attributes =
            ffi::XCB_CW_BORDER_PIXEL | ffi::XCB_CW_COLORMAP | ffi::XCB_CW_EVENT_MASK;

        if pl_attribs.override_redirect {
            window_attributes |= ffi::XCB_CW_OVERRIDE_REDIRECT;
        }

        // finally creating the window
        let xwindow = xconn.generate_id();
        unsafe {
            let pending = xconn
                .xcb
                .xcb_create_window_aux_checked(
                    xconn.c,
                    pl_attribs
                        .visual_infos
                        .depth
                        .unwrap_or(ffi::XCB_COPY_FROM_PARENT as _),
                    xwindow,
                    screen.root,
                    position.map_or(0, |p: PhysicalPosition<i32>| p.x as i16),
                    position.map_or(0, |p: PhysicalPosition<i32>| p.y as i16),
                    dimensions.0 as u16,
                    dimensions.1 as u16,
                    0, // border width
                    ffi::XCB_WINDOW_CLASS_INPUT_OUTPUT as u16,
                    // TODO: If window wants transparency and `visual_infos` is None,
                    // we need to find our own visual which has an `alphaMask` which
                    // is > 0, like we do in glutin.
                    //
                    // It is non obvious which masks, if any, we should pass to
                    // `XGetVisualInfo`. winit doesn't receive any info about what
                    // properties the user wants. Users should consider choosing the
                    // visual themselves as glutin does.
                    pl_attribs
                        .visual_infos
                        .visual_id
                        .unwrap_or(ffi::XCB_COPY_FROM_PARENT as _),
                    window_attributes,
                    &set_win_attr,
                )
                .into();
            commands.push(pending);
        };

        let mut window = UnownedWindow {
            xconn: Arc::clone(xconn),
            xwindow,
            screen: screen.clone(),
            cursor: Default::default(),
            cursor_grabbed: Mutex::new(false),
            cursor_visible: Mutex::new(true),
            shared_state: SharedState::new(guessed_monitor, window_attrs.visible),
            redraw_sender: event_loop.redraw_sender.clone(),
            reset_dead_keys: event_loop.reset_dead_keys.clone(),
        };

        // Title must be set before mapping. Some tiling window managers (i.e. i3) use the window
        // title to determine placement/etc., so doing this after mapping would cause the WM to
        // act on the wrong title state.
        commands.extend(window.set_title_inner(&window_attrs.title));
        commands.push(window.set_decorations_inner(window_attrs.decorations));

        {
            // Enable drag and drop (TODO: extend API to make this toggleable)
            let dnd_aware_atom = xconn.get_atom("XdndAware");
            let version = &[5u32]; // Latest version; hasn't changed since 2002
            commands.push(xconn.change_property(
                window.xwindow,
                dnd_aware_atom,
                ffi::XCB_ATOM_ATOM,
                util::PropMode::Replace,
                version,
            ));

            // WM_CLASS must be set *before* mapping the window, as per ICCCM!
            {
                let (class, instance) = if let Some((instance, class)) = pl_attribs.class {
                    (instance, class)
                } else {
                    let class = env::args()
                        .next()
                        .as_ref()
                        // Default to the name of the binary (via argv[0])
                        .and_then(|path| Path::new(path).file_name())
                        .and_then(|bin_name| bin_name.to_str())
                        .map(|bin_name| bin_name.to_owned())
                        .unwrap_or_else(|| window_attrs.title.clone());
                    // This environment variable is extraordinarily unlikely to actually be used...
                    let instance = env::var("RESOURCE_NAME")
                        .ok()
                        .unwrap_or_else(|| class.clone());
                    (instance, class)
                };

                commands.push(xconn.change_property(
                    window.xwindow,
                    ffi::XCB_ATOM_WM_CLASS,
                    ffi::XCB_ATOM_STRING,
                    PropMode::Replace,
                    format!("{}\0{}\0", class, instance).as_bytes(),
                ));
            }

            window.set_pid().map(|cmds| commands.extend(cmds));

            commands.push(window.set_window_types(pl_attribs.x11_window_types));

            if let Some(variant) = pl_attribs.gtk_theme_variant {
                commands.push(window.set_gtk_theme_variant(variant));
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
                    if screen.wm_name_is_one_of(&["Xfwm4"]) {
                        warn!("To avoid a WM bug, disabling resizing has no effect on Xfwm4");
                    } else {
                        max_inner_size = Some(dimensions.into());
                        min_inner_size = Some(dimensions.into());

                        let mut shared_state = window.shared_state.get_mut();
                        shared_state.min_inner_size = window_attrs.min_inner_size;
                        shared_state.max_inner_size = window_attrs.max_inner_size;
                        shared_state.resize_increments = pl_attribs.resize_increments;
                        shared_state.base_size = pl_attribs.base_size;
                    }
                }

                let mut normal_hints = XcbSizeHints::default();
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
                commands.push(xconn.set_normal_hints(window.xwindow, normal_hints));
            }

            // Set window icons
            if let Some(icon) = window_attrs.window_icon {
                commands.push(window.set_icon_inner(icon));
            }

            // Opt into handling window close
            {
                let prop = xconn.get_atom("WM_PROTOCOLS");
                let pending = xconn
                    .change_property(
                        window.xwindow,
                        prop,
                        ffi::XCB_ATOM_ATOM,
                        PropMode::Replace,
                        &[event_loop.wm_delete_window, event_loop.net_wm_ping],
                    )
                    .into();
                commands.push(pending);
            }

            // Set visibility (map window)
            if window_attrs.visible {
                commands.extend(window.map_raised());
            }

            // Select XInput2 events
            let mask = {
                let mask = ffi::XCB_INPUT_XI_EVENT_MASK_MOTION
                    | ffi::XCB_INPUT_XI_EVENT_MASK_BUTTON_PRESS
                    | ffi::XCB_INPUT_XI_EVENT_MASK_BUTTON_RELEASE
                    | ffi::XCB_INPUT_XI_EVENT_MASK_KEY_PRESS
                    | ffi::XCB_INPUT_XI_EVENT_MASK_KEY_RELEASE
                    | ffi::XCB_INPUT_XI_EVENT_MASK_ENTER
                    | ffi::XCB_INPUT_XI_EVENT_MASK_LEAVE
                    | ffi::XCB_INPUT_XI_EVENT_MASK_FOCUS_IN
                    | ffi::XCB_INPUT_XI_EVENT_MASK_FOCUS_OUT
                    | ffi::XCB_INPUT_XI_EVENT_MASK_TOUCH_BEGIN
                    | ffi::XCB_INPUT_XI_EVENT_MASK_TOUCH_UPDATE
                    | ffi::XCB_INPUT_XI_EVENT_MASK_TOUCH_END;
                mask
            };
            let pending = xconn.select_xinput_events(
                window.xwindow as _,
                ffi::XCB_INPUT_DEVICE_ALL_MASTER as _,
                mask,
            );
            commands.push(pending);

            // These properties must be set after mapping
            if window_attrs.maximized {
                commands.push(window.set_maximized_inner(window_attrs.maximized));
            }
            if window_attrs.fullscreen.is_some() {
                if let Some(pending) = window.set_fullscreen_inner(window_attrs.fullscreen.clone())
                {
                    commands.extend(pending);
                }

                if let Some(PhysicalPosition { x, y }) = position {
                    let shared_state = window.shared_state.get_mut();

                    shared_state.restore_position = Some((x, y));
                }
            }
            if window_attrs.always_on_top {
                commands.push(window.set_always_on_top_inner(window_attrs.always_on_top));
            }
        }

        // We never want to give the user a broken window, since by then, it's too late to handle.
        if let Err(e) = xconn.check_pending(commands) {
            Err(os_error!(OsError::XError(e.into())))
        } else {
            Ok(window)
        }
    }

    fn set_pid(&self) -> Option<XcbPendingCommands> {
        let pid_atom = self.xconn.get_atom("_NET_WM_PID");
        let client_machine_atom = self.xconn.get_atom("WM_CLIENT_MACHINE");
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

            let pending1 = self.xconn.change_property(
                self.xwindow,
                pid_atom,
                ffi::XCB_ATOM_CARDINAL,
                util::PropMode::Replace,
                &[libc::getpid() as util::Cardinal],
            );
            let pending2 = self.xconn.change_property(
                self.xwindow,
                client_machine_atom,
                ffi::XCB_ATOM_STRING,
                util::PropMode::Replace,
                &hostname[0..hostname_length],
            );
            Some(pending1.and_then(pending2))
        }
    }

    fn set_window_types(&self, window_types: Vec<util::WindowType>) -> XcbPendingCommand {
        let hint_atom = self.xconn.get_atom("_NET_WM_WINDOW_TYPE");
        let atoms: Vec<_> = window_types
            .iter()
            .map(|t| t.as_atom(&self.xconn))
            .collect();

        self.xconn.change_property(
            self.xwindow,
            hint_atom,
            ffi::XCB_ATOM_ATOM,
            util::PropMode::Replace,
            &atoms,
        )
    }

    fn set_gtk_theme_variant(&self, variant: String) -> XcbPendingCommand {
        let hint_atom = self.xconn.get_atom("_GTK_THEME_VARIANT");
        let utf8_atom = self.xconn.get_atom("UTF8_STRING");
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
        properties: (u32, u32, u32, u32),
    ) -> XcbPendingCommand {
        let state_atom = self.xconn.get_atom("_NET_WM_STATE");
        self.xconn.send_client_msg(
            self.xwindow,
            self.screen.root,
            state_atom,
            Some(
                ffi::XCB_EVENT_MASK_SUBSTRUCTURE_REDIRECT | ffi::XCB_EVENT_MASK_SUBSTRUCTURE_NOTIFY,
            ),
            [
                operation as u32,
                properties.0,
                properties.1,
                properties.2,
                properties.3,
            ],
        )
    }

    fn set_fullscreen_hint(&self, fullscreen: bool) -> XcbPendingCommands {
        let fullscreen_atom = self.xconn.get_atom("_NET_WM_STATE_FULLSCREEN");
        let mut pending: XcbPendingCommands = self
            .set_netwm(fullscreen.into(), (fullscreen_atom, 0, 0, 0))
            .into();

        if fullscreen {
            // Ensure that the fullscreen window receives input focus to prevent
            // locking up the user's display.
            unsafe {
                let p = self
                    .xconn
                    .xcb
                    .xcb_set_input_focus_checked(
                        self.xconn.c,
                        ffi::XCB_INPUT_FOCUS_PARENT as _,
                        self.xwindow,
                        ffi::XCB_TIME_CURRENT_TIME,
                    )
                    .into();
                pending.push(p);
            }
        }

        pending
    }

    fn set_fullscreen_inner(&self, fullscreen: Option<Fullscreen>) -> Option<XcbPendingCommands> {
        let mut shared_state_lock = self.shared_state.lock();

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
                    Some((monitor.id, self.xconn.get_crtc_mode(monitor.id).unwrap()));
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
                let mut pending = self.set_fullscreen_hint(false);
                let mut shared_state_lock = self.shared_state.lock();
                if let Some(position) = shared_state_lock.restore_position.take() {
                    drop(shared_state_lock);
                    pending.push(self.set_position_inner(position.0, position.1));
                }
                Some(pending)
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

                {
                    let window_position = self.outer_position_physical();
                    let mut ss = self.shared_state.lock();
                    // Don't overwrite the restore position if we're switching between
                    // fullscreen modes.
                    if ss.restore_position.is_none() {
                        ss.restore_position = Some(window_position);
                    }
                }
                let monitor_origin: (i32, i32) = monitor.position().into();
                let mut pending: XcbPendingCommands = self
                    .set_position_inner(monitor_origin.0, monitor_origin.1)
                    .into();
                pending.extend(self.set_fullscreen_hint(true));
                Some(pending)
            }
        }
    }

    #[inline]
    pub fn fullscreen(&self) -> Option<Fullscreen> {
        let shared_state = self.shared_state.lock();

        shared_state
            .desired_fullscreen
            .clone()
            .unwrap_or_else(|| shared_state.fullscreen.clone())
    }

    #[inline]
    pub fn set_fullscreen(&self, fullscreen: Option<Fullscreen>) {
        let fs = fullscreen.is_some();
        if let Some(pending) = self.set_fullscreen_inner(fullscreen) {
            if let Err(e) = self.xconn.check_pending(pending) {
                log::error!("Failed to change window fullscreen state: {}", e);
                if fs {
                    panic!("Could not exit fullscreen.");
                }
            }
            self.invalidate_cached_frame_extents();
        }
    }

    // Called by EventProcessor when a VisibilityNotify event is received
    pub(crate) fn visibility_notify(&self) {
        let mut shared_state = self.shared_state.lock();

        match shared_state.visibility {
            Visibility::No => {
                let pending = self.unmap();
                if let Err(e) = self.xconn.check_pending1(pending) {
                    log::error!("Failed to unmap window: {}", e);
                }
            }
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
        self.shared_state.lock().last_monitor.clone()
    }

    pub fn available_monitors(&self) -> Vec<X11MonitorHandle> {
        match self.xconn.available_monitors_inner() {
            Ok(m) => m,
            Err(e) => {
                log::error!("Could not query available monitors: {}", e);
                vec![]
            }
        }
    }

    pub fn primary_monitor(&self) -> X11MonitorHandle {
        self.xconn.primary_monitor()
    }

    fn set_minimized_inner(&self, minimized: bool) -> XcbPendingCommands {
        let mut pending = XcbPendingCommands::new();
        if minimized {
            pending.push(
                self.xconn
                    .send_client_msg(
                        self.xwindow,
                        self.screen.root,
                        self.xconn.get_atom("WM_CHANGE_STATE"),
                        Some(
                            ffi::XCB_EVENT_MASK_SUBSTRUCTURE_REDIRECT
                                | ffi::XCB_EVENT_MASK_SUBSTRUCTURE_NOTIFY,
                        ),
                        [3, 0, 0, 0, 0],
                    )
                    .into(),
            );
        } else {
            if self.shared_state.lock().visibility != Visibility::No {
                pending.extend(self.map_raised());
            }
        }
        pending
    }

    #[inline]
    pub fn set_minimized(&self, minimized: bool) {
        let pending = self.set_minimized_inner(minimized);
        if let Err(e) = self.xconn.check_pending(pending) {
            log::error!("Could not change minimized state: {}", e);
        }
    }

    fn set_maximized_inner(&self, maximized: bool) -> XcbPendingCommand {
        let horz_atom = self.xconn.get_atom("_NET_WM_STATE_MAXIMIZED_HORZ");
        let vert_atom = self.xconn.get_atom("_NET_WM_STATE_MAXIMIZED_VERT");
        self.set_netwm(maximized.into(), (horz_atom, vert_atom, 0, 0))
    }

    #[inline]
    pub fn set_maximized(&self, maximized: bool) {
        let pending = self.set_maximized_inner(maximized);
        if let Err(e) = self.xconn.check_pending1(pending) {
            log::error!("Failed to change window maximization: {}", e);
        }
        self.invalidate_cached_frame_extents();
    }

    fn set_title_inner(&self, title: &str) -> XcbPendingCommands {
        let pending1 = self.xconn.change_property(
            self.xwindow,
            ffi::XCB_ATOM_WM_NAME,
            ffi::XCB_ATOM_STRING,
            util::PropMode::Replace,
            title.as_bytes(),
        );
        let pending2 = self.xconn.change_property(
            self.xwindow,
            self.xconn.get_atom("_NET_WM_NAME"),
            self.xconn.get_atom("UTF8_STRING"),
            util::PropMode::Replace,
            title.as_bytes(),
        );
        pending1.and_then(pending2)
    }

    #[inline]
    pub fn set_title(&self, title: &str) {
        if let Err(e) = self.xconn.check_pending(self.set_title_inner(title)) {
            log::error!("Could not set window title: {}", e);
        }
    }

    fn set_decorations_inner(&self, decorations: bool) -> XcbPendingCommand {
        let mut hints = self.xconn.get_motif_hints(self.xwindow);

        hints.set_decorations(decorations);

        self.xconn.set_motif_hints(self.xwindow, &hints)
    }

    #[inline]
    pub fn set_decorations(&self, decorations: bool) {
        let pending = self.set_decorations_inner(decorations);
        if let Err(e) = self.xconn.check_pending1(pending) {
            log::error!("Could not set window decorations: {}", e);
        }
        self.invalidate_cached_frame_extents();
    }

    fn set_maximizable_inner(&self, maximizable: bool) -> XcbPendingCommand {
        let mut hints = self.xconn.get_motif_hints(self.xwindow);

        hints.set_maximizable(maximizable);

        self.xconn.set_motif_hints(self.xwindow, &hints)
    }

    fn set_always_on_top_inner(&self, always_on_top: bool) -> XcbPendingCommand {
        let above_atom = self.xconn.get_atom("_NET_WM_STATE_ABOVE");
        self.set_netwm(always_on_top.into(), (above_atom, 0, 0, 0))
    }

    #[inline]
    pub fn set_always_on_top(&self, always_on_top: bool) {
        let pending = self.set_always_on_top_inner(always_on_top);
        if let Err(e) = self.xconn.check_pending1(pending) {
            log::error!("Could not set always-on-top property: {}", e);
        }
    }

    fn set_icon_inner(&self, icon: Icon) -> XcbPendingCommand {
        let icon_atom = self.xconn.get_atom("_NET_WM_ICON");
        let data = icon.to_cardinals();
        self.xconn.change_property(
            self.xwindow,
            icon_atom,
            ffi::XCB_ATOM_CARDINAL,
            util::PropMode::Replace,
            data.as_slice(),
        )
    }

    fn unset_icon_inner(&self) -> XcbPendingCommand {
        let icon_atom = self.xconn.get_atom("_NET_WM_ICON");
        let empty_data: [util::Cardinal; 0] = [];
        self.xconn.change_property(
            self.xwindow,
            icon_atom,
            ffi::XCB_ATOM_CARDINAL,
            util::PropMode::Replace,
            &empty_data,
        )
    }

    #[inline]
    pub fn set_window_icon(&self, icon: Option<Icon>) {
        let pending = match icon {
            Some(icon) => self.set_icon_inner(icon),
            None => self.unset_icon_inner(),
        };
        if let Err(e) = self.xconn.check_pending1(pending) {
            log::error!("Could not set window icon: {}", e);
        }
    }

    #[inline]
    pub fn set_visible(&self, visible: bool) {
        let mut shared_state = self.shared_state.lock();

        match (visible, shared_state.visibility) {
            (true, Visibility::Yes) | (true, Visibility::YesWait) | (false, Visibility::No) => {
                return
            }
            _ => (),
        }

        if visible {
            if let Err(e) = self.xconn.check_pending(self.map_raised()) {
                panic!("Failed to map window: {}", e);
            }
            shared_state.visibility = Visibility::YesWait;
        } else {
            if let Err(e) = self.xconn.check_pending1(self.unmap()) {
                panic!("Failed to unmap window: {}", e);
            }
            shared_state.visibility = Visibility::No;
        }
    }

    fn map_raised(&self) -> XcbPendingCommands {
        unsafe {
            let above = ffi::XCB_STACK_MODE_ABOVE as u32;
            let pending1: XcbPendingCommand = self
                .xconn
                .xcb
                .xcb_configure_window_checked(
                    self.xconn.c,
                    self.xwindow,
                    ffi::XCB_CONFIG_WINDOW_STACK_MODE as u16,
                    &above as *const _ as *const _,
                )
                .into();
            let pending2 = self
                .xconn
                .xcb
                .xcb_map_window_checked(self.xconn.c, self.xwindow)
                .into();
            pending1.and_then(pending2)
        }
    }

    fn unmap(&self) -> XcbPendingCommand {
        unsafe {
            self.xconn
                .xcb
                .xcb_unmap_window_checked(self.xconn.c, self.xwindow)
                .into()
        }
    }

    fn update_cached_frame_extents(&self) {
        let extents = self.xconn.get_frame_extents_heuristic(self);
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
    pub fn outer_position(&self) -> Result<PhysicalPosition<i32>, NotSupportedError> {
        let extents = (*self.shared_state.lock()).frame_extents.clone();
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
            .translate_coords(self.xwindow, self.screen.root)
            .map(|coords| (coords.x_rel_root as i32, coords.y_rel_root as i32))
            .unwrap()
    }

    #[inline]
    pub fn inner_position(&self) -> Result<PhysicalPosition<i32>, NotSupportedError> {
        Ok(self.inner_position_physical().into())
    }

    pub(crate) fn set_position_inner(&self, mut x: i32, mut y: i32) -> XcbPendingCommand {
        // There are a few WMs that set client area position rather than window position, so
        // we'll translate for consistency.
        if self.screen.wm_name_is_one_of(&["Enlightenment", "FVWM"]) {
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
            self.xconn
                .xcb
                .xcb_configure_window_checked(
                    self.xconn.c,
                    self.xwindow,
                    (ffi::XCB_CONFIG_WINDOW_X | ffi::XCB_CONFIG_WINDOW_Y) as _,
                    [x, y].as_ptr() as _,
                )
                .into()
        }
    }

    pub(crate) fn set_position_physical(&self, x: i32, y: i32) {
        let pending = self.set_position_inner(x, y);
        if let Err(e) = self.xconn.check_pending1(pending) {
            log::error!("Failed to set position: {}", e);
        }
    }

    #[inline]
    pub fn set_outer_position(&self, position: Position) {
        let (x, y) = position.to_physical::<i32>(self.scale_factor()).into();
        self.set_position_physical(x, y);
    }

    pub(crate) fn inner_size_physical(&self) -> (u32, u32) {
        // This should be okay to unwrap since the only error XGetGeometry can return
        // is BadWindow, and if the window handle is bad we have bigger problems.
        let res = self
            .xconn
            .get_geometry(self.xwindow)
            .map(|geo| (geo.width as u32, geo.height as u32));
        match res {
            Ok(r) => r,
            Err(e) => {
                panic!("Could not retrieve window size: {}", e);
            }
        }
    }

    #[inline]
    pub fn inner_size(&self) -> PhysicalSize<u32> {
        self.inner_size_physical().into()
    }

    #[inline]
    pub fn outer_size(&self) -> PhysicalSize<u32> {
        let extents = self.shared_state.lock().frame_extents.clone();
        if let Some(extents) = extents {
            let (width, height) = self.inner_size_physical();
            extents.inner_size_to_outer(width, height).into()
        } else {
            self.update_cached_frame_extents();
            self.outer_size()
        }
    }

    pub(crate) fn set_inner_size_physical(&self, width: u32, height: u32) {
        let pending = unsafe {
            self.xconn
                .xcb
                .xcb_configure_window_checked(
                    self.xconn.c,
                    self.xwindow,
                    (ffi::XCB_CONFIG_WINDOW_WIDTH | ffi::XCB_CONFIG_WINDOW_HEIGHT) as _,
                    [width, height].as_ptr() as _,
                )
                .into()
        };
        if let Err(e) = self.xconn.check_pending1(pending) {
            log::error!("Failed to resize window: {}", e);
        }
    }

    #[inline]
    pub fn set_inner_size(&self, size: Size) {
        let scale_factor = self.scale_factor();
        let (width, height) = size.to_physical::<u32>(scale_factor).into();
        self.set_inner_size_physical(width, height);
    }

    fn update_normal_hints<F>(&self, callback: F) -> Result<XcbPendingCommand, HintsError>
    where
        F: FnOnce(&mut XcbSizeHints) -> (),
    {
        let mut normal_hints = self.xconn.get_normal_hints(self.xwindow)?;
        callback(&mut normal_hints);
        Ok(self.xconn.set_normal_hints(self.xwindow, normal_hints))
    }

    pub(crate) fn set_min_inner_size_physical(
        &self,
        dimensions: Option<(u32, u32)>,
    ) -> Result<XcbPendingCommand, HintsError> {
        self.update_normal_hints(|normal_hints| normal_hints.set_min_size(dimensions))
    }

    #[inline]
    pub fn set_min_inner_size(&self, dimensions: Option<Size>) {
        self.shared_state.lock().min_inner_size = dimensions;
        let physical_dimensions =
            dimensions.map(|dimensions| dimensions.to_physical::<u32>(self.scale_factor()).into());
        let pending = self.set_min_inner_size_physical(physical_dimensions);
        if let Err(e) = pending.and_then(|p| self.xconn.check_pending1(p).map_err(|e| e.into())) {
            log::error!("Could not set minimum size: {}", e);
        }
    }

    pub(crate) fn set_max_inner_size_physical(
        &self,
        dimensions: Option<(u32, u32)>,
    ) -> Result<XcbPendingCommand, HintsError> {
        self.update_normal_hints(|normal_hints| normal_hints.set_max_size(dimensions))
    }

    #[inline]
    pub fn set_max_inner_size(&self, dimensions: Option<Size>) {
        self.shared_state.lock().max_inner_size = dimensions;
        let physical_dimensions =
            dimensions.map(|dimensions| dimensions.to_physical::<u32>(self.scale_factor()).into());
        let pending = self.set_max_inner_size_physical(physical_dimensions);
        if let Err(e) = pending.and_then(|p| self.xconn.check_pending1(p).map_err(|e| e.into())) {
            log::error!("Could not set maximum size: {}", e);
        }
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
        let pending = self.update_normal_hints(|normal_hints| {
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
        });
        if let Err(e) = pending.and_then(|p| self.xconn.check_pending1(p).map_err(|e| e.into())) {
            log::error!("Could not update normal hints: {}", e);
        }

        let new_width = (width as f64 * scale_factor).round() as u32;
        let new_height = (height as f64 * scale_factor).round() as u32;

        (new_width, new_height)
    }

    pub fn set_resizable(&self, resizable: bool) {
        if self.screen.wm_name_is_one_of(&["Xfwm4"]) {
            // Making the window unresizable on Xfwm prevents further changes to `WM_NORMAL_HINTS` from being detected.
            // This makes it impossible for resizing to be re-enabled, and also breaks DPI scaling. As such, we choose
            // the lesser of two evils and do nothing.
            warn!("To avoid a WM bug, disabling resizing has no effect on Xfwm4");
            return;
        }

        let (min_size, max_size) = if resizable {
            let shared_state_lock = self.shared_state.lock();
            (
                shared_state_lock.min_inner_size,
                shared_state_lock.max_inner_size,
            )
        } else {
            let window_size = Some(Size::from(self.inner_size()));
            (window_size.clone(), window_size)
        };

        let pending1 = self.set_maximizable_inner(resizable);

        let scale_factor = self.scale_factor();
        let min_inner_size = min_size
            .map(|size| size.to_physical::<u32>(scale_factor))
            .map(Into::into);
        let max_inner_size = max_size
            .map(|size| size.to_physical::<u32>(scale_factor))
            .map(Into::into);
        let pending2 = self.update_normal_hints(|normal_hints| {
            normal_hints.set_min_size(min_inner_size);
            normal_hints.set_max_size(max_inner_size);
        });

        let res = match pending2 {
            Ok(p) => self
                .xconn
                .check_pending(pending1.and_then(p))
                .map_err(|e| e.into()),
            Err(e) => {
                self.xconn.discard(pending1);
                Err(e)
            }
        };

        if let Err(e) = res {
            log::error!("Could not change the resizable property: {}", e);
        }
    }

    #[inline]
    pub fn set_cursor_icon(&self, cursor: CursorIcon) {
        let old_cursor = replace(&mut *self.cursor.lock(), cursor);
        if cursor != old_cursor && *self.cursor_visible.lock() {
            self.xconn.set_cursor_icon(self.xwindow as _, Some(cursor));
        }
    }

    #[inline]
    pub fn set_cursor_grab(&self, grab: bool) -> Result<(), ExternalError> {
        let mut grabbed_lock = self.cursor_grabbed.lock();
        if grab == *grabbed_lock {
            return Ok(());
        }
        let ungrab = unsafe {
            // We ungrab before grabbing to prevent passive grabs from causing `AlreadyGrabbed`.
            // Therefore, this is common to both codepaths.
            self.xconn
                .xcb
                .xcb_ungrab_pointer_checked(self.xconn.c, ffi::XCB_TIME_CURRENT_TIME)
                .into()
        };
        let result = if grab {
            self.xconn.discard(ungrab);
            loop {
                let result = unsafe {
                    let cookie = self.xconn.xcb.xcb_grab_pointer(
                        self.xconn.c,
                        1,
                        self.xwindow,
                        (ffi::XCB_EVENT_MASK_BUTTON_PRESS
                            | ffi::XCB_EVENT_MASK_BUTTON_RELEASE
                            | ffi::XCB_EVENT_MASK_ENTER_WINDOW
                            | ffi::XCB_EVENT_MASK_LEAVE_WINDOW
                            | ffi::XCB_EVENT_MASK_POINTER_MOTION
                            | ffi::XCB_EVENT_MASK_POINTER_MOTION_HINT
                            | ffi::XCB_EVENT_MASK_BUTTON_1_MOTION
                            | ffi::XCB_EVENT_MASK_BUTTON_2_MOTION
                            | ffi::XCB_EVENT_MASK_BUTTON_3_MOTION
                            | ffi::XCB_EVENT_MASK_BUTTON_4_MOTION
                            | ffi::XCB_EVENT_MASK_BUTTON_5_MOTION
                            | ffi::XCB_EVENT_MASK_BUTTON_MOTION
                            | ffi::XCB_EVENT_MASK_KEYMAP_STATE) as u16,
                        ffi::XCB_GRAB_MODE_ASYNC as u8,
                        ffi::XCB_GRAB_MODE_ASYNC as u8,
                        self.xwindow,
                        0,
                        ffi::XCB_TIME_CURRENT_TIME,
                    );
                    let mut err = ptr::null_mut();
                    let reply =
                        self.xconn
                            .xcb
                            .xcb_grab_pointer_reply(self.xconn.c, cookie, &mut err);
                    self.xconn.check(reply, err)
                };
                let reply = match result {
                    Ok(r) => r,
                    Err(e) => {
                        break Err(ExternalError::Os(os_error!(OsError::XError(e.into()))));
                    }
                };

                let res = match reply.status as ffi::xcb_grab_status_t {
                    ffi::XCB_GRAB_STATUS_SUCCESS => Ok(()),
                    ffi::XCB_GRAB_STATUS_ALREADY_GRABBED => {
                        Err("Cursor could not be grabbed: already grabbed by another client")
                    }
                    ffi::XCB_GRAB_STATUS_INVALID_TIME => {
                        Err("Cursor could not be grabbed: invalid time")
                    }
                    ffi::XCB_GRAB_STATUS_NOT_VIEWABLE => {
                        Err("Cursor could not be grabbed: grab location not viewable")
                    }
                    ffi::XCB_GRAB_STATUS_FROZEN => {
                        Err("Cursor could not be grabbed: frozen by another client")
                    }
                    _ => unreachable!(),
                }
                .map_err(|err| ExternalError::Os(os_error!(OsError::XMisc(err))));
                break res;
            }
        } else {
            self.xconn
                .check_pending1(ungrab)
                .map_err(|err| ExternalError::Os(os_error!(OsError::XError(err.into()))))
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
            Some(*self.cursor.lock())
        } else {
            None
        };
        *visible_lock = visible;
        drop(visible_lock);
        self.xconn.set_cursor_icon(self.xwindow as _, cursor);
    }

    #[inline]
    pub fn scale_factor(&self) -> f64 {
        self.current_monitor().scale_factor
    }

    pub fn set_cursor_position_physical(&self, x: i16, y: i16) -> Result<(), ExternalError> {
        unsafe {
            let pending = self
                .xconn
                .xcb
                .xcb_warp_pointer_checked(self.xconn.c, 0, self.xwindow, 0, 0, 0, 0, x, y)
                .into();
            self.xconn
                .check_pending1(pending)
                .map_err(|err| ExternalError::Os(os_error!(OsError::XError(err.into()))))
        }
    }

    #[inline]
    pub fn set_cursor_position(&self, position: Position) -> Result<(), ExternalError> {
        let (x, y) = position.to_physical::<i16>(self.scale_factor()).into();
        self.set_cursor_position_physical(x, y)
    }

    pub fn drag_window(&self) -> Result<(), ExternalError> {
        let pointer = self
            .xconn
            .query_pointer(self.xwindow as _, util::VIRTUAL_CORE_POINTER)
            .map_err(|err| ExternalError::Os(os_error!(OsError::XError(err.into()))))?;

        let window = self.inner_position().map_err(ExternalError::NotSupported)?;

        let message = self.xconn.get_atom("_NET_WM_MOVERESIZE");

        // we can't use `set_cursor_grab(false)` here because it doesn't run `XUngrabPointer`
        // if the cursor isn't currently grabbed
        let mut grabbed_lock = self.cursor_grabbed.lock();
        let pending = unsafe {
            self.xconn
                .xcb
                .xcb_ungrab_pointer_checked(self.xconn.c, ffi::XCB_TIME_CURRENT_TIME)
                .into()
        };
        self.xconn
            .check_pending1(pending)
            .map_err(|err| ExternalError::Os(os_error!(OsError::XError(err.into()))))?;
        *grabbed_lock = false;

        // we keep the lock until we are done
        let pending = self.xconn.send_client_msg(
            self.xwindow,
            self.screen.root,
            message,
            Some(
                ffi::XCB_EVENT_MASK_SUBSTRUCTURE_REDIRECT | ffi::XCB_EVENT_MASK_SUBSTRUCTURE_NOTIFY,
            ),
            [
                window.x + util::fp1616_to_f64(pointer.win_x) as i32,
                window.y + util::fp1616_to_f64(pointer.win_y) as i32,
                8, // _NET_WM_MOVERESIZE_MOVE
                ffi::XCB_BUTTON_INDEX_1 as _,
                1,
            ],
        );

        self.xconn
            .check_pending1(pending)
            .map_err(|err| ExternalError::Os(os_error!(OsError::XError(err.into()))))
    }

    #[inline]
    pub fn set_ime_position(&self, _spot: Position) {}

    #[inline]
    pub fn reset_dead_keys(&self) {
        self.reset_dead_keys.fetch_add(1, Relaxed);
    }

    #[inline]
    pub fn request_user_attention(&self, request_type: Option<UserAttentionType>) {
        let mut wm_hints = match self.xconn.get_wm_hints(self.xwindow) {
            Ok(h) => h,
            Err(e) => {
                log::error!("Could not retrieve WM hints: {}", e);
                return;
            }
        };
        wm_hints.set_urgency(request_type.is_some());
        let pending = self.xconn.set_wm_hints(self.xwindow, wm_hints);
        if let Err(e) = self.xconn.check_pending1(pending) {
            log::error!("Could not set WM hints: {}", e);
        }
    }

    #[inline]
    pub fn id(&self) -> WindowId {
        WindowId(self.xwindow)
    }

    #[inline]
    pub fn request_redraw(&self) {
        self.redraw_sender.send(WindowId(self.xwindow)).unwrap();
    }

    #[inline]
    pub fn raw_window_handle(&self) -> XcbHandle {
        XcbHandle {
            window: self.xwindow,
            connection: self.xconn.c as _,
            ..XcbHandle::empty()
        }
    }
}
