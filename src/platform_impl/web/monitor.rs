use std::cell::{OnceCell, Ref, RefCell};
use std::cmp::Ordering;
use std::fmt::{self, Debug, Formatter};
use std::future::Future;
use std::hash::{Hash, Hasher};
use std::iter::{self, Once};
use std::mem;
use std::num::{NonZeroU16, NonZeroU32};
use std::ops::{Deref, DerefMut};
use std::pin::Pin;
use std::rc::{Rc, Weak};
use std::sync::OnceLock;
use std::task::{ready, Context, Poll};

use dpi::LogicalSize;
use js_sys::{Object, Promise};
use tracing::error;
use wasm_bindgen::closure::Closure;
use wasm_bindgen::prelude::wasm_bindgen;
use wasm_bindgen::{JsCast, JsValue};
use wasm_bindgen_futures::JsFuture;
use web_sys::{
    console, DomException, Navigator, OrientationLockType, OrientationType, PermissionState,
    PermissionStatus, ScreenOrientation, Window,
};

use super::event_loop::runner::WeakShared;
use super::main_thread::MainThreadMarker;
use super::r#async::{Dispatcher, Notified, Notifier};
use super::web_sys::{Engine, EventListenerHandle};
use crate::dpi::{PhysicalPosition, PhysicalSize};
use crate::monitor::MonitorHandle as RootMonitorHandle;
use crate::platform::web::{
    MonitorPermissionError, Orientation, OrientationData, OrientationLock, OrientationLockError,
};

#[derive(Clone, Eq)]
pub struct MonitorHandle {
    /// [`None`] means [`web_sys::Screen`], which is always the same.
    id: Option<u64>,
    inner: Dispatcher<Inner>,
}

impl MonitorHandle {
    fn new(main_thread: MainThreadMarker, inner: Inner) -> Self {
        let id = if let Screen::Detailed { id, .. } = inner.screen { Some(id) } else { None };
        Self { id, inner: Dispatcher::new(main_thread, inner).0 }
    }

    pub fn scale_factor(&self) -> f64 {
        self.inner.queue(|inner| inner.scale_factor())
    }

    pub fn position(&self) -> Option<PhysicalPosition<i32>> {
        self.inner.queue(|inner| inner.position())
    }

    pub fn name(&self) -> Option<String> {
        self.inner.queue(|inner| inner.name())
    }

    pub fn current_video_mode(&self) -> Option<VideoModeHandle> {
        Some(VideoModeHandle(self.clone()))
    }

    pub fn video_modes(&self) -> Once<VideoModeHandle> {
        iter::once(VideoModeHandle(self.clone()))
    }

    pub fn orientation(&self) -> OrientationData {
        self.inner.queue(|inner| inner.orientation())
    }

    pub fn request_lock(&self, orientation_lock: OrientationLock) -> OrientationLockFuture {
        // Short-circuit without blocking.
        if let Some(support) = has_previous_lock_support() {
            if !support {
                return OrientationLockFuture::Ready(Some(Err(OrientationLockError::Unsupported)));
            }
        }

        self.inner.queue(|inner| {
            if !inner.has_lock_support() {
                return OrientationLockFuture::Ready(Some(Err(OrientationLockError::Unsupported)));
            }

            let future =
                JsFuture::from(inner.orientation_raw().lock(orientation_lock.to_js()).unwrap());
            let notifier = Notifier::new();
            let notified = notifier.notified();

            wasm_bindgen_futures::spawn_local(async move {
                notifier.notify(future.await.map(|_| ()).map_err(OrientationLockError::from_js));
            });

            OrientationLockFuture::Future(notified)
        })
    }

    pub fn unlock(&self) -> Result<(), OrientationLockError> {
        // Short-circuit without blocking.
        if let Some(support) = has_previous_lock_support() {
            if !support {
                return Err(OrientationLockError::Unsupported);
            }
        }

        self.inner.queue(|inner| {
            if !inner.has_lock_support() {
                return Err(OrientationLockError::Unsupported);
            }

            inner.orientation_raw().unlock().map_err(OrientationLockError::from_js)
        })
    }

    pub fn is_internal(&self) -> Option<bool> {
        self.inner.queue(|inner| inner.is_internal())
    }

    pub fn is_detailed(&self) -> bool {
        self.inner.queue(|inner| inner.is_detailed())
    }

