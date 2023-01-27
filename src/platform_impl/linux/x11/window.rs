use std::{
    cmp, env,
    mem::{replace, MaybeUninit},
    os::raw::*,
    path::Path,
    ptr, slice,
    sync::{mpsc::Sender, Arc, Mutex, MutexGuard},
};

use libc;
use raw_window_handle::{RawDisplayHandle, RawWindowHandle, XlibDisplayHandle, XlibWindowHandle};
use x11rb::{
    connection::Connection,
    protocol::{
        randr,
        xinput::{self, ConnectionExt as _},
        xkb::{self, ConnectionExt as _},
        xproto::{self, ConnectionExt as _},
    },
};

use crate::{
    dpi::{PhysicalPosition, PhysicalSize, Position, Size},
    error::{ExternalError, NotSupportedError, OsError as RootOsError},
    platform_impl::{
        x11::MonitorHandle as X11MonitorHandle, Fullscreen, MonitorHandle as PlatformMonitorHandle,
        OsError, PlatformSpecificWindowBuilderAttributes, VideoMode as PlatformVideoMode,
    },
    window::{
        CursorGrabMode, CursorIcon, Icon, ResizeDirection, Theme, UserAttentionType,
        WindowAttributes, WindowButtons, WindowLevel,
    },
};

use super::{
    atoms::*,
    ffi,
    ime::ImeRequest,
    util::{self, PlErrorExt, VoidResultExt, XcbVoidCookie},
    EventLoopWindowTarget, PlatformError, WakeSender, WindowId, XConnection,
};
use std::option::Option::None;

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
    pub(crate) fullscreen: Option<Fullscreen>,
    // Set when application calls `set_fullscreen` when window is not visible
    pub(crate) desired_fullscreen: Option<Option<Fullscreen>>,
    // Used to restore position after exiting fullscreen
    pub restore_position: Option<(i32, i32)>,
    // Used to restore video mode after exiting fullscreen
    pub desktop_video_mode: Option<(u32, randr::Mode)>,
    pub frame_extents: Option<util::FrameExtentsHeuristic>,
    pub min_inner_size: Option<Size>,
    pub max_inner_size: Option<Size>,
    pub resize_increments: Option<Size>,
    pub base_size: Option<Size>,
    pub visibility: Visibility,
    pub has_focus: bool,
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
            has_focus: false,
        })
    }
}

unsafe impl Send for UnownedWindow {}
unsafe impl Sync for UnownedWindow {}

pub(crate) struct UnownedWindow {
    pub(crate) xconn: Arc<XConnection>, // never changes
    xwindow: xproto::Window,            // never changes
    root: xproto::Window,               // never changes
    screen_id: usize,                   // never changes
    cursor: Mutex<CursorIcon>,
    cursor_grabbed_mode: Mutex<CursorGrabMode>,
    #[allow(clippy::mutex_atomic)]
    cursor_visible: Mutex<bool>,
    pub shared_state: Mutex<SharedState>,
    redraw_sender: WakeSender<WindowId>,

    /// Send IME update requests to the event loop.
    ime_sender: Mutex<Sender<ImeRequest>>,
}

