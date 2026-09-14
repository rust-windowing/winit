//! Native, non-interactive Quit regression probe. No visible windows or input.
//!
//! On macOS, run with `--features macos-quit-as-close --example macos_quit -- MODE`.
//! Modes: `keep-open`, `early`, `no-window`. Success requires the final `Returned`
//! line, not only exit status 0: without the feature AppKit terminates directly.

#[cfg(not(target_os = "macos"))]
fn main() {
    eprintln!("This example requires macOS");
}

#[cfg(target_os = "macos")]
fn main() -> Result<(), Box<dyn std::error::Error>> {
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::Arc;
    use std::time::Duration;

    use objc2::sel;
    use objc2_app_kit::NSApplication;
    use objc2_foundation::{MainThreadMarker, NSObjectNSDelayedPerforming};
    use winit::application::ApplicationHandler;
    use winit::event::WindowEvent;
    use winit::event_loop::{ActiveEventLoop, EventLoop, EventLoopProxy};
    use winit::platform::macos::{ActivationPolicy, EventLoopBuilderExtMacOS};
    use winit::platform::run_on_demand::EventLoopExtRunOnDemand;
    use winit::window::{Window, WindowId};

    #[derive(Clone, Copy)]
    enum Event {
        CheckKeptOpen,
    }

    fn quit(later: bool) {
        let mtm = MainThreadMarker::new().expect("Quit requires the main thread");
        let app = NSApplication::sharedApplication(mtm);
        if later {
            // SAFETY: terminate: takes an optional object and returns void. Deliver it outside
            // this application callback, as an ordinary menu/Dock request would arrive.
            unsafe {
                app.performSelector_withObject_afterDelay(sel!(terminate:), None, 0.01);
            }
        } else {
            // SAFETY: Main-thread AppKit call with no sender. In early mode this deliberately
            // tests requests made before window creation and from inside a Winit callback.
            unsafe { app.terminate(None) };
        }
    }

    struct Probe {
        mode: String,
        proxy: EventLoopProxy<Event>,
        window: Option<Window>,
        close_requests: usize,
        kept_open: bool,
        exiting: bool,
    }

    impl ApplicationHandler<Event> for Probe {
        fn resumed(&mut self, event_loop: &ActiveEventLoop) {
            if self.mode == "early" {
                quit(false);
            }
            if self.mode != "no-window" {
                self.window = Some(
                    event_loop
                        .create_window(Window::default_attributes().with_visible(false))
                        .expect("create hidden window"),
                );
            }
            if self.mode == "early" {
                // All three requests in this callback should coalesce into one close request.
                quit(false);
                quit(false);
            } else {
                quit(true);
            }
        }

        fn window_event(
            &mut self,
            event_loop: &ActiveEventLoop,
            window_id: WindowId,
            event: WindowEvent,
        ) {
            if matches!(event, WindowEvent::CloseRequested) {
                assert_eq!(self.window.as_ref().unwrap().id(), window_id);
                self.close_requests += 1;
                if self.kept_open {
                    assert_eq!(self.close_requests, 2);
                    self.window.take();
                    event_loop.exit();
                } else {
                    assert_eq!(self.close_requests, 1, "Quit requests were not coalesced");
                    let proxy = self.proxy.clone();
                    std::thread::spawn(move || {
                        std::thread::sleep(Duration::from_millis(150));
                        let _ = proxy.send_event(Event::CheckKeptOpen);
                    });
                }
            }
        }

        fn user_event(&mut self, _event_loop: &ActiveEventLoop, _: Event) {
            assert!(self.window.is_some(), "Quit closed the protected window");
            assert_eq!(self.close_requests, 1);
            self.kept_open = true;
            quit(true);
        }

        fn exiting(&mut self, _event_loop: &ActiveEventLoop) {
            self.exiting = true;
        }
    }

    struct Cleanup(Arc<AtomicBool>);
    impl Drop for Cleanup {
        fn drop(&mut self) {
            self.0.store(true, Ordering::Release);
        }
    }

    let mode = std::env::args().nth(1).unwrap_or_else(|| "keep-open".into());
    assert!(matches!(mode.as_str(), "keep-open" | "early" | "no-window"));
    let completed = Arc::new(AtomicBool::new(false));
    let watchdog = completed.clone();
    std::thread::spawn(move || {
        std::thread::sleep(Duration::from_secs(10));
        if !watchdog.load(Ordering::Acquire) {
            eprintln!("Quit probe did not complete");
            std::process::exit(71);
        }
    });
    let cleaned_up = Arc::new(AtomicBool::new(false));
    let cleanup = Cleanup(cleaned_up.clone());
    let mut builder = EventLoop::<Event>::with_user_event();
    builder
        .with_activation_policy(ActivationPolicy::Prohibited)
        .with_activate_ignoring_other_apps(false)
        .with_default_menu(false);
    let mut event_loop = builder.build()?;
    let mut probe = Probe {
        mode,
        proxy: event_loop.create_proxy(),
        window: None,
        close_requests: 0,
        kept_open: false,
        exiting: false,
    };
    event_loop.run_app_on_demand(&mut probe)?;
    assert!(probe.exiting, "Missing event-loop exit callback");
    assert!(probe.window.is_none(), "Owned window was not closed");
    assert!(probe.mode == "no-window" || probe.close_requests == 2, "Close review was bypassed");
    drop(cleanup);
    assert!(cleaned_up.load(Ordering::Acquire));
    completed.store(true, Ordering::Release);
    println!(
        "Returned: mode={}, close_requests={}, kept_open={}, cleanup=true",
        probe.mode, probe.close_requests, probe.kept_open
    );
    Ok(())
}
