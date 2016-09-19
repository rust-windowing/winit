use std::collections::HashSet;

use TouchPhase;
use Event as GlutinEvent;
use ElementState;
use MouseButton;
use MouseScrollDelta;

use wayland_client::Event as WaylandEvent;
use wayland_client::ProxyId;
use wayland_client::wayland::WaylandProtocolEvent as WPE;
use wayland_client::wayland::seat::{WlSeat, WlSeatEvent, WlPointerEvent,
                                    WlPointerButtonState,
                                    WlPointerAxis, WlSeatCapability};

use super::wayland_kbd::MappedKeyboard;

use super::context::WaylandFocuses;

pub fn translate_event(
    evt: WaylandEvent,
    focuses: &mut WaylandFocuses,
    known_surfaces: &HashSet<ProxyId>,
    seat: Option<&WlSeat>,
    ) -> Option<(GlutinEvent, ProxyId)>
{
    let WaylandEvent::Wayland(wayland_evt) = evt;
    match wayland_evt {
        WPE::WlSeat(_, seat_evt) => match seat_evt {
            WlSeatEvent::Capabilities(cap) => {
                if cap.contains(WlSeatCapability::Pointer) && focuses.pointer.is_none() {
                    if let Some(seat) = seat {
                        focuses.pointer = Some(seat.get_pointer());
                    }
                }
                if cap.contains(WlSeatCapability::Keyboard) && focuses.keyboard.is_none() {
                    if let Some(seat) = seat {
                        match MappedKeyboard::new(seat) {
                            Ok(mk) => {
                                focuses.keyboard = Some(mk)
                            },
                            Err(_) => {}
                        }
                    }
                }
                None
            },
            _ => None
        },
        WPE::WlPointer(_, pointer_evt) => match pointer_evt {
            WlPointerEvent::Enter(_, surface, x, y) => {
                if known_surfaces.contains(&surface) {
                    focuses.pointer_on = Some(surface);
                    focuses.pointer_at = Some((x, y));
                    Some((GlutinEvent::MouseMoved(x as i32, y as i32), surface))
                } else {
                    None
                }
            }
            WlPointerEvent::Leave(_, _) => {
                focuses.pointer_on = None;
                focuses.pointer_at = None;
                None
            }
            WlPointerEvent::Motion(_, x, y) => {
                if let Some(surface) = focuses.pointer_on {
                    focuses.pointer_at = Some((x, y));
                    Some((GlutinEvent::MouseMoved(x as i32, y as i32), surface))
                } else {
                    None
                }
            }
            WlPointerEvent::Button(_, _, button, state) => {
                if let Some(surface) = focuses.pointer_on {
                    Some((GlutinEvent::MouseInput(
                        match state {
                            WlPointerButtonState::Pressed => ElementState::Pressed,
                            WlPointerButtonState::Released => ElementState::Released
                        },
                        match button {
                            0x110 => MouseButton::Left,
                            0x111 => MouseButton::Right,
                            0x112 => MouseButton::Middle,
                            // TODO figure out the translation ?
                            _ => return None
                        }
                    ), surface))
                } else {
                    None
                }
            }
            WlPointerEvent::Axis(_, axis, amplitude) => {
                if let Some(surface) = focuses.pointer_on {
                    Some((GlutinEvent::MouseWheel(
                        match axis {
                            WlPointerAxis::VerticalScroll => {
                                MouseScrollDelta::PixelDelta(amplitude as f32, 0.0)
                            }
                            WlPointerAxis::HorizontalScroll => {
                                MouseScrollDelta::PixelDelta(0.0, amplitude as f32)
                            }
                        },
                        TouchPhase::Moved
                    ), surface))
                } else {
                    None
                }
            }
        },
        _ => None
    }
}
