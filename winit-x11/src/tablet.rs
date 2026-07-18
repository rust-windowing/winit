use std::slice;

use winit_core::event::{Force, TabletToolButton, TabletToolData, TabletToolKind, TabletToolTilt};
use x11_dl::xinput2;
use x11rb::protocol::xproto;

use crate::atoms::{ABS_PRESSURE, ABS_TILT_X, ABS_TILT_Y, ABS_X, ABS_Y, Atoms};
use crate::ffi;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum AxisLabel {
    X,
    Y,
    Pressure,
    TiltX,
    TiltY,
    Other,
}

#[derive(Clone, Debug)]
struct ValuatorAxis {
    number: i32,
    label: AxisLabel,
    absolute: bool,
    min: f64,
    max: f64,
    value: Option<f64>,
}

impl ValuatorAxis {
    fn new(number: i32, label: AxisLabel, absolute: bool, min: f64, max: f64, value: f64) -> Self {
        Self { number, label, absolute, min, max, value: value.is_finite().then_some(value) }
    }

    fn update(&mut self, number: i32, value: f64) {
        if self.number == number && value.is_finite() {
            self.value = Some(value);
        }
    }
}

#[derive(Clone, Debug)]
pub(crate) struct TabletDevice {
    pub(crate) kind: TabletToolKind,
    pressure: Option<ValuatorAxis>,
    tilt_x: Option<ValuatorAxis>,
    tilt_y: Option<ValuatorAxis>,
}

impl TabletDevice {
    pub(crate) fn from_xinput(
        name: &str,
        classes: &[*const ffi::XIAnyClassInfo],
        atoms: &Atoms,
    ) -> Option<Self> {
        let mut has_touch_class = false;
        let mut axes = Vec::new();

        for &class_ptr in classes {
            let class_type = unsafe { (*class_ptr)._type };
            if class_type == ffi::XITouchClass {
                has_touch_class = true;
            } else if class_type == ffi::XIValuatorClass {
                let info = unsafe { &*(class_ptr as *const ffi::XIValuatorClassInfo) };
                let atom = info.label as xproto::Atom;
                let label = if atom == atoms[ABS_X] {
                    AxisLabel::X
                } else if atom == atoms[ABS_Y] {
                    AxisLabel::Y
                } else if atom == atoms[ABS_PRESSURE] {
                    AxisLabel::Pressure
                } else if atom == atoms[ABS_TILT_X] {
                    AxisLabel::TiltX
                } else if atom == atoms[ABS_TILT_Y] {
                    AxisLabel::TiltY
                } else {
                    AxisLabel::Other
                };
                axes.push(ValuatorAxis::new(
                    info.number,
                    label,
                    info.mode == ffi::XIModeAbsolute,
                    info.min,
                    info.max,
                    info.value,
                ));
            }
        }

        classify_device(name, has_touch_class, axes)
    }

    pub(crate) fn update_valuators(&mut self, valuators: &xinput2::XIValuatorState) {
        if valuators.mask_len <= 0 {
            return;
        }
        let mask = unsafe { slice::from_raw_parts(valuators.mask, valuators.mask_len as usize) };

        // XI2 packs one value for every set bit. The values are not indexed by axis number.
        unsafe {
            for_each_packed_valuator(mask, valuators.values, false, |number, value| {
                self.update_value(number, value);
            });
        }
    }

    fn update_value(&mut self, number: i32, value: f64) {
        if let Some(pressure) = self.pressure.as_mut() {
            pressure.update(number, value);
        }
        if let Some(tilt_x) = self.tilt_x.as_mut() {
            tilt_x.update(number, value);
        }
        if let Some(tilt_y) = self.tilt_y.as_mut() {
            tilt_y.update(number, value);
        }
    }

    pub(crate) fn data(&self) -> TabletToolData {
        let force = self.pressure.as_ref().and_then(|axis| {
            let value = axis.value?;
            let range = axis.max - axis.min;
            (range.is_finite() && range > 0.0)
                .then(|| Force::Normalized(((value - axis.min) / range).clamp(0.0, 1.0)))
        });

        let tilt_x = self.tilt_x.as_ref().and_then(|axis| axis.value).map(normalize_tilt);
        let tilt_y = self.tilt_y.as_ref().and_then(|axis| axis.value).map(normalize_tilt);
        let tilt = (tilt_x.is_some() || tilt_y.is_some())
            .then(|| TabletToolTilt { x: tilt_x.unwrap_or(0), y: tilt_y.unwrap_or(0) });

        TabletToolData { force, tangential_force: None, twist: None, tilt, angle: None }
    }
}

