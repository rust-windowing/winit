
//! This temporary module generates types that wrap around the old API (winit v5 and below) and
//! expose the new API (winit v6 and above).
//!
//! This is temporary so that existing backends can smoothly transition. After all implementations
//! have finished transitionning, this module should disappear.

macro_rules! gen_api_transition {
    () => {
        pub struct EventsLoop {
            windows: ::std::sync::Arc<::std::sync::Mutex<Vec<::std::sync::Arc<Window>>>>,
            awakened: ::std::sync::Arc<::std::sync::atomic::AtomicBool>,
        }

        pub struct EventsLoopProxy {
            awakened: ::std::sync::Weak<::std::sync::atomic::AtomicBool>,
        }

        impl EventsLoop {
            pub fn new() -> EventsLoop {
                EventsLoop {
                    windows: ::std::sync::Arc::new(::std::sync::Mutex::new(vec![])),
                    awakened: ::std::sync::Arc::new(::std::sync::atomic::AtomicBool::new(false)),
                }
            }

            pub fn poll_events<F>(&mut self, mut callback: F)
                where F: FnMut(::Event)
            {
                if self.awakened.load(::std::sync::atomic::Ordering::Relaxed) {
                    self.awakened.store(false, ::std::sync::atomic::Ordering::Relaxed);
                    callback(::Event::Awakened);
                }

                let windows = self.windows.lock().unwrap();
                for window in windows.iter() {
                    for event in window.poll_events() {
                        callback(::Event::WindowEvent {
                            window_id: ::WindowId(WindowId(&**window as *const Window as usize)),
                            event: event,
                        })
                    }
                }
            }

            pub fn run_forever<F>(&mut self, mut callback: F)
                where F: FnMut(::Event) -> ::ControlFlow,
            {
                self.awakened.store(false, ::std::sync::atomic::Ordering::Relaxed);

                // Yeah that's a very bad implementation.
                loop {
                    let mut control_flow = ::ControlFlow::Continue;
                    self.poll_events(|e| {
                        if let ::ControlFlow::Break = callback(e) {
                            control_flow = ::ControlFlow::Break;
                        }
                    });
                    if let ::ControlFlow::Break = control_flow {
                        break;
                    }
                    ::std::thread::sleep(::std::time::Duration::from_millis(5));
                }
            }

            pub fn create_proxy(&self) -> EventsLoopProxy {
                EventsLoopProxy {
                    awakened: ::std::sync::Arc::downgrade(&self.awakened),
                }
            }
        }

        impl EventsLoopProxy {
            pub fn wakeup(&self) -> Result<(), ::EventsLoopClosed> {
                match self.awakened.upgrade() {
                    None => Err(::EventsLoopClosed),
                    Some(awakened) => {
                        awakened.store(true, ::std::sync::atomic::Ordering::Relaxed);
                        Ok(())
                    },
                }
            }
        }

        #[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
        pub struct WindowId(usize);

        #[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
        pub struct DeviceId;

        pub struct Window2 {
            pub window: ::std::sync::Arc<Window>,
            windows: ::std::sync::Weak<::std::sync::Mutex<Vec<::std::sync::Arc<Window>>>>
        }

        impl ::std::ops::Deref for Window2 {
            type Target = Window;
            #[inline]
            fn deref(&self) -> &Window {
                &*self.window
            }
        }

        impl Window2 {
            pub fn new(events_loop: &EventsLoop,
                       window: &::WindowAttributes,
                       pl_attribs: &PlatformSpecificWindowBuilderAttributes)
                       -> Result<Window2, CreationError>
            {
                let win = ::std::sync::Arc::new(try!(Window::new(window, pl_attribs)));
                events_loop.windows.lock().unwrap().push(win.clone());
                Ok(Window2 {
                    window: win,
                    windows: ::std::sync::Arc::downgrade(&events_loop.windows),
                })
            }

            #[inline]
            pub fn id(&self) -> WindowId {
                WindowId(&*self.window as *const Window as usize)
            }
        }

        impl Drop for Window2 {
            fn drop(&mut self) {
                if let Some(windows) = self.windows.upgrade() {
                    let mut windows = windows.lock().unwrap();
                    windows.retain(|w| &**w as *const Window != &*self.window as *const _);
                }
            }
        }
    };
}
