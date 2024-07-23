use std::cell::{OnceCell, Ref, RefCell};
use std::future::Future;
use std::hash::Hash;
use std::iter::{self, Once};
use std::mem;
use std::ops::Deref;
use std::pin::Pin;
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
use crate::platform::web::{
    MonitorPermissionError, Orientation, OrientationData, OrientationLock, OrientationLockError,
};

#[derive(Debug, Clone, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct MonitorHandle(Dispatcher<Inner>);

impl MonitorHandle {
    fn new(main_thread: MainThreadMarker, inner: Inner) -> Self {
        Self(Dispatcher::new(main_thread, inner).0)
    }

    pub fn scale_factor(&self) -> f64 {
        self.0.queue(|inner| match &inner.screen {
            Screen::Screen(_) => 0.,
            Screen::Detailed(screen) => screen.device_pixel_ratio(),
        })
    }

    pub fn position(&self) -> PhysicalPosition<i32> {
        self.0.queue(|inner| {
            if let Screen::Detailed(screen) = &inner.screen {
                PhysicalPosition::new(screen.left(), screen.top())
            } else {
                PhysicalPosition::default()
            }
        })
    }

    pub fn name(&self) -> Option<String> {
        self.0.queue(|inner| {
            if let Screen::Detailed(screen) = &inner.screen {
                Some(screen.label())
            } else {
                None
            }
        })
    }

    pub fn refresh_rate_millihertz(&self) -> Option<u32> {
        None
    }

    pub fn size(&self) -> PhysicalSize<u32> {
        self.0.queue(|inner| {
            let width = inner.screen.width().unwrap();
            let height = inner.screen.height().unwrap();

            if let Some(Engine::Chromium) = inner.engine {
                PhysicalSize::new(width, height).cast()
            } else {
                LogicalSize::new(width, height)
                    .to_physical(super::web_sys::scale_factor(&inner.window))
            }
        })
    }

    pub fn video_modes(&self) -> Once<VideoModeHandle> {
        iter::once(VideoModeHandle(self.clone()))
    }

    pub fn orientation(&self) -> OrientationData {
        self.0.queue(|inner| {
            let orientation =
                inner.orientation.get_or_init(|| inner.screen.orientation().unchecked_into());
            let angle = orientation.angle().unwrap();

            match orientation.type_().unwrap() {
                OrientationType::LandscapePrimary => OrientationData {
                    orientation: Orientation::Landscape,
                    flipped: false,
                    natural: angle == 0,
                },
                OrientationType::LandscapeSecondary => OrientationData {
                    orientation: Orientation::Landscape,
                    flipped: true,
                    natural: angle == 180,
                },
                OrientationType::PortraitPrimary => OrientationData {
                    orientation: Orientation::Portrait,
                    flipped: false,
                    natural: angle == 0,
                },
                OrientationType::PortraitSecondary => OrientationData {
                    orientation: Orientation::Portrait,
                    flipped: true,
                    natural: angle == 180,
                },
                _ => {
                    unreachable!("found unrecognized orientation: {}", orientation.type_string())
                },
            }
        })
    }

