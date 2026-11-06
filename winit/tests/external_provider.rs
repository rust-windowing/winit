//! Exercise a provider without creating any platform windows or event loops.

use std::cell::{Cell, RefCell};
use std::fmt;
use std::rc::Rc;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

use winit::event_loop::EventLoop;
use winit::raw_window_handle::{DisplayHandle, HandleError, HasDisplayHandle};
use winit_core::application::ApplicationHandler;
use winit_core::cursor::{CustomCursor, CustomCursorSource};
use winit_core::error::{CreateWindowError, CustomCursorError, EventLoopError};
use winit_core::event::{StartCause, WindowEvent};
use winit_core::event_loop::never_return::RunAppNeverReturn;
use winit_core::event_loop::pump_events::{PumpEvents, PumpStatus};
use winit_core::event_loop::register::RegisterApp;
use winit_core::event_loop::run_on_demand::RunOnDemand;
use winit_core::event_loop::{
    ActiveEventLoop, ControlFlow, DeviceEvents, EventLoopProvider, EventLoopProxy,
    EventLoopProxyProvider, OwnedDisplayHandle,
};
use winit_core::monitor::MonitorHandle;
use winit_core::window::{Theme, Window, WindowAttributes, WindowId};

#[derive(Debug, Default)]
struct Display {
    unavailable: AtomicBool,
}

impl HasDisplayHandle for Display {
    fn display_handle(&self) -> Result<DisplayHandle<'_>, HandleError> {
        if self.unavailable.load(Ordering::Relaxed) {
            Err(HandleError::Unavailable)
        } else {
            Ok(DisplayHandle::web())
        }
    }
}

#[derive(Debug, Default)]
struct Proxy(AtomicUsize);

impl EventLoopProxyProvider for Proxy {
    fn wake_up(&self) {
        self.0.fetch_add(1, Ordering::Relaxed);
    }
}

#[derive(Default)]
struct State {
    control_flow: Cell<ControlFlow>,
    device_events: Cell<DeviceEvents>,
    display_requests: Cell<usize>,
    cursor_requests: RefCell<Vec<CustomCursorSource>>,
    provider_drops: Cell<usize>,
    handler_drops: Cell<usize>,
    callbacks: RefCell<Vec<&'static str>>,
    pending_app: RefCell<Option<Box<dyn ApplicationHandler>>>,
    display: Arc<Display>,
    proxy: Arc<Proxy>,
    exiting: Cell<bool>,
    supports_pump: Cell<bool>,
    supports_on_demand: Cell<bool>,
    supports_register: Cell<bool>,
    supports_never_return: Cell<bool>,
    pump_timeout: Cell<Option<std::time::Duration>>,
    #[cfg(unix)]
    fd: Option<std::os::unix::net::UnixStream>,
}

impl fmt::Debug for State {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("State").finish_non_exhaustive()
    }
}

#[derive(Debug, Default)]
enum RunMode {
    #[default]
    Immediate,
    Deferred,
    Error,
}

#[derive(Debug, Default)]
struct Provider {
    state: Rc<State>,
    mode: RunMode,
}

impl Drop for Provider {
    fn drop(&mut self) {
        self.state.provider_drops.set(self.state.provider_drops.get() + 1);
    }
}

impl EventLoopProvider for Provider {
    fn run_app(
        self: Box<Self>,
        mut app: Box<dyn ApplicationHandler>,
    ) -> Result<(), EventLoopError> {
        match self.mode {
            RunMode::Immediate => {
                let active = Active(self.state.clone());
                app.new_events(&active, StartCause::Init);
                app.can_create_surfaces(&active);
                app.proxy_wake_up(&active);
                app.about_to_wait(&active);
                assert!(active.exiting());
                Ok(())
            },
            RunMode::Deferred => {
                *self.state.pending_app.borrow_mut() = Some(app);
                Ok(())
            },
            RunMode::Error => Err(EventLoopError::ExitFailure(42)),
        }
    }

    fn pump_events_ext(&mut self) -> Option<&mut dyn PumpEvents> {
        if self.state.supports_pump.get() { Some(self) } else { None }
    }

    fn run_on_demand_ext(&mut self) -> Option<&mut dyn RunOnDemand> {
        if self.state.supports_on_demand.get() { Some(self) } else { None }
    }

