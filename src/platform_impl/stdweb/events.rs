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
}

#[derive(Clone)]
struct EventLoopData<T> {
    events: VecDeque<Event<T>>,
    control: ControlFlow,
}

pub struct EventLoopWindowTarget<T: 'static> {
    data: Rc<RefCell<EventLoopData<T>>>,
}

impl<T> EventLoop<T> {
    pub fn new() -> Self {
        EventLoop {
            elw: RootELW {
                p: EventLoopWindowTarget {
                    data: Rc::new(RefCell::new(EventLoopData {
                        events: VecDeque::new(),
                        control: ControlFlow::Poll
                    }))
                },
                _marker: PhantomData
            }
        }
    }

    pub fn get_available_monitors(&self) -> VecDequeIter<MonitorHandle> {
        VecDeque::new().into_iter()
    }

    pub fn get_primary_monitor(&self) -> MonitorHandle {
        MonitorHandle
    }

    pub fn run<F>(mut self, event_handler: F)
        where F: 'static + FnMut(Event<T>, &RootELW<T>, &mut ControlFlow)
    {
        // TODO: Create event handlers for the JS events
        // TODO: how to handle request redraw?
        // TODO: onclose (stdweb PR)
        // TODO: file dropping, PathBuf isn't useful for web

        let document = &document();
        self.elw.p.add_event(document, |mut data, event: BlurEvent| {
        });
        self.elw.p.add_event(document, |mut data, event: FocusEvent| {
        });

        stdweb::event_loop(); // TODO: this is only necessary for stdweb emscripten, should it be here?
    }

    pub fn create_proxy(&self) -> EventLoopProxy<T> {
        EventLoopProxy {
            data: self.elw.p.data.clone()
        }
    }

    pub fn window_target(&self) -> &RootELW<T> {
        &self.elw
    }
}

impl<T> EventLoopWindowTarget<T> {
    pub fn register_window(&self, other: &Window) {
        let canvas = &other.canvas;
        
        self.add_event(canvas, |mut data, event: KeyDownEvent| {
            let key = event.key();
            let mut characters = key.chars();
            let first = characters.next();
            let second = characters.next();
            if let (Some(key), None) = (first, second) {
                data.events.push_back(Event::WindowEvent {
                    window_id: RootWI(WindowId),
                    event: WindowEvent::ReceivedCharacter(key)
                });
            }
            data.events.push_back(Event::WindowEvent {
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
        self.add_event(canvas, |mut data, event: KeyUpEvent| {
            data.events.push_back(Event::WindowEvent {
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
        self.add_event(canvas, |mut data, event: PointerOutEvent| {
            data.events.push_back(Event::WindowEvent {
                window_id: RootWI(WindowId),
                event: WindowEvent::CursorLeft {
                    device_id: RootDI(DeviceId(event.pointer_id()))
                }
            });
        });
        self.add_event(canvas, |mut data, event: PointerOverEvent| {
            data.events.push_back(Event::WindowEvent {
                window_id: RootWI(WindowId),
                event: WindowEvent::CursorEntered {
                    device_id: RootDI(DeviceId(event.pointer_id()))
                }
            });
        });
        self.add_event(canvas, |mut data, event: PointerMoveEvent| {
            data.events.push_back(Event::WindowEvent {
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
        self.add_event(canvas, |mut data, event: PointerUpEvent| {
            data.events.push_back(Event::WindowEvent {
                window_id: RootWI(WindowId),
                event: WindowEvent::MouseInput {
                    device_id: RootDI(DeviceId(event.pointer_id())),
                    state: ElementState::Pressed,
                    button: mouse_button(&event),
                    modifiers: mouse_modifiers_state(&event)
                }
            });
        });
        self.add_event(canvas, |mut data, event: PointerDownEvent| {
            data.events.push_back(Event::WindowEvent {
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


    fn add_event<E, F>(&self, target: &impl IEventTarget, mut handler: F) 
            where E: ConcreteEvent, F: FnMut(RefMut<EventLoopData<T>>, E) + 'static {
        let data = self.data.clone();

        target.add_event_listener(move |event: E| {
            event.prevent_default();
            event.stop_propagation();
            event.cancel_bubble();

            handler(data.borrow_mut(), event);
        });
    }
}

#[derive(Clone)]
pub struct EventLoopProxy<T> {
    data: Rc<RefCell<EventLoopData<T>>>
}

impl<T> EventLoopProxy<T> {
    pub fn send_event(&self, event: T) -> Result<(), EventLoopClosed> {
        self.data.borrow_mut().events.push_back(Event::UserEvent(event));
        Ok(())
    }
}


