use smithay_client_toolkit::{
    reexports::client::protocol::{wl_pointer::{self, ButtonState}, wl_surface::WlSurface},
    get_surface_scale_factor,
    seat::pointer::ThemedPointer,
};
use {crate::{dpi::LogicalPosition, event::{WindowEvent, ElementState, MouseButton, TouchPhase, MouseScrollDelta}}, super::event_loop::Window};

// Track focus and reconstruct scroll events
pub struct Pointer {
    focus : Option<WlSurface>,
    axis_buffer: Option<(f32, f32)>,
    axis_discrete_buffer: Option<(i32, i32)>,
    phase: TouchPhase,
}
impl Default for Pointer { fn default() -> Self { Self{focus: None, axis_buffer: None, axis_discrete_buffer: None, phase: TouchPhase::Cancelled} } }

impl Pointer {
    pub fn handle(&mut self, pointer: ThemedPointer, mut send: impl FnMut(WindowEvent, &WlSurface), windows: &[Window], event : wl_pointer::Event) {
        let Self{focus, axis_buffer, axis_discrete_buffer, phase} = self;
        let device_id = crate::event::DeviceId(super::super::DeviceId::Wayland(super::DeviceId));
        let position = |surface,x,y| LogicalPosition::new(x, y).to_physical(get_surface_scale_factor(surface) as f64);
        use {wl_pointer::Event::*, crate::event::WindowEvent::*};
        match event {
            Enter { surface, surface_x:x,surface_y:y, .. } => if let Some(window) = windows.iter().find(|&w| w == &surface) /*=>*/ {
                // Reload cursor style only when we enter winit's surface.
                // FIXME: Might interfere with CSD
                pointer.set_cursor(window.current_cursor, None).expect("Unknown cursor");

                send(CursorEntered {device_id}, &surface);
                send(CursorMoved {device_id, position: position(&surface, x, y), modifiers: Default::default()}, &surface);
                *focus = Some(surface);
            }
            Leave { surface, .. } => {
                *focus = None;
                if windows.iter().any(|w| w == &surface) {
                    send(CursorLeft {device_id}, &surface);
                }
            }
            Motion { surface_x:x, surface_y:y, .. } => if let Some(surface) = focus /*=>*/ {
                send(CursorMoved {device_id, position: position(&surface, x, y), modifiers: Default::default()}, surface);
            }
            Button { button, state, .. } => if let Some(surface) = focus /*=>*/ {
                let state = if let ButtonState::Pressed = state { ElementState::Pressed } else { ElementState::Released};
                // input-event-codes
                let button = match button {
                    0x110 => MouseButton::Left,
                    0x111 => MouseButton::Right,
                    0x112 => MouseButton::Middle,
                    other => MouseButton::Other(other as u8 /*truncates*/),
                };
                send(MouseInput {device_id, state, button, modifiers: Default::default()}, surface);
            }
            Axis { axis, value, .. } => if focus.is_some() /*=>*/ {
                let (mut x, mut y) = axis_buffer.unwrap_or((0.0, 0.0));
                use wl_pointer::Axis::*;
                match axis { // wayland vertical sign convention is the inverse of winit
                    VerticalScroll => y -= value as f32,
                    HorizontalScroll => x += value as f32,
                    _ => unreachable!(),
                }
                *axis_buffer = Some((x, y));
                *phase = match phase {
                    TouchPhase::Started | TouchPhase::Moved => TouchPhase::Moved,
                    _ => TouchPhase::Started,
                }
            }
            Frame => {
                let delta =
                    if let Some((x,y)) = axis_buffer.take() { MouseScrollDelta::PixelDelta(LogicalPosition {x: x as f64, y: y as f64}) }
                    else if let Some((x,y)) = axis_discrete_buffer.take() { MouseScrollDelta::LineDelta(x as f32,y as f32) }
                    else { debug_assert!(false); MouseScrollDelta::PixelDelta(LogicalPosition {x: 0., y: 0.}) };
                if let Some(surface) = focus {
                    send(MouseWheel {device_id, delta, phase: *phase, modifiers: Default::default()}, surface);
                }
            }
            AxisSource { .. } => (),
            AxisStop { .. } => *phase = TouchPhase::Ended,
            AxisDiscrete { axis, discrete } => {
                let (mut x, mut y) = axis_discrete_buffer.unwrap_or((0, 0));
                use wl_pointer::Axis::*;
                match axis {
                    // wayland vertical sign convention is the inverse of winit
                    VerticalScroll => y -= discrete,
                    HorizontalScroll => x += discrete,
                    _ => unreachable!(),
                }
                *axis_discrete_buffer = Some((x, y));
                *phase = match phase {
                    TouchPhase::Started | TouchPhase::Moved => TouchPhase::Moved,
                    _ => TouchPhase::Started,
                }
            }
            _ => unreachable!(),
        }
    }
}
