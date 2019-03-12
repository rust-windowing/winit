use super::*;

use dpi::{LogicalPosition, LogicalSize};
use event::{DeviceEvent, DeviceId as RootDI, ElementState, Event, KeyboardInput, ModifiersState, MouseButton, ScanCode, StartCause, VirtualKeyCode, WindowEvent};
use event_loop::{ControlFlow, EventLoopWindowTarget as RootELW, EventLoopClosed};
use icon::Icon;
use window::{MouseCursor, WindowId as RootWI};
use stdweb::{
    JsSerialize,
    traits::*,
    unstable::TryInto,
    web::{
        document,
        event::*,
        html_element::CanvasElement,
    },
};
use std::cell::{RefCell, RefMut};
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
    runner: EventLoopRunnerShared<T>
}

pub struct EventLoopWindowTarget<T: 'static> {
    pub(crate) canvases: RefCell<Vec<CanvasElement>>,
    _marker: PhantomData<T>
}

impl<T> EventLoopWindowTarget<T> {
    fn new() -> Self {
        EventLoopWindowTarget {
            canvases: RefCell::new(Vec::new()),
            _marker: PhantomData
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

type EventLoopRunnerShared<T> = Rc<ELRShared<T>>;

struct ELRShared<T> {
    runner: RefCell<Option<EventLoopRunner<T>>>,
    events: RefCell<VecDeque<Event<T>>>, // TODO: this may not be necessary?
}

struct EventLoopRunner<T> {
    control: ControlFlow,
    event_handler: Box<dyn FnMut(Event<T>, &mut ControlFlow)>,
}

impl<T> EventLoop<T> {
    pub fn new() -> Self {
        EventLoop {
            elw: RootELW {
                p: EventLoopWindowTarget::new(),
                _marker: PhantomData
            },
            runner: Rc::new(ELRShared::blank()),
        }
    }

    pub fn get_available_monitors(&self) -> VecDequeIter<MonitorHandle> {
        VecDeque::new().into_iter()
    }

    pub fn get_primary_monitor(&self) -> MonitorHandle {
        MonitorHandle
    }

    pub fn run<F>(mut self, mut event_handler: F) -> !
        where F: 'static + FnMut(Event<T>, &RootELW<T>, &mut ControlFlow)
    {
        // TODO: Create event handlers for the JS events
        // TODO: how to handle request redraw?
        // TODO: onclose (stdweb PR)
        // TODO: file dropping, PathBuf isn't useful for web
        let EventLoop { elw, runner } = self;
        for canvas in elw.p.canvases.borrow().iter() {
            register(&runner, canvas);
        }
        let relw = RootELW {
            p: EventLoopWindowTarget::new(),
            _marker: PhantomData
        };
        runner.set_listener(Box::new(move |evt, ctrl| event_handler(evt, &relw, ctrl)));

        let document = &document();
        add_event(&runner, document, |_, _: BlurEvent| {
        });
        add_event(&runner, document, |_, _: FocusEvent| {

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
                    // TODO: is there a way to get keyboard device?
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
                    // TODO: is there a way to get keyboard device?
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
        js! {
            throw "Using exceptions for control flow, don't mind me";
        }
        unreachable!();
    }

    pub fn create_proxy(&self) -> EventLoopProxy<T> {
        EventLoopProxy {
            runner: self.runner.clone()
        }
    }

    pub fn window_target(&self) -> &RootELW<T> {
        &self.elw
    }
}

fn register<T: 'static>(elrs: &EventLoopRunnerShared<T>, canvas: &CanvasElement) {
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
    let elrs = elrs.clone(); // TODO: necessary?
    
    target.add_event_listener(move |event: E| {
        event.prevent_default();
        event.stop_propagation();
        event.cancel_bubble();

        handler(&elrs, event);
    });
}

impl<T> ELRShared<T> {
    fn blank() -> ELRShared<T> {
        ELRShared {
            runner: RefCell::new(None),
            events: RefCell::new(VecDeque::new())
        }
    }

    fn set_listener(&self, event_handler: Box<dyn FnMut(Event<T>, &mut ControlFlow)>) {
        *self.runner.borrow_mut() = Some(EventLoopRunner {
            control: ControlFlow::Poll,
            event_handler
        });
    }

    // TODO: handle event loop closures
    // TODO: handle event buffer
    fn send_event(&self, event: Event<T>) {
        match *self.runner.borrow_mut() {
            Some(ref mut runner) =>  {
                // TODO: bracket this in control flow events?
                (runner.event_handler)(event, &mut runner.control);
            }
            None => ()
        }
    }

}

