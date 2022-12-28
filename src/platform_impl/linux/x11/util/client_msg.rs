use super::*;
use x11rb::protocol::xproto::{self, ClientMessageData, ConnectionExt};

impl XConnection {
    pub fn send_client_msg(
        &self,
        window: xproto::Window, // The window this is "about"; not necessarily this window
        target_window: xproto::Window, // The window we're sending to
        message_type: xproto::Atom,
        event_mask: Option<xproto::EventMask>,
        format: u8,
        data: impl Into<ClientMessageData>,
    ) -> Result<XcbVoidCookie<'_>, PlatformError> {
        let event = xproto::ClientMessageEvent {
            response_type: xproto::CLIENT_MESSAGE_EVENT,
            sequence: 0xBEEF, // automatically assigned
            window,
            type_: message_type,
            format,
            data: data.into(),
        };

        // Send the event.
        self.connection
            .send_event(
                false,
                target_window,
                event_mask.unwrap_or(xproto::EventMask::NO_EVENT),
                event,
            )
            .map_err(PlatformError::from)
    }
}
