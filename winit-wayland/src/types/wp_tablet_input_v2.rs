//! Handling of wp_tablet_input_v2.

use std::sync::Mutex;

use dpi::LogicalPosition;
use sctk::compositor::SurfaceData;
use sctk::globals::GlobalData;
use sctk::reexports::client::backend::smallvec::SmallVec;
use sctk::reexports::client::globals::{BindError, GlobalList};
use sctk::reexports::client::protocol::wl_seat::WlSeat;
use sctk::reexports::client::protocol::wl_surface::WlSurface;
use sctk::reexports::client::{
    Connection, Dispatch, Proxy, QueueHandle, WEnum, delegate_dispatch, event_created_child,
};
use sctk::reexports::protocols::wp::tablet::zv2::client::zwp_tablet_manager_v2::ZwpTabletManagerV2;
use sctk::reexports::protocols::wp::tablet::zv2::client::zwp_tablet_pad_v2::ZwpTabletPadV2;
use sctk::reexports::protocols::wp::tablet::zv2::client::zwp_tablet_seat_v2::{
    self, ZwpTabletSeatV2,
};
use sctk::reexports::protocols::wp::tablet::zv2::client::zwp_tablet_tool_v2::{
    ButtonState, Event as ToolEvent, Type as ToolType, ZwpTabletToolV2,
};
use sctk::reexports::protocols::wp::tablet::zv2::client::zwp_tablet_v2::ZwpTabletV2;
use wayland_protocols::wp::tablet::zv2::client::zwp_tablet_pad_group_v2::ZwpTabletPadGroupV2;
use wayland_protocols::wp::tablet::zv2::client::zwp_tablet_pad_v2;
use winit_core::event::{
    ButtonSource, ElementState, Force, PointerKind, PointerSource, TabletToolButton,
    TabletToolData as CoreTabletToolData, TabletToolKind, TabletToolTilt, WindowEvent,
};

use crate::state::WinitState;

/// KWin blur manager.
#[derive(Debug, Clone)]
pub struct TabletManager {
    manager: ZwpTabletManagerV2,
}

impl TabletManager {
    pub fn new(
        globals: &GlobalList,
        queue_handle: &QueueHandle<WinitState>,
    ) -> Result<Self, BindError> {
        // Ignore v2 since we are not interested in its events.
        let manager = globals.bind(queue_handle, 1..=1, GlobalData)?;
        Ok(Self { manager })
    }

    pub fn get_tablet_seat(
        &self,
        seat: &WlSeat,
        queue_handle: &QueueHandle<WinitState>,
    ) -> ZwpTabletSeatV2 {
        self.manager.get_tablet_seat(seat, queue_handle, ())
    }
}
impl Dispatch<ZwpTabletManagerV2, GlobalData, WinitState> for TabletManager {
    fn event(
        _: &mut WinitState,
        _: &ZwpTabletManagerV2,
        _: <ZwpTabletManagerV2 as Proxy>::Event,
        _: &GlobalData,
        _: &Connection,
        _: &QueueHandle<WinitState>,
    ) {
        unreachable!("no events defined for zwp_tablet_manager_v2");
    }
}

impl Dispatch<ZwpTabletManagerV2, (), WinitState> for TabletManager {
    fn event(
        _: &mut WinitState,
        _: &ZwpTabletManagerV2,
        _: <ZwpTabletManagerV2 as Proxy>::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<WinitState>,
    ) {
        unreachable!("no events defined for zwp_tablet_manager_v2");
    }
}

impl Dispatch<ZwpTabletSeatV2, (), WinitState> for TabletManager {
    event_created_child!(WinitState, ZwpTabletSeatV2, [
        zwp_tablet_seat_v2::EVT_TABLET_ADDED_OPCODE => (ZwpTabletV2, Default::default()),
        zwp_tablet_seat_v2::EVT_TOOL_ADDED_OPCODE => (ZwpTabletToolV2, Default::default()),
        zwp_tablet_seat_v2::EVT_PAD_ADDED_OPCODE => (ZwpTabletPadV2, Default::default())
    ]);

    fn event(
        _: &mut WinitState,
        _: &ZwpTabletSeatV2,
        _: <ZwpTabletSeatV2 as Proxy>::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<WinitState>,
    ) {
    }
}

