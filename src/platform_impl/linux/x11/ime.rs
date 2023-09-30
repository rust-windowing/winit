//! IME handler, using the xim-rs crate.

use super::{X11Error, X11rbConnection, XConnection};

use x11rb::connection::Connection;
use x11rb::protocol::xproto::Window;
use x11rb::protocol::Event;

use xim::x11rb::{HasConnection, X11rbClient};
use xim::{AttributeName, Client as _, ClientError, ClientHandler, InputStyle, Point};

use std::collections::{HashMap, VecDeque};
use std::fmt;
use std::sync::Arc;

impl HasConnection for XConnection {
    type Connection = X11rbConnection;

    fn conn(&self) -> &Self::Connection {
        self.xcb_connection()
    }
}

/// A collection of the IME events that can occur.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub enum ImeEvent {
    Enabled,
    Start,
    Update(String, Option<usize>),
    Commit(String),
    End,
    Disabled,
}

/// Invalid states that an IME client can enter.
#[derive(Debug, Clone)]
pub enum InvalidImeState {
    /// The IME has no style information.
    NoStyle,

    /// No windows in the pending window queue.
    NoWindows,

    /// Invalid input context.
    InvalidIc(u16),
}

impl fmt::Display for InvalidImeState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            InvalidImeState::NoStyle => write!(f, "IME has no style information"),
            InvalidImeState::NoWindows => {
                write!(f, "IME has no windows in the pending window queue")
            }
            InvalidImeState::InvalidIc(ic) => write!(f, "IME has invalid input context {}", ic),
        }
    }
}

/// Request to control XIM handler from the window.
pub enum ImeRequest {
    /// Set IME spot position for given `window_id`.
    Position(Window, i16, i16),

    /// Allow IME input for the given `window_id`.
    Allow(Window, bool),
}

/// The IME data for winit.
pub(super) struct ImeData {
    /// The XIM client manager.
    client: X11rbClient<Arc<XConnection>>,

    /// Relevant IME data.
    handler: ImeHandler,
}

/// Inner IME handler.
struct ImeHandler {
    /// Whether IME is currently disconnected.
    disconnected: bool,

    /// IME events waiting to be read.
    ime_events: VecDeque<(Window, ImeEvent)>,

    /// Windows waiting to be assigned an input context.
    pending_windows: VecDeque<WindowData>,

    /// Currently registered input styles.
    styles: Option<(Style, Style)>,

    /// The input method for the display, if there is one.
    input_method: Option<u16>,

    /// Hash map between input contexts and their associated data.
    input_contexts: HashMap<u16, IcData>,

    /// Map between window IDs and their associated input contexts.
    window_contexts: HashMap<Window, u16>,
}

/// Data relevant for each input context.
struct IcData {
    /// Data associated with the window.
    window: WindowData,

    /// Newly set point for the context.
    new_spot: Option<Point>,

    /// The current preedit string.
    ///
    /// We use a `Vec<char>` here instead of a string because the IME indices operate on chars,
    /// not bytes.
    text: Vec<char>,

    /// The current cursor position in the preedit string.
    cursor: usize,
}

/// Windows waiting for IME events.
struct WindowData {
    /// The window ID.
    id: Window,

    /// The style of the window.
    style: Style,

    /// Current "spot" for the context.
    spot: Point,
}

#[derive(Copy, Clone)]
enum Style {
    Preedit,
    Nothing,
    None,
}

impl ImeData {
    /// Creates the IME data for the display.
    pub(super) fn new(conn: &Arc<XConnection>, screen: usize) -> Result<Self, X11Error> {
        // IM servers to try, in order:
        //  - None, which defaults to the environment variable `XMODIFIERS` in xim's impl.
        //  - "local", which is the default for most IMEs.
        //  - empty string, which may work in some cases.
        let input_methods = [None, Some("local"), Some("")];
        let mut last_error = X11Error::Ime(ClientError::NoXimServer);

        for im in input_methods {
            // Try to initialize a client here.
            match X11rbClient::init(conn.clone(), screen, im) {
                Ok(client) => {
                    return Ok(Self {
                        client,
                        handler: ImeHandler {
                            disconnected: true,
                            ime_events: VecDeque::new(),
                            pending_windows: VecDeque::new(),
                            styles: None,
                            input_method: None,
                            input_contexts: HashMap::new(),
                            window_contexts: HashMap::new(),
                        },
                    })
                }

                Err(err) => {
                    log::warn!("Failed to create XIM client for {:?}: {err}", ImData(im));
                    last_error = X11Error::Ime(err);
                }
            }
        }

        Err(last_error)
    }

