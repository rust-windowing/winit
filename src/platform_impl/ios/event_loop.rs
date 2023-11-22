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
use icrate::Foundation::{MainThreadMarker, NSString};
use objc2::ClassType;

use crate::{
    error::EventLoopError,
    event::Event,
    event_loop::{
        ControlFlow, DeviceEvents, EventLoopClosed,
        EventLoopWindowTarget as RootEventLoopWindowTarget,
    },
    platform::ios::Idiom,
};

use super::{app_state, monitor, view, MonitorHandle};
use super::{
    app_state::AppState,
    uikit::{UIApplication, UIApplicationMain, UIDevice, UIScreen},
};

#[derive(Debug)]
pub struct EventLoopWindowTarget<T: 'static> {
    pub(super) mtm: MainThreadMarker,
    p: PhantomData<T>,
}

impl<T: 'static> EventLoopWindowTarget<T> {
    pub fn available_monitors(&self) -> VecDeque<MonitorHandle> {
        monitor::uiscreens(self.mtm)
    }

    pub fn primary_monitor(&self) -> Option<MonitorHandle> {
        Some(MonitorHandle::new(UIScreen::main(self.mtm)))
    }

    #[inline]
    pub fn listen_device_events(&self, _allowed: DeviceEvents) {}

    #[cfg(feature = "rwh_05")]
    #[inline]
    pub fn raw_display_handle_rwh_05(&self) -> rwh_05::RawDisplayHandle {
        rwh_05::RawDisplayHandle::UiKit(rwh_05::UiKitDisplayHandle::empty())
    }

    #[cfg(feature = "rwh_06")]
    #[inline]
    pub fn raw_display_handle_rwh_06(
        &self,
    ) -> Result<rwh_06::RawDisplayHandle, rwh_06::HandleError> {
        Ok(rwh_06::RawDisplayHandle::UiKit(
            rwh_06::UiKitDisplayHandle::new(),
        ))
    }

    pub(crate) fn set_control_flow(&self, control_flow: ControlFlow) {
        AppState::get_mut(self.mtm).set_control_flow(control_flow)
    }

    pub(crate) fn control_flow(&self) -> ControlFlow {
        AppState::get_mut(self.mtm).control_flow()
    }

    pub(crate) fn exit(&self) {
        // https://developer.apple.com/library/archive/qa/qa1561/_index.html
        // it is not possible to quit an iOS app gracefully and programatically
        warn!("`ControlFlow::Exit` ignored on iOS");
    }

    pub(crate) fn exiting(&self) -> bool {
        false
    }
}

pub struct EventLoop<T: 'static> {
    mtm: MainThreadMarker,
    sender: Sender<T>,
    receiver: Receiver<T>,
    window_target: RootEventLoopWindowTarget<T>,
}

#[derive(Default, Debug, Copy, Clone, PartialEq, Eq, Hash)]
pub(crate) struct PlatformSpecificEventLoopAttributes {}

