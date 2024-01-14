use std::{rc::Weak, time::Instant};

use icrate::Foundation::MainThreadMarker;

use super::{
    app_delegate::{ApplicationDelegate, EventWrapper},
    event_loop::PanicInfo,
};
use crate::{
    event::{Event, StartCause, WindowEvent},
    event_loop::ControlFlow,
    window::WindowId,
};

pub(crate) enum AppState {}

impl AppState {
    // Called by RunLoopObserver after finishing waiting for new events
    pub fn wakeup(panic_info: Weak<PanicInfo>) {
        let delegate = ApplicationDelegate::get(MainThreadMarker::new().unwrap());
        let panic_info = panic_info
            .upgrade()
            .expect("The panic info must exist here. This failure indicates a developer error.");

        // Return when in callback due to https://github.com/rust-windowing/winit/issues/1779
        if panic_info.is_panicking()
            || delegate.in_callback()
            || !delegate.have_callback()
            || !delegate.is_running()
        {
            return;
        }

        if delegate.stop_after_wait() {
            delegate.stop_app_immediately();
        }

        let start = delegate.start_time().unwrap();
        let cause = match delegate.control_flow() {
            ControlFlow::Poll => StartCause::Poll,
            ControlFlow::Wait => StartCause::WaitCancelled {
                start,
                requested_resume: None,
            },
            ControlFlow::WaitUntil(requested_resume) => {
                if Instant::now() >= requested_resume {
                    StartCause::ResumeTimeReached {
                        start,
                        requested_resume,
                    }
                } else {
                    StartCause::WaitCancelled {
                        start,
                        requested_resume: Some(requested_resume),
                    }
                }
            }
        };
        delegate.set_in_callback(true);
        delegate.handle_nonuser_event(Event::NewEvents(cause));
        delegate.set_in_callback(false);
    }

    // Called by RunLoopObserver before waiting for new events
    pub fn cleared(panic_info: Weak<PanicInfo>) {
        let delegate = ApplicationDelegate::get(MainThreadMarker::new().unwrap());

        let panic_info = panic_info
            .upgrade()
            .expect("The panic info must exist here. This failure indicates a developer error.");

        // Return when in callback due to https://github.com/rust-windowing/winit/issues/1779
        // XXX: how does it make sense that `get_in_callback()` can ever return `true` here if we're
        // about to return to the `CFRunLoop` to poll for new events?
        if panic_info.is_panicking()
            || delegate.in_callback()
            || !delegate.have_callback()
            || !delegate.is_running()
        {
            return;
        }

        delegate.set_in_callback(true);
        delegate.handle_user_events();
        for event in delegate.take_pending_events() {
            match event {
                EventWrapper::StaticEvent(event) => {
                    delegate.handle_nonuser_event(event);
                }
                EventWrapper::ScaleFactorChanged {
                    window,
                    suggested_size,
                    scale_factor,
                } => {
                    delegate.handle_scale_factor_changed_event(
                        &window,
                        suggested_size,
                        scale_factor,
                    );
                }
            }
        }

        for window_id in delegate.take_pending_redraw() {
            delegate.handle_nonuser_event(Event::WindowEvent {
                window_id: WindowId(window_id),
                event: WindowEvent::RedrawRequested,
            });
        }

        delegate.handle_nonuser_event(Event::AboutToWait);
        delegate.set_in_callback(false);

        if delegate.exiting() {
            delegate.stop_app_immediately();
        }

        if delegate.stop_before_wait() {
            delegate.stop_app_immediately();
        }
        delegate.update_start_time();
        let wait_timeout = delegate.wait_timeout(); // configured by pump_events
        let app_timeout = match delegate.control_flow() {
            ControlFlow::Wait => None,
            ControlFlow::Poll => Some(Instant::now()),
            ControlFlow::WaitUntil(instant) => Some(instant),
        };
        delegate
            .waker()
            .start_at(min_timeout(wait_timeout, app_timeout));
    }
}

/// Returns the minimum `Option<Instant>`, taking into account that `None`
/// equates to an infinite timeout, not a zero timeout (so can't just use
/// `Option::min`)
fn min_timeout(a: Option<Instant>, b: Option<Instant>) -> Option<Instant> {
    a.map_or(b, |a_timeout| {
        b.map_or(Some(a_timeout), |b_timeout| Some(a_timeout.min(b_timeout)))
    })
}
