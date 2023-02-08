//! Handles external, user-defined window messages.

use super::super::super::window::set_skip_taskbar;
use super::super::{DESTROY_MSG_ID, SET_RETAIN_STATE_ON_SIZE_MSG_ID, TASKBAR_CREATED};
use super::prelude::*;

fn handle_destroy_msg(
    window: HWND,
    _: u32,
    _: WPARAM,
    _: LPARAM,
    _: &dyn GenericWindowData,
) -> LRESULT {
    unsafe {
        DestroyWindow(window);
    }

    0
}

inventory::submit! {
    WindowMessage::from_user(
        &DESTROY_MSG_ID,
        handle_destroy_msg
    )
}

fn handle_retain_state(
    _: HWND,
    _: u32,
    wparam: WPARAM,
    _: LPARAM,
    userdata: &dyn GenericWindowData,
) -> LRESULT {
    let mut window_state = userdata.window_state_lock();
    window_state.set_window_flags_in_place(|f| {
        f.set(WindowFlags::MARKER_RETAIN_STATE_ON_SIZE, wparam != 0)
    });
    0
}

inventory::submit! {
    WindowMessage::from_user(
        &SET_RETAIN_STATE_ON_SIZE_MSG_ID,
        handle_retain_state
    )
}

fn handle_taskbar_created(
    window: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
    userdata: &dyn GenericWindowData,
) -> LRESULT {
    unsafe {
        let window_state = userdata.window_state_lock();
        set_skip_taskbar(window, window_state.skip_taskbar);
        DefWindowProcW(window, msg, wparam, lparam)
    }
}

inventory::submit! {
    WindowMessage::from_user(
        &TASKBAR_CREATED,
        handle_taskbar_created
    )
}