    /// Filter an event.
    pub(super) fn filter_event(&mut self, event: &Event) -> Result<bool, X11Error> {
        self.client
            .filter_event(event, &mut self.handler)
            .map_err(X11Error::Ime)
    }

    /// Connection to the X server.
    fn conn(&self) -> &X11rbConnection {
        self.client.conn()
    }

    /// Get an IME event.
    pub(super) fn next_ime_event(&mut self) -> Option<(Window, ImeEvent)> {
        self.handler.ime_events.pop_front()
    }

    /// Create a new IME context for the provided window.
    pub(super) fn create_context(
        &mut self,
        window: Window,
        with_preedit: bool,
        spot: Option<Point>,
    ) -> Result<bool, X11Error> {
        // If we aren't connected, nothing can be done.
        if self.handler.disconnected {
            return Ok(false);
        }
        let method = match self.handler.input_method {
            Some(im) => im,
            None => return Ok(false),
        };

        // Get the current style.
        let style = match (self.handler.styles, with_preedit) {
            (None, _) => return Err(X11Error::InvalidImeState(InvalidImeState::NoStyle)),
            (Some((preedit_style, _)), true) => preedit_style,
            (Some((_, none_style)), false) => none_style,
        };

        // Setup IC attributes.
        let ic_attributes = {
            let mut ic_attributes = self
                .client
                .build_ic_attributes()
                .push(AttributeName::ClientWindow, window);

            let ic_style = match style {
                Style::Preedit => InputStyle::PREEDIT_POSITION | InputStyle::STATUS_NOTHING,
                Style::Nothing => InputStyle::PREEDIT_NOTHING | InputStyle::STATUS_NOTHING,
                Style::None => InputStyle::PREEDIT_NONE | InputStyle::STATUS_NONE,
            };

            if let Some(spot) = spot.clone() {
                ic_attributes = ic_attributes.push(AttributeName::SpotLocation, spot);
            }

            ic_attributes
                .push(AttributeName::InputStyle, ic_style)
                .build()
        };

        // Create the IC.
        self.client.create_ic(method, ic_attributes)?;

        // Add to the waiting window list.
        self.handler.pending_windows.push_back(WindowData {
            id: window,
            style,
            spot: spot.unwrap_or(Point { x: 0, y: 0 }),
        });

        Ok(true)
    }

    /// Remove an IME context for a window.
    pub(super) fn remove_context(&mut self, window: Window) -> Result<bool, X11Error> {
        if self.handler.disconnected {
            return Ok(false);
        }
        let method = match self.handler.input_method {
            Some(im) => im,
            None => return Ok(false),
        };

        // Remove the pending window if it's still pending.
        let mut removed = false;
        self.handler.pending_windows.retain(|pending| {
            if pending.id == window {
                removed = true;
                false
            } else {
                true
            }
        });

        if removed {
            return Ok(true);
        }

        // Remove the IC if it's already created.
        if let Some(ic) = self.handler.window_contexts.remove(&window) {
            self.handler.input_contexts.remove(&ic);

            // Destroy the IC.
            self.client.destroy_ic(method, ic)?;
        }

        Ok(false)
    }

    /// Focus an IME context.
    pub(super) fn focus_window(&mut self, window: Window) -> Result<bool, X11Error> {
        if self.handler.disconnected {
            return Ok(false);
        }

        let method = self.wait_for_method()?;
        let ic = self.wait_for_context(window)?;

        if let Some(ic) = ic {
            self.client.set_focus(method, ic)?;
            return Ok(true);
        }

        Ok(false)
    }

    /// Unfocus an IME context.
    pub(super) fn unfocus_window(&mut self, window: Window) -> Result<bool, X11Error> {
        if self.handler.disconnected {
            return Ok(false);
        }

        let method = self.wait_for_method()?;
        let ic = self.wait_for_context(window)?;

        if let Some(ic) = ic {
            self.client.unset_focus(method, ic)?;
            return Ok(true);
        }

        Ok(false)
    }

