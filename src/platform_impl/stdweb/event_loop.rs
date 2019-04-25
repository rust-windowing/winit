use super::*;

use dpi::LogicalPosition;
use event::{DeviceId as RootDI, ElementState, Event, KeyboardInput, StartCause, WindowEvent};
use event_loop::{ControlFlow, EventLoopWindowTarget as RootELW, EventLoopClosed};
use window::{WindowId as RootWI};
use stdweb::{
    traits::*,
    web::{
        document,
        event::*,
        html_element::CanvasElement,
    },
};
use std::cell::RefCell;
use std::collections::VecDeque;
use std::collections::vec_deque::IntoIter as VecDequeIter;
use std::marker::PhantomData;
use std::rc::Rc;

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
            runner: Rc::new(ELRShared {
                runner: RefCell::new(None),
                events: RefCell::new(VecDeque::new())
            })
        }
    }
}

#[derive(Clone)]
pub struct EventLoopProxy<T> {
    runner: EventLoopRunnerShared<T>
}

impl<T> EventLoopProxy<T> {
    pub fn send_event(&self, event: T) -> Result<(), EventLoopClosed> {
        self.runner.send_event(Event::UserEvent(event));
        Ok(())
    }
}

pub type EventLoopRunnerShared<T> = Rc<ELRShared<T>>;

pub struct ELRShared<T> {
    runner: RefCell<Option<EventLoopRunner<T>>>,
    events: RefCell<VecDeque<Event<T>>>, // TODO: this may not be necessary?
}

struct EventLoopRunner<T> {
    control: ControlFlow,
    is_busy: bool,
    event_handler: Box<dyn FnMut(Event<T>, &mut ControlFlow)>,
}

impl<T> EventLoop<T> {
    pub fn new() -> Self {
        EventLoop {
            elw: RootELW {
                p: EventLoopWindowTarget::new(),
                _marker: PhantomData
            },
        }
    }

    pub fn get_available_monitors(&self) -> VecDequeIter<MonitorHandle> {
        VecDeque::new().into_iter()
    }

    pub fn get_primary_monitor(&self) -> MonitorHandle {
        MonitorHandle
    }

    pub fn run<F>(self, mut event_handler: F) -> !
        where F: 'static + FnMut(Event<T>, &RootELW<T>, &mut ControlFlow) {
        let runner = self.elw.p.runner;

        let relw = RootELW {
            p: EventLoopWindowTarget::new(),
            _marker: PhantomData
        };
        runner.set_listener(Box::new(move |evt, ctrl| event_handler(evt, &relw, ctrl)));

        let document = &document();
        add_event(&runner, document, |elrs, _: BlurEvent| {
            elrs.send_event(Event::WindowEvent {
                window_id: RootWI(WindowId),
                event: WindowEvent::Focused(false)
            });
        });
        add_event(&runner, document, |elrs, _: FocusEvent| {
            elrs.send_event(Event::WindowEvent {
                window_id: RootWI(WindowId),
                event: WindowEvent::Focused(true)
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
                    event: WindowEvent::ReceivedCharacter(key)
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
                    }
                }
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
                    }
                }
            });
        });
        stdweb::event_loop(); // TODO: this is only necessary for stdweb emscripten, should it be here?

        // Throw an exception to break out of Rust exceution and use unreachable to tell the
        // compiler this function won't return, giving it a return type of '!'
        js! {
            throw "Using exceptions for control flow, don't mind me";
        }
        unreachable!();
    }

    pub fn create_proxy(&self) -> EventLoopProxy<T> {
        EventLoopProxy {
            runner: self.elw.p.runner.clone()
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
                device_id: RootDI(DeviceId(event.pointer_id()))
            }
        });
    });
    add_event(elrs, canvas, |elrs, event: PointerOverEvent| {
        elrs.send_event(Event::WindowEvent {
            window_id: RootWI(WindowId),
            event: WindowEvent::CursorEntered {
                device_id: RootDI(DeviceId(event.pointer_id()))
            }
        });
    });
    add_event(elrs, canvas, |elrs, event: PointerMoveEvent| {
        elrs.send_event(Event::WindowEvent {
            window_id: RootWI(WindowId),
            event: WindowEvent::CursorMoved {
                device_id: RootDI(DeviceId(event.pointer_id())),
                position: LogicalPosition {
                    x: event.offset_x(),
                    y: event.offset_y()
                },
                modifiers: mouse_modifiers_state(&event)
            }
        });
    });
    add_event(elrs, canvas, |elrs, event: PointerUpEvent| {
        elrs.send_event(Event::WindowEvent {
            window_id: RootWI(WindowId),
            event: WindowEvent::MouseInput {
                device_id: RootDI(DeviceId(event.pointer_id())),
                state: ElementState::Pressed,
                button: mouse_button(&event),
                modifiers: mouse_modifiers_state(&event)
            }
        });
    });
    add_event(elrs, canvas, |elrs, event: PointerDownEvent| {
        elrs.send_event(Event::WindowEvent {
            window_id: RootWI(WindowId),
            event: WindowEvent::MouseInput {
                device_id: RootDI(DeviceId(event.pointer_id())),
                state: ElementState::Released,
                button: mouse_button(&event),
                modifiers: mouse_modifiers_state(&event)
            }
        });
    });
}

