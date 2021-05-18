use std::{
    cell::{RefCell, RefMut},
    collections::VecDeque,
    fmt::{self, Debug},
    hint::unreachable_unchecked,
    mem,
    rc::{Rc, Weak},
    sync::{
        atomic::{AtomicBool, Ordering},
        Mutex, MutexGuard,
    },
    time::Instant,
};

use cocoa::{
    appkit::{NSApp, NSApplication, NSWindow},
    base::{id, nil},
    foundation::{NSAutoreleasePool, NSSize},
};
use objc::runtime::YES;

use objc::runtime::Object;

use crate::{
    dpi::LogicalSize,
    event::{Event, StartCause, WindowEvent},
    event_loop::{ControlFlow, EventLoopWindowTarget as RootWindowTarget},
    platform::macos::ActivationPolicy,
    platform_impl::{
        get_aux_state_mut,
        platform::{
            event::{EventProxy, EventWrapper},
            event_loop::{post_dummy_event, PanicInfo},
            menu,
            observer::{CFRunLoopGetMain, CFRunLoopWakeUp, EventLoopWaker},
            util::{IdRef, Never},
            window::get_window_id,
        },
    },
    window::WindowId,
};

lazy_static! {
    static ref HANDLER: Handler = Default::default();
}

impl<'a, Never> Event<'a, Never> {
    fn userify<T: 'static>(self) -> Event<'a, T> {
        self.map_nonuser_event()
            // `Never` can't be constructed, so the `UserEvent` variant can't
            // be present here.
            .unwrap_or_else(|_| unsafe { unreachable_unchecked() })
    }
}

pub trait EventHandler: Debug {
    // Not sure probably it should accept Event<'static, Never>
    fn handle_nonuser_event(&mut self, event: Event<'_, Never>, control_flow: &mut ControlFlow);
    fn handle_user_events(&mut self, control_flow: &mut ControlFlow);
}

struct EventLoopHandler<T: 'static> {
    callback: Weak<RefCell<dyn FnMut(Event<'_, T>, &RootWindowTarget<T>, &mut ControlFlow)>>,
    will_exit: bool,
    window_target: Rc<RootWindowTarget<T>>,
}

impl<T> EventLoopHandler<T> {
    fn with_callback<F>(&mut self, f: F)
    where
        F: FnOnce(
            &mut EventLoopHandler<T>,
            RefMut<'_, dyn FnMut(Event<'_, T>, &RootWindowTarget<T>, &mut ControlFlow)>,
        ),
    {
        if let Some(callback) = self.callback.upgrade() {
            let callback = callback.borrow_mut();
            (f)(self, callback);
        } else {
            panic!(
                "Tried to dispatch an event, but the event loop that \
                owned the event handler callback seems to be destroyed"
            );
        }
    }
}

impl<T> Debug for EventLoopHandler<T> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("EventLoopHandler")
            .field("window_target", &self.window_target)
            .finish()
    }
}

impl<T> EventHandler for EventLoopHandler<T> {
    fn handle_nonuser_event(&mut self, event: Event<'_, Never>, control_flow: &mut ControlFlow) {
        self.with_callback(|this, mut callback| {
            (callback)(event.userify(), &this.window_target, control_flow);
            this.will_exit |= *control_flow == ControlFlow::Exit;
            if this.will_exit {
                *control_flow = ControlFlow::Exit;
            }
        });
    }

    fn handle_user_events(&mut self, control_flow: &mut ControlFlow) {
        self.with_callback(|this, mut callback| {
            let mut will_exit = this.will_exit;
            for event in this.window_target.p.receiver.try_iter() {
                (callback)(Event::UserEvent(event), &this.window_target, control_flow);
                will_exit |= *control_flow == ControlFlow::Exit;
                if will_exit {
                    *control_flow = ControlFlow::Exit;
                }
            }
            this.will_exit = will_exit;
        });
    }
}

#[derive(Default)]
struct Handler {
    ready: AtomicBool,
    in_callback: AtomicBool,
    dialog_is_closing: AtomicBool,
    control_flow: Mutex<ControlFlow>,
    control_flow_prev: Mutex<ControlFlow>,
    start_time: Mutex<Option<Instant>>,
    callback: Mutex<Option<Box<dyn EventHandler>>>,
    pending_events: Mutex<VecDeque<EventWrapper>>,
    pending_redraw: Mutex<Vec<WindowId>>,
    waker: Mutex<EventLoopWaker>,
}