    pub fn request_lock(&self, orientation_lock: OrientationLock) -> OrientationLockFuture {
        // Short-circuit without blocking.
        if let Some(support) = HAS_LOCK_SUPPORT.with(|support| support.get().cloned()) {
            if !support {
                return OrientationLockFuture::Ready(Some(Err(OrientationLockError::Unsupported)));
            }
        }

        self.0.queue(|inner| {
            let orientation =
                inner.orientation.get_or_init(|| inner.screen.orientation().unchecked_into());

            if !HAS_LOCK_SUPPORT
                .with(|support| *support.get_or_init(|| !orientation.has_lock().is_undefined()))
            {
                return OrientationLockFuture::Ready(Some(Err(OrientationLockError::Unsupported)));
            }

            let future = JsFuture::from(orientation.lock(orientation_lock.to_js()).unwrap());
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
        if let Some(support) = HAS_LOCK_SUPPORT.with(|support| support.get().cloned()) {
            if !support {
                return Err(OrientationLockError::Unsupported);
            }
        }

        self.0.queue(|inner| {
            let orientation =
                inner.orientation.get_or_init(|| inner.screen.orientation().unchecked_into());

            if !HAS_LOCK_SUPPORT
                .with(|support| *support.get_or_init(|| !orientation.has_lock().is_undefined()))
            {
                return Err(OrientationLockError::Unsupported);
            }

            orientation.unlock().map_err(OrientationLockError::from_js)
        })
    }

    pub fn is_internal(&self) -> Option<bool> {
        self.0.queue(|inner| {
            if let Screen::Detailed(screen) = &inner.screen {
                Some(screen.is_internal())
            } else {
                None
            }
        })
    }

    pub fn is_detailed(&self) -> bool {
        self.0.queue(|inner| matches!(inner.screen, Screen::Detailed(_)))
    }

    pub(crate) fn detailed(
        &self,
        main_thread: MainThreadMarker,
    ) -> Option<Ref<'_, ScreenDetailed>> {
        let inner = self.0.value(main_thread);
        match &inner.screen {
            Screen::Screen(_) => None,
            Screen::Detailed(_) => Some(Ref::map(inner, |inner| {
                if let Screen::Detailed(detailed) = &inner.screen {
                    detailed
                } else {
                    unreachable!()
                }
            })),
        }
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

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct VideoModeHandle(pub(super) MonitorHandle);

impl VideoModeHandle {
    pub fn size(&self) -> PhysicalSize<u32> {
        self.0.size()
    }

    pub fn bit_depth(&self) -> u16 {
        self.0 .0.queue(|inner| inner.screen.color_depth().unwrap()).try_into().unwrap()
    }

    pub fn refresh_rate_millihertz(&self) -> u32 {
        0
    }

    pub fn monitor(&self) -> MonitorHandle {
        self.0.clone()
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
}

enum Screen {
    Screen(ScreenExt),
    Detailed(ScreenDetailed),
}

impl Deref for Screen {
    type Target = ScreenExt;

    fn deref(&self) -> &Self::Target {
        match self {
            Screen::Screen(screen) => screen,
            Screen::Detailed(screen) => screen,
        }
    }
}

pub struct MonitorHandler {
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
    Detailed(ScreenDetails),
}

impl MonitorHandler {
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
            let permissions = navigator.permissions().expect(
                "expected the Permissions API to be implemented if the Window Management API is \
                 as well",
            );
            let descriptor: PermissionDescriptor = Object::new().unchecked_into();
            descriptor.set_name("window-management");
            let future = JsFuture::from(permissions.query(&descriptor).unwrap());

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

                let screen_details = match permission.state() {
                    PermissionState::Granted => {
                        let screen_details = match JsFuture::from(window.screen_details()).await {
                            Ok(screen_details) => screen_details.unchecked_into(),
                            Err(error) => unreachable_error(
                                &error,
                                "getting screen details failed even though permission was granted",
                            ),
                        };
                        notifier.notify(Ok(()));
                        Some(screen_details)
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
                    let state = if let Some(screen_details) = screen_details {
                        State::Detailed(screen_details)
                    } else {
                        // If permission is denied we listen for changes so we can catch external
                        // permission granting.
                        let handle =
                            Self::setup_listener(runner.weak(), window, permission.clone());
                        State::Permission { permission, _handle: handle }
                    };

                    *runner.monitor().state.borrow_mut() = state;
                    runner.start_delayed();
                }
            });

            State::Initialize(notified)
        } else {
            State::Unsupported
        };

        Self { state: RefCell::new(state), main_thread, window, engine, screen }
    }

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
                        let screen_details = match future.await {
                            Ok(screen_details) => screen_details.unchecked_into(),
                            Err(error) => unreachable_error(
                                &error,
                                "getting screen details failed even though permission was granted",
                            ),
                        };

                        if let Some(runner) = runner.upgrade() {
                            // We drop the event listener handle here, which
                            // doesn't drop it while we are running it, because
                            // we are in a `spawn_local()` context.
                            *runner.monitor().state.borrow_mut() = State::Detailed(screen_details);
                        }
                    });
                }
            }),
        )
    }

    pub fn is_extended(&self) -> Option<bool> {
        self.screen.is_extended()
    }

    pub fn is_initializing(&self) -> bool {
        matches!(self.state.borrow().deref(), State::Initialize(_))
    }

    pub fn current_monitor(&self) -> MonitorHandle {
        if let State::Detailed(details) = self.state.borrow().deref() {
            MonitorHandle::new(
                self.main_thread,
                Inner::new(
                    self.window.clone(),
                    self.engine,
                    Screen::Detailed(details.current_screen()),
                ),
            )
        } else {
            MonitorHandle::new(
                self.main_thread,
                Inner::new(self.window.clone(), self.engine, Screen::Screen(self.screen.clone())),
            )
        }
    }

    // Note: We have to return a `Vec` here because the iterator is otherwise not `Send` + `Sync`.
    pub fn available_monitors(&self) -> Vec<MonitorHandle> {
        if let State::Detailed(details) = self.state.borrow().deref() {
            details
                .screens()
                .into_iter()
                .map(move |screen| {
                    MonitorHandle::new(
                        self.main_thread,
                        Inner::new(self.window.clone(), self.engine, Screen::Detailed(screen)),
                    )
                })
                .collect()
        } else {
            vec![self.current_monitor()]
        }
    }

    pub fn primary_monitor(&self) -> Option<MonitorHandle> {
        if let State::Detailed(details) = self.state.borrow().deref() {
            details.screens().into_iter().find_map(|screen| {
                screen.is_primary().then(|| {
                    MonitorHandle::new(
                        self.main_thread,
                        Inner::new(self.window.clone(), self.engine, Screen::Detailed(screen)),
                    )
                })
            })
        } else {
            None
        }
    }

    pub(crate) fn request_detailed_monitor_permission(
        &self,
        shared: WeakShared,
    ) -> MonitorPermissionFuture {
        let state = self.state.borrow();
        let (notifier, notified) = match state.deref() {
            State::Unsupported => {
                return MonitorPermissionFuture::Ready(Some(Err(
                    MonitorPermissionError::Unsupported,
                )))
            },
            State::Initialize(notified) => {
                return MonitorPermissionFuture::Initialize {
                    runner: Dispatcher::new(self.main_thread, (shared, self.window.clone())).0,
                    notified: notified.clone(),
                }
            },
            State::Permission { permission, .. } => {
                match permission.state() {
                    PermissionState::Granted | PermissionState::Prompt => (),
                    PermissionState::Denied => {
                        return MonitorPermissionFuture::Ready(Some(Err(
                            MonitorPermissionError::Denied,
                        )))
                    },
                    _ => {
                        error!(
                            "encountered unknown permission state: {}",
                            permission.state_string()
                        );

                        return MonitorPermissionFuture::Ready(Some(Err(
                            MonitorPermissionError::Denied,
                        )));
                    },
                }

                drop(state);

                let notifier = Notifier::new();
                let notified = notifier.notified();
                *self.state.borrow_mut() = State::Upgrade(notified.clone());

                (notifier, notified)
            },
            // A request is already in progress.
            State::Upgrade(notified) => return MonitorPermissionFuture::Upgrade(notified.clone()),
            State::Detailed(_) => return MonitorPermissionFuture::Ready(Some(Ok(()))),
        };

        let future = JsFuture::from(self.window.screen_details());
        wasm_bindgen_futures::spawn_local(async move {
            match future.await {
                Ok(details) => {
                    // Notifying `Future`s is not dependant on the lifetime of the runner, because
                    // they can outlive it.
                    notifier.notify(Ok(()));

                    if let Some(shared) = shared.upgrade() {
                        *shared.monitor().state.borrow_mut() =
                            State::Detailed(details.unchecked_into())
                    }
                },
                Err(error) => unreachable_error(
                    &error,
                    "getting screen details failed even though permission was granted",
                ),
            }
        });

        MonitorPermissionFuture::Upgrade(notified)
    }

    pub fn has_detailed_monitor_permission_async(&self) -> HasMonitorPermissionFuture {
        match self.state.borrow().deref() {
            State::Unsupported | State::Permission { .. } | State::Upgrade(_) => {
                HasMonitorPermissionFuture::Ready(Some(false))
            },
            State::Initialize(notified) => HasMonitorPermissionFuture::Future(notified.clone()),
            State::Detailed(_) => HasMonitorPermissionFuture::Ready(Some(true)),
        }
    }

    pub fn has_detailed_monitor_permission(&self) -> bool {
        match self.state.borrow().deref() {
            State::Unsupported | State::Permission { .. } | State::Upgrade(_) => false,
            State::Initialize(_) => {
                unreachable!("called `has_detailed_monitor_permission()` while initializing")
            },
            State::Detailed(_) => true,
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

        runner.dispatch(|(shared, window)| {
            let future = JsFuture::from(window.screen_details());

            if let Some(shared) = shared.upgrade() {
                *shared.monitor().state.borrow_mut() = State::Upgrade(notified);
            }

            let shared = shared.clone();
            wasm_bindgen_futures::spawn_local(async move {
                match future.await {
                    Ok(details) => {
                        // Notifying `Future`s is not dependant on the lifetime
                        // of
                        // the runner, because
                        // they can outlive it.
                        notifier.notify(Ok(()));

                        if let Some(shared) = shared.upgrade() {
                            *shared.monitor().state.borrow_mut() =
                                State::Detailed(details.unchecked_into())
                        }
                    },
                    Err(error) => unreachable_error(
                        &error,
                        "getting screen details failed even though permission was granted",
                    ),
                }
            });
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

thread_local! {
    static HAS_LOCK_SUPPORT: OnceCell<bool> = const { OnceCell::new() };
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

    #[derive(Clone)]
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