impl Dispatch<ZwpTabletToolV2, TabletToolData, WinitState> for TabletManager {
    fn event(
        state: &mut WinitState,
        _: &ZwpTabletToolV2,
        event: <ZwpTabletToolV2 as Proxy>::Event,
        data: &TabletToolData,
        _: &Connection,
        _: &QueueHandle<WinitState>,
    ) {
        let mut data = data.inner.lock().unwrap();

        match event {
            ToolEvent::Type { tool_type: WEnum::Value(tool_type) } => {
                data.ty = match tool_type {
                    ToolType::Pen => TabletToolKind::Pen,
                    ToolType::Eraser => TabletToolKind::Eraser,
                    ToolType::Brush => TabletToolKind::Brush,
                    ToolType::Pencil => TabletToolKind::Pencil,
                    ToolType::Airbrush => TabletToolKind::Airbrush,
                    ToolType::Finger => TabletToolKind::Finger,
                    ToolType::Mouse => TabletToolKind::Mouse,
                    ToolType::Lens => TabletToolKind::Lens,
                    _ => return,
                };
            },
            ToolEvent::Capability { .. } => {},
            ToolEvent::Done => (),
            ToolEvent::ProximityIn { serial, surface, .. } => {
                data.pending.push(TabletEvent::Enter { serial, surface });
            },
            ToolEvent::ProximityOut => data.pending.push(TabletEvent::Left),
            ToolEvent::Down { serial } => {
                let event = TabletEvent::Button {
                    state: ElementState::Pressed,
                    serial: Some(serial),
                    button: TabletToolButton::Contact,
                };

                data.pending.push(event);
            },
            ToolEvent::Up => {
                let event = TabletEvent::Button {
                    state: ElementState::Released,
                    serial: None,
                    button: TabletToolButton::Contact,
                };

                data.pending.push(event);
            },
            ToolEvent::Tilt { tilt_x, tilt_y } => {
                data.tool_state.tilt = Some(TabletToolTilt { x: tilt_x as i8, y: tilt_y as i8 });
            },
            ToolEvent::Motion { x, y } => {
                data.position = (x, y).into();
                data.pending.push(TabletEvent::Moved);
            },
            ToolEvent::Pressure { pressure } => {
                data.tool_state.force = Some(Force::Normalized(pressure as f64 / u16::MAX as f64));
            },
            ToolEvent::Rotation { degrees } => {
                data.tool_state.twist = Some(degrees as u16);
            },
            ToolEvent::Button { serial, button, state: WEnum::Value(state) } => {
                let state = match state {
                    ButtonState::Released => ElementState::Released,
                    ButtonState::Pressed => ElementState::Pressed,
                    _ => return,
                };

                // Map similar to SDL.
                let button = match button {
                    // BTN_STYLUS.
                    0x14b => TabletToolButton::Contact,
                    0x14c => TabletToolButton::Barrel,
                    // BTN_STYLUS3.
                    0x149 => TabletToolButton::Other(1),
                    // There's no defined conversion for any of that.
                    button => TabletToolButton::Other(button as u16),
                };

                let event = TabletEvent::Button { button, state, serial: Some(serial) };
                data.pending.push(event);
            },
            ToolEvent::Frame { .. } => {
                let kind = data.ty;
                for event in std::mem::take(&mut data.pending) {
                    if let TabletEvent::Enter { surface, serial } = &event {
                        data.latest_enter_serial = Some(*serial);
                        data.surface = Some(surface.clone());
                    }

                    // Handle events only for top-level surface.
                    let surface = match data
                        .surface
                        .as_ref()
                        .map(|surface| (surface, surface.data::<SurfaceData>()))
                    {
                        Some((surface, Some(surface_data)))
                            if surface_data.parent_surface().is_none() =>
                        {
                            surface
                        },
                        _ => continue,
                    };

                    let window_id = crate::make_wid(surface);

                    // Ensure that window exists.
                    let window = match state.windows.get_mut().get_mut(&window_id) {
                        Some(window) => window.lock().unwrap(),
                        None => continue,
                    };

                    let position = data.position.to_physical(window.scale_factor());

                    let window_event = match event {
                        TabletEvent::Enter { .. } => WindowEvent::PointerEntered {
                            device_id: None,
                            position,
                            primary: true,
                            kind: PointerKind::TabletTool(kind),
                        },
                        TabletEvent::Moved => WindowEvent::PointerMoved {
                            device_id: None,
                            position,
                            primary: true,
                            source: PointerSource::TabletTool {
                                kind,
                                data: data.tool_state.clone(),
                            },
                        },
                        TabletEvent::Button { button, state, serial } => {
                            // Update serial if we have it.
                            if let Some(serial) = serial {
                                data.latest_button_serial = Some(serial);
                            }

                            WindowEvent::PointerButton {
                                device_id: None,
                                state,
                                position,
                                primary: true,
                                button: ButtonSource::TabletTool {
                                    kind,
                                    button,
                                    data: data.tool_state.clone(),
                                },
                            }
                        },
                        TabletEvent::Left => WindowEvent::PointerLeft {
                            device_id: None,
                            position: Some(position),
                            primary: true,
                            kind: PointerKind::TabletTool(kind),
                        },
                    };

                    state.events_sink.push_window_event(window_event, window_id);

                    // Clear up the surface after we've processed `Left` event.
                    if matches!(event, TabletEvent::Left) {
                        data.surface = None;
                        data.latest_button_serial = None;
                        data.latest_enter_serial = None;
                        data.tool_state = Default::default();
                    }
                }
            },
            _ => (),
        }
    }
}

