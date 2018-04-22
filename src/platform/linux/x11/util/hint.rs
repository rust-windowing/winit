use super::*;

pub const MWM_HINTS_DECORATIONS: c_ulong = 2;

#[derive(Debug)]
pub enum StateOperation {
    Remove = 0, // _NET_WM_STATE_REMOVE
    Add = 1,    // _NET_WM_STATE_ADD
    _Toggle = 2, // _NET_WM_STATE_TOGGLE
}

impl From<bool> for StateOperation {
    fn from(b: bool) -> Self {
        if b {
            StateOperation::Add
        } else {
            StateOperation::Remove
        }
    }
}
