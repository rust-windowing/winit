extern crate web_sys;

use event_loop::{ControlFlow, EventLoopClosed};
use event::Event;
use super::window::{MonitorHandle, Window};

use std::collections::VecDeque;

use self::web_sys::Element;

// A macro to provide `println!(..)`-style syntax for `console.log` logging.
macro_rules! log {
    ( $( $t:tt )* ) => {
        web_sys::console::log_1(&format!( $( $t )* ).into());
    }
}

pub struct EventLoop<T: 'static> {
    pending_events: Vec<T>,
    window_target: ::event_loop::EventLoopWindowTarget<T>
}

impl<T: 'static> EventLoop<T> {
    pub fn new() -> EventLoop<T> {
        let window = web_sys::window().expect("no global `window` exists");
        let document = window.document().expect("should have a document on window");

        let element = document.get_element_by_id("test").expect("no canvas");
        EventLoop { 
            pending_events: Vec::new(),
            window_target: ::event_loop::EventLoopWindowTarget { 
                p: EventLoopWindowTarget {
                    element, 
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
        std::process::exit(0);
    }

    fn run_return<F>(&self, mut event_handler: F)
        where F: 'static + FnMut(Event<T>, &::event_loop::EventLoopWindowTarget<T>, &mut ControlFlow)
    {
        let mut control_flow = ControlFlow::default();

        event_handler(::event::Event::NewEvents(::event::StartCause::Init), &self.window_target, &mut control_flow);
        loop {
            match control_flow {
                ControlFlow::Poll => {
                    event_handler(::event::Event::NewEvents(::event::StartCause::Poll), &self.window_target, &mut control_flow);
                },
                ControlFlow::Wait => {
                },
                ControlFlow::WaitUntil(Instant) => {
                },
                ControlFlow::Exit => break
            }
        }
        event_handler(::event::Event::LoopDestroyed, &self.window_target, &mut control_flow);
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
    element: Element,
    _marker: std::marker::PhantomData<T>
}
