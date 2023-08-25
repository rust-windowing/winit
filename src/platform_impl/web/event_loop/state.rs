use super::backend;
use crate::event_loop::ControlFlow;

use web_time::Instant;

#[derive(Debug)]
pub enum State {
    Init,
    WaitUntil {
        timeout: backend::Schedule,
        start: Instant,
        end: Instant,
    },
    Wait {
        start: Instant,
    },
    Poll {
        request: backend::Schedule,
    },
    Exit,
}

impl State {
    pub fn is_exit(&self) -> bool {
        matches!(self, State::Exit)
    }

    pub fn control_flow(&self) -> ControlFlow {
        match self {
            State::Init => ControlFlow::Poll,
            State::WaitUntil { end, .. } => ControlFlow::WaitUntil(*end),
            State::Wait { .. } => ControlFlow::Wait,
            State::Poll { .. } => ControlFlow::Poll,
            State::Exit => ControlFlow::Exit,
        }
    }
}
