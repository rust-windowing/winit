//! Decentralized event handling.

use super::{GenericWindowData, LazyMessageId, WindowData};

use once_cell::sync::OnceCell;
use std::collections::HashMap;
use windows_sys::Win32::Foundation::{HWND, LPARAM, LRESULT, WPARAM};
use windows_sys::Win32::UI::WindowsAndMessaging::DefWindowProcW;

/// Submit several event handlers at once.
macro_rules! submit {
    ($(($msg:expr,$handler:expr)),* $(,)?) => {
        $(
            inventory::submit! {
                WindowMessage::from_const(
                    $msg,
                    $handler
                )
            }
        )*
    }
}

mod external;
mod input;
mod keyboard;
mod mouse;
mod pointer;
mod sizing;
mod state_change;

/// A map between message IDs and the functions used to handle them.
pub(crate) struct WindowMessageMap {
    /// Hash map between messages and handlers.
    handlers: HashMap<u32, MessageHandler>,
}

impl WindowMessageMap {
    /// Get the global map between window messages and message handlers.
    pub(crate) fn get() -> &'static WindowMessageMap {
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
    const fn from_const(id: u32, handler: MessageHandler) -> Self {
        Self {
            id: MessageId::Constant(id),
            handler,
        }
    }

    const fn from_user(user: &'static LazyMessageId, handler: MessageHandler) -> Self {
        Self {
            id: MessageId::Runtime(user),
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

impl From<u32> for MessageId {
    fn from(id: u32) -> Self {
        Self::Constant(id)
    }
}

impl From<&'static LazyMessageId> for MessageId {
    fn from(id: &'static LazyMessageId) -> Self {
        Self::Runtime(id)
    }
}

/// Typically-imported items for message handlers.
mod prelude {
    pub(super) use super::{
        super::{
            super::{
                dark_mode::try_theme,
                dpi::dpi_to_scale_factor,
                event::{self, process_key_params},
                get_x_lparam, get_xbutton_wparam, get_y_lparam, hiword,
                ime::ImeContext,
                loword,
                monitor::{self, MonitorHandle},
                util,
                window_state::{CursorFlags, ImeState, WindowFlags, WindowState},
                DEVICE_ID,
            },
            Fullscreen, GenericWindowData,
        },
        input::{
            capture_mouse, gain_active_focus, lose_active_focus, normalize_pointer_pressure,
            release_mouse, update_modifiers,
        },
        WindowMessage,
    };

    pub(super) use crate::dpi::{PhysicalPosition, PhysicalSize};
    pub(super) use crate::event::{Event, Ime, KeyboardInput, Touch, TouchPhase, WindowEvent};
    pub(super) use crate::platform_impl::platform::WindowId;
    pub(super) use crate::window::WindowId as RootWindowId;

    pub(super) use windows_sys::Win32::{
        Foundation::{HWND, LPARAM, LRESULT, POINT, RECT, WPARAM},
        Graphics::Gdi::{
            GetMonitorInfoW, MonitorFromRect, MonitorFromWindow, ScreenToClient, MONITORINFO,
            MONITOR_DEFAULTTONULL, SC_SCREENSAVE,
        },
        UI::{
            Controls::{HOVER_DEFAULT, WM_MOUSELEAVE},
            Input::{
                Ime::{GCS_COMPSTR, GCS_RESULTSTR, ISC_SHOWUICOMPOSITIONWINDOW},
                KeyboardAndMouse::{
                    MapVirtualKeyA, ReleaseCapture, SetCapture, TrackMouseEvent, MAPVK_VK_TO_VSC,
                    TME_LEAVE, TRACKMOUSEEVENT,
                },
                Pointer::{POINTER_FLAG_DOWN, POINTER_FLAG_UP, POINTER_FLAG_UPDATE},
                Touch::{
                    CloseTouchInputHandle, GetTouchInputInfo, TOUCHEVENTF_DOWN, TOUCHEVENTF_MOVE,
                    TOUCHEVENTF_UP, TOUCHINPUT,
                },
            },
            WindowsAndMessaging::{
                DefWindowProcW, DestroyWindow, GetCursorPos, GetMenu, LoadCursorW, PostMessageW,
                SetCursor, SetWindowPos, HTCAPTION, HTCLIENT, MINMAXINFO, MNC_CLOSE,
                NCCALCSIZE_PARAMS, PT_PEN, PT_TOUCH, SC_MINIMIZE, SC_RESTORE, SIZE_MAXIMIZED,
                SWP_NOACTIVATE, SWP_NOMOVE, SWP_NOSIZE, SWP_NOZORDER, WHEEL_DELTA, WINDOWPOS,
                WM_CAPTURECHANGED, WM_CHAR, WM_DPICHANGED, WM_DROPFILES, WM_ENTERSIZEMOVE,
                WM_EXITSIZEMOVE, WM_GETMINMAXINFO, WM_IME_COMPOSITION, WM_IME_ENDCOMPOSITION,
                WM_IME_SETCONTEXT, WM_IME_STARTCOMPOSITION, WM_KEYDOWN, WM_KEYUP, WM_KILLFOCUS,
                WM_LBUTTONDOWN, WM_LBUTTONUP, WM_MBUTTONDOWN, WM_MBUTTONUP, WM_MENUCHAR,
                WM_MOUSEHWHEEL, WM_MOUSEMOVE, WM_MOUSEWHEEL, WM_NCACTIVATE, WM_NCCALCSIZE,
                WM_NCLBUTTONDOWN, WM_POINTERDOWN, WM_POINTERUP, WM_POINTERUPDATE, WM_RBUTTONDOWN,
                WM_RBUTTONUP, WM_SETCURSOR, WM_SETFOCUS, WM_SETTINGCHANGE, WM_SIZE, WM_SYSCHAR,
                WM_SYSCOMMAND, WM_SYSKEYDOWN, WM_SYSKEYUP, WM_TOUCH, WM_WINDOWPOSCHANGED,
                WM_WINDOWPOSCHANGING, WM_XBUTTONDOWN, WM_XBUTTONUP,
            },
        },
    };
}