    /// Set the spot for an IME context.
    pub(super) fn set_spot(&mut self, window: Window, x: i16, y: i16) -> Result<(), X11Error> {
        if self.handler.disconnected {
            return Ok(());
        }

        let method = self.wait_for_method()?;
        let ic = self.wait_for_context(window)?;

        if let Some(ic) = ic {
            // If the IC is not available, or if the spot is the same, then we don't need to update.
            let ic_data = match self.handler.input_contexts.get_mut(&ic) {
                Some(ic_data) => ic_data,
                None => return Ok(()),
            };

            let new_point = Point { x, y };
            if !matches!(ic_data.window.style, Style::None) || ic_data.window.spot == new_point {
                return Ok(());
            }

            let new_attrs = self
                .client
                .build_ic_attributes()
                .push(AttributeName::SpotLocation, new_point.clone())
                .build();
            self.client.set_ic_values(method, ic, new_attrs)?;

            // Indicate that we have a new spot.
            debug_assert!(ic_data.new_spot.is_none());
            ic_data.new_spot = Some(new_point);
        }

        Ok(())
    }

    pub(super) fn set_ime_allowed(
        &mut self,
        window: Window,
        allowed: bool,
    ) -> Result<(), X11Error> {
        if self.handler.disconnected {
            return Ok(());
        }

        // Get the client info.
        let _ = self.wait_for_method()?;
        let ic = self.wait_for_context(window)?;

        if let Some(ic) = ic {
            let mut spot = None;

            // See if we need to update the allowed state.
            if let Some(ic_data) = self.handler.input_contexts.get(&ic) {
                spot = Some(ic_data.window.spot.clone());
                if matches!(ic_data.window.style, Style::None) != allowed {
                    return Ok(());
                }
            }

            // Delete and re-install the IC.
            self.remove_context(window)?;
            self.create_context(window, allowed, spot)?;
        }

        Ok(())
    }

    /// Wait for the input method to be set.
    fn wait_for_method(&mut self) -> Result<u16, X11Error> {
        loop {
            if let Some(im) = self.handler.input_method {
                return Ok(im);
            }

            // Wait and hope the input method is set.
            self.block_for_ime()?;
        }
    }

    /// Wait for an input context to be set.
    fn wait_for_context(&mut self, window: Window) -> Result<Option<u16>, X11Error> {
        if let Some(cid) = self.handler.window_contexts.get(&window) {
            return Ok(Some(*cid));
        }

        // If the window isn't in our pending windows queue, there's no way for it to get an IC.
        if !self
            .handler
            .pending_windows
            .iter()
            .any(|WindowData { id, .. }| *id == window)
        {
            return Ok(None);
        }

        loop {
            self.block_for_ime()?;
            if let Some(cid) = self.handler.window_contexts.get(&window) {
                return Ok(Some(*cid));
            }
        }
    }

    /// Wait until we've acted on an IME event.
    fn block_for_ime(&mut self) -> Result<(), X11Error> {
        let mut last_event = self.conn().poll_for_event()?;

        loop {
            if let Some(last_event) = last_event {
                if self.filter_event(&last_event)? {
                    return Ok(());
                }
            }

            // TODO: have an queue queue.
            log::info!("Waiting for IME event");
            last_event = Some(self.conn().wait_for_event()?);
        }
    }
}

impl<C: xim::Client> ClientHandler<C> for ImeHandler {
    fn handle_connect(&mut self, client: &mut C) -> Result<(), ClientError> {
        // We have been connected, now request a new input method for our current locale.
        self.disconnected = false;
        client.open(&locale())
    }

    fn handle_disconnect(&mut self) {
        // We are now disconnected.
        self.disconnected = true;
    }

    fn handle_open(&mut self, client: &mut C, input_method_id: u16) -> Result<(), ClientError> {
        // We now have an input method.
        debug_assert!(self.input_method.is_none());
        self.input_method = Some(input_method_id);

        // Ask for the IM's attributes.
        client.get_im_values(input_method_id, &[AttributeName::QueryInputStyle])
    }

    fn handle_close(&mut self, _client: &mut C, input_method_id: u16) -> Result<(), ClientError> {
        // No more input method.
        debug_assert_eq!(self.input_method, Some(input_method_id));
        self.input_method = None;

        Ok(())
    }