    pub(crate) fn detailed(
        &self,
        main_thread: MainThreadMarker,
    ) -> Option<Ref<'_, ScreenDetailed>> {
        let inner = self.inner.value(main_thread);
        match &inner.screen {
            Screen::Screen(_) => None,
            Screen::Detailed { .. } => Some(Ref::map(inner, |inner| {
                if let Screen::Detailed { screen, .. } = &inner.screen {
                    screen.deref()
                } else {
                    unreachable!()
                }
            })),
        }
    }
}

impl Debug for MonitorHandle {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        let (name, position, scale_factor, orientation, is_internal, is_detailed) =
            self.inner.queue(|this| {
                (
                    this.name(),
                    this.position(),
                    this.scale_factor(),
                    this.orientation(),
                    this.is_internal(),
                    this.is_detailed(),
                )
            });

        f.debug_struct("MonitorHandle")
            .field("name", &name)
            .field("position", &position)
            .field("scale_factor", &scale_factor)
            .field("orientation", &orientation)
            .field("is_internal", &is_internal)
            .field("is_detailed", &is_detailed)
            .finish()
    }
}

impl Hash for MonitorHandle {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.id.hash(state)
    }
}

impl Ord for MonitorHandle {
    fn cmp(&self, other: &Self) -> Ordering {
        self.id.cmp(&other.id)
    }
}

impl PartialEq for MonitorHandle {
    fn eq(&self, other: &Self) -> bool {
        self.id.eq(&other.id)
    }
}

impl PartialOrd for MonitorHandle {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl From<MonitorHandle> for RootMonitorHandle {
    fn from(inner: MonitorHandle) -> Self {
        RootMonitorHandle { inner }
    }
}

#[derive(Debug)]
pub enum OrientationLockFuture {
    Future(Notified<Result<(), OrientationLockError>>),
    Ready(Option<Result<(), OrientationLockError>>),
}

impl Future for OrientationLockFuture {
    type Output = Result<(), OrientationLockError>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        match self.get_mut() {
            Self::Future(notified) => Pin::new(notified).poll(cx).map(Option::unwrap),
            Self::Ready(result) => {
                Poll::Ready(result.take().expect("`OrientationLockFuture` polled after completion"))
            },
        }
    }
}

impl OrientationLock {
    fn to_js(self) -> OrientationLockType {
        match self {
            OrientationLock::Any => OrientationLockType::Any,
            OrientationLock::Natural => OrientationLockType::Natural,
            OrientationLock::Landscape { flipped: None } => OrientationLockType::Landscape,
            OrientationLock::Landscape { flipped: Some(flipped) } => {
                if flipped {
                    OrientationLockType::LandscapeSecondary
                } else {
                    OrientationLockType::LandscapePrimary
                }
            },
            OrientationLock::Portrait { flipped: None } => OrientationLockType::Portrait,
            OrientationLock::Portrait { flipped: Some(flipped) } => {
                if flipped {
                    OrientationLockType::PortraitSecondary
                } else {
                    OrientationLockType::PortraitPrimary
                }
            },
        }
    }
}

impl OrientationLockError {
    fn from_js(error: JsValue) -> Self {
        debug_assert!(error.has_type::<DomException>());
        let error: DomException = error.unchecked_into();

        if let DomException::ABORT_ERR = error.code() {
            OrientationLockError::Busy
        } else {
            OrientationLockError::Unsupported
        }
    }
}

#[derive(Clone, Eq, Hash, PartialEq)]
pub struct VideoModeHandle(MonitorHandle);

impl VideoModeHandle {
    pub fn size(&self) -> PhysicalSize<u32> {
        self.0.inner.queue(|inner| inner.size())
    }

    pub fn bit_depth(&self) -> Option<NonZeroU16> {
        self.0.inner.queue(|inner| inner.bit_depth())
    }

    pub fn refresh_rate_millihertz(&self) -> Option<NonZeroU32> {
        None
    }

    pub fn monitor(&self) -> MonitorHandle {
        self.0.clone()
    }
}

impl Debug for VideoModeHandle {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        let (size, bit_depth) = self.0.inner.queue(|this| (this.size(), this.bit_depth()));

        f.debug_struct("MonitorHandle").field("size", &size).field("bit_depth", &bit_depth).finish()
    }
}

