#![cfg(target_os = "windows")]

//! Interactive Win32 regression test. Run with:
//! `cargo test -p winit-win32 --test redraw_fairness -- --ignored`.

use std::time::{Duration, Instant};

use winit::application::ApplicationHandler;
use winit::dpi::{PhysicalPosition, PhysicalSize};
use winit::event::{StartCause, WindowEvent};
use winit::event_loop::pump_events::{EventLoopExtPumpEvents, PumpStatus};
use winit::event_loop::run_on_demand::EventLoopExtRunOnDemand;
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
use winit::platform::windows::EventLoopBuilderExtWindows;
use winit::window::{Window, WindowAttributes, WindowId};

const WINDOW_COUNT: usize = 5;
const REDRAWS_PER_WINDOW: u64 = 20;
const CONTINUOUS_REDRAWS_PER_WINDOW: u64 = 20;
const MAX_REDRAWS_PER_PHASE: u64 = REDRAWS_PER_WINDOW * WINDOW_COUNT as u64 * 4;
const MAX_REDRAW_SKEW: u64 = WINDOW_COUNT as u64;
const MAX_TRANSIENT_REDRAW_LAG: u64 = REDRAWS_PER_WINDOW * 5;
const MAX_CONTINUOUS_REDRAWS: u64 = 20_000;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Phase {
    Warmup,
    AboutToWait,
    Continuous,
    Finished,
}

#[derive(Debug, Clone, Copy)]
enum ContinuousRequests {
    CurrentWindow,
    AllWindows,
}

struct RunAppRegression {
    windows: Vec<Box<dyn Window>>,
    continuous_requests: ContinuousRequests,
    phase: Phase,
    warmup_started: Instant,
    warmup_redraws: Vec<bool>,
    about_to_wait_started: Instant,
    about_to_wait_redraws: Vec<u64>,
    redraws: Vec<u64>,
    continuous_new_events: usize,
    continuous_about_to_wait: usize,
    continuous_deadline: Instant,
    continuous_deadline_reached: bool,
    continuous_started: Instant,
    failure: Option<String>,
}

impl RunAppRegression {
    fn new(continuous_requests: ContinuousRequests) -> Self {
        let now = Instant::now();
        Self {
            windows: Vec::new(),
            continuous_requests,
            phase: Phase::Warmup,
            warmup_started: now,
            warmup_redraws: vec![false; WINDOW_COUNT],
            about_to_wait_started: now,
            about_to_wait_redraws: Vec::new(),
            redraws: vec![0; WINDOW_COUNT],
            continuous_new_events: 0,
            continuous_about_to_wait: 0,
            continuous_deadline: now,
            continuous_deadline_reached: false,
            continuous_started: now,
            failure: None,
        }
    }

    fn request_all(&self) {
        for window in &self.windows {
            window.request_redraw();
        }
    }

    fn fail(&mut self, event_loop: &dyn ActiveEventLoop, message: impl Into<String>) {
        self.failure = Some(message.into());
        self.phase = Phase::Finished;
        event_loop.exit();
    }

    fn begin_continuous(&mut self) {
        self.phase = Phase::Continuous;
        self.redraws.fill(0);
        self.continuous_new_events = 0;
        self.continuous_about_to_wait = 0;
        self.continuous_deadline_reached = false;
        self.continuous_started = Instant::now();
        self.continuous_deadline = self.continuous_started + Duration::from_millis(20);
        self.request_all();
    }

    fn continuous_is_complete(&self) -> bool {
        self.redraws.iter().all(|&count| count >= CONTINUOUS_REDRAWS_PER_WINDOW)
            && self.continuous_new_events > 0
            && self.continuous_about_to_wait > 0
            && self.continuous_deadline_reached
    }
}

impl ApplicationHandler for RunAppRegression {
    fn new_events(&mut self, _event_loop: &dyn ActiveEventLoop, cause: StartCause) {
        if self.phase == Phase::Continuous {
            self.continuous_new_events += 1;
            if matches!(cause, StartCause::ResumeTimeReached { .. }) {
                self.continuous_deadline_reached = true;
            }
        }
    }

    fn can_create_surfaces(&mut self, event_loop: &dyn ActiveEventLoop) {
        if !self.windows.is_empty() {
            return;
        }
        for index in 0..WINDOW_COUNT {
            // Deliberately overlap the windows to exercise Win32's non-FIFO paint selection.
            let attributes = WindowAttributes::default()
                .with_title(format!("winit redraw fairness regression {index}"))
                .with_visible(true)
                .with_surface_size(PhysicalSize::new(96, 72))
                .with_position(PhysicalPosition::new(32 + index as i32 * 24, 32));
            self.windows.push(event_loop.create_window(attributes).unwrap());
        }
    }