#[derive(Debug, Default)]
struct TabletToolData {
    inner: Mutex<TabletToolDataInner>,
}

#[derive(Debug, Default)]
pub(crate) struct TabletToolDataInner {
    pub(crate) ty: TabletToolKind,

    /// Core tablet tool data.
    pub(crate) tool_state: CoreTabletToolData,

    /// Pending events until the `frame` is received.
    pub(crate) pending: SmallVec<[TabletEvent; 4]>,

    /// Surface the tablet most recently entered.
    pub(crate) surface: Option<WlSurface>,

    /// Position relative to the surface.
    pub(crate) position: LogicalPosition<f64>,

    // NOTE: even though we don't utilize serials
    // right now, track them anyway.
    /// The serial of the latest enter event for the pointer
    pub(crate) latest_enter_serial: Option<u32>,

    /// The serial of the latest button event for the pointer
    pub(crate) latest_button_serial: Option<u32>,
}

// Due to wayland using logical coordinates,
// delay the conversion to physical until the dispatch actually happens,
// so the scaling is applied at the time of actual dispatch, since it
// can technically change before the `frame` event.
#[derive(Debug, Clone)]
pub(crate) enum TabletEvent {
    Enter { serial: u32, surface: WlSurface },
    Left,
    Moved,
    Button { button: TabletToolButton, state: ElementState, serial: Option<u32> },
}

impl Dispatch<ZwpTabletV2, (), WinitState> for TabletManager {
    fn event(
        _: &mut WinitState,
        _: &ZwpTabletV2,
        _: <ZwpTabletV2 as Proxy>::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<WinitState>,
    ) {
    }
}

impl Dispatch<ZwpTabletPadV2, (), WinitState> for TabletManager {
    event_created_child!(WinitState, ZwpTabletPadV2, [
        zwp_tablet_pad_v2::EVT_GROUP_OPCODE => (ZwpTabletPadGroupV2, Default::default()),
    ]);

    fn event(
        _: &mut WinitState,
        _: &ZwpTabletPadV2,
        _: <ZwpTabletPadV2 as Proxy>::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<WinitState>,
    ) {
    }
}

impl Dispatch<ZwpTabletPadGroupV2, (), WinitState> for TabletManager {
    fn event(
        _: &mut WinitState,
        _: &ZwpTabletPadGroupV2,
        _: <ZwpTabletPadGroupV2 as Proxy>::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<WinitState>,
    ) {
    }
}

delegate_dispatch!(WinitState: [ZwpTabletManagerV2: GlobalData] => TabletManager);
delegate_dispatch!(WinitState: [ZwpTabletManagerV2: ()] => TabletManager);
delegate_dispatch!(WinitState: [ZwpTabletSeatV2: ()] => TabletManager);
delegate_dispatch!(WinitState: [ZwpTabletV2: ()] => TabletManager);
delegate_dispatch!(WinitState: [ZwpTabletToolV2: TabletToolData] => TabletManager);
delegate_dispatch!(WinitState: [ZwpTabletPadV2: ()] => TabletManager);
delegate_dispatch!(WinitState: [ZwpTabletPadGroupV2: ()] => TabletManager);