struct Inner {
    window: WindowExt,
    engine: Option<Engine>,
    screen: Screen,
    orientation: OnceCell<ScreenOrientationExt>,
}

impl Inner {
    fn new(window: WindowExt, engine: Option<Engine>, screen: Screen) -> Self {
        Self { window, engine, screen, orientation: OnceCell::new() }
    }

    fn scale_factor(&self) -> f64 {
        match &self.screen {
            Screen::Screen(_) => 0.,
            Screen::Detailed { screen, .. } => screen.device_pixel_ratio(),
        }
    }

    fn position(&self) -> Option<PhysicalPosition<i32>> {
        if let Screen::Detailed { screen, .. } = &self.screen {
            Some(PhysicalPosition::new(screen.left(), screen.top()))
        } else {
            None
        }
    }

    fn name(&self) -> Option<String> {
        if let Screen::Detailed { screen, .. } = &self.screen {
            Some(screen.label())
        } else {
            None
        }
    }

    fn orientation_raw(&self) -> &ScreenOrientationExt {
        self.orientation.get_or_init(|| self.screen.orientation().unchecked_into())
    }

    fn orientation(&self) -> OrientationData {
        let orientation = self.orientation_raw();

        let angle = orientation.angle().unwrap();

        match orientation.type_().unwrap() {
            OrientationType::LandscapePrimary => OrientationData {
                orientation: Orientation::Landscape,
                flipped: false,
                natural: if angle == 0 { Orientation::Landscape } else { Orientation::Portrait },
            },
            OrientationType::LandscapeSecondary => OrientationData {
                orientation: Orientation::Landscape,
                flipped: true,
                natural: if angle == 180 { Orientation::Landscape } else { Orientation::Portrait },
            },
            OrientationType::PortraitPrimary => OrientationData {
                orientation: Orientation::Portrait,
                flipped: false,
                natural: if angle == 0 { Orientation::Portrait } else { Orientation::Landscape },
            },
            OrientationType::PortraitSecondary => OrientationData {
                orientation: Orientation::Portrait,
                flipped: true,
                natural: if angle == 180 { Orientation::Portrait } else { Orientation::Landscape },
            },
            _ => {
                unreachable!("found unrecognized orientation: {}", orientation.type_string())
            },
        }
    }

    fn is_internal(&self) -> Option<bool> {
        if let Screen::Detailed { screen, .. } = &self.screen {
            Some(screen.is_internal())
        } else {
            None
        }
    }

    fn is_detailed(&self) -> bool {
        matches!(self.screen, Screen::Detailed { .. })
    }

    fn size(&self) -> PhysicalSize<u32> {
        let width = self.screen.width().unwrap();
        let height = self.screen.height().unwrap();

        if let Some(Engine::Chromium) = self.engine {
            PhysicalSize::new(width, height).cast()
        } else {
            LogicalSize::new(width, height).to_physical(super::web_sys::scale_factor(&self.window))
        }
    }

    fn bit_depth(&self) -> Option<NonZeroU16> {
        NonZeroU16::new(self.screen.color_depth().unwrap().try_into().unwrap())
    }

    fn has_lock_support(&self) -> bool {
        *HAS_LOCK_SUPPORT.get_or_init(|| !self.orientation_raw().has_lock().is_undefined())
    }
}

impl Drop for Inner {
    fn drop(&mut self) {
        if let Screen::Detailed { runner, id, screen } = &self.screen {
            // If this is the last screen with its ID, clean it up in the `MonitorHandler`.
            if Rc::strong_count(screen) == 1 {
                if let Some(runner) = runner.upgrade() {
                    let mut state = runner.monitor().state.borrow_mut();
                    let State::Detailed(detailed) = state.deref_mut() else {
                        unreachable!("found a `ScreenDetailed` without being in `State::Detailed`")
                    };

                    detailed.screens.retain(|(id_internal, _)| *id_internal != *id)
                }
            }
        }
    }
}

enum Screen {
    Screen(ScreenExt),
    Detailed { runner: WeakShared, id: u64, screen: Rc<ScreenDetailed> },
}

impl Deref for Screen {
    type Target = ScreenExt;

    fn deref(&self) -> &Self::Target {
        match self {
            Screen::Screen(screen) => screen,
            Screen::Detailed { screen, .. } => screen,
        }
    }
}

