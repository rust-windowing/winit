use std::{mem, ptr};
use std::collections::VecDeque;
use std::ffi::c_void;
use std::fmt::{self, Debug, Formatter};
use std::marker::PhantomData;
use std::sync::mpsc::{self, Sender, Receiver};

use crate::event::Event;
use crate::event_loop::{
    ControlFlow,
    EventLoopWindowTarget as RootEventLoopWindowTarget,
    EventLoopClosed,
};
use crate::platform::ios::Idiom;

use crate::platform_impl::platform::app_state::AppState;
use crate::platform_impl::platform::ffi::{
    id,
    nil,
    CFIndex,
    CFRelease,
    CFRunLoopActivity,
    CFRunLoopAddObserver,
    CFRunLoopAddSource,
    CFRunLoopGetMain,
    CFRunLoopObserverCreate,
    CFRunLoopObserverRef,
    CFRunLoopSourceContext,
    CFRunLoopSourceCreate,
    CFRunLoopSourceInvalidate,
    CFRunLoopSourceRef,
    CFRunLoopSourceSignal,
    CFRunLoopWakeUp,
    kCFRunLoopCommonModes,
    kCFRunLoopDefaultMode,
    kCFRunLoopEntry,
    kCFRunLoopBeforeWaiting,
    kCFRunLoopAfterWaiting,
    kCFRunLoopExit,
    NSOperatingSystemVersion,
    NSString,
    UIApplicationMain,
    UIUserInterfaceIdiom,
};
use crate::platform_impl::platform::monitor;
use crate::platform_impl::platform::MonitorHandle;
use crate::platform_impl::platform::view;

pub struct EventLoopWindowTarget<T: 'static> {
    receiver: Receiver<T>,
    sender_to_clone: Sender<T>,
    capabilities: Capabilities,
}

impl<T: 'static> EventLoopWindowTarget<T> {
    pub fn capabilities(&self) -> &Capabilities {
        &self.capabilities
    }
}

pub struct EventLoop<T: 'static> {
    window_target: RootEventLoopWindowTarget<T>,
}

impl<T: 'static> EventLoop<T> {
    pub fn new() -> EventLoop<T> {
        static mut SINGLETON_INIT: bool = false;
        unsafe {
            assert_main_thread!("`EventLoop` can only be created on the main thread on iOS");
            assert!(!SINGLETON_INIT, "Only one `EventLoop` is supported on iOS. \
                `EventLoopProxy` might be helpful");
            SINGLETON_INIT = true;
            view::create_delegate_class();
        }

        let (sender_to_clone, receiver) = mpsc::channel();

        // this line sets up the main run loop before `UIApplicationMain`
        setup_control_flow_observers();

        let version: NSOperatingSystemVersion = unsafe {
            let process_info: id = msg_send![class!(NSProcessInfo), processInfo];
            msg_send![process_info, operatingSystemVersion]
        };
        let capabilities = version.into();

        EventLoop {
            window_target: RootEventLoopWindowTarget {
                p: EventLoopWindowTarget {
                    receiver,
                    sender_to_clone,
                    capabilities,
                },
                _marker: PhantomData,
            }
        }
    }

    pub fn run<F>(self, event_handler: F) -> !
    where
        F: 'static + FnMut(Event<T>, &RootEventLoopWindowTarget<T>, &mut ControlFlow)
    {
        unsafe {
            let application: *mut c_void = msg_send![class!(UIApplication), sharedApplication];
            assert_eq!(application, ptr::null_mut(), "\
                `EventLoop` cannot be `run` after a call to `UIApplicationMain` on iOS\n\
                Note: `EventLoop::run` calls `UIApplicationMain` on iOS");
            AppState::will_launch(Box::new(EventLoopHandler {
                f: event_handler,
                event_loop: self.window_target,
            }));

            UIApplicationMain(0, ptr::null(), nil, NSString::alloc(nil).init_str("AppDelegate"));
            unreachable!()
        }
    }

    pub fn create_proxy(&self) -> EventLoopProxy<T> {
        EventLoopProxy::new(self.window_target.p.sender_to_clone.clone())
    }

    pub fn available_monitors(&self) -> VecDeque<MonitorHandle> {
        // guaranteed to be on main thread
        unsafe {
            monitor::uiscreens()
        }
    }

    pub fn primary_monitor(&self) -> MonitorHandle {
        // guaranteed to be on main thread
        unsafe {
            monitor::main_uiscreen()
        }
    }

    pub fn window_target(&self) -> &RootEventLoopWindowTarget<T> {
        &self.window_target
    }
}