fn add_event<T: 'static, E, F>(elrs: &EventLoopRunnerShared<T>, target: &impl IEventTarget, mut handler: F) 
        where E: ConcreteEvent, F: FnMut(&EventLoopRunnerShared<T>, E) + 'static {
    let elrs = elrs.clone();
    
    target.add_event_listener(move |event: E| {
        // Don't capture the event if the events loop has been destroyed
        match &*elrs.runner.borrow() {
            Some(ref runner) if runner.control == ControlFlow::Exit => return,
            _ => ()
        }

        event.prevent_default();
        event.stop_propagation();
        event.cancel_bubble();

        handler(&elrs, event);
    });
}

impl<T> ELRShared<T> {
    // Set the event callback to use for the event loop runner
    // This the event callback is a fairly thin layer over the user-provided callback that closes
    // over a RootEventLoopWindowTarget reference
    fn set_listener(&self, event_handler: Box<dyn FnMut(Event<T>, &mut ControlFlow)>) {
        *self.runner.borrow_mut() = Some(EventLoopRunner {
            control: ControlFlow::Poll,
            is_busy: false,
            event_handler
        });
    }

    // Add an event to the event loop runner
    //
    // It will determine if the event should be immediately sent to the user or buffered for later
    pub fn send_event(&self, event: Event<T>) {
        // If the event loop is closed, it should discard any new events
        if self.closed() {
            return;
        }

        let start_cause = StartCause::Poll; // TODO: determine start cause

        // Determine if event handling is in process, and then release the borrow on the runner
        let is_busy = if let Some(ref runner) = *self.runner.borrow() {
            runner.is_busy
        } else {
            true // If there is no event runner yet, then there's no point in processing events
        };

        if is_busy {
            self.events.borrow_mut().push_back(event);
        } else {
            // Handle starting a new batch of events
            //
            // The user is informed via Event::NewEvents that there is a batch of events to process
            // However, there is only one of these per batch of events
            self.handle_event(Event::NewEvents(start_cause));
            self.handle_event(event);
            self.handle_event(Event::EventsCleared);

            // If the event loop is closed, it has been closed this iteration and now the closing
            // event should be emitted
            if self.closed() {
                self.handle_event(Event::LoopDestroyed);
            }
        }
    }

    // Check if the event loop is currntly closed
    fn closed(&self) -> bool {
        match *self.runner.borrow() {
            Some(ref runner) => runner.control == ControlFlow::Exit,
            None => false, // If the event loop is None, it has not been intialised yet, so it cannot be closed
        }
    }

    // handle_event takes in events and either queues them or applies a callback
    //
    // It should only ever be called from send_event
    fn handle_event(&self, event: Event<T>) {
        let closed = self.closed();

        match *self.runner.borrow_mut() {
            Some(ref mut runner) => {
                // An event is being processed, so the runner should be marked busy
                runner.is_busy = true;

                // TODO: bracket this in control flow events?
                (runner.event_handler)(event, &mut runner.control);

                // Maintain closed state, even if the callback changes it
                if closed {
                    runner.control = ControlFlow::Exit;
                }

                // An event is no longer being processed
                runner.is_busy = false;
            }
            // If an event is being handled without a runner somehow, add it to the event queue so
            // it will eventually be processed
            _ => self.events.borrow_mut().push_back(event)
        }

        // Don't take events out of the queue if the loop is closed or the runner doesn't exist
        // If the runner doesn't exist and this method recurses, it will recurse infinitely
        if !closed && self.runner.borrow().is_some() {
            // Take an event out of the queue and handle it
            if let Some(event) = self.events.borrow_mut().pop_front() {
                self.handle_event(event);
            }
        }
    }

}

