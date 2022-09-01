use std::{
    collections::VecDeque,
    ffi::c_void,
    fmt::{self, Debug},
    marker::PhantomData,
    mem, ptr,
    sync::mpsc::{self, Receiver, Sender},
};

use objc::runtime::Object;
use raw_window_handle::{RawDisplayHandle, UiKitDisplayHandle};

use crate::{
    dpi::LogicalSize,
    event::Event,
    event_loop::{
        ControlFlow, EventLoopClosed, EventLoopWindowTarget as RootEventLoopWindowTarget,
    },
    monitor::MonitorHandle as RootMonitorHandle,
    platform::ios::Idiom,
};

use crate::platform_impl::platform::{
    app_state,
    ffi::{
        id, kCFRunLoopAfterWaiting, kCFRunLoopBeforeWaiting, kCFRunLoopCommonModes,
        kCFRunLoopDefaultMode, kCFRunLoopEntry, kCFRunLoopExit, nil, CFIndex, CFRelease,
        CFRunLoopActivity, CFRunLoopAddObserver, CFRunLoopAddSource, CFRunLoopGetMain,
        CFRunLoopObserverCreate, CFRunLoopObserverRef, CFRunLoopSourceContext,
        CFRunLoopSourceCreate, CFRunLoopSourceInvalidate, CFRunLoopSourceRef,
        CFRunLoopSourceSignal, CFRunLoopWakeUp, NSStringRust, UIApplicationMain,
        UIUserInterfaceIdiom,
    },
    monitor, view, MonitorHandle,
};

#[derive(Debug)]
pub enum EventWrapper {
    StaticEvent(Event<'static, Never>),
    EventProxy(EventProxy),
}

#[derive(Debug, PartialEq)]
pub enum EventProxy {
    DpiChangedProxy {
        window_id: id,
        suggested_size: LogicalSize<f64>,
        scale_factor: f64,
    },
}

pub struct EventLoopWindowTarget<T: 'static> {
    receiver: Receiver<T>,
    sender_to_clone: Sender<T>,
}

impl<T: 'static> EventLoopWindowTarget<T> {
    pub fn available_monitors(&self) -> VecDeque<MonitorHandle> {
        // guaranteed to be on main thread
        unsafe { monitor::uiscreens() }
    }

    pub fn primary_monitor(&self) -> Option<RootMonitorHandle> {
        // guaranteed to be on main thread
        let monitor = unsafe { monitor::main_uiscreen() };

        Some(RootMonitorHandle { inner: monitor })
    }

    pub fn raw_display_handle(&self) -> RawDisplayHandle {
        RawDisplayHandle::UiKit(UiKitDisplayHandle::empty())
    }
}

pub struct EventLoop<T: 'static> {
    window_target: RootEventLoopWindowTarget<T>,
}

#[derive(Default, Debug, Copy, Clone, PartialEq, Eq, Hash)]
pub(crate) struct PlatformSpecificEventLoopAttributes {}

impl<T: 'static> EventLoop<T> {
    pub(crate) fn new(_: &PlatformSpecificEventLoopAttributes) -> EventLoop<T> {
        static mut SINGLETON_INIT: bool = false;
        unsafe {
            assert_main_thread!("`EventLoop` can only be created on the main thread on iOS");
            assert!(
                !SINGLETON_INIT,
                "Only one `EventLoop` is supported on iOS. \
                 `EventLoopProxy` might be helpful"
            );
            SINGLETON_INIT = true;
            view::create_delegate_class();
        }

        let (sender_to_clone, receiver) = mpsc::channel();

        // this line sets up the main run loop before `UIApplicationMain`
        setup_control_flow_observers();

        EventLoop {
            window_target: RootEventLoopWindowTarget {
                p: EventLoopWindowTarget {
                    receiver,
                    sender_to_clone,
                },
                _marker: PhantomData,
            },
        }
    }

    pub fn run<F>(self, event_handler: F) -> !
    where
        F: 'static + FnMut(Event<'_, T>, &RootEventLoopWindowTarget<T>, &mut ControlFlow),
    {
        unsafe {
            let application: *mut Object = msg_send![class!(UIApplication), sharedApplication];
            assert_eq!(
                application,
                ptr::null_mut(),
                "\
                 `EventLoop` cannot be `run` after a call to `UIApplicationMain` on iOS\n\
                 Note: `EventLoop::run` calls `UIApplicationMain` on iOS"
            );
            app_state::will_launch(Box::new(EventLoopHandler {
                f: event_handler,
                event_loop: self.window_target,
            }));

            UIApplicationMain(
                0,
                ptr::null(),
                nil,
                NSStringRust::alloc(nil).init_str("AppDelegate"),
            );
            unreachable!()
        }
    }

    pub fn create_proxy(&self) -> EventLoopProxy<T> {
        EventLoopProxy::new(self.window_target.p.sender_to_clone.clone())
    }

    pub fn window_target(&self) -> &RootEventLoopWindowTarget<T> {
        &self.window_target
    }
}

