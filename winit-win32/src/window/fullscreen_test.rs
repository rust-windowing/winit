use std::num::NonZeroU32;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use dpi::{PhysicalPosition, PhysicalSize};
use winit::application::ApplicationHandler;
use winit::event::WindowEvent;
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
use winit::platform::windows::EventLoopBuilderExtWindows;
use winit::window::{Window, WindowAttributes, WindowId};
use winit_core::monitor::{Fullscreen, MonitorHandle, VideoMode};

#[derive(Clone, Copy)]
enum Scenario {
    Success,
    Rejection,
}

struct Probe {
    scenario: Scenario,
    window: Option<Arc<dyn Window>>,
    startup_window: Option<Box<dyn Window>>,
    desktops: Vec<(MonitorHandle, VideoMode)>,
    modes: Vec<(MonitorHandle, VideoMode)>,
    original_size: PhysicalSize<u32>,
    original_position: Option<PhysicalPosition<i32>>,
    phase: u8,
    next_step: Instant,
    started: Instant,
    completed: Arc<Mutex<bool>>,
}

impl Probe {
    fn assert_desktops_restored(&self) {
        for (monitor, original) in &self.desktops {
            assert_eq!(monitor.current_video_mode(), Some(*original), "{}", monitor.native_id());
        }
    }

    fn assert_exclusive(&self, index: usize) {
        let (monitor, requested) = &self.modes[index];
        assert_eq!(
            self.window.as_ref().unwrap().fullscreen(),
            Some(Fullscreen::Exclusive(monitor.clone(), *requested)),
        );
        let actual = monitor.current_video_mode().unwrap();
        assert_eq!(actual.size(), requested.size());
        assert_eq!(actual.bit_depth(), requested.bit_depth());
        let actual_rate = actual.refresh_rate_millihertz().unwrap().get();
        let requested_rate = requested.refresh_rate_millihertz().unwrap().get();
        assert!(
            actual_rate == requested_rate
                || matches!((actual_rate, requested_rate), (59_000, 60_000) | (60_000, 59_000))
        );
        tracing::info!(
            "exclusive native monitor={} requested={requested} actual={actual}",
            monitor.native_id()
        );
    }

    fn finish(&mut self, event_loop: &dyn ActiveEventLoop) {
        self.assert_desktops_restored();
        self.window.take();
        self.startup_window.take();
        *self.completed.lock().unwrap() = true;
        event_loop.exit();
    }

    fn invalid_request(&self) {
        let window = self.window.as_ref().unwrap();
        let before = window.fullscreen();
        let (monitor, original) = &self.desktops[0];
        let invalid = VideoMode::new(original.size(), original.bit_depth(), NonZeroU32::new(1));
        window.set_fullscreen(Some(Fullscreen::Exclusive(monitor.clone(), invalid)));
        assert_eq!(window.fullscreen(), before);
    }

