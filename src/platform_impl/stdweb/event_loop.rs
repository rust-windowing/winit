use super::*;

use dpi::LogicalPosition;
use event::{
    DeviceId as RootDI, ElementState, Event, KeyboardInput, MouseScrollDelta, StartCause,
    TouchPhase, WindowEvent,
};
use event_loop::{ControlFlow, EventLoopClosed, EventLoopWindowTarget as RootELW};
use instant::{Duration, Instant};
use std::{
    cell::RefCell,
    clone::Clone,
    collections::{vec_deque::IntoIter as VecDequeIter, VecDeque},
    marker::PhantomData,
    rc::Rc,
};
use stdweb::{
    traits::*,
    web::{document, event::*, html_element::CanvasElement, window, TimeoutHandle},
};
use window::WindowId as RootWI;

#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct DeviceId(i32);

impl DeviceId {
    pub unsafe fn dummy() -> Self {
        DeviceId(0)
    }
}

pub struct EventLoop<T: 'static> {
    elw: RootELW<T>,
}

pub struct EventLoopWindowTarget<T: 'static> {
    pub(crate) runner: EventLoopRunnerShared<T>,
}

impl<T> EventLoopWindowTarget<T> {
    fn new() -> Self {
        EventLoopWindowTarget {
            runner: EventLoopRunnerShared(Rc::new(ELRShared {
                runner: RefCell::new(None),
                events: RefCell::new(VecDeque::new()),
            })),
        }
    }
}

#[derive(Clone)]
pub struct EventLoopProxy<T: 'static> {
    runner: EventLoopRunnerShared<T>,
}

impl<T: 'static> EventLoopProxy<T> {
    pub fn send_event(&self, event: T) -> Result<(), EventLoopClosed> {
        self.runner.send_event(Event::UserEvent(event));
        Ok(())
    }
}

pub struct EventLoopRunnerShared<T>(Rc<ELRShared<T>>);

impl<T> Clone for EventLoopRunnerShared<T> {
    fn clone(&self) -> Self {
        EventLoopRunnerShared(self.0.clone())
    }
}

pub struct ELRShared<T> {
    runner: RefCell<Option<EventLoopRunner<T>>>,
    events: RefCell<VecDeque<Event<T>>>,
}

struct EventLoopRunner<T> {
    control: ControlFlowStatus,
    is_busy: bool,
    event_handler: Box<dyn FnMut(Event<T>, &mut ControlFlow)>,
}

enum ControlFlowStatus {
    Init,
    WaitUntil {
        timeout: TimeoutHandle,
        start: Instant,
        end: Instant,
    },
    Wait {
        start: Instant,
    },
    Poll {
        timeout: TimeoutHandle,
    },
    Exit,
}

impl ControlFlowStatus {
    fn to_control_flow(&self) -> ControlFlow {
        match self {
            ControlFlowStatus::Init => ControlFlow::Poll, // During the Init loop, the user should get Poll, the default control value
            ControlFlowStatus::WaitUntil { end, .. } => ControlFlow::WaitUntil(*end),
            ControlFlowStatus::Wait { .. } => ControlFlow::Wait,
            ControlFlowStatus::Poll { .. } => ControlFlow::Poll,
            ControlFlowStatus::Exit => ControlFlow::Exit,
        }
    }

    fn is_exit(&self) -> bool {
        match self {
            ControlFlowStatus::Exit => true,
            _ => false,
        }
    }
}

impl<T> EventLoop<T> {
    pub fn new() -> Self {
        EventLoop {
            elw: RootELW {
                p: EventLoopWindowTarget::new(),
                _marker: PhantomData,
            },
        }
    }

    pub fn available_monitors(&self) -> VecDequeIter<MonitorHandle> {
        VecDeque::new().into_iter()
    }

    pub fn primary_monitor(&self) -> MonitorHandle {
        MonitorHandle
    }

