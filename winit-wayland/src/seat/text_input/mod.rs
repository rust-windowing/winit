use std::ops::Deref;

use dpi::{LogicalPosition, LogicalSize};
use sctk::globals::GlobalData;
use sctk::reexports::client::globals::{BindError, GlobalList};
use sctk::reexports::client::protocol::wl_surface::WlSurface;
use sctk::reexports::client::{Connection, Dispatch, Proxy, QueueHandle, delegate_dispatch};
use sctk::reexports::protocols::wp::text_input::zv3::client::zwp_text_input_manager_v3::ZwpTextInputManagerV3;
use sctk::reexports::protocols::wp::text_input::zv3::client::zwp_text_input_v3::{
    ContentHint, ContentPurpose, Event as TextInputEvent, ZwpTextInputV3,
};
use tracing::warn;
use winit_core::event::{Ime, WindowEvent};
use winit_core::window::{
    ImeCapabilities, ImeHint, ImePurpose, ImeRequestData, ImeSurroundingText,
};

use crate::state::WinitState;
use crate::window::state::AnyWindowStateLocked;

#[derive(Debug)]
pub struct TextInputState {
    text_input_manager: ZwpTextInputManagerV3,
}

impl TextInputState {
    pub fn new(
        globals: &GlobalList,
        queue_handle: &QueueHandle<WinitState>,
    ) -> Result<Self, BindError> {
        let text_input_manager = globals.bind(queue_handle, 1..=1, GlobalData)?;
        Ok(Self { text_input_manager })
    }
}

impl Deref for TextInputState {
    type Target = ZwpTextInputManagerV3;

    fn deref(&self) -> &Self::Target {
        &self.text_input_manager
    }
}

impl Dispatch<ZwpTextInputManagerV3, GlobalData, WinitState> for TextInputState {
    fn event(
        _state: &mut WinitState,
        _proxy: &ZwpTextInputManagerV3,
        _event: <ZwpTextInputManagerV3 as Proxy>::Event,
        _data: &GlobalData,
        _conn: &Connection,
        _qhandle: &QueueHandle<WinitState>,
    ) {
    }
}

