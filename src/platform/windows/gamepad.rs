use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use winapi::um::winnt::HANDLE;
use winapi::um::winuser::RAWINPUT;

use events::{AxisHint, ButtonHint, ElementState};
use platform::platform::raw_input::{get_raw_input_device_name, RawGamepad};
use platform::platform::xinput::{self, XInputGamepad};

lazy_static! {
    pub static ref GAMEPADS: Arc<Mutex<GamepadStore>> = Default::default();
}

#[derive(Debug)]
pub struct GamepadStore {
    gamepads: HashMap<isize, Gamepad>,
}

impl GamepadStore {
    pub fn get_or_add(&mut self, handle: HANDLE) -> Option<&mut Gamepad> {
        let key = handle as isize;
        let gamepad_registered = self.gamepads.contains_key(&key);
        if !gamepad_registered {
            self.gamepads.insert(key, Gamepad::new(handle)?);
        }
        self.gamepads.get_mut(&key)
    }

    pub fn remove(&mut self, handle: HANDLE) {
        let key = handle as isize;
        self.gamepads.remove(&key);
    }
}

impl Default for GamepadStore {
    fn default() -> Self {
        GamepadStore {
            gamepads: HashMap::with_capacity(4),
        }
    }
}

unsafe impl Send for GamepadStore {}
unsafe impl Sync for GamepadStore {}

#[derive(Debug)]
pub enum GamepadType {
    Raw(RawGamepad),
    XInput(XInputGamepad),
}

#[derive(Debug)]
pub struct Gamepad {
    handle: HANDLE,
    backend: GamepadType,
}

impl Gamepad {
    pub fn new(handle: HANDLE) -> Option<Self> {
        // TODO: Verify that this is an HID device
        let name = get_raw_input_device_name(handle)?;
        xinput::id_from_name(&name)
            .and_then(XInputGamepad::new)
            .map(GamepadType::XInput)
            .or_else(|| RawGamepad::new(handle)
                .map(GamepadType::Raw))
            .map(|backend| Gamepad {
                handle,
                backend,
            })
    }

    pub unsafe fn update_state(&mut self, input: *mut RAWINPUT) -> Option<()> {
        match self.backend {
            GamepadType::Raw(ref mut gamepad) => gamepad.update_state(input),
            GamepadType::XInput(ref mut gamepad) => gamepad.update_state(),
        }
    }

    pub fn get_changed_buttons(&self) -> Vec<(u32, Option<ButtonHint>, ElementState)> {
        match self.backend {
            GamepadType::Raw(ref gamepad) => gamepad.get_changed_buttons(),
            GamepadType::XInput(ref gamepad) => gamepad.get_changed_buttons(),
        }
    }

    pub fn get_changed_axes(&self) -> Vec<(u32, Option<AxisHint>, f64)> {
        match self.backend {
            GamepadType::Raw(ref gamepad) => gamepad.get_changed_axes(),
            GamepadType::XInput(ref gamepad) => gamepad.get_changed_axes(),
        }
    }

    pub fn rumble(&mut self, left_speed: u16, right_speed: u16) {
        match self.backend {
            GamepadType::Raw(ref mut gamepad) => gamepad.rumble(left_speed, right_speed),
            GamepadType::XInput(ref mut gamepad) => gamepad.rumble(left_speed, right_speed),
        }
    }
}
