
//! This temporary module generates types that wrap around the old API (winit v5 and below) and
//! expose the new API (winit v6 and above).
//!
//! This is temporary so that existing backends can smoothly transition. After all implementations
//! have finished transitionning, this module should disappear.

macro_rules! gen_api_transition {
    () => {
        pub struct EventsLoop {
            windows: ::std::sync::Mutex<Vec<::std::sync::Arc<Window>>>,
            interrupted: ::std::sync::atomic::AtomicBool,
        }

        impl EventsLoop {
            pub fn new() -> EventsLoop {
                EventsLoop {
                    windows: ::std::sync::Mutex::new(vec![]),
                    interrupted: ::std::sync::atomic::AtomicBool::new(false),
                }
            }

            pub fn interrupt(&self) {
                self.interrupted.store(true, ::std::sync::atomic::Ordering::Relaxed);
            }

            pub fn poll_events<F>(&self, mut callback: F)
                where F: FnMut(::Event)
            {
                let mut windows = self.windows.lock().unwrap();
                for window in windows.iter() {
                    for event in window.poll_events() {
                        callback(::Event::WindowEvent {
                            window_id: &**window as *const Window as usize,
                            event: event,
                        })
                    }
                }
            }

            pub fn run_forever<F>(&self, mut callback: F)
                where F: FnMut(::Event)
            {
                self.interrupted.store(false, ::std::sync::atomic::Ordering::Relaxed);

                // Yeah that's a very bad implementation.
                loop {
                    self.poll_events(|e| callback(e));
                    ::std::thread::sleep_ms(5);
                    if self.interrupted.load(::std::sync::atomic::Ordering::Relaxed) {
                        break;
                    }
                }
            }
        }

        pub struct Window2 {
            window: ::std::sync::Arc<Window>,
            events_loop: ::std::sync::Weak<EventsLoop>,
        }

        impl ::std::ops::Deref for Window2 {
            type Target = Window;
            #[inline]
            fn deref(&self) -> &Window {
                &*self.window
            }
        }

        impl Window2 {
            pub fn new(events_loop: ::std::sync::Arc<EventsLoop>, window: &::WindowAttributes,
                       pl_attribs: &PlatformSpecificWindowBuilderAttributes)
                       -> Result<Window2, CreationError>
            {
                let win = ::std::sync::Arc::new(try!(Window::new(window, pl_attribs)));
                events_loop.windows.lock().unwrap().push(win.clone());
                Ok(Window2 {
                    window: win,
                    events_loop: ::std::sync::Arc::downgrade(&events_loop),
                })
            }

            #[inline]
            pub fn id(&self) -> usize {
                &*self.window as *const Window as usize
            }
        }

        impl Drop for Window2 {
            fn drop(&mut self) {
                if let Some(ev) = self.events_loop.upgrade() {
                    let mut windows = ev.windows.lock().unwrap();
                    windows.retain(|w| &**w as *const Window != &*self.window as *const _);
                }
            }
        }
    };
}