impl Dispatch<ZwpTextInputV3, TextInputData, WinitState> for TextInputState {
    fn event(
        state: &mut WinitState,
        text_input: &ZwpTextInputV3,
        event: <ZwpTextInputV3 as Proxy>::Event,
        data: &TextInputData,
        _conn: &Connection,
        _qhandle: &QueueHandle<WinitState>,
    ) {
        let windows = state.windows.get_mut();
        let mut text_input_data = data.inner.lock().unwrap();
        match event {
            TextInputEvent::Enter { surface } => {
                let window_id = crate::make_wid(&surface);
                text_input_data.surface = Some(surface);

                let window = match windows.get(&window_id) {
                    Some(window) => window.lock(),
                    None => return,
                };

                if let Some(text_input_state) = window.text_input_state() {
                    text_input.set_state(Some(text_input_state), true);
                    // The input method doesn't have to reply anything, so a synthetic event
                    // carrying an empty state notifies the application about its presence.
                    state.events_sink.push_window_event(WindowEvent::Ime(Ime::Enabled), window_id);
                }

                match window {
                    AnyWindowStateLocked::TopLevel(mut window) => {
                        window.text_input_entered(text_input)
                    },
                    AnyWindowStateLocked::Popup(mut window) => {
                        window.text_input_entered(text_input)
                    },
                }
            },
            TextInputEvent::Leave { surface } => {
                text_input_data.surface = None;
                text_input_data.last_preedit_empty = true;

                // Always issue a disable.
                text_input.disable();
                text_input.commit();

                let window_id = crate::make_wid(&surface);

                // XXX this check is essential, because `leave` could have a
                // reference to nil surface...
                let window = match windows.get(&window_id) {
                    Some(window) => window.lock(),
                    None => return,
                };

                match window {
                    AnyWindowStateLocked::TopLevel(mut window) => {
                        window.text_input_left(text_input)
                    },
                    AnyWindowStateLocked::Popup(mut window) => window.text_input_left(text_input),
                }

                state.events_sink.push_window_event(WindowEvent::Ime(Ime::Disabled), window_id);
            },
            TextInputEvent::PreeditString { text, cursor_begin, cursor_end } => {
                let text = text.unwrap_or_default();
                let cursor_begin = usize::try_from(cursor_begin)
                    .ok()
                    .and_then(|idx| text.is_char_boundary(idx).then_some(idx));
                let cursor_end = usize::try_from(cursor_end)
                    .ok()
                    .and_then(|idx| text.is_char_boundary(idx).then_some(idx));

                text_input_data.pending_preedit = Some(Preedit { text, cursor_begin, cursor_end })
            },
            TextInputEvent::CommitString { text } => {
                text_input_data.pending_preedit = None;
                text_input_data.pending_commit = text;
            },
            TextInputEvent::DeleteSurroundingText { before_length, after_length } => {
                text_input_data.pending_delete = Some(DeleteSurroundingText {
                    before: before_length as usize,
                    after: after_length as usize,
                });
            },
            TextInputEvent::Done { .. } => {
                let window_id = match text_input_data.surface.as_ref() {
                    Some(surface) => crate::make_wid(surface),
                    None => return,
                };

                // Just in case some IME sends an event for the disabled window.
                if let Some(window) = windows.get(&window_id) {
                    if window.lock().text_input_state().is_none() {
                        return;
                    }
                };

                // The events are sent to the user separately, so
                // CAUTION: events must always arrive in the order compatible with the application
                // order specified by the text-input-v3 protocol:
                //
                // As of version 1:
                // 1. Replace existing preedit string with the cursor.
                // 2. Delete requested surrounding text.
                // 3. Insert commit string with the cursor at its end.
                // 4. Calculate surrounding text to send.
                // 5. Insert new preedit text in cursor position.
                // 6. Place cursor inside preedit text.

                if let Some(DeleteSurroundingText { before, after }) =
                    text_input_data.pending_delete
                {
                    state.events_sink.push_window_event(
                        WindowEvent::Ime(Ime::DeleteSurrounding {
                            before_bytes: before,
                            after_bytes: after,
                        }),
                        window_id,
                    );
                }

                // Clear preedit, unless all we'll be doing next is sending a new preedit and
                // the last preedit wasn't empty.
                if text_input_data.pending_commit.is_some()
                    || (text_input_data.pending_preedit.is_none()
                        && !text_input_data.last_preedit_empty)
                {
                    state.events_sink.push_window_event(
                        WindowEvent::Ime(Ime::Preedit(String::new(), None)),
                        window_id,
                    );
                    text_input_data.last_preedit_empty = true;
                }

                // Send `Commit`.
                if let Some(text) = text_input_data.pending_commit.take() {
                    state
                        .events_sink
                        .push_window_event(WindowEvent::Ime(Ime::Commit(text)), window_id);
                }

                // Send preedit.
                if let Some(preedit) = text_input_data.pending_preedit.take() {
                    let cursor_range =
                        preedit.cursor_begin.map(|b| (b, preedit.cursor_end.unwrap_or(b)));

                    text_input_data.last_preedit_empty = false;
                    state.events_sink.push_window_event(
                        WindowEvent::Ime(Ime::Preedit(preedit.text, cursor_range)),
                        window_id,
                    );
                }
            },
            _ => {},
        }
    }
}

pub trait ZwpTextInputV3Ext {
    /// Applies the entire state atomically to the input method. It will skip the "enable" request
    /// if `already_enabled` is `true`.
    fn set_state(&self, state: Option<&ClientState>, send_enable: bool);
}

