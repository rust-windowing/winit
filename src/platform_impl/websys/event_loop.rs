use ::event_loop::{ControlFlow, EventLoopClosed};
use ::event_loop::EventLoopWindowTarget as WinitELT;
use ::event::{Event, StartCause};
use super::window::{MonitorHandle, WindowId};
#[macro_use]
use platform_impl::platform::wasm_util as util;

use std::collections::VecDeque;
use std::rc::Rc;
use std::cell::{Cell, RefCell};

use ::wasm_bindgen::prelude::*;
use ::wasm_bindgen::JsCast;
use ::web_sys::{HtmlCanvasElement};

#[derive(Clone, Copy, Eq, PartialEq)]
enum EventLoopState {
    Sleeping,
    Waking,
    Polling
}

impl Default for EventLoopState {
    #[inline(always)]
    fn default() -> Self {
        EventLoopState::Polling
    }
}

pub struct EventLoopWindowTarget<T: 'static> {
    pub(crate) window_events: Rc<RefCell<Vec<::event::WindowEvent>>>,
    pub(crate) internal: Rc<EventLoopInternal>,
    _marker: std::marker::PhantomData<T>
}

impl<T> EventLoopWindowTarget<T> {
    pub fn events(&self) -> Vec<::event::WindowEvent> {
        self.window_events.replace(Vec::new())
    }


    pub fn setup_window(&self, element: &HtmlCanvasElement) {
        let events = self.window_events.clone();
        let internal = self.internal.clone();
        let handler = Closure::wrap(Box::new(move |event: ::web_sys::MouseEvent| {
            events.borrow_mut().push(event.into());
            if internal.is_sleeping() {
                internal.wake();
            }
        }) as Box<FnMut(::web_sys::MouseEvent)>);
        element.set_onmousedown(Some(handler.as_ref().unchecked_ref()));
        handler.forget();
    }
}

pub struct EventLoop<T: 'static> {
    window_target: WinitELT<T>,
}

pub(crate) struct EventLoopInternal {
    loop_fn: Rc<RefCell<Option<Closure<FnMut()>>>>,
    state: Cell<EventLoopState>,
}

impl EventLoopInternal {
    pub(crate) fn sleep(&self) {
        self.state.set(EventLoopState::Sleeping);
    }

    pub(crate) fn wake(&self) {
        self.state.set(EventLoopState::Waking);
        let window = web_sys::window().expect("should be a window");
        // TODO: call this directly?
        window.request_animation_frame(self.loop_fn.borrow().as_ref().unwrap().as_ref().unchecked_ref());
    }

    pub(crate) fn is_sleeping(&self) -> bool {
        self.state.get() == EventLoopState::Sleeping
    }
}

impl<T: 'static> EventLoop<T> {
    pub fn new() -> EventLoop<T> {
        let loop_fn: Rc<RefCell<Option<Closure<FnMut()>>>> = Rc::new(RefCell::new(None));
        EventLoop { 
            window_target: WinitELT { 
                p: EventLoopWindowTarget {
                    window_events: Rc::new(RefCell::new(Vec::new())),
                    internal: Rc::new(EventLoopInternal {
                        state: Cell::default(),
                        loop_fn,
                    }),
                    _marker: std::marker::PhantomData 
                },
                _marker: std::marker::PhantomData 
            },
        }
    }

    pub fn create_proxy(&self) -> EventLoopProxy<T> {
        EventLoopProxy { _marker: std::marker::PhantomData }
    }

    #[inline]
    pub fn available_monitors(&self) -> VecDeque<MonitorHandle> {
        vec!(MonitorHandle{}).into_iter().collect()
    }

    #[inline]
    pub fn primary_monitor(&self) -> MonitorHandle {
        MonitorHandle{}
    }

    #[inline]
    pub fn run<F>(self, event_handler: F) -> !
        where F: 'static + FnMut(Event<T>, &WinitELT<T>, &mut ControlFlow)
    {
        self.run_return(event_handler);
        log!("exiting");

        util::js_exit();
        unreachable!()
    }

    fn run_return<F>(self, mut event_handler: F)
        where F: 'static + FnMut(Event<T>, &WinitELT<T>, &mut ControlFlow)
    {
        let mut control_flow = ControlFlow::default();

        let g = self.window_target.p.internal.loop_fn.clone();

        event_handler(Event::NewEvents(StartCause::Init), &self.window_target, &mut control_flow);
        
        *g.borrow_mut() = Some(Closure::wrap(Box::new(move || {
            match control_flow {
                ControlFlow::Poll => {
                    log!("Starting poll!!!");
                    let mut win_events = self.window_target.p.events();
                    win_events.drain(..).for_each(|e| {
                        event_handler(Event::WindowEvent{window_id: ::window::WindowId(WindowId{}), event: e}, &self.window_target, &mut control_flow);
                    });

                    event_handler(Event::NewEvents(StartCause::Poll), &self.window_target, &mut control_flow);

                    let window = web_sys::window().expect("should be a window");
                    window.request_animation_frame(self.window_target.p.internal.loop_fn.borrow().as_ref().unwrap().as_ref().unchecked_ref());
                },
                ControlFlow::Wait => {
                    let mut win_events = self.window_target.p.events();
                    win_events.drain(..).for_each(|e| {
                        event_handler(Event::WindowEvent{window_id: ::window::WindowId(WindowId{}), event: e}, &self.window_target, &mut control_flow);
                    });
                    self.window_target.p.internal.sleep();
                    event_handler(Event::Suspended(true), &self.window_target, &mut control_flow);
                },
                _ => {
                    unreachable!();
                }
            }         
        }) as Box<FnMut()>));
        let window = web_sys::window().expect("should be a window");
        window.request_animation_frame(g.borrow().as_ref().unwrap().as_ref().unchecked_ref());
    }


    pub fn window_target(&self) -> &WinitELT<T> {
        &self.window_target
    }

}

#[derive(Clone)]
pub struct EventLoopProxy<T: 'static> {
    _marker: std::marker::PhantomData<T>
}

impl<T: 'static> EventLoopProxy<T> {
    pub fn send_event(&self, event: T) -> Result<(), EventLoopClosed> {
        unimplemented!()
    }
}