fn classify_device(
    name: &str,
    has_touch_class: bool,
    axes: Vec<ValuatorAxis>,
) -> Option<TabletDevice> {
    // Some touchscreens expose pressure or absolute axes too. The XI touch class is stronger
    // evidence than any tablet heuristic.
    if has_touch_class {
        return None;
    }

    let lower_name = name.to_ascii_lowercase();
    let strong_name = name_word(&lower_name, "stylus")
        || name_word(&lower_name, "pen")
        || name_word(&lower_name, "eraser")
        || name_word(&lower_name, "brush")
        || name_word(&lower_name, "pencil")
        || name_word(&lower_name, "airbrush")
        || name_word(&lower_name, "finger")
        || name_word(&lower_name, "mouse")
        || name_word(&lower_name, "cursor")
        || name_word(&lower_name, "puck")
        || name_word(&lower_name, "lens");

    let labelled_x = axes.iter().any(|axis| axis.label == AxisLabel::X && axis.absolute);
    let labelled_y = axes.iter().any(|axis| axis.label == AxisLabel::Y && axis.absolute);
    let fallback_x = axes.iter().any(|axis| axis.number == 0 && axis.absolute);
    let fallback_y = axes.iter().any(|axis| axis.number == 1 && axis.absolute);
    let has_position = (labelled_x && labelled_y) || (strong_name && fallback_x && fallback_y);

    let pressure = axes.iter().find(|axis| axis.label == AxisLabel::Pressure).cloned();
    let tilt_x = axes.iter().find(|axis| axis.label == AxisLabel::TiltX).cloned();
    let tilt_y = axes.iter().find(|axis| axis.label == AxisLabel::TiltY).cloned();
    let has_tablet_axes = pressure.is_some() || tilt_x.is_some() || tilt_y.is_some();

    if !has_position || (!has_tablet_axes && !strong_name) {
        return None;
    }

    let kind = if name_word(&lower_name, "eraser") {
        TabletToolKind::Eraser
    } else if name_word(&lower_name, "brush") {
        TabletToolKind::Brush
    } else if name_word(&lower_name, "pencil") {
        TabletToolKind::Pencil
    } else if name_word(&lower_name, "airbrush") {
        TabletToolKind::Airbrush
    } else if name_word(&lower_name, "finger") {
        TabletToolKind::Finger
    } else if name_word(&lower_name, "lens") {
        TabletToolKind::Lens
    } else if name_word(&lower_name, "mouse")
        || name_word(&lower_name, "cursor")
        || name_word(&lower_name, "puck")
    {
        TabletToolKind::Mouse
    } else {
        TabletToolKind::Pen
    };

    Some(TabletDevice { kind, pressure, tilt_x, tilt_y })
}

fn name_word(name: &str, word: &str) -> bool {
    name.split(|character: char| !character.is_ascii_alphanumeric()).any(|part| part == word)
}

fn normalize_tilt(value: f64) -> i8 {
    value.round().clamp(-90.0, 90.0) as i8
}

pub(crate) fn tablet_button(detail: u32) -> Option<TabletToolButton> {
    Some(match detail {
        1 => TabletToolButton::Contact,
        3 => TabletToolButton::Barrel,
        2 => TabletToolButton::Other(1),
        8 => TabletToolButton::Other(3),
        9 => TabletToolButton::Other(4),
        detail if detail > 0 && detail <= u16::MAX as u32 => TabletToolButton::Other(detail as u16),
        _ => return None,
    })
}