    pub fn run<F>(self, mut event_handler: F) -> !
    where
        F: 'static + FnMut(Event<T>, &RootELW<T>, &mut ControlFlow),
    {
        let runner = self.elw.p.runner;

        let relw = RootELW {
            p: EventLoopWindowTarget::new(),
            _marker: PhantomData,
        };
        runner.set_listener(Box::new(move |evt, ctrl| event_handler(evt, &relw, ctrl)));

        let document = &document();
        add_event(&runner, document, |elrs, _: BlurEvent| {
            elrs.send_event(Event::WindowEvent {
                window_id: RootWI(WindowId),
                event: WindowEvent::Focused(false),
            });
        });
        add_event(&runner, document, |elrs, _: FocusEvent| {
            elrs.send_event(Event::WindowEvent {
                window_id: RootWI(WindowId),
                event: WindowEvent::Focused(true),
            });
        });
        add_event(&runner, document, |elrs, event: KeyDownEvent| {
            let key = event.key();
            let mut characters = key.chars();
            let first = characters.next();
            let second = characters.next();
            if let (Some(key), None) = (first, second) {
                elrs.send_event(Event::WindowEvent {
                    window_id: RootWI(WindowId),
                    event: WindowEvent::ReceivedCharacter(key),
                });
            }
            elrs.send_event(Event::WindowEvent {
                window_id: RootWI(WindowId),
                event: WindowEvent::KeyboardInput {
                    device_id: RootDI(unsafe { DeviceId::dummy() }),
                    input: KeyboardInput {
                        scancode: scancode(&event),
                        state: ElementState::Pressed,
                        virtual_keycode: button_mapping(&event),
                        modifiers: keyboard_modifiers_state(&event),
                    },
                },
            });
        });
        add_event(&runner, document, |elrs, event: KeyUpEvent| {
            elrs.send_event(Event::WindowEvent {
                window_id: RootWI(WindowId),
                event: WindowEvent::KeyboardInput {
                    device_id: RootDI(unsafe { DeviceId::dummy() }),
                    input: KeyboardInput {
                        scancode: scancode(&event),
                        state: ElementState::Released,
                        virtual_keycode: button_mapping(&event),
                        modifiers: keyboard_modifiers_state(&event),
                    },
                },
            });
        });

        stdweb::event_loop(); // TODO: this is only necessary for stdweb emscripten, should it be here?

        // Throw an exception to break out of Rust exceution and use unreachable to tell the
        // compiler this function won't return, giving it a return type of '!'
        js! {
            throw "Using exceptions for control flow, don't mind me. This isn't actually an error!";
        }
        unreachable!();
    }

    pub fn create_proxy(&self) -> EventLoopProxy<T> {
        EventLoopProxy {
            runner: self.elw.p.runner.clone(),
        }
    }

    pub fn window_target(&self) -> &RootELW<T> {
        &self.elw
    }
}

