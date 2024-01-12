//! Helpers to convert between closure-style and handler trait style.
//!
//! This is only in the interim, and will be removed in the future.
use std::marker::PhantomData;
#[cfg(not(wasm_platform))]
use std::time::Instant;

use crate::{
    event::{
        AxisId, ButtonId, DeviceEvent, DeviceId, ElementState, Event, MouseScrollDelta,
        RawKeyEvent, StartCause, WindowEvent,
    },
    event_loop::ActiveEventLoop,
    handler::{ApplicationHandler, DeviceEventHandler},
    window::WindowId,
};

#[cfg(wasm_platform)]
use web_time::Instant;

pub(crate) fn map_event<T, A: ?Sized + ApplicationHandler<T>>(
    handler: &mut A,
    event: Event<T>,
    active: ActiveEventLoop<'_>,
) {
    match event {
        Event::NewEvents(StartCause::Init) => handler.init(active),
        Event::LoopExiting => handler.exit(active),
        Event::Suspended => handler.suspend(active),
        Event::Resumed => handler.resume(active),
        Event::WindowEvent { window_id, event } => handler.window_event(active, window_id, event),
        Event::DeviceEvent { device_id, event } => {
            if let Some(handler) = handler.device_event() {
                match event {
                    DeviceEvent::Added => handler.added(active, device_id),
                    DeviceEvent::Removed => handler.removed(active, device_id),
                    DeviceEvent::MouseMotion { delta } => {
                        handler.mouse_motion(active, device_id, delta)
                    }
                    DeviceEvent::MouseWheel { delta } => {
                        handler.mouse_wheel(active, device_id, delta)
                    }
                    DeviceEvent::Motion { axis, value } => {
                        handler.motion(active, device_id, axis, value)
                    }
                    DeviceEvent::Button { button, state } => {
                        handler.button(active, device_id, button, state)
                    }
                    DeviceEvent::Key(raw) => handler.key(active, device_id, raw),
                }
            }
        }
        Event::UserEvent(event) => handler.user_event(active, event),
        Event::NewEvents(StartCause::ResumeTimeReached {
            start,
            requested_resume,
        }) => handler.start_resume_time_reached(active, start, requested_resume),
        Event::NewEvents(StartCause::WaitCancelled {
            start,
            requested_resume,
        }) => handler.start_wait_cancelled(active, start, requested_resume),
        Event::NewEvents(StartCause::Poll) => handler.start_poll(active),
        Event::AboutToWait => handler.about_to_wait(active),
        Event::MemoryWarning => handler.memory_warning(active),
    }
}

/// Introduce helper type around closure.
///
/// This is done to avoid implementing these traits on the closures directly,
/// since we don't want that implementation to be part of the public API.
pub(crate) struct MapEventHelper<T, F> {
    p: PhantomData<T>,
    f: F,
}

impl<T, F> MapEventHelper<T, F> {
    pub(crate) fn new(f: F) -> Self {
        Self { p: PhantomData, f }
    }
}

impl<T: 'static, F> ApplicationHandler<T> for MapEventHelper<T, F>
where
    F: FnMut(Event<T>, ActiveEventLoop<'_>),
{
    fn window_event(
        &mut self,
        active: ActiveEventLoop<'_>,
        window_id: WindowId,
        event: WindowEvent,
    ) {
        (self.f)(Event::WindowEvent { window_id, event }, active)
    }

    fn init(&mut self, active: ActiveEventLoop<'_>) {
        (self.f)(Event::NewEvents(StartCause::Init), active)
    }

    fn resume(&mut self, active: ActiveEventLoop<'_>) {
        (self.f)(Event::Resumed, active)
    }

    fn suspend(&mut self, active: ActiveEventLoop<'_>) {
        (self.f)(Event::Suspended, active)
    }

    fn user_event(&mut self, active: ActiveEventLoop<'_>, event: T) {
        (self.f)(Event::UserEvent(event), active)
    }

    fn exit(&mut self, active: ActiveEventLoop<'_>) {
        (self.f)(Event::LoopExiting, active)
    }

    fn start_wait_cancelled(
        &mut self,
        active: ActiveEventLoop<'_>,
        start: Instant,
        requested_resume: Option<Instant>,
    ) {
        (self.f)(
            Event::NewEvents(StartCause::WaitCancelled {
                start,
                requested_resume,
            }),
            active,
        )
    }

    fn start_resume_time_reached(
        &mut self,
        active: ActiveEventLoop<'_>,
        start: Instant,
        requested_resume: Instant,
    ) {
        (self.f)(
            Event::NewEvents(StartCause::ResumeTimeReached {
                start,
                requested_resume,
            }),
            active,
        )
    }

    fn start_poll(&mut self, active: ActiveEventLoop<'_>) {
        (self.f)(Event::NewEvents(StartCause::Poll), active)
    }

    fn about_to_wait(&mut self, active: ActiveEventLoop<'_>) {
        (self.f)(Event::AboutToWait, active)
    }

    fn memory_warning(&mut self, active: ActiveEventLoop<'_>) {
        (self.f)(Event::MemoryWarning, active)
    }

    fn device_event(&mut self) -> Option<&mut dyn DeviceEventHandler> {
        Some(self)
    }
}

impl<T: 'static, F: FnMut(Event<T>, ActiveEventLoop<'_>)> DeviceEventHandler
    for MapEventHelper<T, F>
{
    fn added(&mut self, active: ActiveEventLoop<'_>, device_id: DeviceId) {
        (self.f)(
            Event::DeviceEvent {
                device_id,
                event: DeviceEvent::Added,
            },
            active,
        )
    }

    fn removed(&mut self, active: ActiveEventLoop<'_>, device_id: DeviceId) {
        (self.f)(
            Event::DeviceEvent {
                device_id,
                event: DeviceEvent::Removed,
            },
            active,
        )
    }

    fn mouse_motion(
        &mut self,
        active: ActiveEventLoop<'_>,
        device_id: DeviceId,
        delta: (f64, f64),
    ) {
        (self.f)(
            Event::DeviceEvent {
                device_id,
                event: DeviceEvent::MouseMotion { delta },
            },
            active,
        )
    }

    fn mouse_wheel(
        &mut self,
        active: ActiveEventLoop<'_>,
        device_id: DeviceId,
        delta: MouseScrollDelta,
    ) {
        (self.f)(
            Event::DeviceEvent {
                device_id,
                event: DeviceEvent::MouseWheel { delta },
            },
            active,
        )
    }

    fn motion(
        &mut self,
        active: ActiveEventLoop<'_>,
        device_id: DeviceId,
        axis: AxisId,
        value: f64,
    ) {
        (self.f)(
            Event::DeviceEvent {
                device_id,
                event: DeviceEvent::Motion { axis, value },
            },
            active,
        )
    }

    fn button(
        &mut self,
        active: ActiveEventLoop<'_>,
        device_id: DeviceId,
        button: ButtonId,
        state: ElementState,
    ) {
        (self.f)(
            Event::DeviceEvent {
                device_id,
                event: DeviceEvent::Button { button, state },
            },
            active,
        )
    }

    fn key(&mut self, active: ActiveEventLoop<'_>, device_id: DeviceId, raw: RawKeyEvent) {
        (self.f)(
            Event::DeviceEvent {
                device_id,
                event: DeviceEvent::Key(raw),
            },
            active,
        )
    }
}