impl UnownedWindow {
    pub(crate) fn new<T>(
        event_loop: &EventLoopWindowTarget<T>,
        window_attrs: WindowAttributes,
        pl_attribs: PlatformSpecificWindowBuilderAttributes,
    ) -> Result<UnownedWindow, RootOsError> {
        macro_rules! unwrap_os {
            ($e:expr) => {{
                match $e {
                    Ok(v) => v,
                    Err(e) => return Err(os_error!(OsError::from(PlatformError::from(e)))),
                }
            }};
        }

        let xconn = &event_loop.xconn;
        let root = match window_attrs.parent_window {
            Some(RawWindowHandle::Xlib(handle)) => handle.window as xproto::Window,
            Some(RawWindowHandle::Xcb(handle)) => handle.window,
            Some(raw) => unreachable!("Invalid raw window handle {raw:?} on X11"),
            None => event_loop.root,
        };

        let mut monitors = xconn.available_monitors();
        let guessed_monitor = if monitors.is_empty() {
            X11MonitorHandle::dummy()
        } else {
            xconn
                .connection
                .xinput_xi_query_pointer(root, util::VIRTUAL_CORE_POINTER)
                .platform()
                .and_then(|r| r.reply().platform())
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
            Some(id) => id as _,
            None => xconn.default_screen,
        };

        // creating
        let (visual_id, depth, require_colormap) = match pl_attribs.visual_infos {
            Some(vi) => (vi.visualid as _, vi.depth as u8, false),
            None if window_attrs.transparent => {
                // Find a suitable visual.
                let desired_root = &xconn.connection.setup().roots[screen_id];
                let desired_visual = desired_root
                    .allowed_depths
                    .iter()
                    .filter(|depth| depth.depth == 32)
                    .flat_map(|depth| depth.visuals.iter().map(move |visual| (depth, visual)))
                    .find(|(_, visual)| visual.class == xproto::VisualClass::TRUE_COLOR);

                if let Some((depth, visual)) = desired_visual {
                    (visual.visual_id, depth.depth, true)
                } else {
                    debug!("Could not set transparency, because XMatchVisualInfo returned zero for the required parameters");
                    (x11rb::COPY_FROM_PARENT, x11rb::COPY_FROM_PARENT as _, false)
                }
            }
            _ => (x11rb::COPY_FROM_PARENT, x11rb::COPY_FROM_PARENT as _, false),
        };

        let set_win_attr = {
            let mut swa = xproto::CreateWindowAux::new();
            swa = swa.colormap({
                // See if we should use a direct visual or a colormap.
                let mut visual = pl_attribs.visual_infos.map(|vi| vi.visualid as _);

                if require_colormap {
                    visual = visual.or(Some(visual_id));
                }

                if let Some(visual_id) = visual {
                    let result = xconn.connection.generate_id().platform().and_then(|id| {
                        // Create a colormap.
                        xconn
                            .connection
                            .create_colormap(xproto::ColormapAlloc::NONE, id, root, visual_id)
                            .platform()
                            .map(|tok| {
                                tok.ignore_error();
                                id
                            })
                    });

                    unwrap_os!(result)
                } else {
                    0
                }
            });

            swa = swa.event_mask(
                xproto::EventMask::EXPOSURE
                    | xproto::EventMask::STRUCTURE_NOTIFY
                    | xproto::EventMask::PROPERTY_CHANGE
                    | xproto::EventMask::KEY_PRESS
                    | xproto::EventMask::KEY_RELEASE
                    | xproto::EventMask::KEYMAP_STATE
                    | xproto::EventMask::BUTTON_PRESS
                    | xproto::EventMask::BUTTON_RELEASE
                    | xproto::EventMask::POINTER_MOTION,
            );
            swa = swa.border_pixel(0);
            swa = swa.override_redirect(u32::from(pl_attribs.override_redirect));
            swa
        };

        // Create the window.
        let xwindow = match xconn.connection.generate_id().platform().and_then(|wid| {
            xconn
                .connection
                .create_window(
                    depth,
                    wid,
                    root,
                    position.map_or(0, |p: PhysicalPosition<i32>| p.x as _),
                    position.map_or(0, |p: PhysicalPosition<i32>| p.y as _),
                    dimensions.0 as _,
                    dimensions.1 as _,
                    0,
                    xproto::WindowClass::INPUT_OUTPUT,
                    visual_id,
                    &set_win_attr,
                )
                .platform()
                .map(|tok| {
                    tok.ignore_error();
                    wid
                })
        }) {
            Ok(wid) => wid,
            Err(err) => return Err(os_error!(err.into())),
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
            shared_state: SharedState::new(guessed_monitor, &window_attrs),
            redraw_sender: WakeSender {
                waker: event_loop.redraw_sender.waker.clone(),
                sender: event_loop.redraw_sender.sender.clone(),
            },
            ime_sender: Mutex::new(event_loop.ime_sender.clone()),
        };

        // Title must be set before mapping. Some tiling window managers (i.e. i3) use the window
        // title to determine placement/etc., so doing this after mapping would cause the WM to
        // act on the wrong title state.
        unwrap_os!(window.set_title_inner(&window_attrs.title)).ignore_error();
        unwrap_os!(window.set_decorations_inner(window_attrs.decorations)).ignore_error();

        if let Some(theme) = window_attrs.preferred_theme {
            unwrap_os!(window.set_theme_inner(Some(theme))).ignore_error();
        }

        {
            // Enable drag and drop (TODO: extend API to make this toggleable)
            {
                let dnd_aware_atom = xconn.atoms[XdndAware];
                let version = &[5 as c_ulong]; // Latest version; hasn't changed since 2002
                unwrap_os!(xconn.change_property(
                    window.xwindow,
                    dnd_aware_atom,
                    xproto::AtomEnum::ATOM.into(),
                    xproto::PropMode::REPLACE,
                    version,
                ))
                .ignore_error();
            }

            // WM_CLASS must be set *before* mapping the window, as per ICCCM!
            {
                let (class, instance) = if let Some(name) = pl_attribs.name {
                    (name.instance, name.general)
                } else {
                    let class = env::args_os()
                        .next()
                        .as_ref()
                        // Default to the name of the binary (via argv[0])
                        .and_then(|path| Path::new(path).file_name())
                        .and_then(|bin_name| bin_name.to_str())
                        .map(|bin_name| bin_name.to_owned())
                        .or_else(|| Some(window_attrs.title.clone()))
                        .unwrap();
                    // This environment variable is extraordinarily unlikely to actually be used...
                    let instance = env::var("RESOURCE_NAME")
                        .ok()
                        .or_else(|| Some(class.clone()))
                        .unwrap();
                    (instance, class)
                };

                // Create the class hint and set it.
                let class_hint = format!("{}\0{}\0", class, instance);

                unwrap_os!(xconn.change_property(
                    window.xwindow,
                    xproto::AtomEnum::WM_CLASS.into(),
                    xproto::AtomEnum::ATOM.into(),
                    xproto::PropMode::REPLACE,
                    class_hint.as_bytes(),
                ))
                .ignore_error();
            }

            if let Some(flusher) = unwrap_os!(window.set_pid()) {
                flusher.ignore_error();
            }

            unwrap_os!(window.set_window_types(pl_attribs.x11_window_types)).ignore_error();

            // set size hints
            {
                let mut min_inner_size = window_attrs
                    .min_inner_size
                    .map(|size| size.to_physical::<u32>(scale_factor));
                let mut max_inner_size = window_attrs
                    .max_inner_size
                    .map(|size| size.to_physical::<u32>(scale_factor));

                if !window_attrs.resizable {
                    if xconn.wm_name_is_one_of(&["Xfwm4"]) {
                        warn!("To avoid a WM bug, disabling resizing has no effect on Xfwm4");
                    } else {
                        max_inner_size = Some(dimensions.into());
                        min_inner_size = Some(dimensions.into());
                    }
                }

                let mut shared_state = window.shared_state.get_mut().unwrap();
                shared_state.min_inner_size = min_inner_size.map(Into::into);
                shared_state.max_inner_size = max_inner_size.map(Into::into);
                shared_state.resize_increments = window_attrs.resize_increments;
                shared_state.base_size = pl_attribs.base_size;

                let mut normal_hints = util::NormalHints::new(xconn);
                normal_hints.set_position(position.map(|PhysicalPosition { x, y }| (x, y)));
                normal_hints.set_size(Some(dimensions));
                normal_hints.set_min_size(min_inner_size.map(Into::into));
                normal_hints.set_max_size(max_inner_size.map(Into::into));
                normal_hints.set_resize_increments(
                    window_attrs
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
                unwrap_os!(window.set_icon_inner(icon)).ignore_error();
            }

            // Opt into handling window close
            {
                unwrap_os!(xconn.change_property(
                    window.xwindow,
                    xconn.atoms[WM_PROTOCOLS],
                    xproto::AtomEnum::ATOM.into(),
                    xproto::PropMode::REPLACE,
                    &[xconn.atoms[WM_DELETE_WINDOW], xconn.atoms[_NET_WM_PING]],
                ))
                .ignore_error();
            }

            // Set visibility (map window)
            if window_attrs.visible {
                unwrap_os!(window.map_raised()).ignore_error();
            }

            // Attempt to make keyboard input repeat detectable
            {
                let pcf = unwrap_os!(unwrap_os!(xconn.connection.xkb_per_client_flags(
                    xkb::ID::USE_CORE_KBD.into(),
                    xkb::PerClientFlag::DETECTABLE_AUTO_REPEAT,
                    xkb::PerClientFlag::DETECTABLE_AUTO_REPEAT,
                    xkb::BoolCtrl::from(0u32),
                    xkb::BoolCtrl::from(0u32),
                    xkb::BoolCtrl::from(0u32),
                ))
                .reply());

                if u32::from(pcf.supported) & u32::from(xkb::PerClientFlag::DETECTABLE_AUTO_REPEAT)
                    == 0
                {
                    return Err(os_error!(OsError::XMisc(
                        "`XkbSetDetectableAutoRepeat` not supported"
                    )));
                }
            }

            // Select XInput2 events
            let mask = xinput::XIEventMask::MOTION
                | xinput::XIEventMask::BUTTON_PRESS
                | xinput::XIEventMask::BUTTON_RELEASE
                | xinput::XIEventMask::ENTER
                | xinput::XIEventMask::LEAVE
                | xinput::XIEventMask::FOCUS_IN
                | xinput::XIEventMask::FOCUS_OUT
                | xinput::XIEventMask::TOUCH_BEGIN
                | xinput::XIEventMask::TOUCH_UPDATE
                | xinput::XIEventMask::TOUCH_END;

            unwrap_os!(xconn.connection.xinput_xi_select_events(
                window.xwindow,
                &[xinput::EventMask {
                    deviceid: ffi::XIAllMasterDevices as _,
                    mask: vec![mask]
                }]
            ))
            .ignore_error();

            {
                if let Some(ime) = event_loop.ime.as_ref() {
                    if ime.create_context(window.xwindow, false, None).is_err() {
                        return Err(os_error!(OsError::XMisc("IME Context creation failed")));
                    }
                }
            }

            // These properties must be set after mapping
            if window_attrs.maximized {
                unwrap_os!(window.set_maximized_inner(window_attrs.maximized)).ignore_error();
            }
            if window_attrs.fullscreen.is_some() {
                if let Some(flusher) =
                    unwrap_os!(window
                        .set_fullscreen_inner(window_attrs.fullscreen.clone().map(Into::into)))
                {
                    flusher.ignore_error();
                }

                if let Some(PhysicalPosition { x, y }) = position {
                    let shared_state = window.shared_state.get_mut().unwrap();

                    shared_state.restore_position = Some((x, y));
                }
            }

            unwrap_os!(window.set_window_level_inner(window_attrs.window_level)).ignore_error();
        }

        // We never want to give the user a broken window, since by then, it's too late to handle.
        xconn
            .sync_with_server()
            .map(|_| window)
            .map_err(|x_err| os_error!(OsError::XError(x_err.into())))
    }

    pub(super) fn shared_state_lock(&self) -> MutexGuard<'_, SharedState> {
        self.shared_state.lock().unwrap()
    }