pub struct MonitorHandler {
    runner: WeakShared,
    state: RefCell<State>,
    main_thread: MainThreadMarker,
    window: WindowExt,
    engine: Option<Engine>,
    screen: ScreenExt,
}

enum State {
    Unsupported,
    Initialize(Notified<Result<(), MonitorPermissionError>>),
    Permission { permission: PermissionStatusExt, _handle: EventListenerHandle<dyn Fn()> },
    Upgrade(Notified<Result<(), MonitorPermissionError>>),
    Detailed(Detailed),
}

struct Detailed {
    details: ScreenDetails,
    id_counter: u64,
    screens: Vec<(u64, Weak<ScreenDetailed>)>,
}

impl Detailed {
    fn handle(
        &mut self,
        main_thread: MainThreadMarker,
        runner: WeakShared,
        window: WindowExt,
        engine: Option<Engine>,
        screen: ScreenDetailed,
    ) -> MonitorHandle {
        // Before creating a new entry, see if we have an ID for this screen already.
        let found_screen = self.screens.iter().find_map(|(id, internal_screen)| {
            let internal_screen =
                internal_screen.upgrade().expect("dropped `MonitorHandle` without cleaning up");

            if *internal_screen == screen {
                Some((*id, internal_screen))
            } else {
                None
            }
        });
        let (id, screen) = if let Some((id, screen)) = found_screen {
            (id, screen)
        } else {
            let id = self.id_counter;
            self.id_counter += 1;
            let screen = Rc::new(screen);

            self.screens.push((id, Rc::downgrade(&screen)));

            (id, screen)
        };

        MonitorHandle::new(
            main_thread,
            Inner::new(window, engine, Screen::Detailed { runner, id, screen }),
        )
    }
}

impl MonitorHandler {
    /// When the [`MonitorHandler`] is created, it first checks if permission has already been
    /// granted by the user for this page, in which case it retrieves [`ScreenDetails`].
    ///
    /// If not, it will listen to external changes in the permission and automatically elevate
    /// [`MonitorHandler`].
    pub fn new(
        main_thread: MainThreadMarker,
        window: Window,
        navigator: &Navigator,
        runner: WeakShared,
    ) -> Self {
        let window: WindowExt = window.unchecked_into();
        let engine = super::web_sys::engine(navigator);
        let screen: ScreenExt = window.screen().unwrap().unchecked_into();

        let state = if has_screen_details_support(&window) {
            // First try and get permissions.
            let permissions = navigator.permissions().expect(
                "expected the Permissions API to be implemented if the Window Management API is \
                 as well",
            );
            let descriptor: PermissionDescriptor = Object::new().unchecked_into();
            descriptor.set_name("window-management");
            let future = JsFuture::from(permissions.query(&descriptor).unwrap());

            let runner = runner.clone();
            let window = window.clone();
            let notifier = Notifier::new();
            let notified = notifier.notified();
            wasm_bindgen_futures::spawn_local(async move {
                let permission: PermissionStatusExt = match future.await {
                    Ok(permission) => permission.unchecked_into(),
                    Err(error) => unreachable_error(
                        &error,
                        "retrieving permission for Window Management API failed even though its \
                         implemented",
                    ),
                };

                let details = match permission.state() {
                    // If we have permission, go ahead and get `ScreenDetails`.
                    PermissionState::Granted => {
                        let details = match JsFuture::from(window.screen_details()).await {
                            Ok(details) => details.unchecked_into(),
                            Err(error) => unreachable_error(
                                &error,
                                "getting screen details failed even though permission was granted",
                            ),
                        };
                        notifier.notify(Ok(()));
                        Some(details)
                    },
                    PermissionState::Denied => {
                        notifier.notify(Err(MonitorPermissionError::Denied));
                        None
                    },
                    PermissionState::Prompt => {
                        notifier.notify(Err(MonitorPermissionError::Prompt));
                        None
                    },
                    _ => {
                        error!(
                            "encountered unknown permission state: {}",
                            permission.state_string()
                        );
                        notifier.notify(Err(MonitorPermissionError::Denied));
                        None
                    },
                };

                // Notifying `Future`s is not dependant on the lifetime of the runner,
                // because they can outlive it.
                if let Some(runner) = runner.upgrade() {
                    if let Some(details) = details {
                        runner.monitor().upgrade(details);
                    } else {
                        // If permission is denied we listen for changes so we can catch external
                        // permission granting.
                        let handle =
                            Self::setup_listener(runner.weak(), window, permission.clone());
                        *runner.monitor().state.borrow_mut() =
                            State::Permission { permission, _handle: handle };
                    };

                    runner.start_delayed();
                }
            });

            State::Initialize(notified)
        } else {
            State::Unsupported
        };

        Self { runner, state: RefCell::new(state), main_thread, window, engine, screen }
    }

