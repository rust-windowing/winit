use std::collections::VecDeque;
use std::ffi::{c_char, c_int, c_void};
use std::marker::PhantomData;
use std::ptr::{self, NonNull};
use std::sync::atomic::{AtomicBool, Ordering as AtomicOrdering};
use std::sync::Arc;

use core_foundation::base::{CFIndex, CFRelease};
use core_foundation::runloop::{
    kCFRunLoopAfterWaiting, kCFRunLoopBeforeWaiting, kCFRunLoopCommonModes, kCFRunLoopDefaultMode,
    kCFRunLoopExit, CFRunLoopActivity, CFRunLoopAddObserver, CFRunLoopAddSource, CFRunLoopGetMain,
    CFRunLoopObserverCreate, CFRunLoopObserverRef, CFRunLoopSourceContext, CFRunLoopSourceCreate,
    CFRunLoopSourceInvalidate, CFRunLoopSourceRef, CFRunLoopSourceSignal, CFRunLoopWakeUp,
};
use objc2::rc::Retained;
use objc2::{msg_send_id, ClassType};
use objc2_foundation::{MainThreadMarker, NSString};
use objc2_ui_kit::{UIApplication, UIApplicationMain, UIDevice, UIScreen, UIUserInterfaceIdiom};

use super::app_state::EventLoopHandler;
use crate::application::ApplicationHandler;
use crate::error::EventLoopError;
use crate::event::Event;
use crate::event_loop::{ActiveEventLoop as RootActiveEventLoop, ControlFlow, DeviceEvents};
use crate::platform::ios::Idiom;
use crate::window::{CustomCursor, CustomCursorSource};

use super::app_delegate::AppDelegate;
use super::app_state::AppState;
use super::{app_state, monitor, MonitorHandle};

#[derive(Debug)]
pub struct ActiveEventLoop {
    pub(super) mtm: MainThreadMarker,
}

impl ActiveEventLoop {
    pub fn create_custom_cursor(&self, source: CustomCursorSource) -> CustomCursor {
        let _ = source.inner;
        CustomCursor { inner: super::PlatformCustomCursor }
    }

    pub fn available_monitors(&self) -> VecDeque<MonitorHandle> {
        monitor::uiscreens(self.mtm)
    }

    pub fn primary_monitor(&self) -> Option<MonitorHandle> {
        #[allow(deprecated)]
        Some(MonitorHandle::new(UIScreen::mainScreen(self.mtm)))
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
        Ok(rwh_06::RawDisplayHandle::UiKit(rwh_06::UiKitDisplayHandle::new()))
    }

    pub(crate) fn set_control_flow(&self, control_flow: ControlFlow) {
        AppState::get_mut(self.mtm).set_control_flow(control_flow)
    }

    pub(crate) fn control_flow(&self) -> ControlFlow {
        AppState::get_mut(self.mtm).control_flow()
    }

    pub(crate) fn exit(&self) {
        // https://developer.apple.com/library/archive/qa/qa1561/_index.html
        // it is not possible to quit an iOS app gracefully and programmatically
        tracing::warn!("`ControlFlow::Exit` ignored on iOS");
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

fn map_user_event<A: ApplicationHandler>(
    app: &mut A,
    proxy_wake_up: Arc<AtomicBool>,
) -> impl FnMut(Event, &RootActiveEventLoop) + '_ {
    move |event, window_target| match event {
        Event::NewEvents(cause) => app.new_events(window_target, cause),
        Event::WindowEvent { window_id, event } => {
            app.window_event(window_target, window_id, event)
        },
        Event::DeviceEvent { device_id, event } => {
            app.device_event(window_target, device_id, event)
        },
        Event::UserWakeUp => {
            if proxy_wake_up.swap(false, AtomicOrdering::Relaxed) {
                app.proxy_wake_up(window_target);
            }
        },
        Event::Suspended => app.suspended(window_target),
        Event::Resumed => app.resumed(window_target),
        Event::AboutToWait => app.about_to_wait(window_target),
        Event::LoopExiting => app.exiting(window_target),
        Event::MemoryWarning => app.memory_warning(window_target),
    }
}

pub struct EventLoop {
    mtm: MainThreadMarker,
    proxy_wake_up: Arc<AtomicBool>,
    window_target: RootActiveEventLoop,
}

#[derive(Default, Debug, Copy, Clone, PartialEq, Eq, Hash)]
pub(crate) struct PlatformSpecificEventLoopAttributes {}

impl EventLoop {
    pub(crate) fn new(
        _: &PlatformSpecificEventLoopAttributes,
    ) -> Result<EventLoop, EventLoopError> {
        let mtm = MainThreadMarker::new()
            .expect("On iOS, `EventLoop` must be created on the main thread");

        static mut SINGLETON_INIT: bool = false;
        unsafe {
            assert!(
                !SINGLETON_INIT,
                "Only one `EventLoop` is supported on iOS. `EventLoopProxy` might be helpful"
            );
            SINGLETON_INIT = true;
        }

        // this line sets up the main run loop before `UIApplicationMain`
        setup_control_flow_observers();

        let proxy_wake_up = Arc::new(AtomicBool::new(false));

        Ok(EventLoop {
            mtm,
            proxy_wake_up,
            window_target: RootActiveEventLoop { p: ActiveEventLoop { mtm }, _marker: PhantomData },
        })
    }

