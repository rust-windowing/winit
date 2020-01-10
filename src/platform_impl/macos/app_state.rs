use std::{
    collections::VecDeque,
    fmt::{self, Debug},
    hint::unreachable_unchecked,
    mem,
    rc::Rc,
    sync::{
        atomic::{AtomicBool, Ordering},
        Mutex, MutexGuard,
    },
    time::Instant,
};

use cocoa::{
    appkit::{NSApp, NSEventType::NSApplicationDefined, NSWindow},
    base::{id, nil},
    foundation::{NSAutoreleasePool, NSPoint, NSSize},
};

use objc::runtime::YES;

use crate::{
    dpi::LogicalSize,
    event::{Event, StartCause, WindowEvent},
    event_loop::{ControlFlow, EventLoopWindowTarget as RootWindowTarget},
    platform_impl::platform::{
        event::{EventProxy, EventWrapper},
        observer::EventLoopWaker,
        util::{IdRef, Never},
        window::get_window_id,
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
    callback: Box<dyn FnMut(Event<'_, T>, &RootWindowTarget<T>, &mut ControlFlow)>,
    will_exit: bool,
    window_target: Rc<RootWindowTarget<T>>,
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
        (self.callback)(event.userify(), &self.window_target, control_flow);
        self.will_exit |= *control_flow == ControlFlow::Exit;
        if self.will_exit {
            *control_flow = ControlFlow::Exit;
        }
    }

    fn handle_user_events(&mut self, control_flow: &mut ControlFlow) {
        let mut will_exit = self.will_exit;
        for event in self.window_target.p.receiver.try_iter() {
            (self.callback)(Event::UserEvent(event), &self.window_target, control_flow);
            will_exit |= *control_flow == ControlFlow::Exit;
            if will_exit {
                *control_flow = ControlFlow::Exit;
            }
        }
        self.will_exit = will_exit;
    }
}

#[derive(Default)]
struct Handler {
    ready: AtomicBool,
    in_callback: AtomicBool,
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

pub enum AppState {}

impl AppState {
    // This function extends lifetime of `callback` to 'static as its side effect
    pub unsafe fn set_callback<F, T>(callback: F, window_target: Rc<RootWindowTarget<T>>)
    where
        F: FnMut(Event<'_, T>, &RootWindowTarget<T>, &mut ControlFlow),
    {
        *HANDLER.callback.lock().unwrap() = Some(Box::new(EventLoopHandler {
            // This transmute is always safe, in case it was reached through `run`, since our
            // lifetime will be already 'static. In other cases caller should ensure that all data
            // they passed to callback will actually outlive it, some apps just can't move
            // everything to event loop, so this is something that they should care about.
            callback: mem::transmute::<
                Box<dyn FnMut(Event<'_, T>, &RootWindowTarget<T>, &mut ControlFlow)>,
                Box<dyn FnMut(Event<'_, T>, &RootWindowTarget<T>, &mut ControlFlow)>,
            >(Box::new(callback)),
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

    pub fn launched() {
        HANDLER.set_ready();
        HANDLER.waker().start();
        HANDLER.set_in_callback(true);
        HANDLER.handle_nonuser_event(EventWrapper::StaticEvent(Event::NewEvents(
            StartCause::Init,
        )));
        HANDLER.set_in_callback(false);
    }

    pub fn wakeup() {
        if !HANDLER.is_ready() {
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

    pub fn cleared() {
        if !HANDLER.is_ready() {
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
                let _: () = msg_send![NSApp(), stop: nil];

                let pool = NSAutoreleasePool::new(nil);

                let windows: id = msg_send![NSApp(), windows];
                let window: id = msg_send![windows, objectAtIndex:0];
                assert_ne!(window, nil);

                let dummy_event: id = msg_send![class!(NSEvent),
                    otherEventWithType: NSApplicationDefined
                    location: NSPoint::new(0.0, 0.0)
                    modifierFlags: 0
                    timestamp: 0
                    windowNumber: 0
                    context: nil
                    subtype: 0
                    data1: 0
                    data2: 0
                ];
                // To stop event loop immediately, we need to post some event here.
                let _: () = msg_send![window, postEvent: dummy_event atStart: YES];

                pool.drain();
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
