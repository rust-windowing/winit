use std::{
    collections::VecDeque, marker::PhantomData, mem, os::raw::c_void, process, ptr, rc::Rc,
    sync::mpsc,
};

use cocoa::{
    appkit::NSApp,
    base::{id, nil},
    foundation::NSAutoreleasePool,
};

use crate::{
    event::Event,
    event_loop::{ControlFlow, EventLoopClosed, EventLoopWindowTarget as RootWindowTarget},
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

pub struct EventLoop<T: 'static> {
    window_target: Rc<RootWindowTarget<T>>,
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
            _delegate: delegate,
        }
    }

    #[inline]
    pub fn available_monitors(&self) -> VecDeque<MonitorHandle> {
        monitor::available_monitors()
    }

    #[inline]
    pub fn primary_monitor(&self) -> MonitorHandle {
        monitor::primary_monitor()
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
        unsafe {
            let pool = NSAutoreleasePool::new(nil);
            let app = NSApp();
            assert_ne!(app, nil);
            AppState::set_callback(callback, Rc::clone(&self.window_target));
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
