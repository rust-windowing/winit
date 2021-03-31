//! Wayland keyboard handling.

use std::cell::RefCell;
use std::rc::Rc;

use sctk::reexports::calloop::{LoopHandle, Source};
use sctk::reexports::client::protocol::wl_keyboard::WlKeyboard;
use sctk::reexports::client::protocol::wl_seat::WlSeat;
use sctk::reexports::client::Attached;

use crate::keyboard::ModifiersState;
use crate::platform_impl::platform::common::xkb_state;
use crate::platform_impl::wayland::event_loop::WinitState;
use crate::platform_impl::wayland::WindowId;

mod handlers;

pub(crate) struct Keyboard {
    pub keyboard: WlKeyboard,

    /// The source for repeat keys.
    pub repeat_source: Option<Source<handlers::RepeatSource>>,

    /// LoopHandle to drop `RepeatSource`, when dropping the keyboard.
    pub loop_handle: LoopHandle<WinitState>,
}

impl Keyboard {
    pub fn new(
        seat: &Attached<WlSeat>,
        loop_handle: LoopHandle<WinitState>,
        modifiers_state: Rc<RefCell<ModifiersState>>,
    ) -> Option<Self> {
        let mut inner = KeyboardInner::new(modifiers_state);
        let keyboard_data = handlers::map_keyboard_repeat(
            loop_handle.clone(),
            &seat,
            None,
            handlers::RepeatKind::System,
            move |event, _, mut dispatch_data| {
                let winit_state = dispatch_data.get::<WinitState>().unwrap();
                handlers::handle_keyboard(event, &mut inner, winit_state);
            },
        );

        let (keyboard, repeat_source) = keyboard_data.ok()?;

        Some(Self {
            keyboard,
            loop_handle,
            repeat_source: Some(repeat_source),
        })
    }
}

impl Drop for Keyboard {
    fn drop(&mut self) {
        if self.keyboard.as_ref().version() >= 3 {
            self.keyboard.release();
        }

        if let Some(repeat_source) = self.repeat_source.take() {
            self.loop_handle.remove(repeat_source);
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

impl From<xkb_state::ModifiersState> for ModifiersState {
    fn from(mods: xkb_state::ModifiersState) -> ModifiersState {
        let mut wl_mods = ModifiersState::empty();
        wl_mods.set(ModifiersState::SHIFT, mods.shift);
        wl_mods.set(ModifiersState::CONTROL, mods.ctrl);
        wl_mods.set(ModifiersState::ALT, mods.alt);
        wl_mods.set(ModifiersState::SUPER, mods.logo);
        wl_mods
    }
}