    /// Listens to external permission changes and elevates [`MonitorHandle`] automatically.
    fn setup_listener(
        runner: WeakShared,
        window: WindowExt,
        permission: PermissionStatus,
    ) -> EventListenerHandle<dyn Fn()> {
        EventListenerHandle::new(
            permission.clone(),
            "change",
            Closure::new(move || {
                if let PermissionState::Granted = permission.state() {
                    let future = JsFuture::from(window.screen_details());

                    let runner = runner.clone();
                    wasm_bindgen_futures::spawn_local(async move {
                        let details = match future.await {
                            Ok(details) => details.unchecked_into(),
                            Err(error) => unreachable_error(
                                &error,
                                "getting screen details failed even though permission was granted",
                            ),
                        };

                        if let Some(runner) = runner.upgrade() {
                            // We drop the event listener handle here, which
                            // doesn't drop it during its execution, because
                            // we are in a `spawn_local()` context.
                            runner.monitor().upgrade(details);
                        }
                    });
                }
            }),
        )
    }

    /// Elevate [`MonitorHandler`] to [`ScreenDetails`].
    fn upgrade(&self, details: ScreenDetails) {
        *self.state.borrow_mut() =
            State::Detailed(Detailed { details, id_counter: 0, screens: Vec::new() });
    }

    pub fn is_extended(&self) -> Option<bool> {
        self.screen.is_extended()
    }

    pub fn is_initializing(&self) -> bool {
        matches!(self.state.borrow().deref(), State::Initialize(_))
    }

    fn handle(&self, detailed: &mut Detailed, screen: ScreenDetailed) -> MonitorHandle {
        detailed.handle(
            self.main_thread,
            self.runner.clone(),
            self.window.clone(),
            self.engine,
            screen,
        )
    }

    pub fn current_monitor(&self) -> MonitorHandle {
        if let State::Detailed(detailed) = self.state.borrow_mut().deref_mut() {
            self.handle(detailed, detailed.details.current_screen())
        } else {
            MonitorHandle::new(
                self.main_thread,
                Inner::new(self.window.clone(), self.engine, Screen::Screen(self.screen.clone())),
            )
        }
    }

    // Note: We have to return a `Vec` here because the iterator is otherwise not `Send` + `Sync`.
    pub fn available_monitors(&self) -> Vec<MonitorHandle> {
        let mut state = self.state.borrow_mut();
        if let State::Detailed(detailed) = state.deref_mut() {
            detailed
                .details
                .screens()
                .into_iter()
                .map(move |screen| self.handle(detailed, screen))
                .collect()
        } else {
            drop(state);
            vec![self.current_monitor()]
        }
    }

    pub fn primary_monitor(&self) -> Option<MonitorHandle> {
        if let State::Detailed(detailed) = self.state.borrow_mut().deref_mut() {
            detailed
                .details
                .screens()
                .into_iter()
                .find_map(|screen| screen.is_primary().then(|| self.handle(detailed, screen)))
        } else {
            None
        }
    }

