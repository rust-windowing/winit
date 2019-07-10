#![cfg(feature = "serde")]

use serde::{Deserialize, Serialize};
use winit::{
    dpi::{LogicalPosition, LogicalSize, PhysicalPosition, PhysicalSize},
    event::{
        ElementState, KeyboardInput, ModifiersState, MouseButton, MouseScrollDelta, TouchPhase,
        VirtualKeyCode,
    },
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
    needs_serde::<KeyboardInput>();
    needs_serde::<TouchPhase>();
    needs_serde::<ElementState>();
    needs_serde::<MouseButton>();
    needs_serde::<MouseScrollDelta>();
    needs_serde::<VirtualKeyCode>();
    needs_serde::<ModifiersState>();
}

#[test]
fn dpi_serde() {
    needs_serde::<LogicalPosition>();
    needs_serde::<PhysicalPosition>();
    needs_serde::<LogicalSize>();
    needs_serde::<PhysicalSize>();
}