unsafe impl Send for Handler {}
unsafe impl Sync for Handler {}

impl Handler {
    fn events(&self) -> MutexGuard<'_, VecDeque<EventWrapper>> {
        self.pending_events.lock().unwrap()
    }

    fn redraw<'a>(&'a self) -> MutexGuard<'a, Vec<WindowId>> {
        self.pending_redraw.lock().unwrap()
    }

    fn waker(&self) -> MutexGuard<'_, EventLoopWaker> {
        self.waker.lock().unwrap()
    }

    fn is_ready(&self) -> bool {
        self.ready.load(Ordering::Acquire)
    }

    fn set_ready(&self) {
        self.ready.store(true, Ordering::Release);
    }

    fn should_exit(&self) -> bool {
        *self.control_flow.lock().unwrap() == ControlFlow::Exit
    }

    fn get_control_flow_and_update_prev(&self) -> ControlFlow {
        let control_flow = self.control_flow.lock().unwrap();
        *self.control_flow_prev.lock().unwrap() = *control_flow;
        *control_flow
    }

    fn get_old_and_new_control_flow(&self) -> (ControlFlow, ControlFlow) {
        let old = *self.control_flow_prev.lock().unwrap();
        let new = *self.control_flow.lock().unwrap();
        (old, new)
    }

    fn get_start_time(&self) -> Option<Instant> {
        *self.start_time.lock().unwrap()
    }

    fn update_start_time(&self) {
        *self.start_time.lock().unwrap() = Some(Instant::now());
    }

    fn take_events(&self) -> VecDeque<EventWrapper> {
        mem::replace(&mut *self.events(), Default::default())
    }

    fn should_redraw(&self) -> Vec<WindowId> {
        mem::replace(&mut *self.redraw(), Default::default())
    }

    fn get_in_callback(&self) -> bool {
        self.in_callback.load(Ordering::Acquire)
    }

    fn set_in_callback(&self, in_callback: bool) {
        self.in_callback.store(in_callback, Ordering::Release);
    }

    fn handle_nonuser_event(&self, wrapper: EventWrapper) {
        if let Some(ref mut callback) = *self.callback.lock().unwrap() {
            match wrapper {
                EventWrapper::StaticEvent(event) => {
                    callback.handle_nonuser_event(event, &mut *self.control_flow.lock().unwrap())
                }
                EventWrapper::EventProxy(proxy) => self.handle_proxy(proxy, callback),
            }
        }
    }

    fn handle_user_events(&self) {
        if let Some(ref mut callback) = *self.callback.lock().unwrap() {
            callback.handle_user_events(&mut *self.control_flow.lock().unwrap());
        }
    }

    fn handle_scale_factor_changed_event(
        &self,
        callback: &mut Box<dyn EventHandler + 'static>,
        ns_window: IdRef,
        suggested_size: LogicalSize<f64>,
        scale_factor: f64,
    ) {
        let mut size = suggested_size.to_physical(scale_factor);
        let new_inner_size = &mut size;
        let event = Event::WindowEvent {
            window_id: WindowId(get_window_id(*ns_window)),
            event: WindowEvent::ScaleFactorChanged {
                scale_factor,
                new_inner_size,
            },
        };

        callback.handle_nonuser_event(event, &mut *self.control_flow.lock().unwrap());

        let physical_size = *new_inner_size;
        let logical_size = physical_size.to_logical(scale_factor);
        let size = NSSize::new(logical_size.width, logical_size.height);
        unsafe { NSWindow::setContentSize_(*ns_window, size) };
    }

    fn handle_proxy(&self, proxy: EventProxy, callback: &mut Box<dyn EventHandler + 'static>) {
        match proxy {
            EventProxy::DpiChangedProxy {
                ns_window,
                suggested_size,
                scale_factor,
            } => self.handle_scale_factor_changed_event(
                callback,
                ns_window,
                suggested_size,
                scale_factor,
            ),
        }
    }
}

pub static INTERRUPT_EVENT_LOOP_EXIT: AtomicBool = AtomicBool::new(false);

pub enum AppState {}

