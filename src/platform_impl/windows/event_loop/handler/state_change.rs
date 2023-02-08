//! Handlers for changes to the state of the window.

use super::prelude::*;

use std::mem;

fn handle_ncactivate(
    window: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
    userdata: &dyn GenericWindowData,
) -> LRESULT {
    let is_active = wparam != false.into();
    let active_focus_changed = userdata.window_state_lock().set_active(is_active);
    if active_focus_changed {
        unsafe {
            if is_active {
                gain_active_focus(window, userdata);
            } else {
                lose_active_focus(window, userdata);
            }
        }
    }

    unsafe { DefWindowProcW(window, msg, wparam, lparam) }
}

fn handle_setfocus(
    window: HWND,
    _: u32,
    _wparam: WPARAM,
    _lparam: LPARAM,
    userdata: &dyn GenericWindowData,
) -> LRESULT {
    let active_focus_changed = userdata.window_state_lock().set_focused(true);
    if active_focus_changed {
        unsafe {
            gain_active_focus(window, userdata);
        }
    }
    0
}

fn handle_killfocus(
    window: HWND,
    _msg: u32,
    _wparam: WPARAM,
    _lparam: LPARAM,
    userdata: &dyn GenericWindowData,
) -> LRESULT {
    let active_focus_changed = userdata.window_state_lock().set_focused(false);
    if active_focus_changed {
        unsafe {
            lose_active_focus(window, userdata);
        }
    }
    0
}

fn handle_setcursor(
    window: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
    userdata: &dyn GenericWindowData,
) -> LRESULT {
    let set_cursor_to = {
        let window_state = userdata.window_state_lock();
        // The return value for the preceding `WM_NCHITTEST` message is conveniently
        // provided through the low-order word of lParam. We use that here since
        // `WM_MOUSEMOVE` seems to come after `WM_SETCURSOR` for a given cursor movement.
        let in_client_area = loword(lparam as u32) as u32 == HTCLIENT;
        if in_client_area {
            Some(window_state.mouse.cursor)
        } else {
            None
        }
    };

    unsafe {
        match set_cursor_to {
            Some(cursor) => {
                let cursor = LoadCursorW(0, cursor.to_windows_cursor());
                SetCursor(cursor);
                0
            }
            None => DefWindowProcW(window, msg, wparam, lparam),
        }
    }
}

fn handle_dropfiles(_: HWND, _: u32, _: WPARAM, _: LPARAM, _: &dyn GenericWindowData) -> LRESULT {
    // See `FileDropHandler` for implementation.
    0
}

fn handle_getminmaxinfo(
    window: HWND,
    _: u32,
    _: WPARAM,
    lparam: LPARAM,
    userdata: &dyn GenericWindowData,
) -> LRESULT {
    unsafe {
        let mmi = lparam as *mut MINMAXINFO;

        let window_state = userdata.window_state_lock();
        let window_flags = window_state.window_flags;

        if window_state.min_size.is_some() || window_state.max_size.is_some() {
            if let Some(min_size) = window_state.min_size {
                let min_size = min_size.to_physical(window_state.scale_factor);
                let (width, height): (u32, u32) = window_flags.adjust_size(window, min_size).into();
                (*mmi).ptMinTrackSize = POINT {
                    x: width as i32,
                    y: height as i32,
                };
            }
            if let Some(max_size) = window_state.max_size {
                let max_size = max_size.to_physical(window_state.scale_factor);
                let (width, height): (u32, u32) = window_flags.adjust_size(window, max_size).into();
                (*mmi).ptMaxTrackSize = POINT {
                    x: width as i32,
                    y: height as i32,
                };
            }
        }

        0
    }
}

