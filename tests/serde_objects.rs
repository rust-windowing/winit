#![cfg(feature = "serde")]

use serde::{Deserialize, Serialize};
use winit::{
    dpi::{Pixel, LogicalPosition, LogicalSize, PhysicalPosition, PhysicalSize, PhysicalDelta, LogicalDelta, UnitlessDelta},
    event::{ModifiersState, PointerButton, LogicalKey},
    window::CursorIcon,
};

#[allow(dead_code)]
fn needs_serde<S: Serialize + Deserialize<'static>>() {}

#[test]
fn window_serde() {
    needs_serde::<CursorIcon>();
}

#[test]
fn events_serde() {
    needs_serde::<ModifiersState>();
    needs_serde::<PointerButton>();
    needs_serde::<LogicalKey>();
}

pub fn dpi_serde<T: Pixel + Serialize + for<'a> Deserialize<'a>>() {
    needs_serde::<LogicalPosition<T>>();
    needs_serde::<PhysicalPosition<T>>();
    needs_serde::<LogicalSize<T>>();
    needs_serde::<PhysicalSize<T>>();
    needs_serde::<LogicalDelta<T>>();
    needs_serde::<PhysicalDelta<T>>();
    needs_serde::<UnitlessDelta<T>>();
}
