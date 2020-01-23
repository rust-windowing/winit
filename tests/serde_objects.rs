#![cfg(feature = "serde_feature")]

use serde::{Deserialize, Serialize};
use winit::{
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
