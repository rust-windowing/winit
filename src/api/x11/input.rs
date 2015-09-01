use std::sync::Arc;

use libc;
use std::{mem, ptr};
use std::ffi::CString;
use std::slice::from_raw_parts;

use events::Event;

use super::{events, ffi};
use super::XConnection;

#[derive(Debug)]
enum AxisType {
    HorizontalScroll,
    VerticalScroll
}

#[derive(Debug)]
struct Axis {
    id: i32,
    device_id: i32,
    axis_number: i32,
    axis_type: AxisType,
    scroll_increment: f64,
}

#[derive(Debug)]
struct AxisValue {
    device_id: i32,
    axis_number: i32,
    value: f64
}

struct InputState {
    /// Last-seen cursor position within a window in (x, y)
    /// coordinates
    cursor_pos: (f64, f64),
    /// Last-seen positions of axes, used to report delta
    /// movements when a new absolute axis value is received
    axis_values: Vec<AxisValue>
}

pub struct XInputEventHandler {
    display: Arc<XConnection>,
    window: ffi::Window,
    ic: ffi::XIC,
    axis_list: Vec<Axis>,
    current_state: InputState
}

impl XInputEventHandler {
    pub fn new(display: &Arc<XConnection>, window: ffi::Window, ic: ffi::XIC) -> XInputEventHandler {
        // query XInput support
        let mut opcode: libc::c_int = 0;
        let mut event: libc::c_int = 0;
        let mut error: libc::c_int = 0;
        let xinput_str = CString::new("XInputExtension").unwrap();

        unsafe {
            if (display.xlib.XQueryExtension)(display.display, xinput_str.as_ptr(), &mut opcode, &mut event, &mut error) == ffi::False {
                panic!("XInput not available")
            }
        }

        let mut xinput_major_ver = ffi::XI_2_Major;
        let mut xinput_minor_ver = ffi::XI_2_Minor;

        unsafe {
            if (display.xinput2.XIQueryVersion)(display.display, &mut xinput_major_ver, &mut xinput_minor_ver) != ffi::Success as libc::c_int {
                panic!("Unable to determine XInput version");
            }
        }

        // specify the XInput events we want to receive.
        // Button clicks and mouse events are handled via XInput
        // events. Key presses are still handled via plain core
        // X11 events.
        let mut mask: [libc::c_uchar; 3] = [0; 3];
        let mut input_event_mask = ffi::XIEventMask {
            deviceid: ffi::XIAllMasterDevices,
            mask_len: mask.len() as i32,
            mask: mask.as_mut_ptr()
        };
        let events = &[
            ffi::XI_ButtonPress,
            ffi::XI_ButtonRelease,
            ffi::XI_Motion,
            ffi::XI_Enter,
            ffi::XI_Leave,
            ffi::XI_FocusIn,
            ffi::XI_FocusOut,
            ffi::XI_TouchBegin,
            ffi::XI_TouchUpdate,
            ffi::XI_TouchEnd,
        ];
        for event in events {
            ffi::XISetMask(&mut mask, *event);
        }

        unsafe {
            match (display.xinput2.XISelectEvents)(display.display, window, &mut input_event_mask, 1) {
                status if status as u8 == ffi::Success => (),
                err => panic!("Failed to select events {:?}", err)
            }
        }

        XInputEventHandler {
            display: display.clone(),
            window: window,
            ic: ic,
            axis_list: read_input_axis_info(display),
            current_state: InputState {
                cursor_pos: (0.0, 0.0),
                axis_values: Vec::new()
            }
        }
    }

