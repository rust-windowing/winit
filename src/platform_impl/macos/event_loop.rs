use std::{
    collections::VecDeque,
    marker::PhantomData,
    mem,
    os::raw::c_void,
    process, ptr,
    rc::Rc,
    sync::{mpsc, Arc, Mutex, Weak},
};

use cocoa::{
    appkit::NSApp,
    base::{id, nil},
    foundation::NSAutoreleasePool,
};

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
    window_target: Rc<RootWindowTarget<T>>,

    /// We make sure that the callback closure is dropped during a panic
    /// by making the event loop own it.
    ///
    /// Every other reference should be a Weak reference which is only upgraded
    /// into a strong reference in order to call the callback but then the
    /// strong reference should be dropped as soon as possible.
    _callback:
        Option<Arc<Mutex<Box<dyn FnMut(Event<'_, T>, &RootWindowTarget<T>, &mut ControlFlow)>>>>,

    _delegate: IdRef,
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
            let pool = NSAutoreleasePool::new(nil);
            let _: () = msg_send![app, setDelegate:*delegate];
            let _: () = msg_send![pool, drain];
            delegate
        };
        setup_control_flow_observers();
        EventLoop {
            window_target: Rc::new(RootWindowTarget {
                p: Default::default(),
                _marker: PhantomData,
            }),
            _callback: None,
            _delegate: delegate,
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
        let callback = Arc::new(Mutex::new(unsafe {
            mem::transmute::<
                Box<dyn FnMut(Event<'_, T>, &RootWindowTarget<T>, &mut ControlFlow)>,
                Box<dyn FnMut(Event<'_, T>, &RootWindowTarget<T>, &mut ControlFlow)>,
            >(Box::new(callback))
        }));

        self._callback = Some(callback.clone());

        unsafe {
            let pool = NSAutoreleasePool::new(nil);
            let app = NSApp();
            assert_ne!(app, nil);

            // A bit of juggling with the callback references to make sure
            // that `self.callback` is the only owner of the callback.
            let weak_cb: Weak<_> = Arc::downgrade(&callback);
            std::mem::drop(callback);

            AppState::set_callback(weak_cb, Rc::clone(&self.window_target));
            let _: () = msg_send![app, run];
            AppState::exit();
            pool.drain();
        }
    }

    pub fn create_proxy(&self) -> Proxy<T> {
        Proxy::new(self.window_target.p.sender.clone())
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
