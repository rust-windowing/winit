//! A window event queued while a handler runs, then a nested run loop in the default mode inside
//! that handler, as a drag-tracking session does (`-[NSRunLoop runUntilDate:]` under
//! `NSCoreDragTrackingProc`, #3992).
//!
//! An event AppKit reports while the handler is in use is queued on the main run loop in the
//! default mode (a modal panel's mode is left out, #1779), so a nested loop in the default mode
//! runs it; it used to call the handler there and panic with "tried to handle event while another
//! event is currently being handled". The event must instead be delivered once the outer handler
//! has returned.
//!
//! This has to run on the main thread, hence `harness = false`.

#[cfg(target_os = "macos")]
fn main() {
    use std::time::{Duration, Instant};

    use objc2_core_foundation::{CFRunLoop, kCFRunLoopDefaultMode};
    use winit::application::ApplicationHandler;
    use winit::dpi::LogicalSize;
    use winit::event::{StartCause, WindowEvent};
    use winit::event_loop::run_on_demand::EventLoopExtRunOnDemand;
    use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
    use winit::window::{Window, WindowAttributes, WindowId};

    /// How long the nested loop runs.
    const NESTED: Duration = Duration::from_millis(500);
    const START: LogicalSize<f64> = LogicalSize::new(200.0, 150.0);
    const RESIZED: LogicalSize<f64> = LogicalSize::new(320.0, 240.0);

    #[derive(Default)]
    struct App {
        window: Option<Box<dyn Window>>,
        nested_done: bool,
        resized_after: bool,
    }

    impl ApplicationHandler for App {
        fn can_create_surfaces(&mut self, event_loop: &dyn ActiveEventLoop) {
            let attributes = WindowAttributes::default().with_surface_size(START);
            self.window = Some(event_loop.create_window(attributes).unwrap());
            // Let the window settle before the nested loop.
            let at = Instant::now() + Duration::from_millis(300);
            event_loop.set_control_flow(ControlFlow::WaitUntil(at));
        }

        fn new_events(&mut self, event_loop: &dyn ActiveEventLoop, cause: StartCause) {
            if self.nested_done || !matches!(cause, StartCause::ResumeTimeReached { .. }) {
                return;
            }
            event_loop.set_control_flow(ControlFlow::Wait);
            // AppKit reports the resize while this handler runs: winit queues the event.
            let _ = self.window.as_ref().unwrap().request_surface_size(RESIZED.into());
            // What a drag-tracking session does inside the handler: run the loop in the default
            // mode.
            let mode = unsafe { kCFRunLoopDefaultMode };
            CFRunLoop::run_in_mode(mode, NESTED.as_secs_f64(), false);
            self.nested_done = true;
        }

        fn window_event(&mut self, _: &dyn ActiveEventLoop, _: WindowId, event: WindowEvent) {
            if let WindowEvent::SurfaceResized(_) = event {
                if self.nested_done {
                    self.resized_after = true;
                }
            }
        }

        // The event delivered, the application still hears `about_to_wait` before the loop
        // waits.
        fn about_to_wait(&mut self, event_loop: &dyn ActiveEventLoop) {
            if self.resized_after {
                event_loop.exit();
            }
        }
    }

    // An event lost for good would leave the loop waiting forever.
    std::thread::spawn(|| {
        std::thread::sleep(Duration::from_secs(10));
        eprintln!("the queued event, or `about_to_wait` after it, was not delivered");
        std::process::exit(1);
    });

    let mut event_loop = EventLoop::new().unwrap();
    let mut app = App::default();
    event_loop.run_app_on_demand(&mut app).unwrap();
    assert!(app.resized_after, "the queued event was not delivered after the nested run loop");
}

#[cfg(not(target_os = "macos"))]
fn main() {}