    fn set_pid(&self) -> Result<Option<XcbVoidCookie<'_>>, PlatformError> {
        let pid_atom = self.xconn.atoms[_NET_WM_PID];
        let client_machine_atom = self.xconn.atoms[WM_CLIENT_MACHINE];

        // 64 would suffice for Linux, but 256 will be enough everywhere (as per SUSv2). For instance, this is
        // the limit defined by OpenBSD.
        const MAXHOSTNAMELEN: usize = 256;
        // `assume_init` is safe here because the array consists of `MaybeUninit` values,
        // which do not require initialization.
        let mut buffer: [MaybeUninit<c_char>; MAXHOSTNAMELEN] =
            unsafe { MaybeUninit::uninit().assume_init() };

        // Get the hostname.
        let hostname = unsafe {
            let status = libc::gethostname(buffer.as_mut_ptr() as *mut c_char, buffer.len());
            if status != 0 {
                return Ok(None);
            }
            ptr::write(buffer[MAXHOSTNAMELEN - 1].as_mut_ptr() as *mut u8, b'\0'); // a little extra safety
            let hostname_length = libc::strlen(buffer.as_ptr() as *const c_char);

            slice::from_raw_parts(buffer.as_ptr() as *const u8, hostname_length)
        };

        let pid = unsafe { libc::getpid() as util::Cardinal };

