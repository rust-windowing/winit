//! Implementation of input method handling using `xim-rs`.

use super::XConnection;

use std::cell::{RefCell, RefMut};
use std::collections::VecDeque;
use std::ffi::CStr;
use std::fmt;
use std::sync::Arc;

use xim::x11rb::{HasConnection, X11rbClient};
use xim::AHashMap;
use xim::{AttributeName, InputStyle, Point};
use xim::{Client as _, ClientError, ClientHandler};

use x11rb::connection::Connection;
use x11rb::protocol::xproto::Window;
use x11rb::protocol::Event;
use x11rb::xcb_ffi::XCBConnection;

/// Lock the refcell.
///
/// This exists in case we want to migrate this to a Mutex-based implementation.
macro_rules! lock {
    ($mutex:expr) => {
        $mutex.borrow_mut()
    };
}

/// Get the current locale.
fn locale() -> String {
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

impl HasConnection for XConnection {
    type Connection = XCBConnection;

    fn conn(&self) -> &Self::Connection {
        &self.connection
    }
}

#[derive(Copy, Clone)]
enum Style {
    Preedit,
    Nothing,
    None,
}

/// A collection of the IME events that can occur.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub enum ImeEvent {
    Enabled,
    Start,
    Update(String, usize),
    End,
    Disabled,
}

/// Request to control XIM handler from the window.
pub enum ImeRequest {
    /// Set IME spot position for given `window_id`.
    Position(Window, i16, i16),

    /// Allow IME input for the given `window_id`.
    Allow(Window, bool),
}

type XimClient = X11rbClient<Arc<XConnection>>;

/// IME-related information.
pub(super) struct ImeData {
    /// The XIM client.
    client: RefCell<XimClient>,

    /// State of the IME.
    inner_data: RefCell<ClientData>,
}

/// The current IME state.
#[derive(Default)]
struct ClientData {
    /// The input method we are currently using.
    input_method: Option<u16>,

    /// The registered input styles for the IM.
    styles: Option<(Style, Style)>,

    /// The list of produced IME events.
    events: VecDeque<(Window, ImeEvent)>,

    /// Whether the IME is currently disconnected.
    disconnected: bool,

    /// Hash map of input context IDs to their data.
    ic_data: AHashMap<u16, IcData>,

    /// Hash map of window IDs to their input context IDs.
    window_data: AHashMap<Window, u16>,

    /// Windows that are waiting for an input context.
    pending_windows: VecDeque<PendingData>,
}

/// Per-input context data.
struct IcData {
    /// The identifier of the context.
    id: u16,

    /// The window that the context is attached to.
    window: Window,

    /// The style of the context.
    style: Style,

    /// The current "spot" for the context.
    spot: Point,

    /// The newly set spot for the context.
    new_spot: Option<Point>,

    /// The current preedit string.
    ///
    /// We use a `Vec<char>` here instead of a string because the IME indices operate on chars,
    /// not bytes.
    text: Vec<char>,

    /// The current cursor position in the preedit string.
    cursor: usize,
}

struct PendingData {
    /// The window that the context is attached to.
    window: Window,

    /// The style of the context.
    style: Style,

    /// The current "spot" for the context.
    spot: Point,
}

impl IcData {
    fn available(&self) -> bool {
        !matches!(self.style, Style::None)
    }
}

impl ImeData {
    /// Create a new `ImeData`.
    #[allow(clippy::never_loop)]
    pub(super) fn new(xconn: &Arc<XConnection>, screen: usize) -> Result<Self, ClientError> {
        // IM servers to try, in order:
        //  - None, which defaults to the environment variable `XMODIFIERS` in xim's impl.
        //  - "local", which is the default for most IMEs.
        //  - empty string, which may work in some cases.
        let input_methods = [None, Some("local"), Some("")];

        let client = 'get_client: loop {
            let mut last_error = None;

            for im in input_methods {
                // Try to create a client.
                match XimClient::init(xconn.clone(), screen, im) {
                    Ok(client) => break 'get_client client,
                    Err(err) => {
                        struct ImName(Option<&'static str>);

                        impl fmt::Debug for ImName {
                            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                                match self.0 {
                                    Some(name) => write!(f, "\"{}\"", name),
                                    None => write!(f, "default input method"),
                                }
                            }
                        }

                        log::warn!("Failed to create XIM client for {:?}: {}", ImName(im), &err);
                        last_error = Some(err);
                    }
                }
            }

            return Err(last_error.unwrap_or(ClientError::NoXimServer));
        };

