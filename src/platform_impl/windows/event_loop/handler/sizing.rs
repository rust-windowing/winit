//! Handles resizing/moving events.

use super::prelude::*;

fn handle_nccalcsize(
    window: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
    userdata: &dyn GenericWindowData,
) -> LRESULT {
    let window_flags = userdata.window_state_lock().window_flags;
    if wparam == 0 || window_flags.contains(WindowFlags::MARKER_DECORATIONS) {
        return unsafe { DefWindowProcW(window, msg, wparam, lparam) };
    }

    let params = unsafe { &mut *(lparam as *mut NCCALCSIZE_PARAMS) };

    if util::is_maximized(window) {
        // Limit the window size when maximized to the current monitor.
        // Otherwise it would include the non-existent decorations.
        //
        // Use `MonitorFromRect` instead of `MonitorFromWindow` to select
        // the correct monitor here.
        // See https://github.com/MicrosoftEdge/WebView2Feedback/issues/2549
        let monitor = unsafe { MonitorFromRect(&params.rgrc[0], MONITOR_DEFAULTTONULL) };
        if let Ok(monitor_info) = monitor::get_monitor_info(monitor) {
            params.rgrc[0] = monitor_info.monitorInfo.rcWork;
        }
    } else if window_flags.contains(WindowFlags::MARKER_UNDECORATED_SHADOW) {
        // Extend the client area to cover the whole non-client area.
        // https://docs.microsoft.com/en-us/windows/win32/winmsg/wm-nccalcsize#remarks
        //
        // HACK(msiglreith): To add the drop shadow we slightly tweak the non-client area.
        // This leads to a small black 1px border on the top. Adding a margin manually
        // on all 4 borders would result in the caption getting drawn by the DWM.
        //
        // Another option would be to allow the DWM to paint inside the client area.
        // Unfortunately this results in janky resize behavior, where the compositor is
        // ahead of the window surface. Currently, there seems no option to achieve this
        // with the Windows API.
        params.rgrc[0].top += 1;
        params.rgrc[0].bottom += 1;
    }

    0
}

fn handle_entersizemove(
    _: HWND,
    _: u32,
    _: WPARAM,
    _: LPARAM,
    userdata: &dyn GenericWindowData,
) -> LRESULT {
    userdata
        .window_state_lock()
        .set_window_flags_in_place(|f| f.insert(WindowFlags::MARKER_IN_SIZE_MOVE));
    0
}

fn handle_exitsizemove(
    window: HWND,
    _: u32,
    _: WPARAM,
    lparam: LPARAM,
    userdata: &dyn GenericWindowData,
) -> LRESULT {
    let mut state = userdata.window_state_lock();
    if state.dragging {
        state.dragging = false;
        unsafe { PostMessageW(window, WM_LBUTTONUP, 0, lparam) };
    }

    state.set_window_flags_in_place(|f| f.remove(WindowFlags::MARKER_IN_SIZE_MOVE));
    0
}