pub fn register<T: 'static>(elrs: &EventLoopRunnerShared<T>, canvas: &CanvasElement) {
    add_event(elrs, canvas, |elrs, event: PointerOutEvent| {
        elrs.send_event(Event::WindowEvent {
            window_id: RootWI(WindowId),
            event: WindowEvent::CursorLeft {
                device_id: RootDI(DeviceId(event.pointer_id())),
            },
        });
    });
    add_event(elrs, canvas, |elrs, event: PointerOverEvent| {
        elrs.send_event(Event::WindowEvent {
            window_id: RootWI(WindowId),
            event: WindowEvent::CursorEntered {
                device_id: RootDI(DeviceId(event.pointer_id())),
            },
        });
    });
    add_event(elrs, canvas, |elrs, event: PointerMoveEvent| {
        elrs.send_event(Event::WindowEvent {
            window_id: RootWI(WindowId),
            event: WindowEvent::CursorMoved {
                device_id: RootDI(DeviceId(event.pointer_id())),
                position: LogicalPosition {
                    x: event.offset_x(),
                    y: event.offset_y(),
                },
                modifiers: mouse_modifiers_state(&event),
            },
        });
    });
    add_event(elrs, canvas, |elrs, event: PointerUpEvent| {
        elrs.send_event(Event::WindowEvent {
            window_id: RootWI(WindowId),
            event: WindowEvent::MouseInput {
                device_id: RootDI(DeviceId(event.pointer_id())),
                state: ElementState::Pressed,
                button: mouse_button(&event),
                modifiers: mouse_modifiers_state(&event),
            },
        });
    });
    add_event(elrs, canvas, |elrs, event: PointerDownEvent| {
        elrs.send_event(Event::WindowEvent {
            window_id: RootWI(WindowId),
            event: WindowEvent::MouseInput {
                device_id: RootDI(DeviceId(event.pointer_id())),
                state: ElementState::Released,
                button: mouse_button(&event),
                modifiers: mouse_modifiers_state(&event),
            },
        });
    });
    add_event(elrs, canvas, |elrs, event: MouseWheelEvent| {
        let x = event.delta_x();
        let y = event.delta_y();
        let delta = match event.delta_mode() {
            MouseWheelDeltaMode::Line => MouseScrollDelta::LineDelta(x as f32, y as f32),
            MouseWheelDeltaMode::Pixel => MouseScrollDelta::PixelDelta(LogicalPosition { x, y }),
            MouseWheelDeltaMode::Page => return,
        };
        elrs.send_event(Event::WindowEvent {
            window_id: RootWI(WindowId),
            event: WindowEvent::MouseWheel {
                device_id: RootDI(DeviceId(0)),
                delta,
                phase: TouchPhase::Moved,
                modifiers: mouse_modifiers_state(&event),
            },
        });
    });
}

fn add_event<T: 'static, E, F>(
    elrs: &EventLoopRunnerShared<T>,
    target: &impl IEventTarget,
    mut handler: F,
) where
    E: ConcreteEvent,
    F: FnMut(&EventLoopRunnerShared<T>, E) + 'static,
{
    let elrs = elrs.clone();

    target.add_event_listener(move |event: E| {
        // Don't capture the event if the events loop has been destroyed
        match &*elrs.0.runner.borrow() {
            Some(ref runner) if runner.control.is_exit() => return,
            _ => (),
        }

        event.prevent_default();
        event.stop_propagation();
        event.cancel_bubble();

        handler(&elrs, event);
    });
}