impl<T: 'static> EventLoop<T> {
    pub(crate) fn new(
        _: &PlatformSpecificEventLoopAttributes,
    ) -> Result<EventLoop<T>, EventLoopError> {
        let mtm = MainThreadMarker::new()
            .expect("On iOS, `EventLoop` must be created on the main thread");

        static mut SINGLETON_INIT: bool = false;
        unsafe {
            assert!(
                !SINGLETON_INIT,
                "Only one `EventLoop` is supported on iOS. \
                 `EventLoopProxy` might be helpful"
            );
            SINGLETON_INIT = true;
        }

        let (sender, receiver) = mpsc::channel();

        // this line sets up the main run loop before `UIApplicationMain`
        setup_control_flow_observers();

        Ok(EventLoop {
            mtm,
            sender,
            receiver,
            window_target: RootEventLoopWindowTarget {
                p: EventLoopWindowTarget {
                    mtm,
                    p: PhantomData,
                },
                _marker: PhantomData,
            },
        })
    }

    pub fn run<F>(self, event_handler: F) -> !
    where
        F: FnMut(Event<T>, &RootEventLoopWindowTarget<T>),
    {
        unsafe {
            let application = UIApplication::shared(self.mtm);
            assert!(
                application.is_none(),
                "\
                `EventLoop` cannot be `run` after a call to `UIApplicationMain` on iOS\n\
                 Note: `EventLoop::run` calls `UIApplicationMain` on iOS",
            );

            let event_handler = std::mem::transmute::<
                Box<dyn FnMut(Event<T>, &RootEventLoopWindowTarget<T>)>,
                Box<EventHandlerCallback<T>>,
            >(Box::new(event_handler));

            let handler = EventLoopHandler {
                f: event_handler,
                receiver: self.receiver,
                event_loop: self.window_target,
            };

            app_state::will_launch(self.mtm, Box::new(handler));

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
        EventLoopProxy::new(self.sender.clone())
    }

    pub fn window_target(&self) -> &RootEventLoopWindowTarget<T> {
        &self.window_target
    }
}

// EventLoopExtIOS
impl<T: 'static> EventLoop<T> {
    pub fn idiom(&self) -> Idiom {
        UIDevice::current(self.mtm).userInterfaceIdiom().into()
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
            let mtm = MainThreadMarker::new().unwrap();
            #[allow(non_upper_case_globals)]
            match activity {
                kCFRunLoopAfterWaiting => app_state::handle_wakeup_transition(mtm),
                _ => unreachable!(),
            }
        }

        // Core Animation registers its `CFRunLoopObserver` that performs drawing operations in
        // `CA::Transaction::ensure_implicit` with a priority of `0x1e8480`. We set the main_end
        // priority to be 0, in order to send AboutToWait before RedrawRequested. This value was
        // chosen conservatively to guard against apple using different priorities for their redraw
        // observers in different OS's or on different devices. If it so happens that it's too
        // conservative, the main symptom would be non-redraw events coming in after `AboutToWait`.
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
            let mtm = MainThreadMarker::new().unwrap();
            #[allow(non_upper_case_globals)]
            match activity {
                kCFRunLoopBeforeWaiting => app_state::handle_main_events_cleared(mtm),
                kCFRunLoopExit => {} // may happen when running on macOS
                _ => unreachable!(),
            }
        }

        // end is queued with the lowest priority to ensure it is processed after other observers
        extern "C" fn control_flow_end_handler(
            _: CFRunLoopObserverRef,
            activity: CFRunLoopActivity,
            _: *mut c_void,
        ) {
            let mtm = MainThreadMarker::new().unwrap();
            #[allow(non_upper_case_globals)]
            match activity {
                kCFRunLoopBeforeWaiting => app_state::handle_events_cleared(mtm),
                kCFRunLoopExit => {} // may happen when running on macOS
                _ => unreachable!(),
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

type EventHandlerCallback<T> = dyn FnMut(Event<T>, &RootEventLoopWindowTarget<T>) + 'static;

pub trait EventHandler: Debug {
    fn handle_nonuser_event(&mut self, event: Event<Never>);
    fn handle_user_events(&mut self);
}

struct EventLoopHandler<T: 'static> {
    f: Box<EventHandlerCallback<T>>,
    receiver: Receiver<T>,
    event_loop: RootEventLoopWindowTarget<T>,
}

impl<T: 'static> Debug for EventLoopHandler<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("EventLoopHandler")
            .field("event_loop", &self.event_loop)
            .finish()
    }
}

impl<T: 'static> EventHandler for EventLoopHandler<T> {
    fn handle_nonuser_event(&mut self, event: Event<Never>) {
        (self.f)(event.map_nonuser_event().unwrap(), &self.event_loop);
    }

    fn handle_user_events(&mut self) {
        for event in self.receiver.try_iter() {
            (self.f)(Event::UserEvent(event), &self.event_loop);
        }
    }
}