    pub fn translate_key_event(&self, event: &mut ffi::XKeyEvent) -> Vec<Event> {
        use events::Event::{KeyboardInput, ReceivedCharacter};
        use events::ElementState::{Pressed, Released};

        let mut translated_events = Vec::new();

        let state;
        if event.type_ == ffi::KeyPress {
            let raw_ev: *mut ffi::XKeyEvent = event;
            unsafe { (self.display.xlib.XFilterEvent)(mem::transmute(raw_ev), self.window) };
            state = Pressed;
        } else {
            state = Released;
        }

        let mut kp_keysym = 0;

        let written = unsafe {
            use std::str;

            let mut buffer: [u8; 16] = [mem::uninitialized(); 16];
            let raw_ev: *mut ffi::XKeyEvent = event;
            let count = (self.display.xlib.Xutf8LookupString)(self.ic, mem::transmute(raw_ev),
            mem::transmute(buffer.as_mut_ptr()),
            buffer.len() as libc::c_int, &mut kp_keysym, ptr::null_mut());

            str::from_utf8(&buffer[..count as usize]).unwrap_or("").to_string()
        };

        for chr in written.chars() {
            translated_events.push(ReceivedCharacter(chr));
        }

        let mut keysym = unsafe {
            (self.display.xlib.XKeycodeToKeysym)(self.display.display, event.keycode as ffi::KeyCode, 0)
        };

        if (ffi::XK_KP_Space as libc::c_ulong <= keysym) && (keysym <= ffi::XK_KP_9 as libc::c_ulong) {
            keysym = kp_keysym
        };

        let vkey = events::keycode_to_element(keysym as libc::c_uint);

        translated_events.push(KeyboardInput(state, event.keycode as u8, vkey));
        translated_events
    }

    pub fn translate_event(&mut self, cookie: &ffi::XGenericEventCookie) -> Option<Event> {
        use events::Event::{Focused, MouseInput, MouseMoved, MouseWheel};
        use events::ElementState::{Pressed, Released};
        use events::MouseButton::{Left, Right, Middle};
        use events::MouseScrollDelta::LineDelta;
        use events::{Touch, TouchPhase};

        match cookie.evtype {
            ffi::XI_ButtonPress | ffi::XI_ButtonRelease => {
                let event_data: &ffi::XIDeviceEvent = unsafe{mem::transmute(cookie.data)};
                let state = if cookie.evtype == ffi::XI_ButtonPress {
                    Pressed
                } else {
                    Released
                };
                match event_data.detail as u32 {
                    ffi::Button1 => Some(MouseInput(state, Left)),
                    ffi::Button2 => Some(MouseInput(state, Middle)),
                    ffi::Button3 => Some(MouseInput(state, Right)),
                    ffi::Button4 | ffi::Button5 => {
                        if event_data.flags & ffi::XIPointerEmulated == 0 {
                            // scroll event from a traditional wheel with
                            // distinct 'clicks'
                            let delta = if event_data.detail as u32 == ffi::Button4 {
                                1.0
                            } else {
                                -1.0
                            };
                            Some(MouseWheel(LineDelta(0.0, delta)))
                        } else {
                            // emulated button event from a touch/smooth-scroll
                            // event. Ignore these events and handle scrolling
                            // via XI_Motion event handler instead
                            None
                        }
                    }
                    _ => None
                }
            },
            ffi::XI_Motion => {
                let event_data: &ffi::XIDeviceEvent = unsafe{mem::transmute(cookie.data)};
                let axis_state = event_data.valuators;
                let mask = unsafe{ from_raw_parts(axis_state.mask, axis_state.mask_len as usize) };
                let mut axis_count = 0;

                let mut scroll_delta = (0.0, 0.0);
                for axis_id in 0..axis_state.mask_len {
                    if ffi::XIMaskIsSet(&mask, axis_id) {
                        let axis_value = unsafe{*axis_state.values.offset(axis_count)};
                        let delta = calc_scroll_deltas(event_data, axis_id, axis_value, &self.axis_list, 
                                                       &mut self.current_state.axis_values);
                        scroll_delta.0 += delta.0;
                        scroll_delta.1 += delta.1;
                        axis_count += 1;
                    }
                }

                if scroll_delta.0.abs() > 0.0 || scroll_delta.1.abs() > 0.0 {
                    Some(MouseWheel(LineDelta(scroll_delta.0 as f32, scroll_delta.1 as f32)))
                } else {
                    let new_cursor_pos = (event_data.event_x, event_data.event_y);
                    if new_cursor_pos != self.current_state.cursor_pos {
                        self.current_state.cursor_pos = new_cursor_pos;
                        Some(MouseMoved((new_cursor_pos.0 as i32, new_cursor_pos.1 as i32)))
                    } else {
                        None
                    }
                }
            },
            ffi::XI_Enter => {
                // axis movements whilst the cursor is outside the window
                // will alter the absolute value of the axes. We only want to
                // report changes in the axis value whilst the cursor is above
                // our window however, so clear the previous axis state whenever
                // the cursor re-enters the window
                self.current_state.axis_values.clear();
                None
            },
            ffi::XI_Leave => None,
            ffi::XI_FocusIn => Some(Focused(true)),
            ffi::XI_FocusOut => Some(Focused(false)),
            ffi::XI_TouchBegin | ffi::XI_TouchUpdate | ffi::XI_TouchEnd => {
                let event_data: &ffi::XIDeviceEvent = unsafe{mem::transmute(cookie.data)};
                let phase = match cookie.evtype {
                    ffi::XI_TouchBegin => TouchPhase::Started,
                    ffi::XI_TouchUpdate => TouchPhase::Moved,
                    ffi::XI_TouchEnd => TouchPhase::Ended,
                    _ => unreachable!()
                };
                Some(Event::Touch(Touch {
                    phase: phase,
                    location: (event_data.event_x, event_data.event_y),
                    id: event_data.detail as u64,
                }))
            }
            _ => None
        }
    }
}

