//! A wake-up from another thread while a nested run loop runs inside an event handler, as
//! `NSAlert::runModal` or `NSOpenPanel::runModal` (`rfd`'s synchronous dialogs) do when opened from
//! a handler.
//!
//! The proxy's run loop source is registered in the common modes, so it also fires inside the
//! nested loop; it used to call the handler again there and panic with "tried to handle event while
//! another event is currently being handled" (#3992). The wake-up must instead be delivered once
//! the outer handler has returned.
//!
//! This has to run on the main thread, hence `harness = false`.

#[cfg(target_os = "macos")]
fn main() {
    use std::sync::Arc;
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::time::{Duration, Instant};

    use objc2_core_foundation::{CFRunLoop, CFString};
    use winit::application::ApplicationHandler;
    use winit::event::{StartCause, WindowEvent};
    use winit::event_loop::run_on_demand::EventLoopExtRunOnDemand;
    use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
    use winit::window::WindowId;

    /// How long the nested loop runs, and how long the other thread keeps waking the event loop.
    const NESTED: Duration = Duration::from_millis(600);
    const WAKING: Duration = Duration::from_millis(300);

    #[derive(Default)]
    struct App {
        nested_done: bool,
        woken_after: bool,
    }

    impl ApplicationHandler for App {
        fn can_create_surfaces(&mut self, event_loop: &dyn ActiveEventLoop) {
            // Let the launch settle: a nested loop during it would take its stop for its own.
            let at = Instant::now() + Duration::from_millis(200);
            event_loop.set_control_flow(ControlFlow::WaitUntil(at));
        }

        fn new_events(&mut self, event_loop: &dyn ActiveEventLoop, cause: StartCause) {
            if self.nested_done || !matches!(cause, StartCause::ResumeTimeReached { .. }) {
                return;
            }
            event_loop.set_control_flow(ControlFlow::Wait);
            let proxy = event_loop.create_proxy();
            let waking = Arc::new(AtomicBool::new(true));
            let still_waking = waking.clone();
            std::thread::spawn(move || {
                let end = Instant::now() + WAKING;
                while Instant::now() < end {
                    proxy.wake_up();
                    std::thread::sleep(Duration::from_millis(2));
                }
                still_waking.store(false, Ordering::SeqCst);
            });
            // What a modal panel does inside the handler: run the loop in its own mode.
            let mode = CFString::from_static_str("NSModalPanelRunLoopMode");
            CFRunLoop::run_in_mode(Some(&mode), NESTED.as_secs_f64(), false);
            assert!(!waking.load(Ordering::SeqCst), "the wake-ups outlasted the nested loop");
            self.nested_done = true;
        }

        fn proxy_wake_up(&mut self, _: &dyn ActiveEventLoop) {
            if self.nested_done {
                self.woken_after = true;
            }
        }

        // The wake-up delivered, the application still hears `about_to_wait` before the loop
        // waits.
        fn about_to_wait(&mut self, event_loop: &dyn ActiveEventLoop) {
            if self.woken_after {
                event_loop.exit();
            }
        }

        fn window_event(&mut self, _: &dyn ActiveEventLoop, _: WindowId, _: WindowEvent) {}
    }

    // A wake-up lost for good would leave the loop waiting forever.
    std::thread::spawn(|| {
        std::thread::sleep(Duration::from_secs(10));
        eprintln!("the wake-up, or `about_to_wait` after it, was not delivered");
        std::process::exit(1);
    });

    let mut event_loop = EventLoop::new().unwrap();
    let mut app = App::default();
    event_loop.run_app_on_demand(&mut app).unwrap();
    assert!(app.woken_after, "the wake-up was not delivered after the nested run loop");
}

#[cfg(not(target_os = "macos"))]
fn main() {}
