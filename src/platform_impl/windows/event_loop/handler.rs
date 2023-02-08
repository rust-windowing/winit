//! Decentralized event handling.

mod destroy;

use super::{GenericWindowData, LazyMessageId, WindowData};

use once_cell::sync::OnceCell;
use std::collections::HashMap;
use windows_sys::Win32::Foundation::{HWND, LPARAM, LRESULT, WPARAM};
use windows_sys::Win32::UI::WindowsAndMessaging::DefWindowProcW;

/// A map between message IDs and the functions used to handle them.
pub(super) struct WindowMessageMap {
    /// Hash map between messages and handlers.
    handlers: HashMap<u32, MessageHandler>,
}

impl WindowMessageMap {
    /// Get the global map between window messages and message handlers.
    pub(super) fn get() -> &'static WindowMessageMap {
        static MESSAGE_MAP: OnceCell<WindowMessageMap> = OnceCell::new();

        MESSAGE_MAP.get_or_init(|| {
            let mut handlers = HashMap::new();

            // Iterate over the registered window messages.
            for window_message in inventory::iter::<WindowMessage> {
                let WindowMessage { id, handler } = window_message;
                handlers.insert(id.get(), *handler);
            }

            WindowMessageMap { handlers }
        })
    }

    /// Process the provided window message.
    pub(super) fn handle_message(
        &self,
        hwnd: HWND,
        msg: u32,
        wparam: WPARAM,
        lparam: LPARAM,
        window_data: &WindowData<impl Sized + 'static>,
    ) -> LRESULT {
        if let Some(handler) = self.handlers.get(&msg) {
            // Run the provided handler.
            (*handler)(hwnd, msg, wparam, lparam, window_data)
        } else {
            // Run the default window handler.
            unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) }
        }
    }
}

/// The function pointer for handling messages.
type MessageHandler = fn(HWND, u32, WPARAM, LPARAM, &dyn GenericWindowData) -> LRESULT;

/// A handler for a window message.
struct WindowMessage {
    /// The ID to handle.
    id: MessageId,

    /// The function to use to handle it.
    handler: MessageHandler,
}

impl WindowMessage {
    fn new(id: impl Into<MessageId>, handler: MessageHandler) -> Self {
        Self {
            id: id.into(),
            handler,
        }
    }
}

inventory::collect!(WindowMessage);

/// The message ID that we're responding to.
#[derive(Copy, Clone)]
enum MessageId {
    /// This is a constant window message.
    Constant(u32),

    /// This window message will be allocated at runtime.
    Runtime(&'static LazyMessageId),
}

impl MessageId {
    fn get(self) -> u32 {
        match self {
            Self::Constant(c) => c,
            Self::Runtime(r) => r.get(),
        }
    }
}