fn read_input_axis_info(display: &Arc<XConnection>) -> Vec<Axis> {
    let mut axis_list = Vec::new();
    let mut device_count = 0;

    // Check all input devices for scroll axes.
    let devices = unsafe{
        (display.xinput2.XIQueryDevice)(display.display, ffi::XIAllDevices, &mut device_count)
    };
    for i in 0..device_count {
        let device = unsafe { *(devices.offset(i as isize)) };
        for k in 0..device.num_classes {
            let class = unsafe { *(device.classes.offset(k as isize)) };
            match unsafe { (*class)._type } {
                // Note that scroll axis
                // are reported both as 'XIScrollClass' and 'XIValuatorClass'
                // axes. For the moment we only care about scrolling axes.
                ffi::XIScrollClass => {
                    let scroll_class: &ffi::XIScrollClassInfo = unsafe{mem::transmute(class)};
                    axis_list.push(Axis{
                        id: scroll_class.sourceid,
                        device_id: device.deviceid,
                        axis_number: scroll_class.number,
                        axis_type: match scroll_class.scroll_type {
                            ffi::XIScrollTypeHorizontal => AxisType::HorizontalScroll,
                            ffi::XIScrollTypeVertical => AxisType::VerticalScroll,
                            _ => { unreachable!() }
                        },
                        scroll_increment: scroll_class.increment,
                    })
                },
                _ => {}
            }
        }
    }
    
    axis_list
}

/// Given an input motion event for an axis and the previous
/// state of the axes, return the horizontal/vertical
/// scroll deltas
fn calc_scroll_deltas(event: &ffi::XIDeviceEvent,
                     axis_id: i32,
                     axis_value: f64,
                     axis_list: &[Axis],
                     prev_axis_values: &mut Vec<AxisValue>) -> (f64, f64) {
    let prev_value_pos = prev_axis_values.iter().position(|prev_axis| {
        prev_axis.device_id == event.sourceid &&
            prev_axis.axis_number == axis_id
    });
    let delta = match prev_value_pos {
        Some(idx) => prev_axis_values[idx].value - axis_value,
        None => 0.0
    };

    let new_axis_value = AxisValue{
        device_id: event.sourceid,
        axis_number: axis_id,
        value: axis_value
    };

    match prev_value_pos {
        Some(idx) => prev_axis_values[idx] = new_axis_value,
        None => prev_axis_values.push(new_axis_value)
    }

    let mut scroll_delta = (0.0, 0.0);

    for axis in axis_list.iter() {
        if axis.id == event.sourceid &&
            axis.axis_number == axis_id {
                match axis.axis_type {
                    AxisType::HorizontalScroll => scroll_delta.0 = delta / axis.scroll_increment,
                    AxisType::VerticalScroll => scroll_delta.1 = delta / axis.scroll_increment
                }
            }
    }

    scroll_delta
}

