use sctk::reexports::protocols::wp::text_input::zv3::client::zwp_text_input_v3::ZwpTextInputV3;
use winit_core::window::{ImeCapabilities, ImeRequest, ImeRequestError};

use super::WindowState;
use crate::seat::{TextInputClientState, ZwpTextInputV3Ext};

impl WindowState {
    /// Whether the IME is allowed.
    #[inline]
    pub fn ime_allowed(&self) -> Option<ImeCapabilities> {
        self.text_input_state.as_ref().map(|state| state.capabilities())
    }

    pub(crate) fn text_input_state(&self) -> Option<&TextInputClientState> {
        self.text_input_state.as_ref()
    }

    /// Atomically update input method state.
    ///
    /// Returns `None` if an input method state haven't changed. Alternatively `Some(true)` and
    /// `Some(false)` is returned respectfully.
    pub fn request_ime_update(
        &mut self,
        request: ImeRequest,
    ) -> Result<Option<bool>, ImeRequestError> {
        let state_change = match request {
            ImeRequest::Enable(enable) => {
                let (capabilities, request_data) = enable.into_raw();

                if self.text_input_state.is_some() {
                    return Err(ImeRequestError::AlreadyEnabled);
                }

                self.text_input_state = Some(TextInputClientState::new(
                    capabilities,
                    request_data,
                    self.scale_factor(),
                ));
                true
            },
            ImeRequest::Update(request_data) => {
                let scale_factor = self.scale_factor();
                if let Some(text_input_state) = self.text_input_state.as_mut() {
                    text_input_state.update(request_data, scale_factor);
                } else {
                    return Err(ImeRequestError::NotEnabled);
                }
                false
            },
            ImeRequest::Disable => {
                self.text_input_state = None;
                true
            },
            _ => return Err(ImeRequestError::NotSupported),
        };

        // Only one input method may be active per (seat, surface),
        // but there may be multiple seats focused on a surface,
        // resulting in multiple text input objects.
        //
        // WARNING: this doesn't actually handle different seats with independent cursors. There's
        // no API to set a per-seat input method state, so they all share a single state.
        for text_input in &self.text_inputs {
            text_input.set_state(self.text_input_state.as_ref(), state_change);
        }

        if state_change { Ok(Some(self.text_input_state.is_some())) } else { Ok(None) }
    }

    /// Register text input on the top-level.
    #[inline]
    pub fn text_input_entered(&mut self, text_input: &ZwpTextInputV3) {
        if !self.text_inputs.iter().any(|t| t == text_input) {
            self.text_inputs.push(text_input.clone());
        }
    }

    /// The text input left the top-level.
    #[inline]
    pub fn text_input_left(&mut self, text_input: &ZwpTextInputV3) {
        if let Some(position) = self.text_inputs.iter().position(|t| t == text_input) {
            self.text_inputs.remove(position);
        }
    }
}