    fn into_register(self: Box<Self>) -> Result<Box<dyn RegisterApp>, Box<dyn EventLoopProvider>> {
        if self.state.supports_register.get() { Ok(self) } else { Err(self) }
    }

    fn into_never_return(
        self: Box<Self>,
    ) -> Result<Box<dyn RunAppNeverReturn>, Box<dyn EventLoopProvider>> {
        if self.state.supports_never_return.get() { Ok(self) } else { Err(self) }
    }

    #[cfg(unix)]
    fn as_fd_ext(&self) -> Option<&dyn std::os::fd::AsFd> {
        self.state.fd.as_ref().map(|fd| fd as &dyn std::os::fd::AsFd)
    }

    fn create_proxy(&self) -> EventLoopProxy {
        EventLoopProxy::new(self.state.proxy.clone())
    }

    fn owned_display_handle(&self) -> OwnedDisplayHandle {
        self.state.display_requests.set(self.state.display_requests.get() + 1);
        OwnedDisplayHandle::new(self.state.display.clone())
    }

    fn listen_device_events(&self, allowed: DeviceEvents) {
        self.state.device_events.set(allowed);
    }

    fn set_control_flow(&self, control_flow: ControlFlow) {
        self.state.control_flow.set(control_flow);
    }

    fn create_custom_cursor(
        &self,
        source: CustomCursorSource,
    ) -> Result<CustomCursor, CustomCursorError> {
        self.state.cursor_requests.borrow_mut().push(source);
        Err(CustomCursorError::UnsupportedSource)
    }
}

// The active loop is constructed only when callbacks are dispatched.
#[derive(Debug)]
struct Active(Rc<State>);

impl ActiveEventLoop for Active {
    fn create_proxy(&self) -> EventLoopProxy {
        EventLoopProxy::new(self.0.proxy.clone())
    }

    fn create_window(&self, _: WindowAttributes) -> Result<Box<dyn Window>, CreateWindowError> {
        Err(CreateWindowError::PopupNotSupported)
    }

    fn create_custom_cursor(
        &self,
        _: CustomCursorSource,
    ) -> Result<CustomCursor, CustomCursorError> {
        Err(CustomCursorError::NotSupported)
    }

    fn available_monitors(&self) -> Box<dyn Iterator<Item = MonitorHandle>> {
        Box::new(std::iter::empty())
    }

    fn primary_monitor(&self) -> Option<MonitorHandle> {
        None
    }

    fn listen_device_events(&self, allowed: DeviceEvents) {
        self.0.device_events.set(allowed);
    }

    fn system_theme(&self) -> Option<Theme> {
        None
    }

    fn set_control_flow(&self, control_flow: ControlFlow) {
        self.0.control_flow.set(control_flow);
    }

    fn control_flow(&self) -> ControlFlow {
        self.0.control_flow.get()
    }

    fn exit(&self) {
        self.0.exiting.set(true);
    }

    fn exiting(&self) -> bool {
        self.0.exiting.get()
    }

    fn owned_display_handle(&self) -> OwnedDisplayHandle {
        OwnedDisplayHandle::new(self.0.display.clone())
    }

    fn rwh_06_handle(&self) -> &dyn HasDisplayHandle {
        &*self.0.display
    }
}

struct App(Rc<State>);

impl Drop for App {
    fn drop(&mut self) {
        self.0.handler_drops.set(self.0.handler_drops.get() + 1);
    }
}

impl ApplicationHandler for App {
    fn new_events(&mut self, _: &dyn ActiveEventLoop, cause: StartCause) {
        assert_eq!(cause, StartCause::Init);
        self.0.callbacks.borrow_mut().push("init");
    }

    fn can_create_surfaces(&mut self, active: &dyn ActiveEventLoop) {
        self.0.callbacks.borrow_mut().push("surfaces");
        active.set_control_flow(ControlFlow::Poll);
    }

    fn proxy_wake_up(&mut self, _: &dyn ActiveEventLoop) {
        self.0.callbacks.borrow_mut().push("wake");
    }

    fn window_event(&mut self, _: &dyn ActiveEventLoop, _: WindowId, _: WindowEvent) {}

    fn about_to_wait(&mut self, active: &dyn ActiveEventLoop) {
        self.0.callbacks.borrow_mut().push("wait");
        active.exit();
    }
}