fn handle_dpichanged(
    window: HWND,
    _: u32,
    wparam: WPARAM,
    lparam: LPARAM,
    userdata: &dyn GenericWindowData,
) -> LRESULT {
    unsafe {
        use crate::event::WindowEvent::ScaleFactorChanged;

        // This message actually provides two DPI values - x and y. However MSDN says that
        // "you only need to use either the X-axis or the Y-axis value when scaling your
        // application since they are the same".
        // https://msdn.microsoft.com/en-us/library/windows/desktop/dn312083(v=vs.85).aspx
        let new_dpi_x = loword(wparam as u32) as u32;
        let new_scale_factor = dpi_to_scale_factor(new_dpi_x);
        let old_scale_factor: f64;

        let (allow_resize, window_flags) = {
            let mut window_state = userdata.window_state_lock();
            old_scale_factor = window_state.scale_factor;
            window_state.scale_factor = new_scale_factor;

            if new_scale_factor == old_scale_factor {
                return 0;
            }

            let allow_resize = window_state.fullscreen.is_none()
                && !window_state.window_flags().contains(WindowFlags::MAXIMIZED);

            (allow_resize, window_state.window_flags)
        };

        // New size as suggested by Windows.
        let suggested_rect = *(lparam as *const RECT);

        // The window rect provided is the window's outer size, not it's inner size. However,
        // win32 doesn't provide an `UnadjustWindowRectEx` function to get the client rect from
        // the outer rect, so we instead adjust the window rect to get the decoration margins
        // and remove them from the outer size.
        let margin_left: i32;
        let margin_top: i32;
        // let margin_right: i32;
        // let margin_bottom: i32;
        {
            let adjusted_rect = window_flags
                .adjust_rect(window, suggested_rect)
                .unwrap_or(suggested_rect);
            margin_left = suggested_rect.left - adjusted_rect.left;
            margin_top = suggested_rect.top - adjusted_rect.top;
            // margin_right = adjusted_rect.right - suggested_rect.right;
            // margin_bottom = adjusted_rect.bottom - suggested_rect.bottom;
        }

        let old_physical_inner_rect = util::WindowArea::Inner
            .get_rect(window)
            .expect("failed to query (old) inner window area");
        let old_physical_inner_size = PhysicalSize::new(
            (old_physical_inner_rect.right - old_physical_inner_rect.left) as u32,
            (old_physical_inner_rect.bottom - old_physical_inner_rect.top) as u32,
        );

        // `allow_resize` prevents us from re-applying DPI adjustment to the restored size after
        // exiting fullscreen (the restored size is already DPI adjusted).
        let mut new_physical_inner_size = match allow_resize {
            // We calculate our own size because the default suggested rect doesn't do a great job
            // of preserving the window's logical size.
            true => old_physical_inner_size
                .to_logical::<f64>(old_scale_factor)
                .to_physical::<u32>(new_scale_factor),
            false => old_physical_inner_size,
        };

        userdata.send_event(Event::WindowEvent {
            window_id: RootWindowId(WindowId(window)),
            event: ScaleFactorChanged {
                scale_factor: new_scale_factor,
                new_inner_size: &mut new_physical_inner_size,
            },
        });

        let dragging_window: bool;

        {
            let window_state = userdata.window_state_lock();
            dragging_window = window_state
                .window_flags()
                .contains(WindowFlags::MARKER_IN_SIZE_MOVE);
            // Unset maximized if we're changing the window's size.
            if new_physical_inner_size != old_physical_inner_size {
                WindowState::set_window_flags(window_state, window, |f| {
                    f.set(WindowFlags::MAXIMIZED, false)
                });
            }
        }

        let new_outer_rect: RECT;
        {
            let suggested_ul = (
                suggested_rect.left + margin_left,
                suggested_rect.top + margin_top,
            );

            let mut conservative_rect = RECT {
                left: suggested_ul.0,
                top: suggested_ul.1,
                right: suggested_ul.0 + new_physical_inner_size.width as i32,
                bottom: suggested_ul.1 + new_physical_inner_size.height as i32,
            };

            conservative_rect = window_flags
                .adjust_rect(window, conservative_rect)
                .unwrap_or(conservative_rect);

            // If we're dragging the window, offset the window so that the cursor's
            // relative horizontal position in the title bar is preserved.
            if dragging_window {
                let bias = {
                    let cursor_pos = {
                        let mut pos = mem::zeroed();
                        GetCursorPos(&mut pos);
                        pos
                    };
                    let suggested_cursor_horizontal_ratio = (cursor_pos.x - suggested_rect.left)
                        as f64
                        / (suggested_rect.right - suggested_rect.left) as f64;

                    (cursor_pos.x
                        - (suggested_cursor_horizontal_ratio
                            * (conservative_rect.right - conservative_rect.left) as f64)
                            as i32)
                        - conservative_rect.left
                };
                conservative_rect.left += bias;
                conservative_rect.right += bias;
            }

            // Check to see if the new window rect is on the monitor with the new DPI factor.
            // If it isn't, offset the window so that it is.
            let new_dpi_monitor = MonitorFromWindow(window, MONITOR_DEFAULTTONULL);
            let conservative_rect_monitor =
                MonitorFromRect(&conservative_rect, MONITOR_DEFAULTTONULL);
            new_outer_rect = if conservative_rect_monitor == new_dpi_monitor {
                conservative_rect
            } else {
                let get_monitor_rect = |monitor| {
                    let mut monitor_info = MONITORINFO {
                        cbSize: mem::size_of::<MONITORINFO>() as _,
                        ..mem::zeroed()
                    };
                    GetMonitorInfoW(monitor, &mut monitor_info);
                    monitor_info.rcMonitor
                };
                let wrong_monitor = conservative_rect_monitor;
                let wrong_monitor_rect = get_monitor_rect(wrong_monitor);
                let new_monitor_rect = get_monitor_rect(new_dpi_monitor);

                // The direction to nudge the window in to get the window onto the monitor with
                // the new DPI factor. We calculate this by seeing which monitor edges are
                // shared and nudging away from the wrong monitor based on those.
                #[allow(clippy::bool_to_int_with_if)]
                let delta_nudge_to_dpi_monitor = (
                    if wrong_monitor_rect.left == new_monitor_rect.right {
                        -1
                    } else if wrong_monitor_rect.right == new_monitor_rect.left {
                        1
                    } else {
                        0
                    },
                    if wrong_monitor_rect.bottom == new_monitor_rect.top {
                        1
                    } else if wrong_monitor_rect.top == new_monitor_rect.bottom {
                        -1
                    } else {
                        0
                    },
                );

                let abort_after_iterations = new_monitor_rect.right - new_monitor_rect.left
                    + new_monitor_rect.bottom
                    - new_monitor_rect.top;
                for _ in 0..abort_after_iterations {
                    conservative_rect.left += delta_nudge_to_dpi_monitor.0;
                    conservative_rect.right += delta_nudge_to_dpi_monitor.0;
                    conservative_rect.top += delta_nudge_to_dpi_monitor.1;
                    conservative_rect.bottom += delta_nudge_to_dpi_monitor.1;

                    if MonitorFromRect(&conservative_rect, MONITOR_DEFAULTTONULL) == new_dpi_monitor
                    {
                        break;
                    }
                }

                conservative_rect
            };
        }

        SetWindowPos(
            window,
            0,
            new_outer_rect.left,
            new_outer_rect.top,
            new_outer_rect.right - new_outer_rect.left,
            new_outer_rect.bottom - new_outer_rect.top,
            SWP_NOZORDER | SWP_NOACTIVATE,
        );

        0
    }
}

fn handle_settingchange(
    window: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
    userdata: &dyn GenericWindowData,
) -> LRESULT {
    use crate::event::WindowEvent::ThemeChanged;

    let preferred_theme = userdata.window_state_lock().preferred_theme;

    if preferred_theme.is_none() {
        let new_theme = try_theme(window, preferred_theme);
        let mut window_state = userdata.window_state_lock();

        if window_state.current_theme != new_theme {
            window_state.current_theme = new_theme;
            drop(window_state);

            unsafe {
                userdata.send_event(Event::WindowEvent {
                    window_id: RootWindowId(WindowId(window)),
                    event: ThemeChanged(new_theme),
                });
            }
        }
    }

    unsafe { DefWindowProcW(window, msg, wparam, lparam) }
}

submit! {
    (WM_NCACTIVATE, handle_ncactivate),
    (WM_SETFOCUS, handle_setfocus),
    (WM_KILLFOCUS, handle_killfocus),
    (WM_SETCURSOR, handle_setcursor),
    (WM_DROPFILES, handle_dropfiles),
    (WM_GETMINMAXINFO, handle_getminmaxinfo),
    (WM_DPICHANGED, handle_dpichanged),
    (WM_SETTINGCHANGE, handle_settingchange),
}
