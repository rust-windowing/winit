use std::{
    any::Any,
    cell::{Cell, RefCell},
    collections::VecDeque,
    marker::PhantomData,
    mem,
    os::raw::c_void,
    panic::{catch_unwind, resume_unwind, RefUnwindSafe, UnwindSafe},
    process, ptr,
    rc::{Rc, Weak},
    sync::mpsc,
};

use cocoa::{
    appkit::{NSApp, NSEventType::NSApplicationDefined},
    base::{id, nil, YES},
    foundation::NSPoint,
};
use objc::rc::autoreleasepool;

use crate::{
    event::Event,
    event_loop::{ControlFlow, EventLoopClosed, EventLoopWindowTarget as RootWindowTarget},
    monitor::MonitorHandle as RootMonitorHandle,
    platform_impl::platform::{
        app::APP_CLASS,
        app_delegate::APP_DELEGATE_CLASS,
        app_state::AppState,
        monitor::{self, MonitorHandle},
        observer::*,
        util::IdRef,
    },
};

#[derive(Default)]
pub struct PanicInfo {
    inner: Cell<Option<Box<dyn Any + Send + 'static>>>,
}

// WARNING:
// As long as this struct is used through its `impl`, it is UnwindSafe.
// (If `get_mut` is called on `inner`, unwind safety may get broken.)
impl UnwindSafe for PanicInfo {}
impl RefUnwindSafe for PanicInfo {}
impl PanicInfo {
    pub fn is_panicking(&self) -> bool {
        let inner = self.inner.take();
        let result = inner.is_some();
        self.inner.set(inner);
        result
    }
    /// Overwrites the curret state if the current state is not panicking
    pub fn set_panic(&self, p: Box<dyn Any + Send + 'static>) {
        if !self.is_panicking() {
            self.inner.set(Some(p));
        }
    }
    pub fn take(&self) -> Option<Box<dyn Any + Send + 'static>> {
        self.inner.take()
    }
}

pub struct EventLoopWindowTarget<T: 'static> {
    pub sender: mpsc::Sender<T>, // this is only here to be cloned elsewhere
    pub receiver: mpsc::Receiver<T>,
}

impl<T> Default for EventLoopWindowTarget<T> {
    fn default() -> Self {
        let (sender, receiver) = mpsc::channel();
        EventLoopWindowTarget { sender, receiver }
    }
}

impl<T: 'static> EventLoopWindowTarget<T> {
    #[inline]
    pub fn available_monitors(&self) -> VecDeque<MonitorHandle> {
        monitor::available_monitors()
    }

    #[inline]
    pub fn primary_monitor(&self) -> Option<RootMonitorHandle> {
        let monitor = monitor::primary_monitor();
        Some(RootMonitorHandle { inner: monitor })
    }
}

pub struct EventLoop<T: 'static> {
    pub(crate) delegate: IdRef,

    window_target: Rc<RootWindowTarget<T>>,
    panic_info: Rc<PanicInfo>,

    /// We make sure that the callback closure is dropped during a panic
    /// by making the event loop own it.
    ///
    /// Every other reference should be a Weak reference which is only upgraded
    /// into a strong reference in order to call the callback but then the
    /// strong reference should be dropped as soon as possible.
    _callback: Option<Rc<RefCell<dyn FnMut(Event<'_, T>, &RootWindowTarget<T>, &mut ControlFlow)>>>,
}

impl<T> EventLoop<T> {
    pub fn new() -> Self {
        let delegate = unsafe {
            if !msg_send![class!(NSThread), isMainThread] {
                panic!("On macOS, `EventLoop` must be created on the main thread!");
            }

            // This must be done before `NSApp()` (equivalent to sending
            // `sharedApplication`) is called anywhere else, or we'll end up
            // with the wrong `NSApplication` class and the wrong thread could
            // be marked as main.
            let app: id = msg_send![APP_CLASS.0, sharedApplication];

            let delegate = IdRef::new(msg_send![APP_DELEGATE_CLASS.0, new]);
            autoreleasepool(|| {
                let _: () = msg_send![app, setDelegate:*delegate];
            });
            delegate
        };
        let panic_info: Rc<PanicInfo> = Default::default();
        setup_control_flow_observers(Rc::downgrade(&panic_info));
        EventLoop {
            delegate,
            window_target: Rc::new(RootWindowTarget {
                p: Default::default(),
                _marker: PhantomData,
            }),
            panic_info,
            _callback: None,
        }
    }

    pub fn window_target(&self) -> &RootWindowTarget<T> {
        &self.window_target
    }