    fn window_event(
        &mut self,
        event_loop: &dyn ActiveEventLoop,
        window_id: WindowId,
        event: WindowEvent,
    ) {
        if event != WindowEvent::RedrawRequested {
            return;
        }
        let index = self.windows.iter().position(|window| window.id() == window_id).unwrap();

        match self.phase {
            Phase::Warmup => self.warmup_redraws[index] = true,
            Phase::AboutToWait => {
                self.redraws[index] += 1;
                if self.redraws.iter().sum::<u64>() > MAX_REDRAWS_PER_PHASE
                    || self.about_to_wait_started.elapsed() > Duration::from_secs(5)
                {
                    self.fail(
                        event_loop,
                        format!("AboutToWait redraw starvation: {:?}", self.redraws),
                    );
                }
            },
            Phase::Continuous => {
                self.redraws[index] += 1;
                let total: u64 = self.redraws.iter().sum();
                let min = *self.redraws.iter().min().unwrap();
                let max = *self.redraws.iter().max().unwrap();
                if max - min > MAX_TRANSIENT_REDRAW_LAG
                    || total > MAX_CONTINUOUS_REDRAWS
                    || self.continuous_started.elapsed() > Duration::from_secs(5)
                {
                    self.fail(
                        event_loop,
                        format!(
                            "continuous redraw did not remain fair: redraws={:?}, new_events={}, \
                             about_to_wait={}, deadline_reached={}",
                            self.redraws,
                            self.continuous_new_events,
                            self.continuous_about_to_wait,
                            self.continuous_deadline_reached
                        ),
                    );
                    return;
                }

                if self.continuous_is_complete() {
                    self.phase = Phase::Finished;
                    event_loop.exit();
                    return;
                }

                match self.continuous_requests {
                    ContinuousRequests::CurrentWindow => {
                        self.windows[index].request_redraw();
                    },
                    ContinuousRequests::AllWindows => self.request_all(),
                }
            },
            Phase::Finished => {},
        }
    }

    fn about_to_wait(&mut self, event_loop: &dyn ActiveEventLoop) {
        match self.phase {
            Phase::Warmup => {
                if self.warmup_started.elapsed() > Duration::from_secs(5) {
                    self.fail(
                        event_loop,
                        format!("initial paints timed out: {:?}", self.warmup_redraws),
                    );
                    return;
                }
                if self.warmup_redraws.iter().all(|&redrawn| redrawn) {
                    self.phase = Phase::AboutToWait;
                    self.redraws.fill(0);
                    self.about_to_wait_started = Instant::now();
                }
                self.request_all();
                event_loop.set_control_flow(ControlFlow::Poll);
            },
            Phase::AboutToWait => {
                if self.redraws.iter().sum::<u64>() > MAX_REDRAWS_PER_PHASE
                    || self.about_to_wait_started.elapsed() > Duration::from_secs(5)
                {
                    self.fail(
                        event_loop,
                        format!("AboutToWait redraw starvation: {:?}", self.redraws),
                    );
                    return;
                }
                if self.redraws.iter().all(|&count| count >= REDRAWS_PER_WINDOW) {
                    self.about_to_wait_redraws.clone_from(&self.redraws);
                    self.begin_continuous();
                    event_loop.set_control_flow(ControlFlow::WaitUntil(self.continuous_deadline));
                } else {
                    self.request_all();
                    event_loop.set_control_flow(ControlFlow::Poll);
                }
            },
            Phase::Continuous => {
                self.continuous_about_to_wait += 1;
                if self.continuous_started.elapsed() > Duration::from_secs(5) {
                    self.fail(
                        event_loop,
                        format!(
                            "continuous redraw timed out: redraws={:?}, new_events={}, \
                             about_to_wait={}, deadline_reached={}",
                            self.redraws,
                            self.continuous_new_events,
                            self.continuous_about_to_wait,
                            self.continuous_deadline_reached
                        ),
                    );
                    return;
                }
                event_loop.set_control_flow(ControlFlow::WaitUntil(self.continuous_deadline));
            },
            Phase::Finished => {},
        }
    }
}

struct PumpRegression {
    windows: Vec<Box<dyn Window>>,
    measuring: bool,
    warmup_redraws: Vec<bool>,
    redraws: Vec<u64>,
    new_events: usize,
    about_to_wait: usize,
}

impl ApplicationHandler for PumpRegression {
    fn new_events(&mut self, _event_loop: &dyn ActiveEventLoop, _cause: StartCause) {
        self.new_events += 1;
    }

    fn can_create_surfaces(&mut self, event_loop: &dyn ActiveEventLoop) {
        if !self.windows.is_empty() {
            return;
        }
        for index in 0..WINDOW_COUNT {
            let attributes = WindowAttributes::default()
                .with_title(format!("winit pump redraw fairness regression {index}"))
                .with_visible(true)
                .with_surface_size(PhysicalSize::new(96, 72))
                .with_position(PhysicalPosition::new(32 + index as i32 * 24, 128));
            self.windows.push(event_loop.create_window(attributes).unwrap());
        }
        self.redraws = vec![0; self.windows.len()];
        self.warmup_redraws = vec![false; self.windows.len()];
    }

