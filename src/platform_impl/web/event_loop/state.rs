use super::backend;

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
}