    pub fn run_app<A: ApplicationHandler>(self, app: &mut A) -> ! {
        let application: Option<Retained<UIApplication>> =
            unsafe { msg_send_id![UIApplication::class(), sharedApplication] };
        assert!(
            application.is_none(),
            "\
                `EventLoop` cannot be `run` after a call to `UIApplicationMain` on iOS\nNote: \
             `EventLoop::run_app` calls `UIApplicationMain` on iOS",
        );

        let handler = map_user_event(app, self.proxy_wake_up.clone());

        let handler = unsafe {
            std::mem::transmute::<
                Box<dyn FnMut(Event, &RootActiveEventLoop)>,
                Box<dyn FnMut(Event, &RootActiveEventLoop)>,
            >(Box::new(handler))
        };

        let handler = EventLoopHandler { handler, event_loop: self.window_target };

        app_state::will_launch(self.mtm, handler);

        // Ensure application delegate is initialized
        let _ = AppDelegate::class();

        extern "C" {
            // These functions are in crt_externs.h.
            fn _NSGetArgc() -> *mut c_int;
            fn _NSGetArgv() -> *mut *mut *mut c_char;
        }

        unsafe {
            UIApplicationMain(
                *_NSGetArgc(),
                NonNull::new(*_NSGetArgv()).unwrap(),
                None,
                Some(&NSString::from_str(AppDelegate::NAME)),
            )
        };
        unreachable!()
    }

    pub fn create_proxy(&self) -> EventLoopProxy {
        EventLoopProxy::new(self.proxy_wake_up.clone())
    }

    pub fn window_target(&self) -> &RootActiveEventLoop {
        &self.window_target
    }
}

// EventLoopExtIOS
impl EventLoop {
    pub fn idiom(&self) -> Idiom {
        match UIDevice::currentDevice(self.mtm).userInterfaceIdiom() {
            UIUserInterfaceIdiom::Unspecified => Idiom::Unspecified,
            UIUserInterfaceIdiom::Phone => Idiom::Phone,
            UIUserInterfaceIdiom::Pad => Idiom::Pad,
            UIUserInterfaceIdiom::TV => Idiom::TV,
            UIUserInterfaceIdiom::CarPlay => Idiom::CarPlay,
            _ => Idiom::Unspecified,
        }
    }
}

pub struct EventLoopProxy {
    proxy_wake_up: Arc<AtomicBool>,
    source: CFRunLoopSourceRef,
}

unsafe impl Send for EventLoopProxy {}
unsafe impl Sync for EventLoopProxy {}

impl Clone for EventLoopProxy {
    fn clone(&self) -> EventLoopProxy {
        EventLoopProxy::new(self.proxy_wake_up.clone())
    }
}

impl Drop for EventLoopProxy {
    fn drop(&mut self) {
        unsafe {
            CFRunLoopSourceInvalidate(self.source);
            CFRelease(self.source as _);
        }
    }
}

impl EventLoopProxy {
    fn new(proxy_wake_up: Arc<AtomicBool>) -> EventLoopProxy {
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
            let source = CFRunLoopSourceCreate(ptr::null_mut(), CFIndex::MAX - 1, &mut context);
            CFRunLoopAddSource(rl, source, kCFRunLoopCommonModes);
            CFRunLoopWakeUp(rl);

            EventLoopProxy { proxy_wake_up, source }
        }
    }

    pub fn wake_up(&self) {
        self.proxy_wake_up.store(true, AtomicOrdering::Relaxed);
        unsafe {
            // let the main thread know there's a new event
            CFRunLoopSourceSignal(self.source);
            let rl = CFRunLoopGetMain();
            CFRunLoopWakeUp(rl);
        }
    }
}

fn setup_control_flow_observers() {
    unsafe {
        // begin is queued with the highest priority to ensure it is processed before other
        // observers
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
                kCFRunLoopExit => {}, // may happen when running on macOS
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
                kCFRunLoopExit => {}, // may happen when running on macOS
                _ => unreachable!(),
            }
        }

        let main_loop = CFRunLoopGetMain();

        let begin_observer = CFRunLoopObserverCreate(
            ptr::null_mut(),
            kCFRunLoopAfterWaiting,
            1, // repeat = true
            CFIndex::MIN,
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
            CFIndex::MAX,
            control_flow_end_handler,
            ptr::null_mut(),
        );
        CFRunLoopAddObserver(main_loop, end_observer, kCFRunLoopDefaultMode);
    }
}
