use std::{
    collections::VecDeque,
    ffi::c_void,
    fmt::{self, Debug},
    marker::PhantomData,
    ptr,
    sync::mpsc::{self, Receiver, Sender},
};

use core_foundation::base::{CFIndex, CFRelease};
use core_foundation::runloop::{
    kCFRunLoopAfterWaiting, kCFRunLoopBeforeWaiting, kCFRunLoopCommonModes, kCFRunLoopDefaultMode,
    kCFRunLoopExit, CFRunLoopActivity, CFRunLoopAddObserver, CFRunLoopAddSource, CFRunLoopGetMain,
    CFRunLoopObserverCreate, CFRunLoopObserverRef, CFRunLoopSourceContext, CFRunLoopSourceCreate,
    CFRunLoopSourceInvalidate, CFRunLoopSourceRef, CFRunLoopSourceSignal, CFRunLoopWakeUp,
};
use objc2::foundation::{MainThreadMarker, NSString};
use objc2::rc::{Id, Shared};
use objc2::ClassType;
use raw_window_handle::{RawDisplayHandle, UiKitDisplayHandle};

use super::uikit::{UIApplication, UIApplicationMain, UIDevice, UIScreen};
use super::view::WinitUIWindow;
use super::{app_state, monitor, view, MonitorHandle};
use crate::{
    dpi::LogicalSize,
    event::Event,
    event_loop::{
        ControlFlow, EventLoopClosed, EventLoopWindowTarget as RootEventLoopWindowTarget,
    },
    platform::ios::Idiom,
};

#[derive(Debug)]
pub(crate) enum EventWrapper {
    StaticEvent(Event<'static, Never>),
    EventProxy(EventProxy),
}

#[derive(Debug, PartialEq)]
pub(crate) enum EventProxy {
    DpiChangedProxy {
        window: Id<WinitUIWindow, Shared>,
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
        monitor::uiscreens(MainThreadMarker::new().unwrap())
    }

    pub fn primary_monitor(&self) -> Option<MonitorHandle> {
        Some(MonitorHandle::new(UIScreen::main(
            MainThreadMarker::new().unwrap(),
        )))
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
        assert_main_thread!("`EventLoop` can only be created on the main thread on iOS");

        static mut SINGLETON_INIT: bool = false;
        unsafe {
            assert!(
                !SINGLETON_INIT,
                "Only one `EventLoop` is supported on iOS. \
                 `EventLoopProxy` might be helpful"
            );
            SINGLETON_INIT = true;
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
            let application = UIApplication::shared(MainThreadMarker::new().unwrap());
            assert!(
                application.is_none(),
                "\
                `EventLoop` cannot be `run` after a call to `UIApplicationMain` on iOS\n\
                 Note: `EventLoop::run` calls `UIApplicationMain` on iOS",
            );
            app_state::will_launch(Box::new(EventLoopHandler {
                f: event_handler,
                event_loop: self.window_target,
            }));

            // Ensure application delegate is initialized
            view::WinitApplicationDelegate::class();

            UIApplicationMain(
                0,
                ptr::null(),
                None,
                Some(&NSString::from_str("WinitApplicationDelegate")),
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
        UIDevice::current(MainThreadMarker::new().unwrap())
            .userInterfaceIdiom()
            .into()
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
            extern "C" fn event_loop_proxy_handler(_: *const c_void) {}

            // adding a Source to the main CFRunLoop lets us wake it up and
            // process user events through the normal OS EventLoop mechanisms.
            let rl = CFRunLoopGetMain();
            let mut context = CFRunLoopSourceContext {
                version: 0,
                info: ptr::null_mut(),
                retain: None,
                release: None,
                copyDescription: None,
                equal: None,
                hash: None,
                schedule: None,
                cancel: None,
                perform: event_loop_proxy_handler,
            };
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
            kCFRunLoopAfterWaiting,
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