    pub(crate) fn request_detailed_monitor_permission(&self) -> MonitorPermissionFuture {
        let state = self.state.borrow();
        let permission = match state.deref() {
            State::Unsupported => {
                return MonitorPermissionFuture::Ready(Some(Err(
                    MonitorPermissionError::Unsupported,
                )))
            },
            // If we are currently initializing, wait for initialization to finish before we do our
            // thing.
            State::Initialize(notified) => {
                return MonitorPermissionFuture::Initialize {
                    runner: Dispatcher::new(
                        self.main_thread,
                        (self.runner.clone(), self.window.clone()),
                    )
                    .0,
                    notified: notified.clone(),
                }
            },
            // If we finished initialization we at least possess `PermissionStatus`.
            State::Permission { permission, .. } => permission,
            // A request is already in progress. Use that!
            State::Upgrade(notified) => return MonitorPermissionFuture::Upgrade(notified.clone()),
            State::Detailed { .. } => return MonitorPermissionFuture::Ready(Some(Ok(()))),
        };

        match permission.state() {
            PermissionState::Granted | PermissionState::Prompt => (),
            PermissionState::Denied => {
                return MonitorPermissionFuture::Ready(Some(Err(MonitorPermissionError::Denied)))
            },
            _ => {
                error!("encountered unknown permission state: {}", permission.state_string());

                return MonitorPermissionFuture::Ready(Some(Err(MonitorPermissionError::Denied)));
            },
        }

        drop(state);

        // We are ready to explicitly ask the user for permission, lets go!

        let notifier = Notifier::new();
        let notified = notifier.notified();
        *self.state.borrow_mut() = State::Upgrade(notified.clone());

        MonitorPermissionFuture::upgrade_internal(self.runner.clone(), &self.window, notifier);

        MonitorPermissionFuture::Upgrade(notified)
    }

    pub fn has_detailed_monitor_permission_async(&self) -> HasMonitorPermissionFuture {
        match self.state.borrow().deref() {
            State::Unsupported | State::Permission { .. } | State::Upgrade(_) => {
                HasMonitorPermissionFuture::Ready(Some(false))
            },
            State::Initialize(notified) => HasMonitorPermissionFuture::Future(notified.clone()),
            State::Detailed { .. } => HasMonitorPermissionFuture::Ready(Some(true)),
        }
    }

    pub fn has_detailed_monitor_permission(&self) -> bool {
        match self.state.borrow().deref() {
            State::Unsupported | State::Permission { .. } | State::Upgrade(_) => false,
            State::Initialize(_) => {
                unreachable!("called `has_detailed_monitor_permission()` while initializing")
            },
            State::Detailed { .. } => true,
        }
    }
}

#[derive(Debug)]
pub(crate) enum MonitorPermissionFuture {
    Initialize {
        runner: Dispatcher<(WeakShared, WindowExt)>,
        notified: Notified<Result<(), MonitorPermissionError>>,
    },
    Upgrade(Notified<Result<(), MonitorPermissionError>>),
    Ready(Option<Result<(), MonitorPermissionError>>),
}

impl MonitorPermissionFuture {
    fn upgrade(&mut self) {
        let notifier = Notifier::new();
        let notified = notifier.notified();
        let Self::Initialize { runner, .. } = mem::replace(self, Self::Upgrade(notified.clone()))
        else {
            unreachable!()
        };

        runner.dispatch(|(runner, window)| {
            if let Some(runner) = runner.upgrade() {
                *runner.monitor().state.borrow_mut() = State::Upgrade(notified);
            }

            Self::upgrade_internal(runner.clone(), window, notifier);
        });
    }

    fn upgrade_internal(
        runner: WeakShared,
        window: &WindowExt,
        notifier: Notifier<Result<(), MonitorPermissionError>>,
    ) {
        let future = JsFuture::from(window.screen_details());

        wasm_bindgen_futures::spawn_local(async move {
            match future.await {
                Ok(details) => {
                    // Notifying `Future`s is not dependant on the lifetime of the runner, because
                    // they can outlive it.
                    notifier.notify(Ok(()));

                    if let Some(runner) = runner.upgrade() {
                        runner.monitor().upgrade(details.unchecked_into());
                    }
                },
                Err(error) => unreachable_error(
                    &error,
                    "getting screen details failed even though permission was granted",
                ),
            }
        });
    }
}

impl Future for MonitorPermissionFuture {
    type Output = Result<(), MonitorPermissionError>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let this = self.get_mut();

        match this {
            Self::Initialize { notified, .. } => {
                if let Err(error) = ready!(Pin::new(notified).poll(cx).map(Option::unwrap)) {
                    match error {
                        MonitorPermissionError::Denied | MonitorPermissionError::Unsupported => {
                            Poll::Ready(Err(error))
                        },
                        MonitorPermissionError::Prompt => {
                            this.upgrade();
                            Poll::Pending
                        },
                    }
                } else {
                    Poll::Ready(Ok(()))
                }
            },
            Self::Upgrade(notified) => Pin::new(notified).poll(cx).map(Option::unwrap),
            Self::Ready(result) => Poll::Ready(
                result.take().expect("`MonitorPermissionFuture` polled after completion"),
            ),
        }
    }
}