fn handle_windowposchanging(
    window: HWND,
    _: u32,
    _: WPARAM,
    lparam: LPARAM,
    userdata: &dyn GenericWindowData,
) -> LRESULT {
    let mut window_state = userdata.window_state_lock();
    if let Some(ref mut fullscreen) = window_state.fullscreen {
        let window_pos = unsafe { &mut *(lparam as *mut WINDOWPOS) };
        let new_rect = RECT {
            left: window_pos.x,
            top: window_pos.y,
            right: window_pos.x + window_pos.cx,
            bottom: window_pos.y + window_pos.cy,
        };

        const NOMOVE_OR_NOSIZE: u32 = SWP_NOMOVE | SWP_NOSIZE;

        let new_rect = if window_pos.flags & NOMOVE_OR_NOSIZE != 0 {
            let cur_rect = util::WindowArea::Outer.get_rect(window)
                        .expect("Unexpected GetWindowRect failure; please report this error to https://github.com/rust-windowing/winit");

            match window_pos.flags & NOMOVE_OR_NOSIZE {
                NOMOVE_OR_NOSIZE => None,

                SWP_NOMOVE => Some(RECT {
                    left: cur_rect.left,
                    top: cur_rect.top,
                    right: cur_rect.left + window_pos.cx,
                    bottom: cur_rect.top + window_pos.cy,
                }),

                SWP_NOSIZE => Some(RECT {
                    left: window_pos.x,
                    top: window_pos.y,
                    right: window_pos.x - cur_rect.left + cur_rect.right,
                    bottom: window_pos.y - cur_rect.top + cur_rect.bottom,
                }),

                _ => unreachable!(),
            }
        } else {
            Some(new_rect)
        };

        if let Some(new_rect) = new_rect {
            let new_monitor = unsafe { MonitorFromRect(&new_rect, MONITOR_DEFAULTTONULL) };
            match fullscreen {
                Fullscreen::Borderless(ref mut fullscreen_monitor) => {
                    if new_monitor != 0
                        && fullscreen_monitor
                            .as_ref()
                            .map(|monitor| new_monitor != monitor.hmonitor())
                            .unwrap_or(true)
                    {
                        if let Ok(new_monitor_info) = monitor::get_monitor_info(new_monitor) {
                            let new_monitor_rect = new_monitor_info.monitorInfo.rcMonitor;
                            window_pos.x = new_monitor_rect.left;
                            window_pos.y = new_monitor_rect.top;
                            window_pos.cx = new_monitor_rect.right - new_monitor_rect.left;
                            window_pos.cy = new_monitor_rect.bottom - new_monitor_rect.top;
                        }
                        *fullscreen_monitor = Some(MonitorHandle::new(new_monitor));
                    }
                }
                Fullscreen::Exclusive(ref video_mode) => {
                    let old_monitor = video_mode.monitor.hmonitor();
                    if let Ok(old_monitor_info) = monitor::get_monitor_info(old_monitor) {
                        let old_monitor_rect = old_monitor_info.monitorInfo.rcMonitor;
                        window_pos.x = old_monitor_rect.left;
                        window_pos.y = old_monitor_rect.top;
                        window_pos.cx = old_monitor_rect.right - old_monitor_rect.left;
                        window_pos.cy = old_monitor_rect.bottom - old_monitor_rect.top;
                    }
                }
            }
        }
    }

    0
}

fn handle_windowposchanged(
    window: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
    userdata: &dyn GenericWindowData,
) -> LRESULT {
    use crate::event::WindowEvent::Moved;

    let windowpos = unsafe { *(lparam as *const WINDOWPOS) };
    if windowpos.flags & SWP_NOMOVE != SWP_NOMOVE {
        let physical_position = PhysicalPosition::new(windowpos.x, windowpos.y);
        unsafe {
            userdata.send_event(Event::WindowEvent {
                window_id: RootWindowId(WindowId(window)),
                event: Moved(physical_position),
            });
        }
    }

    // This is necessary for us to still get sent WM_SIZE.
    unsafe { DefWindowProcW(window, msg, wparam, lparam) }
}

fn handle_size(
    window: HWND,
    _: u32,
    wparam: WPARAM,
    lparam: LPARAM,
    userdata: &dyn GenericWindowData,
) -> LRESULT {
    use crate::event::WindowEvent::Resized;
    let w = loword(lparam as u32) as u32;
    let h = hiword(lparam as u32) as u32;

    let physical_size = PhysicalSize::new(w, h);
    let event = Event::WindowEvent {
        window_id: RootWindowId(WindowId(window)),
        event: Resized(physical_size),
    };

    {
        let mut w = userdata.window_state_lock();
        // See WindowFlags::MARKER_RETAIN_STATE_ON_SIZE docs for info on why this `if` check exists.
        if !w
            .window_flags()
            .contains(WindowFlags::MARKER_RETAIN_STATE_ON_SIZE)
        {
            let maximized = wparam == SIZE_MAXIMIZED as usize;
            w.set_window_flags_in_place(|f| f.set(WindowFlags::MAXIMIZED, maximized));
        }
    }
    unsafe {
        userdata.send_event(event);
    }
    0
}

submit! {
    (WM_NCCALCSIZE, handle_nccalcsize),
    (WM_ENTERSIZEMOVE, handle_entersizemove),
    (WM_EXITSIZEMOVE, handle_exitsizemove),
    (WM_WINDOWPOSCHANGING, handle_windowposchanging),
    (WM_WINDOWPOSCHANGED, handle_windowposchanged),
    (WM_SIZE, handle_size),
}
