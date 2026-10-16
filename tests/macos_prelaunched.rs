//! Native startup regressions. Each scenario owns a disposable process because
//! AppKit launches once and winit permits one event loop per process.
//! No scenario creates a window, activates the app, or installs a menu.

#[cfg(not(target_os = "macos"))]
fn main() {}

#[cfg(target_os = "macos")]
fn main() {
    use std::process::Command;
    use std::time::{Duration, Instant};

    let args: Vec<_> = std::env::args().collect();
    if args.get(1).map(String::as_str) == Some("--scenario") {
        scenario(&args[2], args[3] == "prelaunched");
        return;
    }

    for mode in ["run", "pump"] {
        for launch in ["normal", "prelaunched"] {
            let mut child = Command::new(std::env::current_exe().unwrap())
                .args(["--scenario", mode, launch])
                .spawn()
                .unwrap();
            let deadline = Instant::now() + Duration::from_secs(10);
            loop {
                if let Some(status) = child.try_wait().unwrap() {
                    assert!(status.success(), "{mode}/{launch}: {status}");
                    break;
                }
                if Instant::now() >= deadline {
                    child.kill().unwrap();
                    child.wait().unwrap();
                    panic!("{mode}/{launch}: launch callbacks never arrived");
                }
                std::thread::sleep(Duration::from_millis(10));
            }
        }
    }
}

#[cfg(target_os = "macos")]
fn scenario(mode: &str, prelaunched: bool) {
    use std::time::Duration;

    use objc2_app_kit::{NSApplication, NSApplicationActivationPolicy, NSRunningApplication};
    use objc2_foundation::MainThreadMarker;
    use winit::application::ApplicationHandler;
    use winit::event::{StartCause, WindowEvent};
    use winit::event_loop::{ActiveEventLoop, EventLoop};
    use winit::platform::macos::{ActivationPolicy, EventLoopBuilderExtMacOS};
    use winit::platform::pump_events::{EventLoopExtPumpEvents, PumpStatus};
    use winit::platform::run_on_demand::EventLoopExtRunOnDemand;
    use winit::window::WindowId;

    #[derive(Default)]
    struct Probe {
        events: Vec<&'static str>,
    }
    impl ApplicationHandler for Probe {
        fn new_events(&mut self, _: &ActiveEventLoop, cause: StartCause) {
            if cause == StartCause::Init {
                self.events.push("init");
            }
        }
        fn resumed(&mut self, _: &ActiveEventLoop) {
            self.events.push("resumed");
        }
        fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) {
            event_loop.exit();
        }
        fn exiting(&mut self, _: &ActiveEventLoop) {
            self.events.push("exiting");
        }
        fn window_event(&mut self, _: &ActiveEventLoop, _: WindowId, _: WindowEvent) {
            panic!("the probe does not create windows");
        }
    }

    let app = NSApplication::sharedApplication(MainThreadMarker::new().unwrap());
    app.setActivationPolicy(NSApplicationActivationPolicy::Prohibited);
    if prelaunched {
        prelaunch::run(&app);
        // SAFETY: This query has no additional requirements.
        unsafe {
            assert!(NSRunningApplication::currentApplication().isFinishedLaunching());
        }
    }

    let mut event_loop = EventLoop::builder()
        .with_activation_policy(ActivationPolicy::Prohibited)
        .with_activate_ignoring_other_apps(false)
        .with_default_menu(false)
        .build()
        .unwrap();
    let mut probe = Probe::default();
    for _ in 0..2 {
        probe.events.clear();
        if mode == "run" {
            event_loop.run_app_on_demand(&mut probe).unwrap();
        } else {
            while let PumpStatus::Continue =
                event_loop.pump_app_events(Some(Duration::ZERO), &mut probe)
            {}
        }
        assert_eq!(probe.events, ["init", "resumed", "exiting"]);
    }
    println!("PASS {mode}, prelaunched={prelaunched}, two runs");
}

#[cfg(target_os = "macos")]
mod prelaunch {
    use objc2::runtime::AnyObject;
    use objc2::{declare_class, msg_send_id, mutability, sel, ClassType, DeclaredClass};
    use objc2_app_kit::{NSApplication, NSEvent, NSEventModifierFlags, NSEventType};
    use objc2_foundation::{
        MainThreadMarker, NSObject, NSObjectNSDelayedPerforming, NSObjectProtocol, NSPoint,
    };

    declare_class!(
        struct Stop;
        unsafe impl ClassType for Stop {
            type Super = NSObject;
            type Mutability = mutability::MainThreadOnly;
            const NAME: &'static str = "WinitPrelaunchedTestStop";
        }
        impl DeclaredClass for Stop {
            type Ivars = ();
        }
        unsafe impl NSObjectProtocol for Stop {}
        unsafe impl Stop {
            #[method(stopRun:)]
            fn stop_run(&self, _: Option<&AnyObject>) {
                let app = NSApplication::sharedApplication(MainThreadMarker::from(self));
                app.stop(None);
                // SAFETY: A private, application-defined event wakes AppKit
                // after stop. It has no window, pointer, or keyboard effect.
                let event = unsafe {
                    NSEvent::otherEventWithType_location_modifierFlags_timestamp_windowNumber_context_subtype_data1_data2(
                        NSEventType::ApplicationDefined, NSPoint::new(0.0, 0.0),
                        NSEventModifierFlags::empty(), 0.0, 0, None, 0, 0, 0,
                    )
                }.unwrap();
                app.postEvent_atStart(&event, true);
            }
        }
    );

    pub fn run(app: &NSApplication) {
        let this = MainThreadMarker::new().unwrap().alloc().set_ivars(());
        // SAFETY: Initialize the main-thread NSObject, schedule its own
        // selector, and run a bounded, non-reentrant AppKit loop.
        unsafe {
            let stop: objc2::rc::Retained<Stop> = msg_send_id![super(this), init];
            stop.performSelector_withObject_afterDelay(sel!(stopRun:), None, 0.05);
            app.run();
        }
    }
}