impl AppState {
    pub fn set_callback<T>(
        callback: Weak<RefCell<dyn FnMut(Event<'_, T>, &RootWindowTarget<T>, &mut ControlFlow)>>,
        window_target: Rc<RootWindowTarget<T>>,
    ) {
        *HANDLER.callback.lock().unwrap() = Some(Box::new(EventLoopHandler {
            callback,
            will_exit: false,
            window_target,
        }));
    }

    pub fn exit() {
        HANDLER.set_in_callback(true);
        HANDLER.handle_nonuser_event(EventWrapper::StaticEvent(Event::LoopDestroyed));
        HANDLER.set_in_callback(false);
        HANDLER.callback.lock().unwrap().take();
    }

    pub fn launched(app_delegate: &Object) {
        apply_activation_policy(app_delegate);
        unsafe {
            let ns_app = NSApp();
            window_activation_hack(ns_app);
            // TODO: Consider allowing the user to specify they don't want their application activated
            ns_app.activateIgnoringOtherApps_(YES);
        };
        HANDLER.set_ready();
        HANDLER.waker().start();
        let create_default_menu = unsafe { get_aux_state_mut(app_delegate).create_default_menu };
        if create_default_menu {
            // The menubar initialization should be before the `NewEvents` event, to allow
            // overriding of the default menu even if it's created
            menu::initialize();
        }
        HANDLER.set_in_callback(true);
        HANDLER.handle_nonuser_event(EventWrapper::StaticEvent(Event::NewEvents(
            StartCause::Init,
        )));
        HANDLER.set_in_callback(false);
    }

    pub fn wakeup(panic_info: Weak<PanicInfo>) {
        let panic_info = panic_info
            .upgrade()
            .expect("The panic info must exist here. This failure indicates a developer error.");
        if panic_info.is_panicking() || !HANDLER.is_ready() {
            return;
        }
        let start = HANDLER.get_start_time().unwrap();
        let cause = match HANDLER.get_control_flow_and_update_prev() {
            ControlFlow::Poll => StartCause::Poll,
            ControlFlow::Wait => StartCause::WaitCancelled {
                start,
                requested_resume: None,
            },
            ControlFlow::WaitUntil(requested_resume) => {
                if Instant::now() >= requested_resume {
                    StartCause::ResumeTimeReached {
                        start,
                        requested_resume,
                    }
                } else {
                    StartCause::WaitCancelled {
                        start,
                        requested_resume: Some(requested_resume),
                    }
                }
            }
            ControlFlow::Exit => StartCause::Poll, //panic!("unexpected `ControlFlow::Exit`"),
        };
        HANDLER.set_in_callback(true);
        HANDLER.handle_nonuser_event(EventWrapper::StaticEvent(Event::NewEvents(cause)));
        HANDLER.set_in_callback(false);
    }

    // This is called from multiple threads at present
    pub fn queue_redraw(window_id: WindowId) {
        let mut pending_redraw = HANDLER.redraw();
        if !pending_redraw.contains(&window_id) {
            pending_redraw.push(window_id);
        }
        unsafe {
            let rl = CFRunLoopGetMain();
            CFRunLoopWakeUp(rl);
        }
    }

    pub fn handle_redraw(window_id: WindowId) {
        HANDLER.handle_nonuser_event(EventWrapper::StaticEvent(Event::RedrawRequested(window_id)));
    }

    pub fn queue_event(wrapper: EventWrapper) {
        if !unsafe { msg_send![class!(NSThread), isMainThread] } {
            panic!("Event queued from different thread: {:#?}", wrapper);
        }
        HANDLER.events().push_back(wrapper);
    }

    pub fn queue_events(mut wrappers: VecDeque<EventWrapper>) {
        if !unsafe { msg_send![class!(NSThread), isMainThread] } {
            panic!("Events queued from different thread: {:#?}", wrappers);
        }
        HANDLER.events().append(&mut wrappers);
    }

    pub fn cleared(panic_info: Weak<PanicInfo>) {
        let panic_info = panic_info
            .upgrade()
            .expect("The panic info must exist here. This failure indicates a developer error.");
        if panic_info.is_panicking() || !HANDLER.is_ready() {
            return;
        }
        if !HANDLER.get_in_callback() {
            HANDLER.set_in_callback(true);
            HANDLER.handle_user_events();
            for event in HANDLER.take_events() {
                HANDLER.handle_nonuser_event(event);
            }
            HANDLER.handle_nonuser_event(EventWrapper::StaticEvent(Event::MainEventsCleared));
            for window_id in HANDLER.should_redraw() {
                HANDLER.handle_nonuser_event(EventWrapper::StaticEvent(Event::RedrawRequested(
                    window_id,
                )));
            }
            HANDLER.handle_nonuser_event(EventWrapper::StaticEvent(Event::RedrawEventsCleared));
            HANDLER.set_in_callback(false);
        }
        if HANDLER.should_exit() {
            unsafe {
                let app: id = NSApp();
                let windows: id = msg_send![app, windows];
                let window_count: usize = msg_send![windows, count];

                let dialog_open = if window_count > 1 {
                    let dialog: id = msg_send![windows, lastObject];
                    let is_main_window: bool = msg_send![dialog, isMainWindow];
                    msg_send![dialog, isVisible] && !is_main_window
                } else {
                    false
                };

                let dialog_is_closing = HANDLER.dialog_is_closing.load(Ordering::SeqCst);
                let pool = NSAutoreleasePool::new(nil);
                if !INTERRUPT_EVENT_LOOP_EXIT.load(Ordering::SeqCst)
                    && !dialog_open
                    && !dialog_is_closing
                {
                    let () = msg_send![app, stop: nil];
                    // To stop event loop immediately, we need to post some event here.
                    post_dummy_event(app);
                }
                pool.drain();

                if window_count > 0 {
                    let window: id = msg_send![windows, objectAtIndex:0];
                    let window_has_focus = msg_send![window, isKeyWindow];
                    if !dialog_open && window_has_focus && dialog_is_closing {
                        HANDLER.dialog_is_closing.store(false, Ordering::SeqCst);
                    }
                    if dialog_open {
                        HANDLER.dialog_is_closing.store(true, Ordering::SeqCst);
                    }
                }
            };
        }
        HANDLER.update_start_time();
        match HANDLER.get_old_and_new_control_flow() {
            (ControlFlow::Exit, _) | (_, ControlFlow::Exit) => (),
            (old, new) if old == new => (),
            (_, ControlFlow::Wait) => HANDLER.waker().stop(),
            (_, ControlFlow::WaitUntil(instant)) => HANDLER.waker().start_at(instant),
            (_, ControlFlow::Poll) => HANDLER.waker().start(),
        }
    }
}

/// A hack to make activation of multiple windows work when creating them before
/// `applicationDidFinishLaunching:` / `Event::Event::NewEvents(StartCause::Init)`.
///
/// Alternative to this would be the user calling `window.set_visible(true)` in
/// `StartCause::Init`.
///
/// If this becomes too bothersome to maintain, it can probably be removed
/// without too much damage.
unsafe fn window_activation_hack(ns_app: id) {
    // Get the application's windows
    // TODO: Proper ordering of the windows
    let ns_windows: id = msg_send![ns_app, windows];
    let ns_enumerator: id = msg_send![ns_windows, objectEnumerator];
    loop {
        // Enumerate over the windows
        let ns_window: id = msg_send![ns_enumerator, nextObject];
        if ns_window == nil {
            break;
        }
        // And call `makeKeyAndOrderFront` if it was called on the window in `UnownedWindow::new`
        // This way we preserve the user's desired initial visiblity status
        // TODO: Also filter on the type/"level" of the window, and maybe other things?
        if ns_window.isVisible() == YES {
            trace!("Activating visible window");
            ns_window.makeKeyAndOrderFront_(nil);
        } else {
            trace!("Skipping activating invisible window");
        }
    }
}
fn apply_activation_policy(app_delegate: &Object) {
    unsafe {
        use cocoa::appkit::NSApplicationActivationPolicy::*;
        let ns_app = NSApp();
        // We need to delay setting the activation policy and activating the app
        // until `applicationDidFinishLaunching` has been called. Otherwise the
        // menu bar won't be interactable.
        let act_pol = get_aux_state_mut(app_delegate).activation_policy;
        ns_app.setActivationPolicy_(match act_pol {
            ActivationPolicy::Regular => NSApplicationActivationPolicyRegular,
            ActivationPolicy::Accessory => NSApplicationActivationPolicyAccessory,
            ActivationPolicy::Prohibited => NSApplicationActivationPolicyProhibited,
        });
    }
}
