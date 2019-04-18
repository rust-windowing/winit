use {event_loop::EventLoopClosed};

pub struct EventLoop<T: 'static> {
    pending_events: Vec<T>
}

impl<T: 'static> EventLoop<T> {
    pub fn new() -> EventLoop<T> {
        EventLoop { pending_events: Vec::new() }
    }

    pub fn create_proxy(&self) -> EventLoopProxy<T> {
        EventLoopProxy { _marker: std::marker::PhantomData }
    }
}

pub struct EventLoopProxy<T: 'static> {
    _marker: std::marker::PhantomData<T>
}

impl<T: 'static> EventLoopProxy<T> {
    pub fn send_event(&self, event: T) -> Result<(), EventLoopClosed> {
        !unimplemented!()
    }
}

pub struct EventLoopWindowTarget<T: 'static> {
    _marker: std::marker::PhantomData<T>
}
