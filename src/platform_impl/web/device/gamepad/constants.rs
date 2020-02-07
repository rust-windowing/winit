use super::EventCode;

pub const AXIS_LSTICKX: EventCode = EventCode(0);
pub const AXIS_LSTICKY: EventCode = EventCode(1);
// pub const AXIS_LEFTZ: EventCode = EventCode(2);
pub const AXIS_RSTICKX: EventCode = EventCode(3);
pub const AXIS_RSTICKY: EventCode = EventCode(4);
// pub const AXIS_RIGHTZ: EventCode = EventCode(5);
// pub const AXIS_DPADX: EventCode = EventCode(6);
// pub const AXIS_DPADY: EventCode = EventCode(7);
pub const AXIS_RT: EventCode = EventCode(8);
pub const AXIS_LT: EventCode = EventCode(9);
// pub const AXIS_RT2: EventCode = EventCode(10);
// pub const AXIS_LT2: EventCode = EventCode(11);

pub const BTN_SOUTH: EventCode = EventCode(12);
pub const BTN_EAST: EventCode = EventCode(13);
// pub const BTN_C: EventCode = EventCode(14);
pub const BTN_NORTH: EventCode = EventCode(15);
pub const BTN_WEST: EventCode = EventCode(16);
// pub const BTN_Z: EventCode = EventCode(17);
pub const BTN_LT: EventCode = EventCode(18);
pub const BTN_RT: EventCode = EventCode(19);
pub const BTN_LT2: EventCode = EventCode(20);
pub const BTN_RT2: EventCode = EventCode(21);
pub const BTN_SELECT: EventCode = EventCode(22);
pub const BTN_START: EventCode = EventCode(23);
pub const BTN_MODE: EventCode = EventCode(24);
pub const BTN_LTHUMB: EventCode = EventCode(25);
pub const BTN_RTHUMB: EventCode = EventCode(26);

pub const BTN_DPAD_UP: EventCode = EventCode(27);
pub const BTN_DPAD_DOWN: EventCode = EventCode(28);
pub const BTN_DPAD_LEFT: EventCode = EventCode(29);
pub const BTN_DPAD_RIGHT: EventCode = EventCode(30);

pub(crate) static BUTTONS: [EventCode; 17] = [
    BTN_SOUTH,
    BTN_EAST,
    BTN_WEST,
    BTN_NORTH,
    BTN_LT,
    BTN_RT,
    BTN_LT2,
    BTN_RT2,
    BTN_SELECT,
    BTN_START,
    BTN_LTHUMB,
    BTN_RTHUMB,
    BTN_DPAD_UP,
    BTN_DPAD_DOWN,
    BTN_DPAD_LEFT,
    BTN_DPAD_RIGHT,
    BTN_MODE,
];

pub(crate) static AXES: [EventCode; 4] = [AXIS_LSTICKX, AXIS_LSTICKY, AXIS_RSTICKX, AXIS_RSTICKY];

pub(crate) fn button_code(index: usize) -> EventCode {
    BUTTONS
        .get(index)
        .map(|ev| ev.clone())
        .unwrap_or(EventCode(index as u8 + 31))
}

pub(crate) fn axis_code(index: usize) -> EventCode {
    AXES.get(index)
        .map(|ev| ev.clone())
        .unwrap_or(EventCode((index + BUTTONS.len()) as u8 + 31))
}