#[derive(Debug)]
pub enum HasMonitorPermissionFuture {
    Future(Notified<Result<(), MonitorPermissionError>>),
    Ready(Option<bool>),
}

impl Future for HasMonitorPermissionFuture {
    type Output = bool;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        match self.get_mut() {
            Self::Future(notified) => {
                Pin::new(notified).poll(cx).map(Option::unwrap).map(|result| result.is_ok())
            },
            Self::Ready(result) => Poll::Ready(
                result.take().expect("`MonitorPermissionFuture` polled after completion"),
            ),
        }
    }
}

#[track_caller]
fn unreachable_error(error: &JsValue, message: &str) -> ! {
    if let Some(error) = error.dyn_ref::<DomException>() {
        unreachable!("{message}. {}: {}", error.name(), error.message());
    } else {
        console::error_1(error);
        unreachable!("{message}");
    }
}

static HAS_LOCK_SUPPORT: OnceLock<bool> = OnceLock::new();

fn has_previous_lock_support() -> Option<bool> {
    HAS_LOCK_SUPPORT.get().cloned()
}

pub fn has_screen_details_support(window: &Window) -> bool {
    thread_local! {
        static HAS_SCREEN_DETAILS: OnceCell<bool> = const { OnceCell::new() };
    }

    HAS_SCREEN_DETAILS.with(|support| {
        *support.get_or_init(|| {
            let window: &WindowExt = window.unchecked_ref();
            !window.has_screen_details().is_undefined()
        })
    })
}

#[wasm_bindgen]
extern "C" {
    #[derive(Clone)]
    #[wasm_bindgen(extends = Window)]
    pub(crate) type WindowExt;

    #[wasm_bindgen(method, getter, js_name = getScreenDetails)]
    fn has_screen_details(this: &WindowExt) -> JsValue;

    #[wasm_bindgen(method, js_name = getScreenDetails)]
    fn screen_details(this: &WindowExt) -> Promise;

    type ScreenDetails;

    #[wasm_bindgen(method, getter, js_name = currentScreen)]
    fn current_screen(this: &ScreenDetails) -> ScreenDetailed;

    #[wasm_bindgen(method, getter)]
    fn screens(this: &ScreenDetails) -> Vec<ScreenDetailed>;

    #[derive(Clone, PartialEq)]
    #[wasm_bindgen(extends = web_sys::Screen)]
    pub(crate) type ScreenExt;

    #[wasm_bindgen(method, getter, js_name = isExtended)]
    fn is_extended(this: &ScreenExt) -> Option<bool>;

    #[wasm_bindgen(extends = ScreenOrientation)]
    type ScreenOrientationExt;

    #[wasm_bindgen(method, getter, js_name = type)]
    fn type_string(this: &ScreenOrientationExt) -> String;

    #[wasm_bindgen(method, getter, js_name = lock)]
    fn has_lock(this: &ScreenOrientationExt) -> JsValue;

    #[derive(PartialEq)]
    #[wasm_bindgen(extends = ScreenExt)]
    pub(crate) type ScreenDetailed;

    #[wasm_bindgen(method, getter, js_name = devicePixelRatio)]
    fn device_pixel_ratio(this: &ScreenDetailed) -> f64;

    #[wasm_bindgen(method, getter, js_name = isInternal)]
    fn is_internal(this: &ScreenDetailed) -> bool;

    #[wasm_bindgen(method, getter, js_name = isPrimary)]
    fn is_primary(this: &ScreenDetailed) -> bool;

    #[wasm_bindgen(method, getter)]
    fn label(this: &ScreenDetailed) -> String;

    #[wasm_bindgen(method, getter)]
    fn left(this: &ScreenDetailed) -> i32;

    #[wasm_bindgen(method, getter)]
    fn top(this: &ScreenDetailed) -> i32;

    #[wasm_bindgen(extends = Object)]
    type PermissionDescriptor;

    #[wasm_bindgen(method, setter, js_name = name)]
    fn set_name(this: &PermissionDescriptor, name: &str);

    #[wasm_bindgen(extends = PermissionStatus)]
    type PermissionStatusExt;

    #[wasm_bindgen(method, getter, js_name = state)]
    fn state_string(this: &PermissionStatusExt) -> String;
}