    fn handle_get_im_values(
        &mut self,
        _client: &mut C,
        input_method_id: u16,
        mut attributes: xim::AHashMap<xim::AttributeName, Vec<u8>>,
    ) -> Result<(), ClientError> {
        debug_assert_eq!(self.input_method, Some(input_method_id));

        // Get the input styles.
        let mut preedit_style = None;
        let mut none_style = None;

        let styles = {
            let style = attributes
                .remove(&AttributeName::QueryInputStyle)
                .expect("No query input style");
            let mut result = vec![0u32; style.len() / 4];

            bytemuck::cast_slice_mut::<u32, u8>(&mut result).copy_from_slice(&style);

            result
        };

        {
            // The styles that we're looking for.
            let lu_preedit_style = InputStyle::PREEDIT_CALLBACKS | InputStyle::STATUS_NOTHING;
            let lu_nothing_style = InputStyle::PREEDIT_NOTHING | InputStyle::STATUS_NOTHING;
            let lu_none_style = InputStyle::PREEDIT_NONE | InputStyle::STATUS_NONE;

            for style in styles {
                let style = InputStyle::from_bits_truncate(style);

                if style == lu_preedit_style {
                    preedit_style = Some(Style::Preedit);
                } else if style == lu_nothing_style {
                    preedit_style = Some(Style::Nothing);
                } else if style == lu_none_style {
                    none_style = Some(Style::None);
                }
            }
        }

        let (preedit_style, none_style) = match (preedit_style, none_style) {
            (None, None) => {
                log::error!("No supported input styles found");
                return Ok(());
            }

            (Some(style), None) | (None, Some(style)) => (style, style),

            (Some(preedit_style), Some(none_style)) => (preedit_style, none_style),
        };

        self.styles = Some((preedit_style, none_style));

        Ok(())
    }

    fn handle_create_ic(
        &mut self,
        _client: &mut C,
        input_method_id: u16,
        input_context_id: u16,
    ) -> Result<(), ClientError> {
        debug_assert_eq!(self.input_method, Some(input_method_id));

        // Get the window that wanted the IC context.
        let window = self
            .pending_windows
            .pop_front()
            .ok_or_else(|| invalid_state(InvalidImeState::NoWindows))?;

        // Create the IC data.
        let ic_data = IcData {
            window,
            new_spot: None,
            text: Vec::new(),
            cursor: 0,
        };

        // Store the context.
        let (window, style) = (ic_data.window.id, ic_data.window.style);
        self.input_contexts.insert(input_context_id, ic_data);
        self.window_contexts.insert(window, input_context_id);

        // Indicate our status.
        let event = if matches!(style, Style::Nothing) {
            ImeEvent::Disabled
        } else {
            ImeEvent::Enabled
        };
        self.ime_events.push_back((window, event));

        Ok(())
    }

    fn handle_destroy_ic(
        &mut self,
        _client: &mut C,
        _input_method_id: u16,
        _input_context_id: u16,
    ) -> Result<(), ClientError> {
        // This is already handled by the higher-level function.
        Ok(())
    }

    fn handle_set_ic_values(
        &mut self,
        _client: &mut C,
        input_method_id: u16,
        input_context_id: u16,
    ) -> Result<(), ClientError> {
        debug_assert_eq!(self.input_method, Some(input_method_id));

        // Get the IC data.
        let ic_data = self
            .input_contexts
            .get_mut(&input_context_id)
            .ok_or_else(|| invalid_state(InvalidImeState::InvalidIc(input_context_id)))?;

        // Move up the new spot
        if let Some(spot) = ic_data.new_spot.take() {
            ic_data.window.spot = spot;
        }

        Ok(())
    }

    fn handle_preedit_start(
        &mut self,
        _client: &mut C,
        input_method_id: u16,
        input_context_id: u16,
    ) -> Result<(), ClientError> {
        debug_assert_eq!(self.input_method, Some(input_method_id));

        if let Some(ic_data) = self.input_contexts.get_mut(&input_context_id) {
            // Start a pre-edit.
            ic_data.text.clear();
            ic_data.cursor = 0;

            // Indicate the start.
            self.ime_events
                .push_back((ic_data.window.id, ImeEvent::Start));
        }

        Ok(())
    }

    fn handle_preedit_draw(
        &mut self,
        _client: &mut C,
        input_method_id: u16,
        input_context_id: u16,
        caret: i32,
        chg_first: i32,
        chg_len: i32,
        _status: xim::PreeditDrawStatus,
        preedit_string: &str,
        _feedbacks: Vec<xim::Feedback>,
    ) -> Result<(), ClientError> {
        debug_assert_eq!(self.input_method, Some(input_method_id));

        if let Some(ic_data) = self.input_contexts.get_mut(&input_context_id) {
            // Set the cursor.
            ic_data.cursor = caret as usize;

            // Figure out the range of text to change.
            let change_range = chg_first as usize..(chg_first + chg_len) as usize;

            // If the range doesn't fit our current text, warn and return.
            if change_range.start > ic_data.text.len() || change_range.end > ic_data.text.len() {
                warn!(
                    "Preedit draw range {}..{} doesn't fit text of length {}",
                    change_range.start,
                    change_range.end,
                    ic_data.text.len()
                );
                return Ok(());
            }

            // Update the text in the changed range.
            {
                let text = &mut ic_data.text;
                let mut old_text_tail = text.split_off(change_range.end);

                text.truncate(change_range.start);
                text.extend(preedit_string.chars());
                text.append(&mut old_text_tail);
            }

            // Send the event.
            let cursor_byte_pos = calc_byte_position(&ic_data.text, ic_data.cursor);
            let event = ImeEvent::Update(ic_data.text.iter().collect(), Some(cursor_byte_pos));

            self.ime_events.push_back((ic_data.window.id, event));
        }

        Ok(())
    }