#[test]
fn wraps_provider_without_initializing_or_running_it() {
    let provider = Provider::default();
    let state = provider.state.clone();
    let event_loop: EventLoop = EventLoop::from_backend(provider);
    assert_eq!(state.display_requests.get(), 0);
    assert!(state.callbacks.borrow().is_empty());
    assert_eq!(state.provider_drops.get(), 0);
    drop(event_loop);
    assert_eq!(state.provider_drops.get(), 1);
}

#[test]
fn forwards_operations_before_running() {
    let provider = Provider::default();
    let state = provider.state.clone();
    let event_loop: EventLoop = EventLoop::from_backend(provider);
    EventLoopProvider::set_control_flow(&event_loop, ControlFlow::Poll);
    EventLoopProvider::listen_device_events(&event_loop, DeviceEvents::Always);
    EventLoopProvider::create_proxy(&event_loop).wake_up();
    assert_eq!(state.control_flow.get(), ControlFlow::Poll);
    assert_eq!(state.device_events.get(), DeviceEvents::Always);
    assert_eq!(state.proxy.0.load(Ordering::Relaxed), 1);
    let source = CustomCursorSource::Url { url: "cursor.png".into(), hotspot_x: 3, hotspot_y: 5 };
    assert!(matches!(
        EventLoopProvider::create_custom_cursor(&event_loop, source.clone()),
        Err(CustomCursorError::UnsupportedSource)
    ));
    assert_eq!(*state.cursor_requests.borrow(), [source]);
}

#[test]
fn display_handle_retains_its_owner_and_propagates_availability() {
    let provider = Provider::default();
    let state = provider.state.clone();
    let display = Arc::downgrade(&state.display);
    let event_loop: EventLoop = EventLoop::from_backend(provider);
    state.display.unavailable.store(true, Ordering::Relaxed);
    assert!(matches!(event_loop.display_handle(), Err(HandleError::Unavailable)));
    state.display.unavailable.store(false, Ordering::Relaxed);
    assert_eq!(event_loop.display_handle().unwrap(), DisplayHandle::web());
    assert_eq!(state.display_requests.get(), 1);
    let owned = EventLoopProvider::owned_display_handle(&event_loop);
    assert_eq!(state.display_requests.get(), 2);
    drop(event_loop);
    drop(state);
    assert!(display.upgrade().is_some());
    assert_eq!(owned.display_handle().unwrap(), DisplayHandle::web());
    drop(owned);
    assert!(display.upgrade().is_none());
}

#[test]
fn runs_with_backend_supplied_active_loop_and_drops_handler() {
    let provider = Provider::default();
    let state = provider.state.clone();
    EventLoopProvider::run_app(
        Box::new(EventLoop::from_backend(provider)),
        Box::new(App(state.clone())),
    )
    .unwrap();
    assert_eq!(*state.callbacks.borrow(), ["init", "surfaces", "wake", "wait"]);
    assert_eq!(state.control_flow.get(), ControlFlow::Poll);
    assert_eq!(state.provider_drops.get(), 1);
    assert_eq!(state.handler_drops.get(), 1);
}

#[test]
fn backend_can_retain_handler_after_run_app_returns() {
    let provider = Provider { mode: RunMode::Deferred, state: Rc::default() };
    let state = provider.state.clone();
    EventLoop::from_backend(provider).run_app(App(state.clone())).unwrap();
    assert!(state.callbacks.borrow().is_empty());
    assert_eq!(state.handler_drops.get(), 0);
    let mut app = state.pending_app.borrow_mut().take().unwrap();
    app.can_create_surfaces(&Active(state.clone()));
    assert_eq!(*state.callbacks.borrow(), ["surfaces"]);
    drop(app);
    assert_eq!(state.handler_drops.get(), 1);
}

#[test]
fn preserves_backend_error_and_releases_handler() {
    let provider = Provider { mode: RunMode::Error, state: Rc::default() };
    let state = provider.state.clone();
    assert!(matches!(
        EventLoop::from_backend(provider).run_app(App(state.clone())),
        Err(EventLoopError::ExitFailure(42))
    ));
    assert!(state.callbacks.borrow().is_empty());
    assert_eq!(state.provider_drops.get(), 1);
    assert_eq!(state.handler_drops.get(), 1);
}

