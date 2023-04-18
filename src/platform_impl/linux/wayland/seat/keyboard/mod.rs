//! The keyboard input handling.

use std::sync::Mutex;

use sctk::reexports::client::delegate_dispatch;
use sctk::reexports::client::protocol::wl_keyboard::WlKeyboard;
use sctk::reexports::client::protocol::wl_seat::WlSeat;
use sctk::reexports::client::protocol::wl_surface::WlSurface;
use sctk::reexports::client::{Connection, Proxy, QueueHandle};

use sctk::seat::keyboard::{KeyEvent, KeyboardData, KeyboardDataExt, KeyboardHandler, Modifiers};
use sctk::seat::SeatState;

use crate::event::{ElementState, ModifiersState, WindowEvent};

use crate::platform_impl::wayland::state::WinitState;
use crate::platform_impl::wayland::{self, DeviceId, WindowId};

mod keymap;

impl WinitState {
    pub fn handle_key_input(
        &mut self,
        keyboard: &WlKeyboard,
        event: KeyEvent,
        state: ElementState,
    ) {
        let window_id = match *keyboard.winit_data().window_id.lock().unwrap() {
            Some(window_id) => window_id,
            None => return,
        };

        let virtual_keycode = keymap::keysym_to_vkey(event.keysym);

        let seat_state = self.seats.get(&keyboard.seat().id()).unwrap();
        let modifiers = seat_state.modifiers;

        self.events_sink.push_window_event(
            #[allow(deprecated)]
            WindowEvent::KeyboardInput {
                device_id: crate::event::DeviceId(crate::platform_impl::DeviceId::Wayland(
                    DeviceId,
                )),
                input: crate::event::KeyboardInput {
                    state,
                    scancode: event.raw_code,
                    virtual_keycode,
                    modifiers,
                },
                is_synthetic: false,
            },
            window_id,
        );

        // Don't send utf8 chars on release.
        if state == ElementState::Released {
            return;
        }

        if let Some(txt) = event.utf8 {
            for ch in txt.chars() {
                self.events_sink
                    .push_window_event(WindowEvent::ReceivedCharacter(ch), window_id);
            }
        }
    }
}

impl KeyboardHandler for WinitState {
    fn enter(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        keyboard: &WlKeyboard,
        surface: &WlSurface,
        _serial: u32,
        _raw: &[u32],
        _keysyms: &[u32],
    ) {
        let window_id = wayland::make_wid(surface);

        // Mark the window as focused.
        match self.windows.get_mut().get(&window_id) {
            Some(window) => window.lock().unwrap().set_has_focus(true),
            None => return,
        };

        // Window gained focus.
        self.events_sink
            .push_window_event(WindowEvent::Focused(true), window_id);

        *keyboard.winit_data().window_id.lock().unwrap() = Some(window_id);

        // NOTE: GNOME still hasn't violates the specification wrt ordering of such
        // events. See https://gitlab.gnome.org/GNOME/mutter/-/issues/2231.
        let seat_state = self.seats.get_mut(&keyboard.seat().id()).unwrap();
        if std::mem::take(&mut seat_state.modifiers_pending) {
            self.events_sink.push_window_event(
                WindowEvent::ModifiersChanged(seat_state.modifiers),
                window_id,
            );
        }
    }

    fn leave(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        keyboard: &WlKeyboard,
        surface: &WlSurface,
        _serial: u32,
    ) {
        let window_id = wayland::make_wid(surface);

        // XXX The check whether the window exists is essential as we might get a nil surface...
        match self.windows.get_mut().get(&window_id) {
            Some(window) => window.lock().unwrap().set_has_focus(false),
            None => return,
        };

        // Notify that no modifiers are being pressed.
        self.events_sink.push_window_event(
            WindowEvent::ModifiersChanged(ModifiersState::empty()),
            window_id,
        );

        *keyboard.winit_data().window_id.lock().unwrap() = None;

        // Window lost focus.
        self.events_sink
            .push_window_event(WindowEvent::Focused(false), window_id);
    }

    fn press_key(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        keyboard: &WlKeyboard,
        _serial: u32,
        event: KeyEvent,
    ) {
        self.handle_key_input(keyboard, event, ElementState::Pressed);
    }

    fn release_key(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        keyboard: &WlKeyboard,
        _serial: u32,
        event: KeyEvent,
    ) {
        self.handle_key_input(keyboard, event, ElementState::Released);
    }

    // FIXME(kchibisov): recent spec suggested that this event could be sent
    // without any focus to indicate modifiers for the pointer, so update
    // for will be required once https://github.com/rust-windowing/winit/issues/2768
    // wrt winit design of the event is resolved.
    fn update_modifiers(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        keyboard: &WlKeyboard,
        _serial: u32,
        modifiers: Modifiers,
    ) {
        let modifiers = ModifiersState::from(modifiers);
        let mut seat_state = self.seats.get_mut(&keyboard.seat().id()).unwrap();
        seat_state.modifiers = modifiers;

        // NOTE: part of the workaround from `fn enter`, see it above.
        let window_id = match *keyboard.winit_data().window_id.lock().unwrap() {
            Some(window_id) => window_id,
            None => {
                seat_state.modifiers_pending = true;
                return;
            }
        };

        self.events_sink
            .push_window_event(WindowEvent::ModifiersChanged(modifiers), window_id);
    }
}

/// The extension to KeyboardData used to store the `window_id`.
pub struct WinitKeyboardData {
    /// The currently focused window surface. Could be `None` on bugged compositors, like mutter.
    window_id: Mutex<Option<WindowId>>,

    /// The original keyboard date.
    keyboard_data: KeyboardData<WinitState>,
}

impl WinitKeyboardData {
    pub fn new(seat: WlSeat) -> Self {
        Self {
            window_id: Default::default(),
            keyboard_data: KeyboardData::new(seat),
        }
    }
}

impl KeyboardDataExt for WinitKeyboardData {
    type State = WinitState;

    fn keyboard_data(&self) -> &KeyboardData<Self::State> {
        &self.keyboard_data
    }

    fn keyboard_data_mut(&mut self) -> &mut KeyboardData<Self::State> {
        &mut self.keyboard_data
    }
}

pub trait WinitKeyboardDataExt {
    fn winit_data(&self) -> &WinitKeyboardData;

    fn seat(&self) -> &WlSeat {
        self.winit_data().keyboard_data().seat()
    }
}

impl WinitKeyboardDataExt for WlKeyboard {
    fn winit_data(&self) -> &WinitKeyboardData {
        self.data::<WinitKeyboardData>()
            .expect("failed to get keyboard data.")
    }
}

impl From<Modifiers> for ModifiersState {
    fn from(mods: Modifiers) -> ModifiersState {
        let mut wl_mods = ModifiersState::empty();
        wl_mods.set(ModifiersState::SHIFT, mods.shift);
        wl_mods.set(ModifiersState::CTRL, mods.ctrl);
        wl_mods.set(ModifiersState::ALT, mods.alt);
        wl_mods.set(ModifiersState::LOGO, mods.logo);
        wl_mods
    }
}

delegate_dispatch!(WinitState: [ WlKeyboard: WinitKeyboardData] => SeatState);