    fn window_event(
        &mut self,
        _event_loop: &dyn ActiveEventLoop,
        window_id: WindowId,
        event: WindowEvent,
    ) {
        if event == WindowEvent::RedrawRequested {
            let index = self.windows.iter().position(|window| window.id() == window_id).unwrap();
            if self.measuring {
                self.redraws[index] += 1;
                self.windows[index].request_redraw();
            } else {
                self.warmup_redraws[index] = true;
            }
        }
    }

    fn about_to_wait(&mut self, event_loop: &dyn ActiveEventLoop) {
        self.about_to_wait += 1;
        if !self.measuring && self.warmup_redraws.iter().all(|&redrawn| redrawn) {
            self.measuring = true;
            self.redraws.fill(0);
        }
        for window in &self.windows {
            window.request_redraw();
        }
        event_loop.set_control_flow(ControlFlow::Poll);
    }
}

#[test]
#[ignore = "requires an interactive Windows desktop"]
fn redraw_batches_are_fair_and_keep_event_loop_cycles() {
    let mut builder = EventLoop::builder();
    builder.with_any_thread(true);
    let mut event_loop = builder.build().unwrap();

    for continuous_requests in [ContinuousRequests::CurrentWindow, ContinuousRequests::AllWindows] {
        let mut run_app = RunAppRegression::new(continuous_requests);
        event_loop.run_app_on_demand(&mut run_app).unwrap();
        assert_eq!(run_app.phase, Phase::Finished);
        assert!(
            run_app.failure.is_none(),
            "{continuous_requests:?}: {}",
            run_app.failure.unwrap_or_default()
        );
        assert!(run_app.continuous_new_events > 0);
        assert!(run_app.continuous_about_to_wait > 0);
        assert!(run_app.continuous_deadline_reached);
        let min = *run_app.about_to_wait_redraws.iter().min().unwrap();
        let max = *run_app.about_to_wait_redraws.iter().max().unwrap();
        // Win32 may coalesce explicit requests with system paints, so callback counts need not be
        // exactly equal. Reaching the target within the strict total budget rules out starvation.
        assert!(min >= REDRAWS_PER_WINDOW);
        assert!(
            max - min <= MAX_REDRAW_SKEW,
            "AboutToWait redraw counts diverged: {:?}",
            run_app.about_to_wait_redraws
        );
        let min = *run_app.redraws.iter().min().unwrap();
        let max = *run_app.redraws.iter().max().unwrap();
        assert!(min >= CONTINUOUS_REDRAWS_PER_WINDOW);
        assert!(
            max - min <= MAX_REDRAW_SKEW,
            "{continuous_requests:?} redraw counts diverged: {:?}",
            run_app.redraws
        );
        drop(run_app);
    }

    let mut pump = PumpRegression {
        windows: Vec::new(),
        measuring: false,
        warmup_redraws: Vec::new(),
        redraws: Vec::new(),
        new_events: 0,
        about_to_wait: 0,
    };
    let mut complete_pump_frames = 0;
    for _ in 0..100 {
        let was_measuring = pump.measuring;
        let before = pump.redraws.clone();
        let new_events_before = pump.new_events;
        let about_to_wait_before = pump.about_to_wait;
        assert_eq!(
            event_loop.pump_app_events(Some(Duration::ZERO), &mut pump),
            PumpStatus::Continue
        );
        if was_measuring && pump.measuring {
            let deltas: Vec<_> =
                before.iter().zip(&pump.redraws).map(|(before, after)| after - before).collect();
            assert!(deltas.iter().all(|&delta| delta <= 1), "pump dispatched an HWND twice");
            assert!(pump.new_events > new_events_before, "pump emitted no NewEvents");
            assert!(pump.about_to_wait > about_to_wait_before, "pump emitted no AboutToWait");
            complete_pump_frames += 1;
        }
        if pump.measuring
            && pump.redraws.iter().all(|&count| count >= CONTINUOUS_REDRAWS_PER_WINDOW)
        {
            break;
        }
    }

    let min = *pump.redraws.iter().min().unwrap();
    let max = *pump.redraws.iter().max().unwrap();
    assert!(min >= CONTINUOUS_REDRAWS_PER_WINDOW);
    assert!(max - min <= MAX_REDRAW_SKEW, "pump redraw counts diverged: {:?}", pump.redraws);
    assert!(pump.new_events > 0);
    assert!(pump.about_to_wait > 0);
    assert!(complete_pump_frames > 0);
}