#[cfg(not(builtin_platform))]
#[test]
fn missing_builtin_backend_returns_error() {
    assert!(matches!(EventLoop::new(), Err(EventLoopError::NotSupported(_))));
    assert!(matches!(EventLoop::builder().build(), Err(EventLoopError::NotSupported(_))));
}

#[test]
fn accepts_a_type_erased_provider() {
    let provider = Provider::default();
    let state = provider.state.clone();
    let backend: Box<dyn EventLoopProvider> = Box::new(provider);
    let event_loop: EventLoop = EventLoop::from_boxed_backend(backend);
    event_loop.run_app(App(state.clone())).unwrap();
    assert_eq!(*state.callbacks.borrow(), ["init", "surfaces", "wake", "wait"]);
    assert_eq!(state.handler_drops.get(), 1);
    assert_eq!(state.provider_drops.get(), 1);
}

#[test]
fn boxed_provider_supports_owned_run_app() {
    let provider = Provider::default();
    let state = provider.state.clone();
    let backend: Box<dyn EventLoopProvider> = Box::new(provider);
    backend.run_app(Box::new(App(state.clone()))).unwrap();
    assert_eq!(state.handler_drops.get(), 1);
    assert_eq!(state.provider_drops.get(), 1);
}

#[test]
fn unsupported_capabilities_preserve_the_event_loop() {
    use winit::event_loop::run_on_demand::EventLoopExtRunOnDemand;
    let provider = Provider::default();
    let state = provider.state.clone();
    let mut event_loop = EventLoop::from_backend(provider);
    let backend_ptr = event_loop.backend().cast_ref::<Provider>().unwrap() as *const Provider;
    event_loop.set_control_flow(ControlFlow::Poll);
    event_loop.display_handle().unwrap();
    assert!(event_loop.pump_events_ext().is_none());
    assert!(event_loop.run_on_demand_ext().is_none());
    #[cfg(unix)]
    assert!(event_loop.as_fd_ext().is_none());
    assert!(matches!(
        event_loop.run_app_on_demand(App(state.clone())),
        Err(EventLoopError::NotSupported(_))
    ));
    let event_loop = event_loop.into_register().unwrap_err();
    let event_loop = event_loop.into_never_return().unwrap_err();
    assert_eq!(
        event_loop.backend().cast_ref::<Provider>().unwrap() as *const Provider,
        backend_ptr
    );
    assert_eq!(event_loop.display_handle().unwrap(), DisplayHandle::web());
    assert_eq!(state.display_requests.get(), 1);
    assert_eq!(state.control_flow.get(), ControlFlow::Poll);
    assert_eq!(state.provider_drops.get(), 0);
    assert!(state.callbacks.borrow().is_empty());
    event_loop.run_app(App(state.clone())).unwrap();
    assert_eq!(state.provider_drops.get(), 1);
}

#[cfg(x11_platform)]
#[test]
fn external_provider_is_not_builtin_x11() {
    use winit::platform::x11::EventLoopExtX11;
    assert!(!EventLoop::from_backend(Provider::default()).is_x11());
}

#[cfg(wayland_platform)]
#[test]
fn external_provider_is_not_builtin_wayland() {
    use winit::platform::wayland::EventLoopExtWayland;
    assert!(!EventLoop::from_backend(Provider::default()).is_wayland());
}

impl PumpEvents for Provider {
    fn pump_app_events(
        &mut self,
        timeout: Option<std::time::Duration>,
        app: &mut dyn ApplicationHandler,
    ) -> PumpStatus {
        self.state.pump_timeout.set(timeout);
        app.new_events(&Active(self.state.clone()), StartCause::Init);
        PumpStatus::Exit(17)
    }
}

impl RunOnDemand for Provider {
    fn run_app_on_demand(
        &mut self,
        app: &mut dyn ApplicationHandler,
    ) -> Result<(), EventLoopError> {
        if matches!(self.mode, RunMode::Error) {
            return Err(EventLoopError::ExitFailure(42));
        }
        app.new_events(&Active(self.state.clone()), StartCause::Init);
        Ok(())
    }
}

impl RegisterApp for Provider {
    fn register_app(self: Box<Self>, app: Box<dyn ApplicationHandler>) {
        *self.state.pending_app.borrow_mut() = Some(app);
    }
}

