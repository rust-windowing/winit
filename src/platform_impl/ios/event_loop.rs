use std::{
    collections::VecDeque,
    ffi::c_void,
    marker::PhantomData,
    ptr,
    sync::mpsc::{self, Receiver, Sender},
    task::{RawWaker, RawWakerVTable, Waker},
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
    platform_impl::platform::app_state::{EventLoopHandler, HandlePendingUserEvents},
};

use super::{app_state, monitor, view, MonitorHandle};
use super::{
    app_state::AppState,
    uikit::{UIApplication, UIApplicationMain, UIDevice, UIScreen},
};

#[derive(Debug)]
pub struct EventLoopWindowTarget {
    pub(super) mtm: MainThreadMarker,
}

impl EventLoopWindowTarget {
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
        log::warn!("`ControlFlow::Exit` ignored on iOS");
    }

    pub(crate) fn exiting(&self) -> bool {
        false
    }

    pub(crate) fn owned_display_handle(&self) -> OwnedDisplayHandle {
        OwnedDisplayHandle
    }
}

#[derive(Clone)]
pub(crate) struct OwnedDisplayHandle;

impl OwnedDisplayHandle {
    #[cfg(feature = "rwh_05")]
    #[inline]
    pub fn raw_display_handle_rwh_05(&self) -> rwh_05::RawDisplayHandle {
        rwh_05::UiKitDisplayHandle::empty().into()
    }

    #[cfg(feature = "rwh_06")]
    #[inline]
    pub fn raw_display_handle_rwh_06(
        &self,
    ) -> Result<rwh_06::RawDisplayHandle, rwh_06::HandleError> {
        Ok(rwh_06::UiKitDisplayHandle::new().into())
    }
}

#[allow(deprecated)]
fn map_user_event<T: 'static>(
    mut handler: impl FnMut(Event<T>, &RootEventLoopWindowTarget),
    receiver: mpsc::Receiver<T>,
) -> impl FnMut(Event<HandlePendingUserEvents>, &RootEventLoopWindowTarget) {
    move |event, window_target| match event.map_nonuser_event() {
        Ok(event) => (handler)(event, window_target),
        Err(_) => {
            for event in receiver.try_iter() {
                (handler)(Event::UserEvent(event), window_target);
            }
        }
    }
}

pub struct EventLoop<T: 'static> {
    mtm: MainThreadMarker,
    sender: Sender<T>,
    receiver: Receiver<T>,
    window_target: RootEventLoopWindowTarget,
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
                p: EventLoopWindowTarget { mtm },
                _marker: PhantomData,
            },
        })
    }

    pub fn run<F>(self, handler: F) -> !
    where
        F: FnMut(Event<T>, &RootEventLoopWindowTarget),
    {
        let application = UIApplication::shared(self.mtm);
        assert!(
            application.is_none(),
            "\
                `EventLoop` cannot be `run` after a call to `UIApplicationMain` on iOS\n\
                 Note: `EventLoop::run` calls `UIApplicationMain` on iOS",
        );

        let handler = map_user_event(handler, self.receiver);

        let handler = unsafe {
            std::mem::transmute::<
                Box<dyn FnMut(Event<HandlePendingUserEvents>, &RootEventLoopWindowTarget)>,
                Box<dyn FnMut(Event<HandlePendingUserEvents>, &RootEventLoopWindowTarget)>,
            >(Box::new(handler))
        };

        let handler = EventLoopHandler {
            handler,
            event_loop: self.window_target,
        };

        app_state::will_launch(self.mtm, handler);

        // Ensure application delegate is initialized
        view::WinitApplicationDelegate::class();

        unsafe {
            UIApplicationMain(
                0,
                ptr::null(),
                None,
                Some(&NSString::from_str("WinitApplicationDelegate")),
            )
        };
        unreachable!()
    }

    pub fn create_proxy(&self) -> EventLoopProxy<T> {
        EventLoopProxy {
            sender: self.sender.clone(),
            waker: waker(),
        }
    }

    pub fn window_target(&self) -> &RootEventLoopWindowTarget {
        &self.window_target
    }
}

// EventLoopExtIOS
impl<T: 'static> EventLoop<T> {
    pub fn idiom(&self) -> Idiom {
        UIDevice::current(self.mtm).userInterfaceIdiom().into()
    }
}

pub fn waker() -> Waker {
    fn new_raw_waker() -> RawWaker {
        // just wake up the eventloop
        extern "C" fn event_loop_proxy_handler(_: *const c_void) {}

        // adding a Source to the main CFRunLoop lets us wake it up and
        // process user events through the normal OS EventLoop mechanisms.
        let rl = unsafe { CFRunLoopGetMain() };
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
        let source = unsafe {
            CFRunLoopSourceCreate(ptr::null_mut(), CFIndex::max_value() - 1, &mut context)
        };
        unsafe { CFRunLoopAddSource(rl, source, kCFRunLoopCommonModes) };
        unsafe { CFRunLoopWakeUp(rl) };
        RawWaker::new(
            source as *const (),
            &RawWakerVTable::new(clone_waker, wake, wake_by_ref, drop_waker),
        )
    }

    unsafe fn clone_waker(waker: *const ()) -> RawWaker {
        let _source = waker as CFRunLoopSourceRef;
        new_raw_waker()
    }

    unsafe fn wake(waker: *const ()) {
        unsafe { wake_by_ref(waker) };
        unsafe { drop_waker(waker) };
    }

    unsafe fn wake_by_ref(waker: *const ()) {
        let source = waker as CFRunLoopSourceRef;
        unsafe {
            // let the main thread know there's a new event
            CFRunLoopSourceSignal(source);
            let rl = CFRunLoopGetMain();
            CFRunLoopWakeUp(rl);
        }
    }

    unsafe fn drop_waker(waker: *const ()) {
        let source = waker as CFRunLoopSourceRef;
        unsafe { CFRunLoopSourceInvalidate(source) };
        unsafe { CFRelease(source as _) };
    }

    unsafe { Waker::from_raw(new_raw_waker()) }
}

pub struct EventLoopProxy<T> {
    sender: Sender<T>,
    waker: Waker,
}

impl<T> Clone for EventLoopProxy<T> {
    fn clone(&self) -> Self {
        Self {
            sender: self.sender.clone(),
            waker: self.waker.clone(),
        }
    }
}

impl<T> EventLoopProxy<T> {
    pub fn send_event(&self, event: T) -> Result<(), EventLoopClosed<T>> {
        self.sender
            .send(event)
            .map_err(|mpsc::SendError(x)| EventLoopClosed(x))?;
        self.waker.wake_by_ref();
        Ok(())
    }

    pub fn waker(self) -> Waker {
        self.waker
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