// EventLoopExtIOS
impl<T: 'static> EventLoop<T> {
    pub fn idiom(&self) -> Idiom {
        // guaranteed to be on main thread
        unsafe { self::get_idiom() }
    }
}

pub struct EventLoopProxy<T> {
    sender: Sender<T>,
    source: CFRunLoopSourceRef,
}

unsafe impl<T: Send> Send for EventLoopProxy<T> {}

impl<T> Clone for EventLoopProxy<T> {
    fn clone(&self) -> EventLoopProxy<T> {
        EventLoopProxy::new(self.sender.clone())
    }
}

impl<T> Drop for EventLoopProxy<T> {
    fn drop(&mut self) {
        unsafe {
            CFRunLoopSourceInvalidate(self.source);
            CFRelease(self.source as _);
        }
    }
}

impl<T> EventLoopProxy<T> {
    fn new(sender: Sender<T>) -> EventLoopProxy<T> {
        unsafe {
            // just wake up the eventloop
            extern "C" fn event_loop_proxy_handler(_: *mut c_void) {}

            // adding a Source to the main CFRunLoop lets us wake it up and
            // process user events through the normal OS EventLoop mechanisms.
            let rl = CFRunLoopGetMain();
            // we want all the members of context to be zero/null, except one
            let mut context: CFRunLoopSourceContext = mem::zeroed();
            context.perform = Some(event_loop_proxy_handler);
            let source =
                CFRunLoopSourceCreate(ptr::null_mut(), CFIndex::max_value() - 1, &mut context);
            CFRunLoopAddSource(rl, source, kCFRunLoopCommonModes);
            CFRunLoopWakeUp(rl);

            EventLoopProxy { sender, source }
        }
    }

    pub fn send_event(&self, event: T) -> Result<(), EventLoopClosed<T>> {
        self.sender
            .send(event)
            .map_err(|::std::sync::mpsc::SendError(x)| EventLoopClosed(x))?;
        unsafe {
            // let the main thread know there's a new event
            CFRunLoopSourceSignal(self.source);
            let rl = CFRunLoopGetMain();
            CFRunLoopWakeUp(rl);
        }
        Ok(())
    }
}

