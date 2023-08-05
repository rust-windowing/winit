//! Handles `xinput` events.

use super::prelude::*;

use crate::platform_impl::platform::common::keymap;
use crate::platform_impl::x11::{
    xinput_fp1616_to_float, xinput_fp3232_to_float, DeviceInfo, ScrollOrientation,
};
use crate::{
    event::{
        ElementState::{self, Pressed, Released},
        MouseButton::{Back, Forward, Left, Middle, Other, Right},
        MouseScrollDelta::LineDelta,
        RawKeyEvent, Touch, TouchPhase,
        WindowEvent::{
            AxisMotion, CursorEntered, CursorLeft, CursorMoved, Focused, MouseInput, MouseWheel,
        },
    },
    keyboard::ModifiersState,
};

use std::sync::Arc;

/// The X11 documentation states: "Keycodes lie in the inclusive range `[8, 255]`".
pub(super) const KEYCODE_OFFSET: u8 = 8;

impl EventProcessor {
    /// Handle `ButtonPress` and `ButtonRelease`.
    fn handle_button(
        &mut self,
        wt: &WindowTarget,
        xev: xinput::ButtonPressEvent,
        callback: &mut dyn FnMut(Event<Infallible>),
    ) {
        let window_id = mkwid(xev.event);
        let device_id = mkdid(xev.deviceid);

        // Set the timestamp.
        wt.xconn.set_timestamp(xev.time);

        if xev
            .flags
            .contains(xinput::PointerEventFlags::POINTER_EMULATED)
        {
            // Deliver multi-touch events instead of emulated mouse events.
            return;
        }

        let state = if xev.event_type & 0x7F == xinput::BUTTON_PRESS_EVENT {
            Pressed
        } else {
            Released
        };
        match xev.detail {
            1 => callback(Event::WindowEvent {
                window_id,
                event: MouseInput {
                    device_id,
                    state,
                    button: Left,
                },
            }),
            2 => callback(Event::WindowEvent {
                window_id,
                event: MouseInput {
                    device_id,
                    state,
                    button: Middle,
                },
            }),
            3 => callback(Event::WindowEvent {
                window_id,
                event: MouseInput {
                    device_id,
                    state,
                    button: Right,
                },
            }),

            // Suppress emulated scroll wheel clicks, since we handle the real motion events for those.
            // In practice, even clicky scroll wheels appear to be reported by evdev (and XInput2 in
            // turn) as axis motion, so we don't otherwise special-case these button presses.
            4 | 5 | 6 | 7 => {
                callback(Event::WindowEvent {
                    window_id,
                    event: MouseWheel {
                        device_id,
                        delta: match xev.detail {
                            4 => LineDelta(0.0, 1.0),
                            5 => LineDelta(0.0, -1.0),
                            6 => LineDelta(1.0, 0.0),
                            7 => LineDelta(-1.0, 0.0),
                            _ => unreachable!(),
                        },
                        phase: TouchPhase::Moved,
                    },
                });
            }

            8 => callback(Event::WindowEvent {
                window_id,
                event: MouseInput {
                    device_id,
                    state,
                    button: Back,
                },
            }),
            9 => callback(Event::WindowEvent {
                window_id,
                event: MouseInput {
                    device_id,
                    state,
                    button: Forward,
                },
            }),

            x => callback(Event::WindowEvent {
                window_id,
                event: MouseInput {
                    device_id,
                    state,
                    button: Other(x as u16),
                },
            }),
        }
    }

