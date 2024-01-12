#[cfg(not(wasm_platform))]
use std::time::Instant;

use crate::{
    event::{AxisId, ButtonId, DeviceId, ElementState, MouseScrollDelta, RawKeyEvent, WindowEvent},
    event_loop::ActiveEventLoop,
    window::WindowId,
};

#[cfg(wasm_platform)]
use web_time::Instant;

// Design choice: Always pass `ActiveEventLoop<'_>`, never allow the user to
// store that, to make it possible for backends to migrate to `&mut`.
pub trait ApplicationHandler<T = ()> {
    // TODO: Migrate this to a trait too
    fn window_event(
        &mut self,
        active: ActiveEventLoop<'_>,
        window_id: WindowId,
        event: WindowEvent,
    );

    // Default noop events

    fn init(&mut self, active: ActiveEventLoop<'_>) {
        let _ = active;
    }

    fn resume(&mut self, active: ActiveEventLoop<'_>) {
        let _ = active;
    }

    fn suspend(&mut self, active: ActiveEventLoop<'_>) {
        let _ = active;
    }

    fn user_event(&mut self, active: ActiveEventLoop<'_>, event: T) {
        let _ = active;
        let _ = event;
    }

    fn memory_warning(&mut self, active: ActiveEventLoop<'_>) {
        let _ = active;
    }

    // TODO: Migrate this to `Drop`
    fn exit(&mut self, active: ActiveEventLoop<'_>) {
        let _ = active;
    }

    // TODO: Figure out better timer support

    fn start_wait_cancelled(
        &mut self,
        active: ActiveEventLoop<'_>,
        start: Instant,
        requested_resume: Option<Instant>,
    ) {
        let _ = active;
        let _ = start;
        let _ = requested_resume;
    }

    fn start_resume_time_reached(
        &mut self,
        active: ActiveEventLoop<'_>,
        start: Instant,
        requested_resume: Instant,
    ) {
        let _ = active;
        let _ = start;
        let _ = requested_resume;
    }

    fn start_poll(&mut self, active: ActiveEventLoop<'_>) {
        let _ = active;
    }

    fn about_to_wait(&mut self, active: ActiveEventLoop<'_>) {
        let _ = active;
    }

    // TODO: Consider returning `&mut dyn DeviceEventHandler` instead,
    // and have this return a noop implementation by default.
    //
    // Note that we cannot return `impl DeviceEventHandler`, since
    // `ApplicationHandler` has to remain object safe.
    #[inline(always)]
    fn device_event(&mut self) -> Option<&mut dyn DeviceEventHandler> {
        None
    }
}

pub trait DeviceEventHandler {
    fn added(&mut self, active: ActiveEventLoop<'_>, device_id: DeviceId) {
        let _ = active;
        let _ = device_id;
    }

    fn removed(&mut self, active: ActiveEventLoop<'_>, device_id: DeviceId) {
        let _ = active;
        let _ = device_id;
    }

    fn mouse_motion(
        &mut self,
        active: ActiveEventLoop<'_>,
        device_id: DeviceId,
        delta: (f64, f64),
    ) {
        let _ = active;
        let _ = device_id;
        let _ = delta;
    }

    fn mouse_wheel(
        &mut self,
        active: ActiveEventLoop<'_>,
        device_id: DeviceId,
        delta: MouseScrollDelta,
    ) {
        let _ = active;
        let _ = device_id;
        let _ = delta;
    }

    fn motion(
        &mut self,
        active: ActiveEventLoop<'_>,
        device_id: DeviceId,
        axis: AxisId,
        value: f64,
    ) {
        let _ = active;
        let _ = device_id;
        let _ = axis;
        let _ = value;
    }

    fn button(
        &mut self,
        active: ActiveEventLoop<'_>,
        device_id: DeviceId,
        button: ButtonId,
        state: ElementState,
    ) {
        let _ = active;
        let _ = device_id;
        let _ = button;
        let _ = state;
    }

    fn key(&mut self, active: ActiveEventLoop<'_>, device_id: DeviceId, raw: RawKeyEvent) {
        let _ = active;
        let _ = device_id;
        let _ = raw;
    }
}