    fn success_step(&mut self, event_loop: &dyn ActiveEventLoop) {
        match self.phase {
            0 => {
                let window = self.window.as_ref().unwrap();
                self.original_size = window.surface_size();
                self.original_position = window.outer_position().ok();
                self.invalid_request();
                window
                    .set_fullscreen(Some(Fullscreen::Borderless(Some(self.desktops[0].0.clone()))));
            },
            1 => {
                self.invalid_request();
                let (monitor, mode) = &self.modes[0];
                self.window
                    .as_ref()
                    .unwrap()
                    .set_fullscreen(Some(Fullscreen::Exclusive(monitor.clone(), *mode)));
                self.assert_exclusive(0);
            },
            2 => {
                self.invalid_request();
                let window = self.window.as_ref().unwrap();
                let original = self.desktops[0].1;
                window.set_fullscreen(Some(Fullscreen::Exclusive(
                    self.desktops[0].0.clone(),
                    original,
                )));
                assert_eq!(self.desktops[0].0.current_video_mode(), Some(original));
                let (monitor, mode) = &self.modes[0];
                window.set_fullscreen(Some(Fullscreen::Exclusive(monitor.clone(), *mode)));
                self.assert_exclusive(0);
                if self.modes.len() > 1 {
                    let (monitor, mode) = &self.modes[1];
                    window.set_fullscreen(Some(Fullscreen::Exclusive(monitor.clone(), *mode)));
                    self.assert_exclusive(1);
                    assert_eq!(self.desktops[0].0.current_video_mode(), Some(self.desktops[0].1));
                }
            },
            3 => {
                let window = self.window.as_ref().unwrap();
                window.set_fullscreen(Some(Fullscreen::Borderless(Some(self.modes[0].0.clone()))));
                self.assert_desktops_restored();
                let (monitor, mode) = &self.modes[0];
                window.set_fullscreen(Some(Fullscreen::Exclusive(monitor.clone(), *mode)));
                self.assert_exclusive(0);
                window.set_fullscreen(None);
                self.assert_desktops_restored();
                let startup = event_loop
                    .create_window(
                        WindowAttributes::default()
                            .with_title("winit fullscreen startup probe")
                            .with_surface_size(PhysicalSize::new(900, 650))
                            .with_fullscreen(Some(Fullscreen::Exclusive(monitor.clone(), *mode))),
                    )
                    .unwrap();
                assert_eq!(
                    startup.fullscreen(),
                    Some(Fullscreen::Exclusive(monitor.clone(), *mode))
                );
                assert_ne!(monitor.current_video_mode(), Some(self.desktops[0].1));
                startup.set_fullscreen(None);
                self.assert_desktops_restored();
                self.startup_window = Some(startup);
            },
            4 => {
                let startup = self.startup_window.take().unwrap();
                assert_eq!(startup.fullscreen(), None);
                assert_eq!(startup.surface_size(), PhysicalSize::new(900, 650));
                drop(startup);
                let window = self.window.as_ref().unwrap();
                assert_eq!(window.fullscreen(), None);
                assert_eq!(window.surface_size(), self.original_size);
                let actual = window.outer_position().unwrap();
                let original = self.original_position.unwrap();
                assert!((actual.x - original.x).abs() <= 2 && (actual.y - original.y).abs() <= 2);
                let window = Arc::clone(window);
                let (monitor, mode) = self.modes[0].clone();
                std::thread::spawn(move || {
                    window.set_fullscreen(Some(Fullscreen::Exclusive(monitor.clone(), mode)));
                    window.set_fullscreen(None);
                    window.set_fullscreen(Some(Fullscreen::Borderless(Some(monitor))));
                })
                .join()
                .unwrap();
                assert_eq!(self.window.as_ref().unwrap().fullscreen(), None);
            },
            5 => {
                let window = self.window.as_ref().unwrap();
                assert_eq!(
                    window.fullscreen(),
                    Some(Fullscreen::Borderless(Some(self.modes[0].0.clone())))
                );
                self.assert_desktops_restored();
                window.set_fullscreen(None);
            },
            6 => {
                let window = self.window.take().unwrap();
                let (monitor, mode) = self.modes[0].clone();
                std::thread::spawn(move || {
                    window.set_fullscreen(Some(Fullscreen::Exclusive(monitor, mode)));
                    drop(window);
                })
                .join()
                .unwrap();
            },
            7 => self.finish(event_loop),
            _ => unreachable!(),
        }
    }

    fn rejection_step(&mut self, event_loop: &dyn ActiveEventLoop) {
        match self.phase {
            0 => {
                let (monitor, mode) = &self.modes[0];
                let startup = event_loop
                    .create_window(
                        WindowAttributes::default()
                            .with_title("winit rejected fullscreen startup probe")
                            .with_fullscreen(Some(Fullscreen::Exclusive(monitor.clone(), *mode))),
                    )
                    .unwrap();
                assert_eq!(startup.fullscreen(), None);
                drop(startup);
                self.assert_desktops_restored();
                self.window
                    .as_ref()
                    .unwrap()
                    .set_fullscreen(Some(Fullscreen::Borderless(Some(monitor.clone()))));
            },
            1 => {
                let window = self.window.as_ref().unwrap();
                let before = window.fullscreen();
                let (monitor, mode) = &self.modes[0];
                window.set_fullscreen(Some(Fullscreen::Exclusive(monitor.clone(), *mode)));
                assert_eq!(
                    window.fullscreen(),
                    before,
                    "requires a process token that rejects display changes"
                );
                self.assert_desktops_restored();
                self.invalid_request();
                window.set_fullscreen(None);
            },
            2 => {
                assert_eq!(self.window.as_ref().unwrap().fullscreen(), None);
                self.finish(event_loop);
            },
            _ => unreachable!(),
        }
    }
}

