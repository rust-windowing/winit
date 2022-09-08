use super::backend;
use crate::event_loop::ControlFlow;

use instant::Instant;

#[derive(Debug)]
pub enum State {
    Init,
    WaitUntil {
        timeout: backend::Timeout,
        start: Instant,
        end: Instant,
    },
    Wait {
        start: Instant,
    },
    Poll {
        request: backend::AnimationFrameRequest,
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
