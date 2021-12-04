use crate::event::Event;
use crate::eventstream::EventStream;
use std::collections::VecDeque;
use std::future::Future;
use std::pin::Pin;

#[derive(Default)]
pub struct EventStash {
    events: VecDeque<Event>,
}

struct Consume<'a> {
    events: &'a mut VecDeque<Event>,
}

impl<'a> EventStream for Consume<'a> {
    fn event<'b>(&'b mut self) -> Pin<Box<dyn Future<Output = Event> + 'b>> {
        if let Some(event) = self.events.pop_front() {
            Box::pin(std::future::ready(event))
        } else {
            panic!("Stash is empty");
        }
    }

    fn has_more(&self) -> bool {
        !self.events.is_empty()
    }
}

struct Stash<'a> {
    events: &'a mut VecDeque<Event>,
    el: &'a mut dyn EventStream,
}

impl<'a> EventStream for Stash<'a> {
    fn event<'d>(&'d mut self) -> Pin<Box<dyn Future<Output = Event> + 'd>> {
        Box::pin(async {
            let event = self.el.event().await;
            self.events.push_back(event.clone());
            event
        })
    }
}

impl EventStash {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn stash<'a>(&'a mut self, el: &'a mut dyn EventStream) -> Box<dyn EventStream + 'a> {
        Box::new(Stash {
            events: &mut self.events,
            el,
        })
    }

    pub fn consume<'a>(&'a mut self) -> Box<dyn EventStream + 'a> {
        Box::new(Consume {
            events: &mut self.events,
        })
    }
}