    /// Handle `XI_Motion` events.
    fn handle_motion(
        &mut self,
        wt: &WindowTarget,
        xev: xinput::MotionEvent,
        callback: &mut dyn FnMut(Event<Infallible>),
    ) {
        // Set the timestamp.
        wt.xconn.set_timestamp(xev.time);

        let device_id = mkdid(xev.deviceid);
        let window = xev.event as xproto::Window;
        let window_id = mkwid(window);
        let new_cursor_pos = (
            xinput_fp1616_to_float(xev.event_x),
            xinput_fp1616_to_float(xev.event_y),
        );

        let cursor_moved = self.with_window(wt, window, |window| {
            let mut shared_state_lock = window.shared_state_lock();
            util::maybe_change(&mut shared_state_lock.cursor_pos, new_cursor_pos)
        });
        if cursor_moved == Some(true) {
            let position = PhysicalPosition::new(new_cursor_pos.0, new_cursor_pos.1);

            callback(Event::WindowEvent {
                window_id,
                event: CursorMoved {
                    device_id,
                    position,
                },
            });
        } else if cursor_moved.is_none() {
            return;
        }

        // More gymnastics, for self.devices
        let mut events = Vec::new();
        {
            let mut devices = self.devices.borrow_mut();
            let physical_device = match devices.get_mut(&DeviceId(xev.sourceid)) {
                Some(device) => device,
                None => return,
            };

            let mask = bytemuck::cast_slice::<u32, u8>(&xev.valuator_mask);
            let mut values = &*xev.axisvalues;

            for i in 0..mask.len() * 8 {
                let byte_index = i / 8;
                let bit_index = i % 8;

                if mask[byte_index] & (1 << bit_index) == 0 {
                    continue;
                }

                // This mask is set, get the value.
                let value = {
                    let (value, rest) = values.split_first().unwrap();
                    values = rest;
                    xinput_fp3232_to_float(*value)
                };

                if let Some(&mut (_, ref mut info)) = physical_device
                    .scroll_axes
                    .iter_mut()
                    .find(|&&mut (axis, _)| usize::from(axis) == i)
                {
                    let delta = (value - info.position) / info.increment;
                    info.position = value;
                    events.push(Event::WindowEvent {
                        window_id,
                        event: MouseWheel {
                            device_id,
                            delta: match info.orientation {
                                // X11 vertical scroll coordinates are opposite to winit's
                                ScrollOrientation::Horizontal => LineDelta(-delta as f32, 0.0),
                                ScrollOrientation::Vertical => LineDelta(0.0, -delta as f32),
                            },
                            phase: TouchPhase::Moved,
                        },
                    });
                } else {
                    events.push(Event::WindowEvent {
                        window_id,
                        event: AxisMotion {
                            device_id,
                            axis: i as u32,
                            value,
                        },
                    });
                }
            }
        }
        for event in events {
            callback(event);
        }
    }

    /// Handle `XI_Enter` events.
    fn handle_enter(
        &mut self,
        wt: &WindowTarget,
        xev: xinput::EnterEvent,
        callback: &mut dyn FnMut(Event<Infallible>),
    ) {
        // Set the timestamp.
        wt.xconn.set_timestamp(xev.time as xproto::Timestamp);

        let window = xev.event as xproto::Window;
        let window_id = mkwid(window);
        let device_id = mkdid(xev.deviceid);

        if let Some(all_info) = DeviceInfo::get(&wt.xconn, ALL_DEVICES) {
            let mut devices = self.devices.borrow_mut();
            for device_info in all_info.info.iter() {
                if device_info.deviceid == xev.sourceid
                                // This is needed for resetting to work correctly on i3, and
                                // presumably some other WMs. On those, `XI_Enter` doesn't include
                                // the physical device ID, so both `sourceid` and `deviceid` are
                                // the virtual device.
                                || device_info.attachment == xev.sourceid
                {
                    let device_id = DeviceId(device_info.deviceid);
                    if let Some(device) = devices.get_mut(&device_id) {
                        device.reset_scroll_position(device_info);
                    }
                }
            }
        }

        if self.window_exists(wt, window) {
            callback(Event::WindowEvent {
                window_id,
                event: CursorEntered { device_id },
            });

            let position = PhysicalPosition::new(
                xinput_fp1616_to_float(xev.event_x),
                xinput_fp1616_to_float(xev.event_y),
            );

            callback(Event::WindowEvent {
                window_id,
                event: CursorMoved {
                    device_id,
                    position,
                },
            });
        }
    }

    /// Handle `XI_Leave` events.
    fn handle_leave(
        &mut self,
        wt: &WindowTarget,
        xev: xinput::LeaveEvent,
        callback: &mut dyn FnMut(Event<Infallible>),
    ) {
        let window = xev.event;

        // Set the timestamp.
        wt.xconn.set_timestamp(xev.time as xproto::Timestamp);

        // Leave, FocusIn, and FocusOut can be received by a window that's already
        // been destroyed, which the user presumably doesn't want to deal with.
        let window_closed = !self.window_exists(wt, window);
        if !window_closed {
            callback(Event::WindowEvent {
                window_id: mkwid(window),
                event: CursorLeft {
                    device_id: mkdid(xev.deviceid),
                },
            });
        }
    }

