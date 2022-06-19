//! Wayland keyboard handling.

use std::cell::RefCell;
use std::rc::Rc;

use sctk::reexports::client::protocol::wl_keyboard::WlKeyboard;
use sctk::reexports::client::protocol::wl_seat::WlSeat;
use sctk::reexports::client::Attached;

use sctk::reexports::calloop::LoopHandle;

use sctk::seat::keyboard;

use crate::event::ModifiersState;
use crate::platform_impl::wayland::event_loop::WinitState;
use crate::platform_impl::wayland::WindowId;

mod handlers;
mod keymap;

pub(crate) struct Keyboard {
    pub keyboard: WlKeyboard,
}

impl Keyboard {
    pub fn new(
        seat: &Attached<WlSeat>,
        loop_handle: LoopHandle<'static, WinitState>,
        modifiers_state: Rc<RefCell<ModifiersState>>,
    ) -> Option<Self> {
        let mut inner = KeyboardInner::new(modifiers_state);
        let keyboard = keyboard::map_keyboard_repeat(
            loop_handle.clone(),
            seat,
            None,
            keyboard::RepeatKind::System,
            move |event, _, mut dispatch_data| {
                let winit_state = dispatch_data.get::<WinitState>().unwrap();
                handlers::handle_keyboard(event, &mut inner, winit_state);
            },
        )
        .ok()?;

        Some(Self { keyboard })
    }
}

impl Drop for Keyboard {
    fn drop(&mut self) {
        if self.keyboard.as_ref().version() >= 3 {
            self.keyboard.release();
        }
    }
}

struct KeyboardInner {
    /// Currently focused surface.
    target_window_id: Option<WindowId>,

    /// A pending state of modifiers.
    ///
    /// This state is getting set if we've got a modifiers update
    /// before `Enter` event, which shouldn't happen in general, however
    /// some compositors are still doing so.
    pending_modifers_state: Option<ModifiersState>,

    /// Current state of modifiers keys.
    modifiers_state: Rc<RefCell<ModifiersState>>,
}

impl KeyboardInner {
    fn new(modifiers_state: Rc<RefCell<ModifiersState>>) -> Self {
        Self {
            target_window_id: None,
            pending_modifers_state: None,
            modifiers_state,
        }
    }
}

impl From<keyboard::ModifiersState> for ModifiersState {
    fn from(mods: keyboard::ModifiersState) -> ModifiersState {
        let mut wl_mods = ModifiersState::empty();
        wl_mods.set(ModifiersState::SHIFT, mods.shift);
        wl_mods.set(ModifiersState::CTRL, mods.ctrl);
        wl_mods.set(ModifiersState::ALT, mods.alt);
        wl_mods.set(ModifiersState::LOGO, mods.logo);
        wl_mods
    }
}