        self.xconn
            .change_property(
                self.xwindow,
                pid_atom,
                xproto::AtomEnum::CARDINAL.into(),
                xproto::PropMode::REPLACE,
                &[pid],
            )?
            .ignore_error();
        self.xconn
            .change_property(
                self.xwindow,
                client_machine_atom,
                xproto::AtomEnum::STRING.into(),
                xproto::PropMode::REPLACE,
                hostname,
            )
            .map(Some)
    }

    fn set_window_types(
        &self,
        window_types: Vec<util::WindowType>,
    ) -> Result<XcbVoidCookie<'_>, PlatformError> {
        let hint_atom = self.xconn.atoms[_NET_WM_WINDOW_TYPE];
        let atoms: Vec<_> = window_types
            .iter()
            .map(|t| t.as_atom(&self.xconn))
            .collect();

        self.xconn.change_property(
            self.xwindow,
            hint_atom,
            xproto::AtomEnum::ATOM.into(),
            xproto::PropMode::REPLACE,
            &atoms,
        )
    }

    pub fn set_theme_inner(
        &self,
        theme: Option<Theme>,
    ) -> Result<XcbVoidCookie<'_>, PlatformError> {
        let hint_atom = self.xconn.atoms[_GTK_THEME_VARIANT];
        let utf8_atom = self.xconn.atoms[UTF8_STRING];
        let variant = match theme {
            Some(Theme::Dark) => "dark",
            Some(Theme::Light) => "light",
            None => "dark",
        };
        self.xconn.change_property(
            self.xwindow,
            hint_atom,
            utf8_atom,
            xproto::PropMode::REPLACE,
            variant.as_bytes(),
        )
    }

    #[inline]
    pub fn set_theme(&self, theme: Option<Theme>) {
        self.set_theme_inner(theme)
            .check()
            .expect("Failed to change window theme")
    }

    fn set_netwm(
        &self,
        operation: util::StateOperation,
        properties: (u32, u32, u32, u32),
    ) -> Result<XcbVoidCookie<'_>, PlatformError> {
        let state_atom = self.xconn.atoms[_NET_WM_STATE];
        self.xconn.send_client_msg(
            self.xwindow,
            self.root,
            state_atom,
            Some(xproto::EventMask::SUBSTRUCTURE_REDIRECT | xproto::EventMask::SUBSTRUCTURE_NOTIFY),
            32,
            [
                operation as u32,
                properties.0,
                properties.1,
                properties.2,
                properties.3,
            ],
        )
    }

    fn set_fullscreen_hint(&self, fullscreen: bool) -> Result<XcbVoidCookie<'_>, PlatformError> {
        let fullscreen_atom = self.xconn.atoms[_NET_WM_STATE_FULLSCREEN];
        let flusher = self.set_netwm(fullscreen.into(), (fullscreen_atom, 0, 0, 0))?;

        if fullscreen {
            // Ensure that the fullscreen window receives input focus to prevent
            // locking up the user's display.
            self.xconn
                .connection
                .set_input_focus(
                    xproto::InputFocus::PARENT,
                    self.xwindow,
                    xproto::Time::CURRENT_TIME,
                )?
                .ignore_error();
        }

        Ok(flusher)
    }

    fn set_fullscreen_inner(
        &self,
        fullscreen: Option<Fullscreen>,
    ) -> Result<Option<XcbVoidCookie<'_>>, PlatformError> {
        let mut shared_state_lock = self.shared_state_lock();

        match shared_state_lock.visibility {
            // Setting fullscreen on a window that is not visible will generate an error.
            Visibility::No | Visibility::YesWait => {
                shared_state_lock.desired_fullscreen = Some(fullscreen);
                return Ok(None);
            }
            Visibility::Yes => (),
        }

        let old_fullscreen = shared_state_lock.fullscreen.clone();
        if old_fullscreen == fullscreen {
            return Ok(None);
        }
        shared_state_lock.fullscreen = fullscreen.clone();

        match (&old_fullscreen, &fullscreen) {
            // Store the desktop video mode before entering exclusive
            // fullscreen, so we can restore it upon exit, as XRandR does not
            // provide a mechanism to set this per app-session or restore this
            // to the desktop video mode as macOS and Windows do
            (&None, &Some(Fullscreen::Exclusive(PlatformVideoMode::X(ref video_mode))))
            | (
                &Some(Fullscreen::Borderless(_)),
                &Some(Fullscreen::Exclusive(PlatformVideoMode::X(ref video_mode))),
            ) => {
                let monitor = video_mode.monitor.as_ref().unwrap();
                shared_state_lock.desktop_video_mode =
                    Some((monitor.id, self.xconn.get_crtc_mode(monitor.id)?));
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
                let flusher = self.set_fullscreen_hint(false)?;
                let mut shared_state_lock = self.shared_state_lock();
                if let Some(position) = shared_state_lock.restore_position.take() {
                    drop(shared_state_lock);
                    self.set_position_inner(position.0, position.1)?
                        .ignore_error();
                }
                Ok(Some(flusher))
            }
            Some(fullscreen) => {
                let (video_mode, monitor) = match fullscreen {
                    Fullscreen::Exclusive(PlatformVideoMode::X(ref video_mode)) => {
                        (Some(video_mode), video_mode.monitor.clone().unwrap())
                    }
                    Fullscreen::Borderless(Some(PlatformMonitorHandle::X(monitor))) => {
                        (None, monitor)
                    }
                    Fullscreen::Borderless(None) => (None, self.current_monitor()),
                    #[cfg(wayland_platform)]
                    _ => unreachable!(),
                };

                // Don't set fullscreen on an invalid dummy monitor handle
                if monitor.is_dummy() {
                    return Ok(None);
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
                self.set_position_inner(monitor_origin.0, monitor_origin.1)?
                    .ignore_error();
                Ok(Some(self.set_fullscreen_hint(true)?))
            }
        }
    }

    #[inline]
    pub(crate) fn fullscreen(&self) -> Option<Fullscreen> {
        let shared_state = self.shared_state_lock();

        shared_state
            .desired_fullscreen
            .clone()
            .unwrap_or_else(|| shared_state.fullscreen.clone())
    }

    #[inline]
    pub(crate) fn set_fullscreen(&self, fullscreen: Option<Fullscreen>) {
        if let Ok(Some(flusher)) = self.set_fullscreen_inner(fullscreen) {
            flusher
                .check()
                .expect("Failed to change window fullscreen state");
            self.invalidate_cached_frame_extents();
        }
    }

    // Called by EventProcessor when a VisibilityNotify event is received
    pub(crate) fn visibility_notify(&self) {
        let mut shared_state = self.shared_state_lock();

        match shared_state.visibility {
            Visibility::No => {
                self.xconn
                    .connection
                    .unmap_window(self.xwindow)
                    .expect("Failed to unmap window")
                    .ignore_error();
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
        self.shared_state_lock().last_monitor.clone()
    }

    pub fn available_monitors(&self) -> Vec<X11MonitorHandle> {
        self.xconn.available_monitors()
    }

    pub fn primary_monitor(&self) -> X11MonitorHandle {
        self.xconn.primary_monitor()
    }

    #[inline]
    pub fn is_minimized(&self) -> Option<bool> {
        let state_atom = self.xconn.atoms[_NET_WM_STATE];
        let state =
            self.xconn
                .get_property(self.xwindow, state_atom, xproto::AtomEnum::ATOM.into());
        let hidden_atom = self.xconn.atoms[_NET_WM_STATE_HIDDEN];

        Some(match state {
            Ok(atoms) => atoms.iter().any(|atom: &xproto::Atom| *atom == hidden_atom),
            _ => false,
        })
    }

    fn set_minimized_inner(&self, minimized: bool) -> Result<XcbVoidCookie<'_>, PlatformError> {
        if minimized {
            let screen_root = self.xconn.connection.setup().roots[self.xconn.default_screen].root;

            self.xconn.send_client_msg(
                self.xwindow,
                screen_root,
                self.xconn.atoms[WM_CHANGE_STATE],
                Some(
                    xproto::EventMask::SUBSTRUCTURE_REDIRECT
                        | xproto::EventMask::SUBSTRUCTURE_NOTIFY,
                ),
                32,
                [x11rb::properties::WmHintsState::Iconic as u32, 0, 0, 0, 0],
            )
        } else {
            self.xconn.send_client_msg(
                self.xwindow,
                self.root,
                self.xconn.atoms[_NET_ACTIVE_WINDOW],
                Some(
                    xproto::EventMask::SUBSTRUCTURE_REDIRECT
                        | xproto::EventMask::SUBSTRUCTURE_NOTIFY,
                ),
                32,
                [1, x11rb::CURRENT_TIME, 0, 0, 0],
            )
        }
    }

    #[inline]
    pub fn set_minimized(&self, minimized: bool) {
        self.set_minimized_inner(minimized)
            .expect("Failed to change window minimization")
            .check()
            .expect("Failed to change window minimization");
    }

    #[inline]
    pub fn is_maximized(&self) -> bool {
        let state_atom = self.xconn.atoms[_NET_WM_STATE];
        let state =
            self.xconn
                .get_property(self.xwindow, state_atom, xproto::AtomEnum::ATOM.into());
        let horz_atom = self.xconn.atoms[_NET_WM_STATE_MAXIMIZED_HORZ];
        let vert_atom = self.xconn.atoms[_NET_WM_STATE_MAXIMIZED_VERT];
        match state {
            Ok(atoms) => {
                let horz_maximized = atoms.iter().any(|atom: &xproto::Atom| *atom == horz_atom);
                let vert_maximized = atoms.iter().any(|atom: &xproto::Atom| *atom == vert_atom);
                horz_maximized && vert_maximized
            }
            _ => false,
        }
    }

    fn set_maximized_inner(&self, maximized: bool) -> Result<XcbVoidCookie<'_>, PlatformError> {
        let horz_atom = self.xconn.atoms[_NET_WM_STATE_MAXIMIZED_HORZ];
        let vert_atom = self.xconn.atoms[_NET_WM_STATE_MAXIMIZED_VERT];
        self.set_netwm(maximized.into(), (horz_atom, vert_atom, 0, 0))
    }

    #[inline]
    pub fn set_maximized(&self, maximized: bool) {
        self.set_maximized_inner(maximized)
            .check()
            .expect("Failed to change window maximization");
        self.invalidate_cached_frame_extents();
    }

    fn set_title_inner(&self, title: &str) -> Result<util::XcbVoidCookie<'_>, PlatformError> {
        let wm_name_atom = self.xconn.atoms[_NET_WM_NAME];
        let utf8_atom = self.xconn.atoms[UTF8_STRING];

        self.xconn.change_property(
            self.xwindow,
            wm_name_atom,
            utf8_atom,
            xproto::PropMode::REPLACE,
            title.as_bytes(),
        )
    }

    #[inline]
    pub fn set_title(&self, title: &str) {
        self.set_title_inner(title)
            .check()
            .expect("Failed to set window title");
    }

    #[inline]
    pub fn set_transparent(&self, _transparent: bool) {}

    fn set_decorations_inner(&self, decorations: bool) -> Result<XcbVoidCookie<'_>, PlatformError> {
        self.shared_state_lock().is_decorated = decorations;
        let mut hints = self.xconn.get_motif_hints(self.xwindow)?;

        hints.set_decorations(decorations);

        self.xconn.set_motif_hints(self.xwindow, &hints)
    }

    #[inline]
    pub fn set_decorations(&self, decorations: bool) {
        self.set_decorations_inner(decorations)
            .check()
            .expect("Failed to set decoration state");
        self.invalidate_cached_frame_extents();
    }

    #[inline]
    pub fn is_decorated(&self) -> bool {
        self.shared_state_lock().is_decorated
    }

    fn set_maximizable_inner(&self, maximizable: bool) -> Result<XcbVoidCookie<'_>, PlatformError> {
        let mut hints = self.xconn.get_motif_hints(self.xwindow)?;

        hints.set_maximizable(maximizable);

        self.xconn.set_motif_hints(self.xwindow, &hints)
    }

    fn toggle_atom(
        &self,
        atom_name: AtomType,
        enable: bool,
    ) -> Result<XcbVoidCookie<'_>, PlatformError> {
        let atom = self.xconn.atoms[atom_name];
        self.set_netwm(enable.into(), (atom, 0, 0, 0))
    }

    fn set_window_level_inner(
        &self,
        level: WindowLevel,
    ) -> Result<XcbVoidCookie<'_>, PlatformError> {
        self.toggle_atom(_NET_WM_STATE_ABOVE, level == WindowLevel::AlwaysOnTop)?
            .ignore_error();
        self.toggle_atom(_NET_WM_STATE_BELOW, level == WindowLevel::AlwaysOnBottom)
    }

    #[inline]
    pub fn set_window_level(&self, level: WindowLevel) {
        self.set_window_level_inner(level)
            .check()
            .expect("Failed to set window-level state");
    }

    fn set_icon_inner(&self, icon: Icon) -> Result<XcbVoidCookie<'_>, PlatformError> {
        let icon_atom = self.xconn.atoms[_NET_WM_ICON];
        let data = icon.to_cardinals();
        self.xconn.change_property(
            self.xwindow,
            icon_atom,
            xproto::AtomEnum::CARDINAL.into(),
            xproto::PropMode::REPLACE,
            data.as_slice(),
        )
    }

    fn unset_icon_inner(&self) -> Result<XcbVoidCookie<'_>, PlatformError> {
        let icon_atom = self.xconn.atoms[_NET_WM_ICON];
        let empty_data: [util::Cardinal; 0] = [];
        self.xconn.change_property(
            self.xwindow,
            icon_atom,
            xproto::AtomEnum::CARDINAL.into(),
            xproto::PropMode::REPLACE,
            &empty_data,
        )
    }

    #[inline]
    pub fn set_window_icon(&self, icon: Option<Icon>) {
        match icon {
            Some(icon) => self.set_icon_inner(icon),
            None => self.unset_icon_inner(),
        }
        .check()
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
            self.map_raised()
                .check()
                .expect("Failed to call XMapRaised");
            shared_state.visibility = Visibility::YesWait;
        } else {
            self.xconn
                .connection
                .unmap_window(self.xwindow)
                .check()
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
        self.shared_state_lock().frame_extents = Some(extents);
    }

    pub(crate) fn invalidate_cached_frame_extents(&self) {
        self.shared_state_lock().frame_extents.take();
    }

    pub(crate) fn outer_position_physical(&self) -> (i32, i32) {
        let extents = self.shared_state_lock().frame_extents.clone();
        if let Some(extents) = extents {
            let (x, y) = self
                .inner_position_physical()
                .expect("Failed to get inner position");
            extents.inner_pos_to_outer(x, y)
        } else {
            self.update_cached_frame_extents();
            self.outer_position_physical()
        }
    }

    #[inline]
    pub fn outer_position(&self) -> Result<PhysicalPosition<i32>, NotSupportedError> {
        let extents = self.shared_state_lock().frame_extents.clone();
        if let Some(extents) = extents {
            let (x, y) = self
                .inner_position_physical()
                .expect("Failed to get inner position");
            Ok(extents.inner_pos_to_outer(x, y).into())
        } else {
            self.update_cached_frame_extents();
            self.outer_position()
        }
    }

    pub(crate) fn inner_position_physical(&self) -> Result<(i32, i32), PlatformError> {
        // This should be okay to unwrap since the only error XTranslateCoordinates can return
        // is BadWindow, and if the window handle is bad we have bigger problems.
        self.xconn
            .connection
            .translate_coordinates(self.xwindow, self.root, 0, 0)
            .platform()
            .and_then(|c| c.reply().platform())
            .map(|coords| (coords.dst_x.into(), coords.dst_y.into()))
    }

    #[inline]
    pub fn inner_position(&self) -> Result<PhysicalPosition<i32>, NotSupportedError> {
        Ok(self
            .inner_position_physical()
            .expect("Failed to get physical position")
            .into())
    }

    pub(crate) fn set_position_inner(
        &self,
        mut x: i32,
        mut y: i32,
    ) -> Result<XcbVoidCookie<'_>, PlatformError> {
        // There are a few WMs that set client area position rather than window position, so
        // we'll translate for consistency.
        if self.xconn.wm_name_is_one_of(&["Enlightenment", "FVWM"]) {
            let extents = self.shared_state_lock().frame_extents.clone();
            if let Some(extents) = extents {
                x += extents.frame_extents.left as i32;
                y += extents.frame_extents.top as i32;
            } else {
                self.update_cached_frame_extents();
                return self.set_position_inner(x, y);
            }
        }

        self.xconn
            .connection
            .configure_window(self.xwindow, &xproto::ConfigureWindowAux::new().x(x).y(y))
            .platform()
    }

    pub(crate) fn set_position_physical(&self, x: i32, y: i32) {
        self.set_position_inner(x, y)
            .check()
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
            .connection
            .get_geometry(self.xwindow)
            .platform()
            .and_then(|c| c.reply().platform())
            .map(|geo| (geo.width.into(), geo.height.into()))
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
        self.xconn
            .connection
            .configure_window(
                self.xwindow,
                &xproto::ConfigureWindowAux::new()
                    .width(width)
                    .height(height),
            )
            .check()
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

    fn update_normal_hints<F>(&self, callback: F) -> Result<(), PlatformError>
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

    #[inline]
    pub fn resize_increments(&self) -> Option<PhysicalSize<u32>> {
        self.xconn
            .get_normal_hints(self.xwindow)
            .ok()
            .and_then(|hints| hints.get_resize_increments())
            .map(Into::into)
    }

    #[inline]
    pub fn set_resize_increments(&self, increments: Option<Size>) {
        self.shared_state_lock().resize_increments = increments;
        let physical_increments =
            increments.map(|increments| increments.to_physical::<u32>(self.scale_factor()).into());
        self.update_normal_hints(|hints| hints.set_resize_increments(physical_increments))
            .expect("Failed to call `XSetWMNormalHints`");
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
            let max_size = shared_state.max_inner_size.map(dpi_adjuster);
            let min_size = shared_state.min_inner_size.map(dpi_adjuster);
            let resize_increments = shared_state.resize_increments.map(dpi_adjuster);
            let base_size = shared_state.base_size.map(dpi_adjuster);
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
        if self.xconn.wm_name_is_one_of(&["Xfwm4"]) {
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

        self.set_maximizable_inner(resizable)
            .check()
            .expect("Failed to set maximizable");

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
    pub fn set_enabled_buttons(&self, _buttons: WindowButtons) {}

    #[inline]
    pub fn enabled_buttons(&self) -> WindowButtons {
        WindowButtons::all()
    }

    #[inline]
    pub fn xlib_display(&self) -> *mut c_void {
        self.xconn.display.as_ptr() as _
    }

    #[inline]
    pub fn xlib_screen_id(&self) -> c_int {
        self.screen_id as _
    }

    #[inline]
    pub fn xlib_window(&self) -> c_ulong {
        self.xwindow as _
    }

    #[inline]
    pub fn xcb_connection(&self) -> *mut c_void {
        self.xconn.connection.get_raw_xcb_connection()
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

        {
            // We ungrab before grabbing to prevent passive grabs from causing `AlreadyGrabbed`.
            // Therefore, this is common to both codepaths.
            self.xconn
                .connection
                .ungrab_pointer(xproto::Time::CURRENT_TIME)
                .check()
                .map_err(|err| ExternalError::Os(os_error!(OsError::XError(err.into()))))?;
        }

        let result = match mode {
            CursorGrabMode::None => self
                .xconn
                .flush_requests()
                .map_err(|err| ExternalError::Os(os_error!(OsError::XError(err.into())))),
            CursorGrabMode::Confined => {
                let reply = self
                    .xconn
                    .connection
                    .grab_pointer(
                        true,
                        self.xwindow,
                        xproto::EventMask::BUTTON_PRESS
                            | xproto::EventMask::BUTTON_RELEASE
                            | xproto::EventMask::ENTER_WINDOW
                            | xproto::EventMask::LEAVE_WINDOW
                            | xproto::EventMask::POINTER_MOTION
                            | xproto::EventMask::POINTER_MOTION_HINT
                            | xproto::EventMask::BUTTON1_MOTION
                            | xproto::EventMask::BUTTON2_MOTION
                            | xproto::EventMask::BUTTON3_MOTION
                            | xproto::EventMask::BUTTON4_MOTION
                            | xproto::EventMask::BUTTON5_MOTION
                            | xproto::EventMask::BUTTON_MOTION
                            | xproto::EventMask::KEYMAP_STATE,
                        xproto::GrabMode::ASYNC,
                        xproto::GrabMode::ASYNC,
                        self.xwindow,
                        0u32,
                        xproto::Time::CURRENT_TIME,
                    )
                    .platform()
                    .and_then(|r| r.reply().platform())
                    .expect("Failed to call `XGrabPointer`");

                match reply.status {
                    xproto::GrabStatus::SUCCESS => Ok(()),
                    xproto::GrabStatus::ALREADY_GRABBED => {
                        Err("Cursor could not be confined: already confined by another client")
                    }
                    xproto::GrabStatus::INVALID_TIME => {
                        Err("Cursor could not be confined: invalid time")
                    }
                    xproto::GrabStatus::NOT_VIEWABLE => {
                        Err("Cursor could not be confined: confine location not viewable")
                    }
                    xproto::GrabStatus::FROZEN => {
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
        self.xconn
            .connection
            .warp_pointer(self.xwindow, 0u32, 0, 0, 0, 0, x as i16, y as i16)
            .check()
            .map_err(|e| ExternalError::Os(os_error!(OsError::XError(e.into()))))
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

    /// Moves the window while it is being dragged.
    pub fn drag_window(&self) -> Result<(), ExternalError> {
        self.drag_initiate(util::MOVERESIZE_MOVE)
    }

    /// Resizes the window while it is being dragged.
    pub fn drag_resize_window(&self, direction: ResizeDirection) -> Result<(), ExternalError> {
        self.drag_initiate(match direction {
            ResizeDirection::East => util::MOVERESIZE_RIGHT,
            ResizeDirection::North => util::MOVERESIZE_TOP,
            ResizeDirection::NorthEast => util::MOVERESIZE_TOPRIGHT,
            ResizeDirection::NorthWest => util::MOVERESIZE_TOPLEFT,
            ResizeDirection::South => util::MOVERESIZE_BOTTOM,
            ResizeDirection::SouthEast => util::MOVERESIZE_BOTTOMRIGHT,
            ResizeDirection::SouthWest => util::MOVERESIZE_BOTTOMLEFT,
            ResizeDirection::West => util::MOVERESIZE_LEFT,
        })
    }

    /// Initiates a drag operation while the left mouse button is pressed.
    fn drag_initiate(&self, action: isize) -> Result<(), ExternalError> {
        let (win_x, win_y) = self
            .xconn
            .connection
            .xinput_xi_query_pointer(self.xwindow, util::VIRTUAL_CORE_POINTER)
            .platform()
            .and_then(|r| r.reply().platform())
            .map_err(|err| ExternalError::Os(os_error!(OsError::XError(err.into()))))
            .map(|reply| (reply.win_x, reply.win_y))?;

        let window = self.inner_position().map_err(ExternalError::NotSupported)?;

        let message = self.xconn.atoms[_NET_WM_MOVERESIZE];

        // we can't use `set_cursor_grab(false)` here because it doesn't run `XUngrabPointer`
        // if the cursor isn't currently grabbed
        let mut grabbed_lock = self.cursor_grabbed_mode.lock().unwrap();
        self.xconn
            .connection
            .ungrab_pointer(xproto::Time::CURRENT_TIME)
            .check()
            .map_err(|e| ExternalError::Os(os_error!(OsError::XError(e.into()))))?;
        *grabbed_lock = CursorGrabMode::None;

        // we keep the lock until we are done
        self.xconn
            .send_client_msg(
                self.xwindow,
                self.root,
                message,
                Some(
                    xproto::EventMask::SUBSTRUCTURE_NOTIFY
                        | xproto::EventMask::SUBSTRUCTURE_REDIRECT,
                ),
                32,
                [
                    (window.x as u32 + win_x as u32),
                    (window.y as u32 + win_y as u32),
                    action.try_into().unwrap(),
                    1,
                    1,
                ],
            )
            .check()
            .map_err(|err| ExternalError::Os(os_error!(OsError::XError(err.into()))))
    }

    #[inline]
    pub fn set_ime_position(&self, spot: Position) {
        let (x, y) = spot.to_physical::<i32>(self.scale_factor()).into();

        let _ = self
            .ime_sender
            .lock()
            .unwrap()
            .send(ImeRequest::Position(self.xwindow as _, x, y));
    }

    #[inline]
    pub fn set_ime_allowed(&self, allowed: bool) {
        let _ = self
            .ime_sender
            .lock()
            .unwrap()
            .send(ImeRequest::Allow(self.xwindow as _, allowed));
    }

    #[inline]
    pub fn focus_window(&self) {
        let state_atom = self.xconn.atoms[WM_STATE];
        let state_type_atom = self.xconn.atoms[CARD32];
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
            let atom = self.xconn.atoms[_NET_ACTIVE_WINDOW];
            let flusher = self.xconn.send_client_msg(
                self.xwindow,
                self.root,
                atom,
                Some(
                    xproto::EventMask::SUBSTRUCTURE_REDIRECT
                        | xproto::EventMask::SUBSTRUCTURE_NOTIFY,
                ),
                32,
                [1u32, xproto::Time::CURRENT_TIME.into(), 0, 0, 0],
            );
            if let Err(e) = flusher.check() {
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
            .get_wm_hints(self.xwindow as _)
            .expect("`XGetWMHints` failed");
        if request_type.is_some() {
            wm_hints.flags |= ffi::XUrgencyHint;
        } else {
            wm_hints.flags &= !ffi::XUrgencyHint;
        }
        self.xconn
            .set_wm_hints(self.xwindow as _, wm_hints)
            .flush()
            .expect("Failed to set urgency hint");
    }

    #[inline]
    pub fn id(&self) -> WindowId {
        WindowId(self.xwindow as _)
    }

    #[inline]
    pub fn request_redraw(&self) {
        self.redraw_sender
            .sender
            .send(WindowId(self.xwindow as _))
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
        display_handle.screen = self.screen_id as _;
        RawDisplayHandle::Xlib(display_handle)
    }

    #[inline]
    pub fn theme(&self) -> Option<Theme> {
        None
    }

    #[inline]
    pub fn has_focus(&self) -> bool {
        self.shared_state_lock().has_focus
    }

    pub fn title(&self) -> String {
        String::new()
    }

    fn map_raised(&self) -> Result<XcbVoidCookie<'_>, PlatformError> {
        self.xconn
            .connection
            .configure_window(
                self.xwindow,
                &xproto::ConfigureWindowAux::new().stack_mode(xproto::StackMode::ABOVE),
            )?
            .ignore_error();

        self.xconn.connection.map_window(self.xwindow).platform()
    }
}