impl ApplicationHandler for Probe {
    fn can_create_surfaces(&mut self, event_loop: &dyn ActiveEventLoop) {
        let window: Arc<dyn Window> = event_loop
            .create_window(
                WindowAttributes::default()
                    .with_title("winit fullscreen regression probe")
                    .with_surface_size(PhysicalSize::new(900, 650)),
            )
            .unwrap()
            .into();
        self.desktops = event_loop
            .available_monitors()
            .filter_map(|monitor| monitor.current_video_mode().map(|mode| (monitor, mode)))
            .collect();
        for (monitor, desktop) in &self.desktops {
            let alternative = monitor
                .video_modes()
                .filter(|mode| {
                    mode.size() == desktop.size()
                        && mode.bit_depth() == desktop.bit_depth()
                        && mode.refresh_rate_millihertz() != desktop.refresh_rate_millihertz()
                })
                .min_by_key(|mode| {
                    mode.refresh_rate_millihertz()
                        .map_or(u32::MAX, |rate| rate.get().abs_diff(60_000))
                })
                .expect("requires a monitor with an alternate refresh rate at desktop resolution");
            self.modes.push((monitor.clone(), alternative));
        }
        assert!(!self.modes.is_empty());
        self.window = Some(window);
        event_loop.set_control_flow(ControlFlow::WaitUntil(self.next_step));
    }

    fn window_event(&mut self, event_loop: &dyn ActiveEventLoop, _: WindowId, event: WindowEvent) {
        if matches!(event, WindowEvent::CloseRequested) {
            self.window.as_ref().unwrap().set_fullscreen(None);
            event_loop.exit();
        }
    }

    fn about_to_wait(&mut self, event_loop: &dyn ActiveEventLoop) {
        if event_loop.exiting() {
            return;
        }
        assert!(
            self.started.elapsed() < Duration::from_secs(45),
            "native fullscreen probe timed out"
        );
        if Instant::now() < self.next_step {
            return;
        }
        match self.scenario {
            Scenario::Success => self.success_step(event_loop),
            Scenario::Rejection => self.rejection_step(event_loop),
        }
        self.phase += 1;
        self.next_step = Instant::now() + Duration::from_millis(300);
        event_loop.set_control_flow(ControlFlow::WaitUntil(self.next_step));
    }
}

fn run(scenario: Scenario) {
    let completed = Arc::new(Mutex::new(false));
    let mut builder = EventLoop::builder();
    builder.with_any_thread(true);
    let event_loop = builder.build().unwrap();
    let panics = Arc::new(AtomicUsize::new(0));
    let panic_counter = Arc::clone(&panics);
    let previous_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |_| {
        panic_counter.fetch_add(1, Ordering::Relaxed);
    }));
    let result = event_loop.run_app(Probe {
        scenario,
        window: None,
        startup_window: None,
        desktops: Vec::new(),
        modes: Vec::new(),
        original_size: PhysicalSize::new(0, 0),
        original_position: None,
        phase: 0,
        next_step: Instant::now() + Duration::from_millis(300),
        started: Instant::now(),
        completed: Arc::clone(&completed),
    });
    std::panic::set_hook(previous_hook);
    result.unwrap();
    assert_eq!(panics.load(Ordering::Relaxed), 0, "panic in a native callback or worker");
    assert!(*completed.lock().unwrap());
}

#[test]
#[ignore = "changes desktop modes; run alone in an ordinary interactive Windows process"]
fn native_fullscreen_transitions_restore_modes_geometry_and_queued_drop() {
    run(Scenario::Success);
}

#[test]
#[ignore = "run alone in a restricted Windows process that rejects display changes"]
fn native_display_rejection_preserves_state_and_does_not_panic_on_drop() {
    run(Scenario::Rejection);
}