        Ok(Self {
            client: RefCell::new(client),
            inner_data: RefCell::new(ClientData {
                disconnected: true,
                ..Default::default()
            }),
        })
    }

    /// Run the IME if this event has any relevance to IME.
    pub(super) fn filter_event(&self, event: &Event) -> bool {
        let mut this = self;

        lock!(self.client)
            .filter_event(event, &mut this)
            .expect("Failed to filter event")
    }

    /// Get the next pending IME event.
    pub(super) fn next_event(&self) -> Option<(Window, ImeEvent)> {
        lock!(self.inner_data).events.pop_front()
    }

    /// Block until we've acted on a new IME event.
    pub(super) fn block_for_ime(&self, conn: &XConnection) -> Result<(), ClientError> {
        let mut last_event = conn.connection.poll_for_event()?;

        loop {
            let mut event_queue = conn.event_queue.lock().unwrap_or_else(|e| e.into_inner());

            // Check the last event we've seen.
            if let Some(last_event) = last_event.as_ref() {
                if self.filter_event(last_event) {
                    return Ok(());
                }
            }

            // See if there's anything worth filtering in the event queue.
            let mut filtered_any = false;
            event_queue.retain(|event| {
                if self.filter_event(event) {
                    filtered_any = true;
                    false
                } else {
                    true
                }
            });

            event_queue.extend(last_event);
            if filtered_any {
                return Ok(());
            }

            // Wait for a new event.
            log::info!("x11(ime): Entering wait for IME event");
            last_event = Some(conn.connection.wait_for_event()?);
        }
    }

    /// Create a new IME context for a window.
    pub(super) fn create_context(
        &self,
        window: Window,
        with_preedit: bool,
        spot: Option<Point>,
    ) -> Result<bool, ClientError> {
        let mut client_data = lock!(self.inner_data);
        let mut client = lock!(self.client);

        if client_data.disconnected {
            return Ok(false);
        }

        // Get the current style.
        let style = match (client_data.styles, with_preedit) {
            (None, _) => return Err(ClientError::Other("No input styles".into())),
            (Some((preedit_style, _)), true) => preedit_style,
            (Some((_, none_style)), false) => none_style,
        };

        // Setup IC attributes.
        let ic_attributes = {
            let mut ic_attributes = client
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
        client.create_ic(client_data.input_method.unwrap(), ic_attributes)?;

        // Assign this window to the next IC.
        client_data.pending_windows.push_back(PendingData {
            window,
            style,
            spot: spot.unwrap_or(Point { x: 0, y: 0 }),
        });

        Ok(true)
    }

    /// Remove an IME context for a window.
    pub(super) fn remove_context(&self, window: Window) -> Result<bool, ClientError> {
        let mut client_data = lock!(self.inner_data);

        if client_data.disconnected {
            return Ok(false);
        }

        // Remove the pending window if it's still pending.
        let mut removed = false;
        client_data.pending_windows.retain(|pending| {
            if pending.window == window {
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
        if let Some(ic) = client_data.window_data.remove(&window) {
            client_data.ic_data.remove(&ic);

            // Destroy the IC.
            let im = client_data.input_method.unwrap();
            drop(client_data);
            lock!(self.client).destroy_ic(im, ic)?;
        }

        Ok(false)
    }

    /// Focus an IME context.
    pub(super) fn focus_window(
        &self,
        conn: &XConnection,
        window: Window,
    ) -> Result<bool, ClientError> {
        let client_data = lock!(self.inner_data);

        if client_data.disconnected {
            return Ok(false);
        }

        let (im, client_data) = self.wait_for_method(conn, client_data)?;
        let (ic, _) = self.wait_for_context_for(conn, client_data, window)?;

        if let Some(ic) = ic {
            lock!(self.client).set_focus(im, ic)?;
            return Ok(true);
        }

        Ok(false)
    }

    /// Unfocus an IME context.
    pub(super) fn unfocus_window(
        &self,
        conn: &XConnection,
        window: Window,
    ) -> Result<bool, ClientError> {
        let client_data = lock!(self.inner_data);

        if client_data.disconnected {
            return Ok(false);
        }

        let (im, client_data) = self.wait_for_method(conn, client_data)?;
        let (ic, _) = self.wait_for_context_for(conn, client_data, window)?;

        if let Some(ic) = ic {
            lock!(self.client).unset_focus(im, ic)?;
            return Ok(true);
        }

        Ok(false)
    }

    /// Set the spot for an IME context.
    pub(super) fn set_spot(
        &self,
        conn: &XConnection,
        window: Window,
        x: i16,
        y: i16,
    ) -> Result<(), ClientError> {
        let client_data = lock!(self.inner_data);

        if client_data.disconnected {
            return Ok(());
        }

        let (im, client_data) = self.wait_for_method(conn, client_data)?;
        let (ic, mut client_data) = self.wait_for_context_for(conn, client_data, window)?;

        if let Some(ic) = ic {
            // If the IC is not available, or if the spot is the same, then we don't need to update.
            let ic_data = match client_data.ic_data.get_mut(&ic) {
                Some(ic_data) => ic_data,
                None => return Ok(()),
            };

            let new_point = Point { x, y };
            if !ic_data.available() || ic_data.spot == new_point {
                return Ok(());
            }

            let mut client = lock!(self.client);
            let new_attrs = client
                .build_ic_attributes()
                .push(AttributeName::SpotLocation, new_point.clone())
                .build();
            client.set_ic_values(im, ic, new_attrs)?;

            // Indicate that we have a new spot.
            debug_assert!(ic_data.new_spot.is_none());
            ic_data.new_spot = Some(new_point);
        }

        Ok(())
    }

    pub(super) fn set_ime_allowed(
        &self,
        conn: &XConnection,
        window: Window,
        allowed: bool,
    ) -> Result<(), ClientError> {
        let client_data = lock!(self.inner_data);

        if client_data.disconnected {
            return Ok(());
        }

        // Get the client info.
        let (_, client_data) = self.wait_for_method(conn, client_data)?;
        let (ic, client_data) = self.wait_for_context_for(conn, client_data, window)?;

        if let Some(ic) = ic {
            let mut spot = None;

            // See if we need to update the allowed state.
            if let Some(ic_data) = client_data.ic_data.get(&ic) {
                spot = Some(ic_data.spot.clone());
                if ic_data.available() == allowed {
                    return Ok(());
                }
            }

            // Delete and re-install the IC.
            drop(client_data);
            self.remove_context(window)?;
            self.create_context(window, allowed, spot)?;
        }

        Ok(())
    }

    /// Wait for this display to have an IM method.
    fn wait_for_method<'a>(
        &'a self,
        xconn: &XConnection,
        mut client_data: RefMut<'a, ClientData>,
    ) -> Result<(u16, RefMut<'a, ClientData>), ClientError> {
        // See if we already have an input method.
        if let Some(im) = client_data.input_method {
            return Ok((im, client_data));
        }

        // Wait for a new IME event.
        loop {
            drop(client_data);

            self.block_for_ime(xconn)?;

            // See if we have an input method now.
            client_data = lock!(self.inner_data);
            if let Some(im) = client_data.input_method {
                return Ok((im, client_data));
            }
        }
    }

    /// Wait for this window to have an IME context.
    ///
    /// Returns `None` if the window is not registered for IME.
    fn wait_for_context_for<'a>(
        &'a self,
        xconn: &XConnection,
        mut client_data: RefMut<'a, ClientData>,
        target_window: Window,
    ) -> Result<(Option<u16>, RefMut<'a, ClientData>), ClientError> {
        if let Some(cid) = client_data.window_data.get(&target_window) {
            // We already have a context for this window.
            return Ok((Some(*cid), client_data));
        }

        // See if the window is in the pending queue.
        if !client_data
            .pending_windows
            .iter()
            .any(|PendingData { window, .. }| *window == target_window)
        {
            // We don't have a context for this window, and it's not in the pending queue.
            return Ok((None, client_data));
        }

        loop {
            // Wait for a new IME event.
            drop(client_data);

            self.block_for_ime(xconn)?;

            client_data = lock!(self.inner_data);

            // See if we have a context for this window now.
            if let Some(cid) = client_data.window_data.get(&target_window) {
                return Ok((Some(*cid), client_data));
            }
        }
    }
}

impl ClientHandler<XimClient> for &ImeData {
    fn handle_connect(&mut self, client: &mut XimClient) -> Result<(), ClientError> {
        // We are now connected. Request an input method with our current locale.
        lock!(self.inner_data).disconnected = false;
        client.open(&locale())
    }

    fn handle_open(
        &mut self,
        client: &mut XimClient,
        input_method_id: u16,
    ) -> Result<(), ClientError> {
        // Store the client's input method ID.
        let mut client_data = lock!(self.inner_data);
        debug_assert!(client_data.input_method.is_none());
        client_data.input_method = Some(input_method_id);

        // Ask for the IM's attributes.
        client.get_im_values(input_method_id, &[AttributeName::QueryInputStyle])
    }

    fn handle_get_im_values(
        &mut self,
        _client: &mut XimClient,
        input_method_id: u16,
        mut attributes: xim::AHashMap<xim::AttributeName, Vec<u8>>,
    ) -> Result<(), ClientError> {
        let mut client_data = lock!(self.inner_data);
        debug_assert_eq!(client_data.input_method, Some(input_method_id));

        let mut preedit_style = None;
        let mut none_style = None;

        // Get the input styles.
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

        client_data.styles = Some((preedit_style, none_style));

        Ok(())
    }

    fn handle_close(
        &mut self,
        _client: &mut XimClient,
        input_method_id: u16,
    ) -> Result<(), ClientError> {
        // We're disconnected.
        let mut client_data = lock!(self.inner_data);
        debug_assert_eq!(client_data.input_method, Some(input_method_id));
        client_data.input_method = None;

        Ok(())
    }

    fn handle_disconnect(&mut self) {
        // Indicate that we are now disconnected.
        let mut client_data = lock!(self.inner_data);
        client_data.disconnected = true;
    }

    fn handle_create_ic(
        &mut self,
        _client: &mut XimClient,
        input_method_id: u16,
        input_context_id: u16,
    ) -> Result<(), ClientError> {
        let mut client_data = lock!(self.inner_data);
        debug_assert_eq!(client_data.input_method, Some(input_method_id));

        // Assign the input context's values.
        let PendingData {
            window,
            style,
            spot,
        } = client_data
            .pending_windows
            .pop_front()
            .expect("No pending windows");
        let ic_data = IcData {
            window,
            style,
            spot,
            new_spot: None,
            id: input_context_id,
            text: Vec::new(),
            cursor: 0,
        };

        // Store the input context.
        client_data.window_data.insert(ic_data.window, ic_data.id);
        client_data.ic_data.insert(input_context_id, ic_data);

        // Indicate our status.
        let event = if matches!(style, Style::None) {
            ImeEvent::Disabled
        } else {
            ImeEvent::Enabled
        };

        client_data.events.push_back((window, event));

        Ok(())
    }

    fn handle_set_ic_values(
        &mut self,
        _client: &mut XimClient,
        input_method_id: u16,
        input_context_id: u16,
    ) -> Result<(), ClientError> {
        let mut client_data = lock!(self.inner_data);
        debug_assert_eq!(client_data.input_method, Some(input_method_id));

        // The input context has had its spot updated.
        let ic_data = client_data
            .ic_data
            .get_mut(&input_context_id)
            .expect("No input context data");

        if let Some(spot) = ic_data.new_spot.take() {
            ic_data.spot = spot;
        }

        Ok(())
    }

    // IME Callbacks

    fn handle_preedit_start(
        &mut self,
        _client: &mut XimClient,
        input_method_id: u16,
        input_context_id: u16,
    ) -> Result<(), ClientError> {
        let mut client_data = lock!(self.inner_data);
        debug_assert_eq!(client_data.input_method, Some(input_method_id));

        // Get the client data.
        if let Some(ic_data) = client_data.ic_data.get_mut(&input_context_id) {
            // We're starting a preedit.
            ic_data.text.clear();
            ic_data.cursor = 0;

            // Send a message to the window.
            let window = ic_data.window;
            client_data.events.push_back((window, ImeEvent::Start));
        }

        Ok(())
    }

    fn handle_preedit_done(
        &mut self,
        _client: &mut XimClient,
        input_method_id: u16,
        input_context_id: u16,
    ) -> Result<(), ClientError> {
        let mut client_data = lock!(self.inner_data);
        debug_assert_eq!(client_data.input_method, Some(input_method_id));

        // Get the client data.
        if let Some(ic_data) = client_data.ic_data.get_mut(&input_context_id) {
            // We're done with a preedit.
            ic_data.text.clear();
            ic_data.cursor = 0;

            // Send a message to the window.
            let window = ic_data.window;
            client_data.events.push_back((window, ImeEvent::End));
        }

        Ok(())
    }

    fn handle_preedit_draw(
        &mut self,
        _client: &mut XimClient,
        input_method_id: u16,
        input_context_id: u16,
        caret: i32,
        chg_first: i32,
        chg_len: i32,
        _status: xim::PreeditDrawStatus,
        preedit_string: &str,
        _feedbacks: Vec<xim::Feedback>,
    ) -> Result<(), ClientError> {
        let mut client_data = lock!(self.inner_data);
        debug_assert_eq!(client_data.input_method, Some(input_method_id));

        // Get the client data.
        if let Some(ic_data) = client_data.ic_data.get_mut(&input_context_id) {
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
            let event = ImeEvent::Update(ic_data.text.iter().collect(), cursor_byte_pos);
            let window = ic_data.window;

            client_data.events.push_back((window, event));
        }

        Ok(())
    }

    fn handle_preedit_caret(
        &mut self,
        _client: &mut XimClient,
        input_method_id: u16,
        input_context_id: u16,
        position: &mut i32,
        direction: xim::CaretDirection,
        _style: xim::CaretStyle,
    ) -> Result<(), ClientError> {
        if matches!(direction, xim::CaretDirection::AbsolutePosition) {
            let mut client_data = lock!(self.inner_data);
            debug_assert_eq!(client_data.input_method, Some(input_method_id));

            if let Some(ic_data) = client_data.ic_data.get_mut(&input_context_id) {
                ic_data.cursor = *position as usize;

                // Send the event.
                let window = ic_data.window;
                let event = ImeEvent::Update(ic_data.text.iter().collect(), *position as usize);

                client_data.events.push_back((window, event));
            }
        }

        Ok(())
    }

    // Callbacks we don't care about.

    fn handle_commit(
        &mut self,
        _client: &mut XimClient,
        _input_method_id: u16,
        _input_context_id: u16,
        _text: &str,
    ) -> Result<(), ClientError> {
        // Don't care.
        Ok(())
    }

    fn handle_destroy_ic(
        &mut self,
        _client: &mut XimClient,
        _input_method_id: u16,
        _input_context_id: u16,
    ) -> Result<(), ClientError> {
        // Don't care.
        Ok(())
    }

    fn handle_forward_event(
        &mut self,
        _client: &mut XimClient,
        _input_method_id: u16,
        _input_context_id: u16,
        _flag: xim::ForwardEventFlag,
        _xev: <XimClient as xim::Client>::XEvent,
    ) -> Result<(), ClientError> {
        // Don't care.
        Ok(())
    }

    fn handle_query_extension(
        &mut self,
        _client: &mut XimClient,
        _extensions: &[xim::Extension],
    ) -> Result<(), ClientError> {
        // Don't care.
        Ok(())
    }

    fn handle_set_event_mask(
        &mut self,
        _client: &mut XimClient,
        _input_method_id: u16,
        _input_context_id: u16,
        _forward_event_mask: u32,
        _synchronous_event_mask: u32,
    ) -> Result<(), ClientError> {
        // Don't care.
        Ok(())
    }
}

fn calc_byte_position(text: &[char], pos: usize) -> usize {
    text.iter().take(pos).map(|c| c.len_utf8()).sum()
}