impl<T: 'static> EventLoopRunnerShared<T> {
    // Set the event callback to use for the event loop runner
    // This the event callback is a fairly thin layer over the user-provided callback that closes
    // over a RootEventLoopWindowTarget reference
    fn set_listener(&self, event_handler: Box<dyn FnMut(Event<T>, &mut ControlFlow)>) {
        *self.0.runner.borrow_mut() = Some(EventLoopRunner {
            control: ControlFlowStatus::Init,
            is_busy: false,
            event_handler,
        });
        self.send_event(Event::NewEvents(StartCause::Init));
    }

    // Add an event to the event loop runner
    //
    // It will determine if the event should be immediately sent to the user or buffered for later
    pub fn send_event(&self, event: Event<T>) {
        // If the event loop is closed, it should discard any new events
        if self.closed() {
            return;
        }

        // Determine if event handling is in process, and then release the borrow on the runner
        let (start_cause, event_is_start) = match *self.0.runner.borrow() {
            Some(ref runner) if !runner.is_busy => {
                if let Event::NewEvents(cause) = event {
                    (cause, true)
                } else {
                    (
                        match runner.control {
                            ControlFlowStatus::Init => StartCause::Init,
                            ControlFlowStatus::Poll { .. } => StartCause::Poll,
                            ControlFlowStatus::Wait { start } => StartCause::WaitCancelled {
                                start,
                                requested_resume: None,
                            },
                            ControlFlowStatus::WaitUntil { start, end, .. } => {
                                StartCause::WaitCancelled {
                                    start,
                                    requested_resume: Some(end),
                                }
                            }
                            ControlFlowStatus::Exit => {
                                return;
                            }
                        },
                        false,
                    )
                }
            }
            _ => {
                // Events are currently being handled, so queue this one and don't try to
                // double-process the event queue
                self.0.events.borrow_mut().push_back(event);
                return;
            }
        };
        let mut control = self.current_control_flow();
        // Handle starting a new batch of events
        //
        // The user is informed via Event::NewEvents that there is a batch of events to process
        // However, there is only one of these per batch of events
        self.handle_event(Event::NewEvents(start_cause), &mut control);
        if !event_is_start {
            self.handle_event(event, &mut control);
        }
        self.handle_event(Event::EventsCleared, &mut control);
        self.apply_control_flow(control);
        // If the event loop is closed, it has been closed this iteration and now the closing
        // event should be emitted
        if self.closed() {
            self.handle_event(Event::LoopDestroyed, &mut control);
        }
    }

    // handle_event takes in events and either queues them or applies a callback
    //
    // It should only ever be called from send_event
    fn handle_event(&self, event: Event<T>, control: &mut ControlFlow) {
        let closed = self.closed();

        match *self.0.runner.borrow_mut() {
            Some(ref mut runner) => {
                // An event is being processed, so the runner should be marked busy
                runner.is_busy = true;

                (runner.event_handler)(event, control);

                // Maintain closed state, even if the callback changes it
                if closed {
                    *control = ControlFlow::Exit;
                }

                // An event is no longer being processed
                runner.is_busy = false;
            }
            // If an event is being handled without a runner somehow, add it to the event queue so
            // it will eventually be processed
            _ => self.0.events.borrow_mut().push_back(event),
        }

        // Don't take events out of the queue if the loop is closed or the runner doesn't exist
        // If the runner doesn't exist and this method recurses, it will recurse infinitely
        if !closed && self.0.runner.borrow().is_some() {
            // Take an event out of the queue and handle it
            if let Some(event) = self.0.events.borrow_mut().pop_front() {
                self.handle_event(event, control);
            }
        }
    }

    // Apply the new ControlFlow that has been selected by the user
    // Start any necessary timeouts etc
    fn apply_control_flow(&self, control_flow: ControlFlow) {
        let mut control_flow_status = match control_flow {
            ControlFlow::Poll => {
                let cloned = self.clone();
                ControlFlowStatus::Poll {
                    timeout: window().set_clearable_timeout(
                        move || cloned.send_event(Event::NewEvents(StartCause::Poll)),
                        1,
                    ),
                }
            }
            ControlFlow::Wait => ControlFlowStatus::Wait {
                start: Instant::now(),
            },
            ControlFlow::WaitUntil(end) => {
                let cloned = self.clone();
                let start = Instant::now();
                let delay = if end <= start {
                    Duration::from_millis(0)
                } else {
                    end - start
                };
                ControlFlowStatus::WaitUntil {
                    start,
                    end,
                    timeout: window().set_clearable_timeout(
                        move || cloned.send_event(Event::NewEvents(StartCause::Poll)),
                        delay.as_millis() as u32,
                    ),
                }
            }
            ControlFlow::Exit => ControlFlowStatus::Exit,
        };

        match *self.0.runner.borrow_mut() {
            Some(ref mut runner) => {
                // Put the new control flow status in the runner, and take out the old one
                // This way we can safely take ownership of the TimeoutHandle and clear it,
                // so that we don't get 'ghost' invocations of Poll or WaitUntil from earlier
                // set_timeout invocations
                std::mem::swap(&mut runner.control, &mut control_flow_status);
                match control_flow_status {
                    ControlFlowStatus::Poll { timeout }
                    | ControlFlowStatus::WaitUntil { timeout, .. } => timeout.clear(),
                    _ => (),
                }
            }
            None => (),
        }
    }

    // Check if the event loop is currntly closed
    fn closed(&self) -> bool {
        match *self.0.runner.borrow() {
            Some(ref runner) => runner.control.is_exit(),
            None => false, // If the event loop is None, it has not been intialised yet, so it cannot be closed
        }
    }

    // Get the current control flow state
    fn current_control_flow(&self) -> ControlFlow {
        match *self.0.runner.borrow() {
            Some(ref runner) => runner.control.to_control_flow(),
            None => ControlFlow::Poll,
        }
    }
}
