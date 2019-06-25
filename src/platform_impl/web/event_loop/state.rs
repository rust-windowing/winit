use super::backend;
use crate::event_loop::ControlFlow;

use instant::Instant;

#[derive(Debug, Clone, Copy)]
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
        timeout: backend::Timeout,
    },
    Exit,
}

impl State {
    pub fn is_exit(&self) -> bool {
        match self {
            State::Exit => true,
            _ => false,
        }
    }
}

impl From<State> for ControlFlow {
    fn from(state: State) -> ControlFlow {
        match state {
            State::Init => ControlFlow::Poll,
            State::WaitUntil { end, .. } => ControlFlow::WaitUntil(end),
            State::Wait { .. } => ControlFlow::Wait,
            State::Poll { .. } => ControlFlow::Poll,
            State::Exit => ControlFlow::Exit,
        }
    }
}