    pub fn run<F>(mut self, callback: F) -> !
    where
        F: 'static + FnMut(Event<'_, T>, &RootWindowTarget<T>, &mut ControlFlow),
    {
        self.run_return(callback);
        process::exit(0);
    }

    pub fn run_return<F>(&mut self, callback: F)
    where
        F: FnMut(Event<'_, T>, &RootWindowTarget<T>, &mut ControlFlow),
    {
        // This transmute is always safe, in case it was reached through `run`, since our
        // lifetime will be already 'static. In other cases caller should ensure that all data
        // they passed to callback will actually outlive it, some apps just can't move
        // everything to event loop, so this is something that they should care about.
        let callback = unsafe {
            mem::transmute::<
                Rc<RefCell<dyn FnMut(Event<'_, T>, &RootWindowTarget<T>, &mut ControlFlow)>>,
                Rc<RefCell<dyn FnMut(Event<'_, T>, &RootWindowTarget<T>, &mut ControlFlow)>>,
            >(Rc::new(RefCell::new(callback)))
        };

        self._callback = Some(Rc::clone(&callback));

        autoreleasepool(|| unsafe {
            let app = NSApp();
            assert_ne!(app, nil);

            // A bit of juggling with the callback references to make sure
            // that `self.callback` is the only owner of the callback.
            let weak_cb: Weak<_> = Rc::downgrade(&callback);
            mem::drop(callback);

            AppState::set_callback(weak_cb, Rc::clone(&self.window_target));
            let () = msg_send![app, run];

            if let Some(panic) = self.panic_info.take() {
                drop(self._callback.take());
                resume_unwind(panic);
            }
            AppState::exit();
        });
        drop(self._callback.take());
    }

    pub fn create_proxy(&self) -> Proxy<T> {
        Proxy::new(self.window_target.p.sender.clone())
    }
}

#[inline]
pub unsafe fn post_dummy_event(target: id) {
    let event_class = class!(NSEvent);
    let dummy_event: id = msg_send![
        event_class,
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
    let () = msg_send![target, postEvent: dummy_event atStart: YES];
}

/// Catches panics that happen inside `f` and when a panic
/// happens, stops the `sharedApplication`
#[inline]
pub fn stop_app_on_panic<F: FnOnce() -> R + UnwindSafe, R>(
    panic_info: Weak<PanicInfo>,
    f: F,
) -> Option<R> {
    match catch_unwind(f) {
        Ok(r) => Some(r),
        Err(e) => {
            // It's important that we set the panic before requesting a `stop`
            // because some callback are still called during the `stop` message
            // and we need to know in those callbacks if the application is currently
            // panicking
            {
                let panic_info = panic_info.upgrade().unwrap();
                panic_info.set_panic(e);
            }
            unsafe {
                let app_class = class!(NSApplication);
                let app: id = msg_send![app_class, sharedApplication];
                let () = msg_send![app, stop: nil];

                // Posting a dummy event to get `stop` to take effect immediately.
                // See: https://stackoverflow.com/questions/48041279/stopping-the-nsapplication-main-event-loop/48064752#48064752
                post_dummy_event(app);
            }
            None
        }
    }
}

pub struct Proxy<T> {
    sender: mpsc::Sender<T>,
    source: CFRunLoopSourceRef,
}

unsafe impl<T: Send> Send for Proxy<T> {}

impl<T> Drop for Proxy<T> {
    fn drop(&mut self) {
        unsafe {
            CFRelease(self.source as _);
        }
    }
}

impl<T> Clone for Proxy<T> {
    fn clone(&self) -> Self {
        Proxy::new(self.sender.clone())
    }
}

impl<T> Proxy<T> {
    fn new(sender: mpsc::Sender<T>) -> Self {
        unsafe {
            // just wake up the eventloop
            extern "C" fn event_loop_proxy_handler(_: *mut c_void) {}

            // adding a Source to the main CFRunLoop lets us wake it up and
            // process user events through the normal OS EventLoop mechanisms.
            let rl = CFRunLoopGetMain();
            let mut context: CFRunLoopSourceContext = mem::zeroed();
            context.perform = Some(event_loop_proxy_handler);
            let source =
                CFRunLoopSourceCreate(ptr::null_mut(), CFIndex::max_value() - 1, &mut context);
            CFRunLoopAddSource(rl, source, kCFRunLoopCommonModes);
            CFRunLoopWakeUp(rl);

            Proxy { sender, source }
        }
    }

    pub fn send_event(&self, event: T) -> Result<(), EventLoopClosed<T>> {
        self.sender
            .send(event)
            .map_err(|mpsc::SendError(x)| EventLoopClosed(x))?;
        unsafe {
            // let the main thread know there's a new event
            CFRunLoopSourceSignal(self.source);
            let rl = CFRunLoopGetMain();
            CFRunLoopWakeUp(rl);
        }
        Ok(())
    }
}