impl RunAppNeverReturn for Provider {
    fn run_app_never_return(self: Box<Self>, mut app: Box<dyn ApplicationHandler>) -> ! {
        app.can_create_surfaces(&Active(self.state.clone()));
        // Model non-returning control flow without terminating the test process.
        panic!("test never-return provider");
    }
}

struct BorrowedApp<'a>(&'a mut usize);

impl ApplicationHandler for BorrowedApp<'_> {
    fn new_events(&mut self, _: &dyn ActiveEventLoop, _: StartCause) {
        *self.0 += 1;
    }
    fn can_create_surfaces(&mut self, _: &dyn ActiveEventLoop) {}
    fn window_event(&mut self, _: &dyn ActiveEventLoop, _: WindowId, _: WindowEvent) {}
}

#[test]
fn pumping_uses_external_provider_and_borrowed_handler() {
    use winit::event_loop::pump_events::EventLoopExtPumpEvents;
    let provider = Provider::default();
    let state = provider.state.clone();
    state.supports_pump.set(true);
    let mut event_loop = EventLoop::from_backend(provider);
    let mut calls = 0;
    assert_eq!(
        event_loop
            .pump_events_ext()
            .unwrap()
            .pump_app_events(Some(std::time::Duration::ZERO), &mut BorrowedApp(&mut calls)),
        PumpStatus::Exit(17)
    );
    assert_eq!(state.pump_timeout.get(), Some(std::time::Duration::ZERO));
    assert_eq!(event_loop.pump_app_events(None, BorrowedApp(&mut calls)), PumpStatus::Exit(17));
    assert_eq!(state.pump_timeout.get(), None);
    assert_eq!(calls, 2);
    assert_eq!(state.provider_drops.get(), 0);
}

#[test]
fn running_on_demand_reuses_provider_and_propagates_errors() {
    use winit::event_loop::run_on_demand::EventLoopExtRunOnDemand;
    let provider = Provider::default();
    provider.state.supports_on_demand.set(true);
    let mut event_loop = EventLoop::from_backend(provider);
    let mut calls = 0;
    event_loop
        .run_on_demand_ext()
        .unwrap()
        .run_app_on_demand(&mut BorrowedApp(&mut calls))
        .unwrap();
    event_loop.run_app_on_demand(BorrowedApp(&mut calls)).unwrap();
    assert_eq!(calls, 2);
    event_loop.backend_mut().cast_mut::<Provider>().unwrap().mode = RunMode::Error;
    assert!(matches!(
        event_loop.run_app_on_demand(BorrowedApp(&mut calls)),
        Err(EventLoopError::ExitFailure(42))
    ));
    assert_eq!(calls, 2);
}

#[test]
fn registration_keeps_handler_alive_for_later_callbacks() {
    let provider = Provider::default();
    let state = provider.state.clone();
    state.supports_register.set(true);
    let registration = EventLoop::from_backend(provider).into_register().unwrap();
    assert_eq!(state.provider_drops.get(), 0);
    registration.register_app(Box::new(App(state.clone())));
    assert!(state.callbacks.borrow().is_empty());
    assert_eq!(state.handler_drops.get(), 0);
    let mut app = state.pending_app.borrow_mut().take().unwrap();
    app.can_create_surfaces(&Active(state.clone()));
    assert_eq!(*state.callbacks.borrow(), ["surfaces"]);
    drop(app);
    assert_eq!(state.handler_drops.get(), 1);
    assert_eq!(state.provider_drops.get(), 1);
}

#[test]
fn existing_registration_extension_dispatches_to_external_provider() {
    use winit::event_loop::register::EventLoopExtRegister;
    let provider = Provider::default();
    let state = provider.state.clone();
    state.supports_register.set(true);
    EventLoop::from_backend(provider).register_app(App(state.clone()));
    drop(state.pending_app.borrow_mut().take().unwrap());
    assert_eq!(state.handler_drops.get(), 1);
}