/// Visits XI2's packed valuator values in axis-number order.
///
/// # Safety
///
/// `values` must point to at least as many readable `f64`s as there are set bits in `mask`.
pub(crate) unsafe fn for_each_packed_valuator(
    mask: &[u8],
    mut values: *const f64,
    unaligned: bool,
    mut visitor: impl FnMut(i32, f64),
) {
    for number in 0..mask.len() * 8 {
        if mask[number / 8] & (1 << (number % 8)) == 0 {
            continue;
        }

        let value =
            if unaligned { unsafe { values.read_unaligned() } } else { unsafe { values.read() } };
        visitor(number as i32, value);
        values = unsafe { values.add(1) };
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn axis(number: i32, label: AxisLabel, value: f64) -> ValuatorAxis {
        ValuatorAxis::new(number, label, true, 0.0, 100.0, value)
    }

    #[test]
    fn touch_class_wins_over_tablet_axes() {
        let axes = vec![
            axis(0, AxisLabel::X, 0.0),
            axis(1, AxisLabel::Y, 0.0),
            axis(2, AxisLabel::Pressure, 50.0),
        ];
        assert!(classify_device("Pen touchscreen", true, axes).is_none());
    }

    #[test]
    fn absolute_xy_alone_is_not_a_tablet() {
        let axes = vec![axis(0, AxisLabel::X, 0.0), axis(1, AxisLabel::Y, 0.0)];
        assert!(classify_device("Generic absolute pointer", false, axes).is_none());
    }

    #[test]
    fn labels_and_pressure_classify_a_pen_without_name_heuristics() {
        let axes = vec![
            axis(0, AxisLabel::X, 0.0),
            axis(1, AxisLabel::Y, 0.0),
            axis(5, AxisLabel::Pressure, 25.0),
        ];
        let tablet = classify_device("Unknown device", false, axes).unwrap();
        assert_eq!(tablet.kind, TabletToolKind::Pen);
        assert_eq!(tablet.data().force, Some(Force::Normalized(0.25)));
    }

    #[test]
    fn strong_names_choose_supported_tool_kinds() {
        let axes = vec![axis(0, AxisLabel::Other, 0.0), axis(1, AxisLabel::Other, 0.0)];
        for (name, expected) in [
            ("Wacom Eraser", TabletToolKind::Eraser),
            ("Wacom Brush", TabletToolKind::Brush),
            ("Wacom Pencil", TabletToolKind::Pencil),
            ("Wacom Airbrush", TabletToolKind::Airbrush),
            ("Wacom Finger", TabletToolKind::Finger),
            ("Wacom Mouse", TabletToolKind::Mouse),
            ("Tablet Cursor Puck", TabletToolKind::Mouse),
            ("Tablet Lens", TabletToolKind::Lens),
        ] {
            assert_eq!(classify_device(name, false, axes.clone()).unwrap().kind, expected);
        }
    }

    #[test]
    fn packed_valuators_consume_only_set_bits() {
        let values = [12.0, 34.0, 56.0];
        let mut visited = Vec::new();
        unsafe {
            for_each_packed_valuator(&[0b0010_0101], values.as_ptr(), false, |axis, value| {
                visited.push((axis, value));
            });
        }
        assert_eq!(visited, [(0, 12.0), (2, 34.0), (5, 56.0)]);
    }

    #[test]
    fn packed_valuators_handle_empty_and_multiple_mask_bytes() {
        let mut empty = Vec::new();
        unsafe {
            for_each_packed_valuator(&[], std::ptr::null(), false, |axis, value| {
                empty.push((axis, value));
            });
        }
        assert!(empty.is_empty());

        let values = [1.0, 2.0];
        let mut visited = Vec::new();
        unsafe {
            for_each_packed_valuator(
                &[0b1000_0000, 0b0000_0010],
                values.as_ptr(),
                false,
                |a, v| {
                    visited.push((a, v));
                },
            );
        }
        assert_eq!(visited, [(7, 1.0), (9, 2.0)]);
    }

    #[test]
    fn tablet_data_normalizes_pressure_and_clamps_tilt() {
        let tablet = TabletDevice {
            kind: TabletToolKind::Pen,
            pressure: Some(ValuatorAxis::new(2, AxisLabel::Pressure, true, 10.0, 20.0, 25.0)),
            tilt_x: Some(ValuatorAxis::new(3, AxisLabel::TiltX, true, -64.0, 63.0, 91.0)),
            tilt_y: None,
        };
        let data = tablet.data();
        assert_eq!(data.force, Some(Force::Normalized(1.0)));
        assert_eq!(data.tilt, Some(TabletToolTilt { x: 90, y: 0 }));
    }

    #[test]
    fn invalid_pressure_range_and_non_finite_tilt_are_absent() {
        let tablet = TabletDevice {
            kind: TabletToolKind::Pen,
            pressure: Some(ValuatorAxis::new(2, AxisLabel::Pressure, true, 1.0, 1.0, 1.0)),
            tilt_x: Some(ValuatorAxis::new(3, AxisLabel::TiltX, true, -90.0, 90.0, f64::NAN)),
            tilt_y: None,
        };
        let data = tablet.data();
        assert_eq!(data.force, None);
        assert_eq!(data.tilt, None);
    }

    #[test]
    fn sparse_updates_retain_absent_axes_and_ignore_non_finite_values() {
        let mut tablet = TabletDevice {
            kind: TabletToolKind::Pen,
            pressure: Some(ValuatorAxis::new(2, AxisLabel::Pressure, true, 0.0, 100.0, 40.0)),
            tilt_x: Some(ValuatorAxis::new(3, AxisLabel::TiltX, true, -90.0, 90.0, 10.0)),
            tilt_y: None,
        };

        tablet.update_value(3, 20.0);
        assert_eq!(tablet.data().force, Some(Force::Normalized(0.4)));
        assert_eq!(tablet.data().tilt, Some(TabletToolTilt { x: 20, y: 0 }));

        tablet.update_value(2, f64::NAN);
        assert_eq!(tablet.data().force, Some(Force::Normalized(0.4)));
    }

    #[test]
    fn maps_x_buttons_to_tablet_buttons() {
        assert_eq!(tablet_button(1), Some(TabletToolButton::Contact));
        assert_eq!(tablet_button(3), Some(TabletToolButton::Barrel));
        assert_eq!(tablet_button(2), Some(TabletToolButton::Other(1)));
        assert_eq!(tablet_button(8), Some(TabletToolButton::Other(3)));
        assert_eq!(tablet_button(9), Some(TabletToolButton::Other(4)));
        assert_eq!(tablet_button(42), Some(TabletToolButton::Other(42)));
        assert_eq!(tablet_button(0), None);
        assert_eq!(tablet_button(u16::MAX as u32 + 1), None);
    }
}