    /// Handle `XI_FocusIn` events.
    fn handle_focus_in(
        &mut self,
        wt: &WindowTarget,
        xev: xinput::FocusInEvent,
        mut callback: &mut dyn FnMut(Event<Infallible>),
    ) {
        let window = xev.event as xproto::Window;

        // Set the timestamp.
        wt.xconn.set_timestamp(xev.time as xproto::Timestamp);

        wt.ime
            .borrow_mut()
            .focus(xev.event as ffi::Window)
            .expect("Failed to focus input context");

        if self.active_window != Some(window) {
            self.active_window = Some(window);

            wt.update_listen_device_events(true);

            let window_id = mkwid(window);
            let position = PhysicalPosition::new(
                xinput_fp1616_to_float(xev.event_x),
                xinput_fp1616_to_float(xev.event_y),
            );

            if let Some(window) = self.with_window(wt, window, Arc::clone) {
                window.shared_state_lock().has_focus = true;
            }

            callback(Event::WindowEvent {
                window_id,
                event: Focused(true),
            });

            let modifiers: crate::keyboard::ModifiersState = self.kb_state.mods_state().into();
            if !modifiers.is_empty() {
                callback(Event::WindowEvent {
                    window_id,
                    event: WindowEvent::ModifiersChanged(modifiers.into()),
                });
            }

            // The deviceid for this event is for a keyboard instead of a pointer,
            // so we have to do a little extra work.
            let pointer_id = self
                .devices
                .borrow()
                .get(&DeviceId(xev.deviceid))
                .map(|device| device.attachment)
                .unwrap_or(2);

            callback(Event::WindowEvent {
                window_id,
                event: CursorMoved {
                    device_id: mkdid(pointer_id),
                    position,
                },
            });

            // Issue key press events for all pressed keys
            Self::handle_pressed_keys(
                wt,
                window_id,
                ElementState::Pressed,
                &mut self.kb_state,
                &mut callback,
            );
        }
    }

    /// Handle the `XI_FocusOut` events.
    fn handle_focus_out(
        &mut self,
        wt: &WindowTarget,
        xev: xinput::FocusOutEvent,
        mut callback: &mut dyn FnMut(Event<Infallible>),
    ) {
        let window = xev.event as xproto::Window;

        // Set the timestamp.
        wt.xconn.set_timestamp(xev.time as xproto::Timestamp);

        if !self.window_exists(wt, window) {
            return;
        }

        wt.ime
            .borrow_mut()
            .unfocus(xev.event as ffi::Window)
            .expect("Failed to unfocus input context");

        if self.active_window.take() == Some(window) {
            let window_id = mkwid(window);

            wt.update_listen_device_events(false);

            // Issue key release events for all pressed keys
            Self::handle_pressed_keys(
                wt,
                window_id,
                ElementState::Released,
                &mut self.kb_state,
                &mut callback,
            );
            // Clear this so detecting key repeats is consistently handled when the
            // window regains focus.
            self.held_key_press = None;

            callback(Event::WindowEvent {
                window_id,
                event: WindowEvent::ModifiersChanged(ModifiersState::empty().into()),
            });

            if let Some(window) = self.with_window(wt, window, Arc::clone) {
                window.shared_state_lock().has_focus = false;
            }

            callback(Event::WindowEvent {
                window_id,
                event: Focused(false),
            })
        }
    }