#[test]
fn never_return_capability_dispatches_and_transfers_ownership() {
    let provider = Provider::default();
    let state = provider.state.clone();
    state.supports_never_return.set(true);
    let never_return = EventLoop::from_backend(provider).into_never_return().unwrap();
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        never_return.run_app_never_return(Box::new(App(state.clone())))
    }));
    assert!(result.is_err());
    assert_eq!(*state.callbacks.borrow(), ["surfaces"]);
    assert_eq!(state.provider_drops.get(), 1);
    assert_eq!(state.handler_drops.get(), 1);
}

#[test]
fn dropping_unused_owned_capabilities_releases_provider() {
    let provider = Provider::default();
    let state = provider.state.clone();
    state.supports_register.set(true);
    drop(EventLoop::from_backend(provider).into_register().unwrap());
    assert_eq!(state.provider_drops.get(), 1);
    assert!(state.callbacks.borrow().is_empty());
}

#[test]
fn backend_can_be_inspected_configured_and_recovered() {
    let provider = Provider::default();
    let state = provider.state.clone();
    let backend: Box<dyn EventLoopProvider> = Box::new(provider);
    let mut event_loop = EventLoop::from_boxed_backend(backend);
    assert!(event_loop.backend().cast_ref::<Provider>().is_some());
    assert!(event_loop.backend().cast_ref::<EventLoop>().is_none());
    event_loop.backend_mut().cast_mut::<Provider>().unwrap().mode = RunMode::Error;
    let backend = event_loop.into_backend();
    let backend = backend.cast::<EventLoop>().unwrap_err();
    let backend = backend.cast::<Provider>().unwrap();
    assert!(matches!(backend.mode, RunMode::Error));
    assert_eq!(state.provider_drops.get(), 0);
    drop(backend);
    assert_eq!(state.provider_drops.get(), 1);
}

#[test]
fn facade_used_as_provider_forwards_optional_capabilities() {
    let provider = Provider::default();
    let state = provider.state.clone();
    state.supports_pump.set(true);
    state.supports_on_demand.set(true);
    let mut facade: Box<dyn EventLoopProvider> = Box::new(EventLoop::from_backend(provider));
    let mut calls = 0;
    facade.pump_events_ext().unwrap().pump_app_events(None, &mut BorrowedApp(&mut calls));
    facade.run_on_demand_ext().unwrap().run_app_on_demand(&mut BorrowedApp(&mut calls)).unwrap();
    assert_eq!(calls, 2);
    let facade = facade.into_register().unwrap_err();
    let facade = facade.into_never_return().unwrap_err();
    state.supports_register.set(true);
    facade.into_register().unwrap().register_app(Box::new(App(state.clone())));
    drop(state.pending_app.borrow_mut().take().unwrap());
    assert_eq!(state.provider_drops.get(), 1);
}

#[cfg(unix)]
#[test]
fn file_descriptor_capability_preserves_backend_descriptor() {
    use std::os::fd::AsRawFd;
    let (stream, _peer) = std::os::unix::net::UnixStream::pair().unwrap();
    let raw_fd = stream.as_raw_fd();
    let mut provider = Provider::default();
    Rc::get_mut(&mut provider.state).unwrap().fd = Some(stream);
    let event_loop = EventLoop::from_backend(provider);
    assert_eq!(event_loop.as_fd_ext().unwrap().as_fd().as_raw_fd(), raw_fd);
    #[cfg(any(x11_platform, wayland_platform))]
    assert_eq!(event_loop.as_raw_fd(), raw_fd);
    let facade: Box<dyn EventLoopProvider> = Box::new(event_loop);
    assert_eq!(facade.as_fd_ext().unwrap().as_fd().as_raw_fd(), raw_fd);
    let provider = (*facade.cast::<EventLoop>().unwrap()).into_backend();
    assert_eq!(provider.as_fd_ext().unwrap().as_fd().as_raw_fd(), raw_fd);
}

#[test]
fn existing_never_return_extension_dispatches_to_external_provider() {
    use winit::event_loop::never_return::EventLoopExtNeverReturn;
    let provider = Provider::default();
    let state = provider.state.clone();
    state.supports_never_return.set(true);
    let event_loop = EventLoop::from_backend(provider);
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        event_loop.run_app_never_return(App(state.clone()))
    }));
    assert!(result.is_err());
    assert_eq!(*state.callbacks.borrow(), ["surfaces"]);
    assert_eq!(state.provider_drops.get(), 1);
    assert_eq!(state.handler_drops.get(), 1);
}