impl ZwpTextInputV3Ext for ZwpTextInputV3 {
    fn set_state(&self, state: Option<&ClientState>, send_enable: bool) {
        let state = match state {
            Some(state) => state,
            None => {
                self.disable();
                self.commit();
                return;
            },
        };

        if send_enable {
            self.enable();
        }

        if let Some(content_type) = state.content_type() {
            self.set_content_type(content_type.hint, content_type.purpose);
        }

        if let Some((position, size)) = state.cursor_area() {
            let (x, y) = (position.x as i32, position.y as i32);
            let (width, height) = (size.width as i32, size.height as i32);
            // The same cursor can be applied on different seats.
            // It's the compositor's responsibility to make sure that any present popups don't
            // overlap.
            self.set_cursor_rectangle(x, y, width, height);
        }

        if let Some(surrounding) = state.surrounding_text() {
            self.set_surrounding_text(
                surrounding.text().into(),
                surrounding.cursor() as i32,
                surrounding.anchor() as i32,
            );
        }

        self.commit();
    }
}

/// The Data associated with the text input.
#[derive(Default)]
pub struct TextInputData {
    inner: std::sync::Mutex<TextInputDataInner>,
}

pub struct TextInputDataInner {
    /// The `WlSurface` we're performing input to.
    surface: Option<WlSurface>,

    /// The commit to submit on `done`.
    pending_commit: Option<String>,

    /// The preedit to submit on `done`.
    pending_preedit: Option<Preedit>,

    /// The text around the cursor to delete on `done`
    pending_delete: Option<DeleteSurroundingText>,

    /// Last preedit empty.
    last_preedit_empty: bool,
}

impl Default for TextInputDataInner {
    fn default() -> Self {
        Self {
            surface: None,
            pending_commit: None,
            pending_preedit: None,
            pending_delete: None,
            last_preedit_empty: true,
        }
    }
}

/// The state of the preedit.
#[derive(Clone)]
struct Preedit {
    text: String,
    cursor_begin: Option<usize>,
    cursor_end: Option<usize>,
}

/// The delete request
#[derive(Clone)]
struct DeleteSurroundingText {
    /// Bytes before cursor
    before: usize,
    /// Bytes after cursor
    after: usize,
}

/// State change requested by the application.
///
/// This is a version that uses text_input abstractions translated from the ones used in
/// winit::core::window::ImeStateChange.
///
/// Fields that are initially set to None are unsupported capabilities
/// and trying to set them raises an error.
#[derive(Debug, PartialEq, Clone)]
pub struct ClientState {
    capabilities: ImeCapabilities,
    content_type: ContentType,
    /// The IME cursor area which should not be covered by the input method popup.
    cursor_area: (LogicalPosition<u32>, LogicalSize<u32>),

    /// The `ImeSurroundingText` struct is based on the Wayland model.
    /// When this changes, another struct might be needed.
    surrounding_text: ImeSurroundingText,
}

impl ClientState {
    pub fn new(
        capabilities: ImeCapabilities,
        request_data: ImeRequestData,
        scale_factor: f64,
    ) -> Self {
        let mut this = Self {
            capabilities,
            content_type: Default::default(),
            cursor_area: Default::default(),
            surrounding_text: ImeSurroundingText::new(String::new(), 0, 0).unwrap(),
        };

        let unsupported_flags = capabilities
            .without_hint_and_purpose()
            .without_cursor_area()
            .without_surrounding_text();

        if unsupported_flags != ImeCapabilities::new() {
            warn!(
                "Backend doesn't support all requested IME capabilities: {:?}.\n Ignoring.",
                unsupported_flags
            );
        }

        this.update(request_data, scale_factor);
        this
    }

    pub fn capabilities(&self) -> ImeCapabilities {
        self.capabilities
    }

    /// Updates the fields of the state which are present in update_fields.
    pub fn update(&mut self, request_data: ImeRequestData, scale_factor: f64) {
        if let Some((hint, purpose)) =
            request_data.hint_and_purpose.filter(|_| self.capabilities.hint_and_purpose())
        {
            self.content_type = (hint, purpose).into();
        }

        if let Some((position, size)) = request_data.cursor_area {
            if self.capabilities.cursor_area() {
                let position: LogicalPosition<u32> = position.to_logical(scale_factor);
                let size: LogicalSize<u32> = size.to_logical(scale_factor);
                self.cursor_area = (position, size);
            } else {
                warn!("discarding IME cursor area update without capability enabled.");
            }
        }

        if let Some(surrounding) = request_data.surrounding_text {
            if self.capabilities.surrounding_text() {
                self.surrounding_text = surrounding;
            } else {
                warn!("discarding IME surrounding text update without capability enabled.");
            }
        }
    }