fn setup_control_flow_observers() {
    unsafe {
        // begin is queued with the highest priority to ensure it is processed before other observers
        extern "C" fn control_flow_begin_handler(
            _: CFRunLoopObserverRef,
            activity: CFRunLoopActivity,
            _: *mut c_void,
        ) {
            unsafe {
                #[allow(non_upper_case_globals)]
                match activity {
                    kCFRunLoopAfterWaiting => app_state::handle_wakeup_transition(),
                    kCFRunLoopEntry => unimplemented!(), // not expected to ever happen
                    _ => unreachable!(),
                }
            }
        }

        // Core Animation registers its `CFRunLoopObserver` that performs drawing operations in
        // `CA::Transaction::ensure_implicit` with a priority of `0x1e8480`. We set the main_end
        // priority to be 0, in order to send MainEventsCleared before RedrawRequested. This value was
        // chosen conservatively to guard against apple using different priorities for their redraw
        // observers in different OS's or on different devices. If it so happens that it's too
        // conservative, the main symptom would be non-redraw events coming in after `MainEventsCleared`.
        //
        // The value of `0x1e8480` was determined by inspecting stack traces and the associated
        // registers for every `CFRunLoopAddObserver` call on an iPad Air 2 running iOS 11.4.
        //
        // Also tested to be `0x1e8480` on iPhone 8, iOS 13 beta 4.
        extern "C" fn control_flow_main_end_handler(
            _: CFRunLoopObserverRef,
            activity: CFRunLoopActivity,
            _: *mut c_void,
        ) {
            unsafe {
                #[allow(non_upper_case_globals)]
                match activity {
                    kCFRunLoopBeforeWaiting => app_state::handle_main_events_cleared(),
                    kCFRunLoopExit => unimplemented!(), // not expected to ever happen
                    _ => unreachable!(),
                }
            }
        }

        // end is queued with the lowest priority to ensure it is processed after other observers
        extern "C" fn control_flow_end_handler(
            _: CFRunLoopObserverRef,
            activity: CFRunLoopActivity,
            _: *mut c_void,
        ) {
            unsafe {
                #[allow(non_upper_case_globals)]
                match activity {
                    kCFRunLoopBeforeWaiting => app_state::handle_events_cleared(),
                    kCFRunLoopExit => unimplemented!(), // not expected to ever happen
                    _ => unreachable!(),
                }
            }
        }

        let main_loop = CFRunLoopGetMain();

        let begin_observer = CFRunLoopObserverCreate(
            ptr::null_mut(),
            kCFRunLoopEntry | kCFRunLoopAfterWaiting,
            1, // repeat = true
            CFIndex::min_value(),
            control_flow_begin_handler,
            ptr::null_mut(),
        );
        CFRunLoopAddObserver(main_loop, begin_observer, kCFRunLoopDefaultMode);

        let main_end_observer = CFRunLoopObserverCreate(
            ptr::null_mut(),
            kCFRunLoopExit | kCFRunLoopBeforeWaiting,
            1, // repeat = true
            0, // see comment on `control_flow_main_end_handler`
            control_flow_main_end_handler,
            ptr::null_mut(),
        );
        CFRunLoopAddObserver(main_loop, main_end_observer, kCFRunLoopDefaultMode);

        let end_observer = CFRunLoopObserverCreate(
            ptr::null_mut(),
            kCFRunLoopExit | kCFRunLoopBeforeWaiting,
            1, // repeat = true
            CFIndex::max_value(),
            control_flow_end_handler,
            ptr::null_mut(),
        );
        CFRunLoopAddObserver(main_loop, end_observer, kCFRunLoopDefaultMode);
    }
}

#[derive(Debug)]
pub enum Never {}

pub trait EventHandler: Debug {
    fn handle_nonuser_event(&mut self, event: Event<'_, Never>, control_flow: &mut ControlFlow);
    fn handle_user_events(&mut self, control_flow: &mut ControlFlow);
}

struct EventLoopHandler<F, T: 'static> {
    f: F,
    event_loop: RootEventLoopWindowTarget<T>,
}

impl<F, T: 'static> Debug for EventLoopHandler<F, T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("EventLoopHandler")
            .field("event_loop", &self.event_loop)
            .finish()
    }
}

impl<F, T> EventHandler for EventLoopHandler<F, T>
where
    F: 'static + FnMut(Event<'_, T>, &RootEventLoopWindowTarget<T>, &mut ControlFlow),
    T: 'static,
{
    fn handle_nonuser_event(&mut self, event: Event<'_, Never>, control_flow: &mut ControlFlow) {
        (self.f)(
            event.map_nonuser_event().unwrap(),
            &self.event_loop,
            control_flow,
        );
    }

    fn handle_user_events(&mut self, control_flow: &mut ControlFlow) {
        for event in self.event_loop.p.receiver.try_iter() {
            (self.f)(Event::UserEvent(event), &self.event_loop, control_flow);
        }
    }
}

// must be called on main thread
pub unsafe fn get_idiom() -> Idiom {
    let device: id = msg_send![class!(UIDevice), currentDevice];
    let raw_idiom: UIUserInterfaceIdiom = msg_send![device, userInterfaceIdiom];
    raw_idiom.into()
}