    /// Handle the touch events.
    fn handle_touch(
        &mut self,
        wt: &WindowTarget,
        xev: xinput::TouchBeginEvent,
        callback: &mut dyn FnMut(Event<Infallible>),
    ) {
        // Set the timestamp.
        wt.xconn.set_timestamp(xev.time as xproto::Timestamp);

        let window = xev.event as xproto::Window;
        let window_id = mkwid(window);
        let phase = match xev.event_type & 0x7F {
            xinput::TOUCH_BEGIN_EVENT => TouchPhase::Started,
            xinput::TOUCH_UPDATE_EVENT => TouchPhase::Moved,
            xinput::TOUCH_END_EVENT => TouchPhase::Ended,
            _ => unreachable!(),
        };
        if self.window_exists(wt, window) {
            let id = xev.detail as u64;
            let location = PhysicalPosition::new(
                xinput_fp1616_to_float(xev.event_x),
                xinput_fp1616_to_float(xev.event_y),
            );

            // Mouse cursor position changes when touch events are received.
            // Only the first concurrently active touch ID moves the mouse cursor.
            if is_first_touch(&mut self.first_touch, &mut self.num_touch, id, phase) {
                callback(Event::WindowEvent {
                    window_id,
                    event: WindowEvent::CursorMoved {
                        device_id: mkdid(util::VIRTUAL_CORE_POINTER),
                        position: location.cast(),
                    },
                });
            }

            callback(Event::WindowEvent {
                window_id,
                event: WindowEvent::Touch(Touch {
                    device_id: mkdid(xev.deviceid),
                    phase,
                    location,
                    force: None, // TODO
                    id,
                }),
            })
        }
    }

    /// Handle the raw button events.
    fn handle_raw_button(
        &mut self,
        wt: &WindowTarget,
        xev: xinput::RawButtonPressEvent,
        callback: &mut dyn FnMut(Event<Infallible>),
    ) {
        // Set the timestamp.
        wt.xconn.set_timestamp(xev.time);

        if xev
            .flags
            .contains(xinput::PointerEventFlags::POINTER_EMULATED)
        {
            // Deliver multi-touch events instead of emulated mouse events.
            return;
        }

        callback(Event::DeviceEvent {
            device_id: mkdid(xev.deviceid),
            event: DeviceEvent::Button {
                button: xev.detail,
                state: match xev.event_type & 0x7F {
                    xinput::RAW_BUTTON_PRESS_EVENT => Pressed,
                    xinput::RAW_BUTTON_RELEASE_EVENT => Released,
                    _ => unreachable!(),
                },
            },
        });
    }

    /// Handle the `XI_RawMotion` event.
    fn handle_raw_motion(
        &mut self,
        wt: &WindowTarget,
        xev: xinput::RawMotionEvent,
        callback: &mut dyn FnMut(Event<Infallible>),
    ) {
        // Set the timestamp.
        wt.xconn.set_timestamp(xev.time);

        let did = mkdid(xev.deviceid);
        let mut mouse_delta = (0.0, 0.0);
        let mut scroll_delta = (0.0, 0.0);

        // Iterate over the valulators.
        let mask = bytemuck::cast_slice::<u32, u8>(&xev.valuator_mask);
        let mut values = &*xev.axisvalues_raw;

        for i in 0..mask.len() * 8 {
            let byte_index = i / 8;
            let bit_index = i % 8;

            if mask[byte_index] & (1 << bit_index) == 0 {
                continue;
            }

            // This mask is set, get the value.
            let value = {
                let (value, rest) = values.split_first().unwrap();
                values = rest;
                xinput_fp3232_to_float(*value)
            };

            // We assume that every XInput2 device with analog axes is a pointing device emitting
            // relative coordinates.
            match i {
                0 => mouse_delta.0 = value,
                1 => mouse_delta.1 = value,
                2 => scroll_delta.0 = value as f32,
                3 => scroll_delta.1 = value as f32,
                _ => {}
            }
            callback(Event::DeviceEvent {
                device_id: did,
                event: DeviceEvent::Motion {
                    axis: i as u32,
                    value,
                },
            });
        }

        if mouse_delta != (0.0, 0.0) {
            callback(Event::DeviceEvent {
                device_id: did,
                event: DeviceEvent::MouseMotion { delta: mouse_delta },
            });
        }
        if scroll_delta != (0.0, 0.0) {
            callback(Event::DeviceEvent {
                device_id: did,
                event: DeviceEvent::MouseWheel {
                    delta: LineDelta(scroll_delta.0, scroll_delta.1),
                },
            });
        }
    }