    pub fn content_type(&self) -> Option<ContentType> {
        self.capabilities.hint_and_purpose().then_some(self.content_type)
    }

    pub fn cursor_area(&self) -> Option<(LogicalPosition<u32>, LogicalSize<u32>)> {
        self.capabilities.cursor_area().then_some(self.cursor_area)
    }

    pub fn surrounding_text(&self) -> Option<&ImeSurroundingText> {
        self.capabilities.surrounding_text().then_some(&self.surrounding_text)
    }
}

/// Arguments to content_type
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ContentType {
    /// Text input hint.
    hint: ContentHint,
    /// Text input purpose.
    purpose: ContentPurpose,
}

/// The two options influence each other, so they must be converted together.
impl From<(ImeHint, ImePurpose)> for ContentType {
    fn from((hint, purpose): (ImeHint, ImePurpose)) -> Self {
        let purpose = match purpose {
            ImePurpose::Password => ContentPurpose::Password,
            ImePurpose::Terminal => ContentPurpose::Terminal,
            ImePurpose::Phone => ContentPurpose::Phone,
            ImePurpose::Number => ContentPurpose::Number,
            ImePurpose::Url => ContentPurpose::Url,
            ImePurpose::Email => ContentPurpose::Email,
            ImePurpose::Pin => ContentPurpose::Pin,
            ImePurpose::Date => ContentPurpose::Date,
            ImePurpose::Time => ContentPurpose::Time,
            ImePurpose::DateTime => ContentPurpose::Datetime,
            _ => ContentPurpose::Normal,
        };

        let base_hint = match purpose {
            // Before the hint API was introduced, password  purpose guaranteed the
            // sensitive hint. Keep this behaviour for the sake of backwards compatibility.
            ContentPurpose::Password | ContentPurpose::Pin => ContentHint::SensitiveData,
            _ => ContentHint::None,
        };

        let mut new_hint = base_hint;
        if hint.contains(ImeHint::COMPLETION) {
            new_hint |= ContentHint::Completion;
        }
        if hint.contains(ImeHint::SPELLCHECK) {
            new_hint |= ContentHint::Spellcheck;
        }
        if hint.contains(ImeHint::AUTO_CAPITALIZATION) {
            new_hint |= ContentHint::AutoCapitalization;
        }
        if hint.contains(ImeHint::LOWERCASE) {
            new_hint |= ContentHint::Lowercase;
        }
        if hint.contains(ImeHint::UPPERCASE) {
            new_hint |= ContentHint::Uppercase;
        }
        if hint.contains(ImeHint::TITLECASE) {
            new_hint |= ContentHint::Titlecase;
        }
        if hint.contains(ImeHint::HIDDEN_TEXT) {
            new_hint |= ContentHint::HiddenText;
        }
        if hint.contains(ImeHint::SENSITIVE_DATA) {
            new_hint |= ContentHint::SensitiveData;
        }
        if hint.contains(ImeHint::LATIN) {
            new_hint |= ContentHint::Latin;
        }
        if hint.contains(ImeHint::MULTILINE) {
            new_hint |= ContentHint::Multiline;
        }

        Self { hint: new_hint, purpose }
    }
}

impl Default for ContentType {
    fn default() -> Self {
        ContentType { purpose: ContentPurpose::Normal, hint: ContentHint::None }
    }
}

delegate_dispatch!(WinitState: [ZwpTextInputManagerV3: GlobalData] => TextInputState);
delegate_dispatch!(WinitState: [ZwpTextInputV3: TextInputData] => TextInputState);
