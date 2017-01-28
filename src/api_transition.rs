
//! This temporary module generates types that wrap around the old API (winit v5 and below) and
//! expose the new API (winit v6 and above).
//!
//! This is temporary so that existing backends can smoothly transition. After all implementations
//! have finished transitionning, this module should disappear.

macro_rules! gen_api_transition {
    () => {
        pub struct EventsLoop {
            windows: ::std::sync::Mutex<Vec<::std::sync::Arc<Window>>>,
        }

        impl EventsLoop {
            pub fn new() -> EventsLoop {
                EventsLoop {
                    windows: ::std::sync::Mutex::new(vec![]),
                }
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
                // Yeah that's a very bad implementation.
                loop {
                    self.poll_events(|e| callback(e));
                    ::std::thread::sleep_ms(5);
                }
            }
        }

        pub struct Window2 {
            window: ::std::sync::Arc<Window>,
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
                })
            }

            #[inline]
            pub fn id(&self) -> usize {
                &*self.window as *const Window as usize
            }
        }
    };
}
