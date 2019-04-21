extern crate web_sys;
extern crate wasm_bindgen;

use event_loop::{ControlFlow, EventLoopClosed};
use event::Event;
use super::window::{MonitorHandle, Window, WindowInternal};

use std::collections::VecDeque;
use std::rc::Rc;
use std::cell::RefCell;

use self::wasm_bindgen::prelude::*;
use self::wasm_bindgen::JsCast;
use self::web_sys::Element;

// A macro to provide `println!(..)`-style syntax for `console.log` logging.
macro_rules! log {
    ( $( $t:tt )* ) => {
        web_sys::console::log_1(&format!( $( $t )* ).into());
    }
}

#[wasm_bindgen(inline_js = "export function js_exit() { throw 'hacky exit!'; }")]
extern "C" {
    fn js_exit();
}

pub struct EventLoop<T: 'static> {
    window_target: ::event_loop::EventLoopWindowTarget<T>
}

impl<T: 'static> EventLoop<T> {
    pub fn new() -> EventLoop<T> {
        EventLoop { 
            window_target: ::event_loop::EventLoopWindowTarget { 
                p: EventLoopWindowTarget {
                    window: RefCell::new(None),
                    _marker: std::marker::PhantomData 
                },
                _marker: std::marker::PhantomData 
            } 
        }
    }

    pub fn create_proxy(&self) -> EventLoopProxy<T> {
        EventLoopProxy { _marker: std::marker::PhantomData }
    }

    #[inline]
    pub fn get_available_monitors(&self) -> VecDeque<MonitorHandle> {
        vec!(MonitorHandle{}).into_iter().collect()
    }

    #[inline]
    pub fn get_primary_monitor(&self) -> MonitorHandle {
        MonitorHandle{}
    }

    /// Hijacks the calling thread and initializes the `winit` event loop with the provided
    /// closure. Since the closure is `'static`, it must be a `move` closure if it needs to
    /// access any data from the calling context.
    ///
    /// See the [`ControlFlow`] docs for information on how changes to `&mut ControlFlow` impact the
    /// event loop's behavior.
    ///
    /// Any values not passed to this function will *not* be dropped.
    ///
    /// [`ControlFlow`]: ./enum.ControlFlow.html
    #[inline]
    pub fn run<F>(self, event_handler: F) -> !
        where F: 'static + FnMut(Event<T>, &::event_loop::EventLoopWindowTarget<T>, &mut ControlFlow)
    {
        self.run_return(event_handler);
        log!("exiting");
        js_exit();
        unreachable!()
    }

    fn run_return<F>(self, mut event_handler: F)
        where F: 'static + FnMut(Event<T>, &::event_loop::EventLoopWindowTarget<T>, &mut ControlFlow)
    {
        let mut control_flow = ControlFlow::default();

        let f: Rc<RefCell<Option<Closure<FnMut()>>>> = Rc::new(RefCell::new(None));
        let g = f.clone();

        event_handler(::event::Event::NewEvents(::event::StartCause::Init), &self.window_target, &mut control_flow);
        
        *g.borrow_mut() = Some(Closure::wrap(Box::new(move || {
            if control_flow == ControlFlow::Poll {
                event_handler(::event::Event::NewEvents(::event::StartCause::Poll), &self.window_target, &mut control_flow);
            }

            let window = web_sys::window().expect("should be a window");
            window.request_animation_frame(f.borrow().as_ref().unwrap().as_ref().unchecked_ref());
        }) as Box<FnMut()>));
        let window = web_sys::window().expect("should be a window");
        window.request_animation_frame(g.borrow().as_ref().unwrap().as_ref().unchecked_ref());
    }


    pub fn window_target(&self) -> &::event_loop::EventLoopWindowTarget<T> {
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

pub struct EventLoopWindowTarget<T: 'static> {
    window: RefCell<Option<Rc<WindowInternal>>>,
    _marker: std::marker::PhantomData<T>
}

impl<T> EventLoopWindowTarget<T> {
    pub(crate) fn set_window(&self, window: Rc<WindowInternal>) {
        self.window.borrow_mut().replace(window.clone());
    }

    pub(crate) fn window(&self) -> Rc<WindowInternal> {
        self.window.borrow().as_ref().map(|w| w.clone()).unwrap()
    }
}