// EventLoopExtIOS
impl<T: 'static> EventLoop<T> {
    pub fn idiom(&self) -> Idiom {
        // guaranteed to be on main thread
        unsafe {
            self::get_idiom()
        }
    }
}

pub struct EventLoopProxy<T> {
    sender: Sender<T>,
    source: CFRunLoopSourceRef,
}

unsafe impl<T> Send for EventLoopProxy<T> {}
unsafe impl<T> Sync for EventLoopProxy<T> {}

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
            // just wakeup the eventloop
            extern "C" fn event_loop_proxy_handler(_: *mut c_void) {}

            // adding a Source to the main CFRunLoop lets us wake it up and
            // process user events through the normal OS EventLoop mechanisms.
            let rl = CFRunLoopGetMain();
            // we want all the members of context to be zero/null, except one
            let mut context: CFRunLoopSourceContext = mem::zeroed();
            context.perform = event_loop_proxy_handler;
            let source = CFRunLoopSourceCreate(
                ptr::null_mut(),
                CFIndex::max_value() - 1,
                &mut context,
            );
            CFRunLoopAddSource(rl, source, kCFRunLoopCommonModes);
            CFRunLoopWakeUp(rl);

            EventLoopProxy {
                sender,
                source,
            }
        }
    }

    pub fn send_event(&self, event: T) -> Result<(), EventLoopClosed> {
        self.sender.send(event).map_err(|_| EventLoopClosed)?;
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
        extern fn control_flow_begin_handler(
            _: CFRunLoopObserverRef,
            activity: CFRunLoopActivity,
            _: *mut c_void,
        ) {
            unsafe {
                #[allow(non_upper_case_globals)]
                match activity {
                    kCFRunLoopAfterWaiting => AppState::handle_wakeup_transition(),
                    kCFRunLoopEntry => unimplemented!(), // not expected to ever happen
                    _ => unreachable!(),
                }
            }
        }

        // end is queued with the lowest priority to ensure it is processed after other observers
        // without that, LoopDestroyed will get sent after EventsCleared
        extern fn control_flow_end_handler(
            _: CFRunLoopObserverRef,
            activity: CFRunLoopActivity,
            _: *mut c_void,
        ) {
            unsafe {
                #[allow(non_upper_case_globals)]
                match activity {
                    kCFRunLoopBeforeWaiting => AppState::handle_events_cleared(),
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
    fn handle_nonuser_event(&mut self, event: Event<Never>, control_flow: &mut ControlFlow);
    fn handle_user_events(&mut self, control_flow: &mut ControlFlow);
}

struct EventLoopHandler<F, T: 'static> {
    f: F,
    event_loop: RootEventLoopWindowTarget<T>,
}

impl<F, T: 'static> Debug for EventLoopHandler<F, T> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.debug_struct("EventLoopHandler")
            .field("event_loop", &self.event_loop)
            .finish()
    }
}

impl<F, T> EventHandler for EventLoopHandler<F, T>
where
    F: 'static + FnMut(Event<T>, &RootEventLoopWindowTarget<T>, &mut ControlFlow),
    T: 'static,
{
    fn handle_nonuser_event(&mut self, event: Event<Never>, control_flow: &mut ControlFlow) {
        (self.f)(
            event.map_nonuser_event().unwrap(),
            &self.event_loop,
            control_flow,
        );
    }

    fn handle_user_events(&mut self, control_flow: &mut ControlFlow) {
        for event in self.event_loop.p.receiver.try_iter() {
            (self.f)(
                Event::UserEvent(event),
                &self.event_loop,
                control_flow,
            );
        }
    }
}

// must be called on main thread
pub unsafe fn get_idiom() -> Idiom {
    let device: id = msg_send![class!(UIDevice), currentDevice];
    let raw_idiom: UIUserInterfaceIdiom = msg_send![device, userInterfaceIdiom];
    raw_idiom.into()
}

pub struct Capabilities {
    pub supports_safe_area: bool,
}

impl From<NSOperatingSystemVersion> for Capabilities {
    fn from(os_version: NSOperatingSystemVersion) -> Capabilities {
        assert!(os_version.major >= 8, "`winit` current requires iOS version 8 or greater");

        let supports_safe_area = os_version.major >= 11;

        Capabilities { supports_safe_area }
    }
}