    /// Handle a raw key press.
    fn handle_raw_key(
        &mut self,
        wt: &WindowTarget,
        xev: xinput::RawKeyPressEvent,
        callback: &mut dyn FnMut(Event<Infallible>),
    ) {
        // Set the timestamp.
        wt.xconn.set_timestamp(xev.time as xproto::Timestamp);

        let state = match xev.event_type & 0x7F {
            xinput::RAW_KEY_PRESS_EVENT => Pressed,
            xinput::RAW_KEY_RELEASE_EVENT => Released,
            _ => unreachable!(),
        };

        let device_id = mkdid(xev.sourceid);
        let keycode = xev.detail;
        if keycode < KEYCODE_OFFSET as u32 {
            return;
        }
        let physical_key = keymap::raw_keycode_to_keycode(keycode);

        callback(Event::DeviceEvent {
            device_id,
            event: DeviceEvent::Key(RawKeyEvent {
                physical_key,
                state,
            }),
        });
    }

    /// Handle a hierarchy change.
    fn handle_hierarchy_change(
        &mut self,
        wt: &WindowTarget,
        xev: xinput::HierarchyEvent,
        callback: &mut dyn FnMut(Event<Infallible>),
    ) {
        // Set the timestamp.
        wt.xconn.set_timestamp(xev.time as xproto::Timestamp);

        for info in xev.infos {
            if info
                .flags
                .contains(xinput::HierarchyMask::SLAVE_ADDED | xinput::HierarchyMask::MASTER_ADDED)
            {
                self.init_device(wt, info.deviceid);
                callback(Event::DeviceEvent {
                    device_id: mkdid(info.deviceid),
                    event: DeviceEvent::Added,
                });
            } else if info.flags.contains(
                xinput::HierarchyMask::SLAVE_REMOVED | xinput::HierarchyMask::MASTER_REMOVED,
            ) {
                callback(Event::DeviceEvent {
                    device_id: mkdid(info.deviceid),
                    event: DeviceEvent::Removed,
                });
                let mut devices = self.devices.borrow_mut();
                devices.remove(&DeviceId(info.deviceid));
            }
        }
    }
}

fn is_first_touch(first: &mut Option<u64>, num: &mut u32, id: u64, phase: TouchPhase) -> bool {
    match phase {
        TouchPhase::Started => {
            if *num == 0 {
                *first = Some(id);
            }
            *num += 1;
        }
        TouchPhase::Cancelled | TouchPhase::Ended => {
            if *first == Some(id) {
                *first = None;
            }
            *num = num.saturating_sub(1);
        }
        _ => (),
    }

    *first == Some(id)
}

event_handlers! {
    xi_code(xinput::BUTTON_PRESS_EVENT) => EventProcessor::handle_button,
    xi_code(xinput::BUTTON_RELEASE_EVENT) => EventProcessor::handle_button,
    xi_code(xinput::MOTION_EVENT) => EventProcessor::handle_motion,
    xi_code(xinput::ENTER_EVENT) => EventProcessor::handle_enter,
    xi_code(xinput::LEAVE_EVENT) => EventProcessor::handle_leave,
    xi_code(xinput::FOCUS_IN_EVENT) => EventProcessor::handle_focus_in,
    xi_code(xinput::FOCUS_OUT_EVENT) => EventProcessor::handle_focus_out,
    xi_code(xinput::TOUCH_BEGIN_EVENT) => EventProcessor::handle_touch,
    xi_code(xinput::TOUCH_UPDATE_EVENT) => EventProcessor::handle_touch,
    xi_code(xinput::TOUCH_END_EVENT) => EventProcessor::handle_touch,
    xi_code(xinput::RAW_BUTTON_PRESS_EVENT) => EventProcessor::handle_raw_button,
    xi_code(xinput::RAW_BUTTON_RELEASE_EVENT) => EventProcessor::handle_raw_button,
    xi_code(xinput::RAW_MOTION_EVENT) => EventProcessor::handle_raw_motion,
    xi_code(xinput::RAW_KEY_PRESS_EVENT) => EventProcessor::handle_raw_key,
    xi_code(xinput::RAW_KEY_RELEASE_EVENT) => EventProcessor::handle_raw_key,
    xi_code(xinput::HIERARCHY_EVENT) => EventProcessor::handle_hierarchy_change,
}