    fn handle_preedit_caret(
        &mut self,
        _client: &mut C,
        input_method_id: u16,
        input_context_id: u16,
        position: &mut i32,
        direction: xim::CaretDirection,
        _style: xim::CaretStyle,
    ) -> Result<(), ClientError> {
        // We only care about absolute position.
        if matches!(direction, xim::CaretDirection::AbsolutePosition) {
            debug_assert_eq!(self.input_method, Some(input_method_id));

            if let Some(ic_data) = self.input_contexts.get_mut(&input_context_id) {
                ic_data.cursor = *position as usize;

                // Send the event
                let event =
                    ImeEvent::Update(ic_data.text.iter().collect(), Some(*position as usize));
                self.ime_events.push_back((ic_data.window.id, event));
            }
        }

        Ok(())
    }

    fn handle_preedit_done(
        &mut self,
        _client: &mut C,
        input_method_id: u16,
        input_context_id: u16,
    ) -> Result<(), ClientError> {
        debug_assert_eq!(self.input_method, Some(input_method_id));

        // Get the client data.
        if let Some(ic_data) = self.input_contexts.get_mut(&input_context_id) {
            // We're done with a preedit.
            ic_data.text.clear();
            ic_data.cursor = 0;

            // Send a message to the window.
            let window = ic_data.window.id;
            self.ime_events.push_back((window, ImeEvent::End));
        }

        Ok(())
    }

    fn handle_commit(
        &mut self,
        _client: &mut C,
        input_method_id: u16,
        input_context_id: u16,
        text: &str,
    ) -> Result<(), ClientError> {
        debug_assert_eq!(self.input_method, Some(input_method_id));

        // Get the client data.
        if let Some(ic_data) = self.input_contexts.get_mut(&input_context_id) {
            // Send a message to the window.
            let window = ic_data.window.id;
            self.ime_events
                .push_back((window, ImeEvent::Commit(text.to_owned())));
        }

        Ok(())
    }

    fn handle_query_extension(
        &mut self,
        _client: &mut C,
        _extensions: &[xim::Extension],
    ) -> Result<(), ClientError> {
        // Don't care.
        Ok(())
    }

    fn handle_forward_event(
        &mut self,
        _client: &mut C,
        _input_method_id: u16,
        _input_context_id: u16,
        _flag: xim::ForwardEventFlag,
        _xev: C::XEvent,
    ) -> Result<(), ClientError> {
        // Don't care.
        Ok(())
    }

    fn handle_set_event_mask(
        &mut self,
        _client: &mut C,
        _input_method_id: u16,
        _input_context_id: u16,
        _forward_event_mask: u32,
        _synchronous_event_mask: u32,
    ) -> Result<(), ClientError> {
        // Don't care.
        Ok(())
    }
}

#[inline(always)]
fn invalid_state(state: InvalidImeState) -> ClientError {
    ClientError::Other(Box::new(X11Error::InvalidImeState(state)))
}

struct ImData(Option<&'static str>);

impl fmt::Debug for ImData {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.0 {
            Some(name) => write!(f, "\"{}\"", name),
            None => write!(f, "default input method"),
        }
    }
}

/// Get the current locale.
fn locale() -> String {
    use std::ffi::CStr;

    const EN_US: &str = "en_US.UTF-8";

    // Get the pointer to the current locale.
    let locale_ptr = unsafe { libc::setlocale(libc::LC_CTYPE, std::ptr::null()) };

    // If locale_ptr is null, just default to en_US.UTF-8.
    if locale_ptr.is_null() {
        return EN_US.to_owned();
    }

    // Convert the pointer to a CStr.
    let locale_cstr = unsafe { CStr::from_ptr(locale_ptr) };

    // Convert the CStr to a String to prevent the result from getting clobbered.
    locale_cstr.to_str().unwrap_or(EN_US).to_owned()
}

fn calc_byte_position(text: &[char], pos: usize) -> usize {
    text.iter().take(pos).map(|c| c.len_utf8()).sum()
}
