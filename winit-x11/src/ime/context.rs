use std::error::Error;
use std::ffi::CStr;
use std::sync::Arc;
use std::{fmt, mem, ptr};

use x11_dl::xlib::{XIMCallback, XIMPreeditCaretCallbackStruct, XIMPreeditDrawCallbackStruct};

use super::input_method::{InputMethod, Style, XIMStyle};
use super::{ImeEvent, ImeEventSender, ffi, util};
use crate::xdisplay::{XConnection, XError};

/// IME creation error.
#[derive(Debug)]
pub enum ImeContextCreationError {
    /// Got the error from Xlib.
    XError(XError),

    /// Got null pointer from Xlib but without exact reason.
    Null,
}

impl fmt::Display for ImeContextCreationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ImeContextCreationError::XError(err) => err.fmt(f),
            ImeContextCreationError::Null => {
                write!(f, "got null pointer from Xlib without exact reason")
            },
        }
    }
}

impl Error for ImeContextCreationError {}

/// The callback used by XIM preedit functions.
type XIMProcNonnull = unsafe extern "C" fn(ffi::XIM, ffi::XPointer, ffi::XPointer);

/// Wrapper for creating XIM callbacks.
#[inline]
fn create_xim_callback(client_data: ffi::XPointer, callback: XIMProcNonnull) -> ffi::XIMCallback {
    XIMCallback { client_data, callback: Some(callback) }
}

/// The server started preedit.
extern "C" fn preedit_start_callback(
    _xim: ffi::XIM,
    client_data: ffi::XPointer,
    _call_data: ffi::XPointer,
) -> i32 {
    let client_data = unsafe { &mut *(client_data as *mut ImeContextClientData) };

    client_data.text.clear();
    client_data.cursor_pos = 0;
    client_data
        .event_sender
        .send((client_data.window, ImeEvent::Start))
        .expect("failed to send preedit start event");
    -1
}

/// Done callback is used when the preedit should be hidden.
extern "C" fn preedit_done_callback(
    _xim: ffi::XIM,
    client_data: ffi::XPointer,
    _call_data: ffi::XPointer,
) {
    let client_data = unsafe { &mut *(client_data as *mut ImeContextClientData) };

    // Drop text buffer and reset cursor position on done.
    client_data.text = Vec::new();
    client_data.cursor_pos = 0;

    client_data
        .event_sender
        .send((client_data.window, ImeEvent::End))
        .expect("failed to send preedit end event");
}

fn calc_byte_position(text: &[char], pos: usize) -> usize {
    text.iter().take(pos).fold(0, |byte_pos, text| byte_pos + text.len_utf8())
}

/// Preedit text information to be drawn inline by the client.
extern "C" fn preedit_draw_callback(
    _xim: ffi::XIM,
    client_data: ffi::XPointer,
    call_data: ffi::XPointer,
) {
    let client_data = unsafe { &mut *(client_data as *mut ImeContextClientData) };
    let call_data = unsafe { &mut *(call_data as *mut XIMPreeditDrawCallbackStruct) };
    client_data.cursor_pos = call_data.caret as usize;

    let chg_range =
        call_data.chg_first as usize..(call_data.chg_first + call_data.chg_length) as usize;
    if chg_range.start > client_data.text.len() || chg_range.end > client_data.text.len() {
        tracing::warn!(
            "invalid chg range: buffer length={}, but chg_first={} chg_length={}",
            client_data.text.len(),
            call_data.chg_first,
            call_data.chg_length
        );
        return;
    }

    // NULL indicate text deletion
    let mut new_chars = if call_data.text.is_null() {
        Vec::new()
    } else {
        let xim_text = unsafe { &mut *(call_data.text) };
        if xim_text.encoding_is_wchar > 0 {
            return;
        }

        let new_text = unsafe { xim_text.string.multi_byte };

        if new_text.is_null() {
            return;
        }

        let new_text = unsafe { CStr::from_ptr(new_text) };

        String::from(new_text.to_str().expect("Invalid UTF-8 String from IME")).chars().collect()
    };
    let mut old_text_tail = client_data.text.split_off(chg_range.end);
    client_data.text.truncate(chg_range.start);
    client_data.text.append(&mut new_chars);
    client_data.text.append(&mut old_text_tail);
    let cursor_byte_pos = calc_byte_position(&client_data.text, client_data.cursor_pos);

    client_data
        .event_sender
        .send((
            client_data.window,
            ImeEvent::Update(client_data.text.iter().collect(), cursor_byte_pos),
        ))
        .expect("failed to send preedit update event");
}

/// Handling of cursor movements in preedit text.
extern "C" fn preedit_caret_callback(
    _xim: ffi::XIM,
    client_data: ffi::XPointer,
    call_data: ffi::XPointer,
) {
    let client_data = unsafe { &mut *(client_data as *mut ImeContextClientData) };
    let call_data = unsafe { &mut *(call_data as *mut XIMPreeditCaretCallbackStruct) };

    if call_data.direction == ffi::XIMCaretDirection::XIMAbsolutePosition {
        client_data.cursor_pos = call_data.position as usize;
        let cursor_byte_pos = calc_byte_position(&client_data.text, client_data.cursor_pos);

        client_data
            .event_sender
            .send((
                client_data.window,
                ImeEvent::Update(client_data.text.iter().collect(), cursor_byte_pos),
            ))
            .expect("failed to send preedit update event");
    }
}

/// Struct to simplify callback creation and latter passing into Xlib XIM.
struct PreeditCallbacks {
    start_callback: ffi::XIMCallback,
    done_callback: ffi::XIMCallback,
    draw_callback: ffi::XIMCallback,
    caret_callback: ffi::XIMCallback,
}

impl PreeditCallbacks {
    pub fn new(client_data: ffi::XPointer) -> PreeditCallbacks {
        let start_callback = create_xim_callback(client_data, unsafe {
            mem::transmute::<usize, unsafe extern "C" fn(ffi::XIM, ffi::XPointer, ffi::XPointer)>(
                preedit_start_callback as *const () as usize,
            )
        });
        let done_callback = create_xim_callback(client_data, preedit_done_callback);
        let caret_callback = create_xim_callback(client_data, preedit_caret_callback);
        let draw_callback = create_xim_callback(client_data, preedit_draw_callback);

        PreeditCallbacks { start_callback, done_callback, caret_callback, draw_callback }
    }
}

struct ImeContextClientData {
    window: ffi::Window,
    event_sender: ImeEventSender,
    text: Vec<char>,
    cursor_pos: usize,
}

// XXX: this struct doesn't destroy its XIC resource when dropped.
// This is intentional, as it doesn't have enough information to know whether or not the context
// still exists on the server. Since `ImeInner` has that awareness, destruction must be handled
// through `ImeInner`.
pub struct ImeContext {
    pub(crate) ic: ffi::XIC,
    pub(crate) ic_area: ffi::XRectangle,
    pub(crate) allowed: bool,
    // Since the data is passed shared between X11 XIM callbacks, but couldn't be directly free
    // from there we keep the pointer to automatically deallocate it.
    _client_data: Box<ImeContextClientData>,
}

impl ImeContext {
    pub(crate) unsafe fn new(
        xconn: &Arc<XConnection>,
        im: &InputMethod,
        window: ffi::Window,
        ic_area: Option<ffi::XRectangle>,
        event_sender: ImeEventSender,
        allowed: bool,
    ) -> Result<Self, ImeContextCreationError> {
        let client_data = Box::into_raw(Box::new(ImeContextClientData {
            window,
            event_sender,
            text: Vec::new(),
            cursor_pos: 0,
        }));

        let style = if allowed { im.preedit_style } else { im.none_style };

        let ic = match style as _ {
            Style::Preedit(style) => unsafe {
                ImeContext::create_preedit_ic(
                    xconn,
                    im.im,
                    style,
                    window,
                    client_data as ffi::XPointer,
                )
            },
            Style::Nothing(style) => unsafe {
                ImeContext::create_nothing_ic(xconn, im.im, style, window)
            },
            Style::None(style) => unsafe {
                ImeContext::create_none_ic(xconn, im.im, style, window)
            },
        }
        .ok_or(ImeContextCreationError::Null)?;

        xconn.check_errors().map_err(ImeContextCreationError::XError)?;

        let mut context = ImeContext {
            ic,
            ic_area: ffi::XRectangle { x: 0, y: 0, width: 0, height: 0 },
            allowed,
            _client_data: unsafe { Box::from_raw(client_data) },
        };

        // Set the preedit cursor area, if it's present.
        if let Some(ic_area) = ic_area {
            context.set_area(xconn, ic_area.x, ic_area.y, ic_area.width, ic_area.height);
        }

        Ok(context)
    }

    unsafe fn create_none_ic(
        xconn: &Arc<XConnection>,
        im: ffi::XIM,
        style: XIMStyle,
        window: ffi::Window,
    ) -> Option<ffi::XIC> {
        let ic = unsafe {
            (xconn.xlib.XCreateIC)(
                im,
                ffi::XNInputStyle_0.as_ptr() as *const _,
                style,
                ffi::XNClientWindow_0.as_ptr() as *const _,
                window,
                ptr::null_mut::<()>(),
            )
        };

        (!ic.is_null()).then_some(ic)
    }

    unsafe fn create_preedit_ic(
        xconn: &Arc<XConnection>,
        im: ffi::XIM,
        style: XIMStyle,
        window: ffi::Window,
        client_data: ffi::XPointer,
    ) -> Option<ffi::XIC> {
        let preedit_callbacks = PreeditCallbacks::new(client_data);
        let preedit_attr = util::memory::XSmartPointer::new(xconn, unsafe {
            (xconn.xlib.XVaCreateNestedList)(
                0,
                ffi::XNPreeditStartCallback_0.as_ptr() as *const _,
                &(preedit_callbacks.start_callback) as *const _,
                ffi::XNPreeditDoneCallback_0.as_ptr() as *const _,
                &(preedit_callbacks.done_callback) as *const _,
                ffi::XNPreeditCaretCallback_0.as_ptr() as *const _,
                &(preedit_callbacks.caret_callback) as *const _,
                ffi::XNPreeditDrawCallback_0.as_ptr() as *const _,
                &(preedit_callbacks.draw_callback) as *const _,
                ptr::null_mut::<()>(),
            )
        })
        .expect("XVaCreateNestedList returned NULL");

        let ic = unsafe {
            (xconn.xlib.XCreateIC)(
                im,
                ffi::XNInputStyle_0.as_ptr() as *const _,
                style,
                ffi::XNClientWindow_0.as_ptr() as *const _,
                window,
                ffi::XNPreeditAttributes_0.as_ptr() as *const _,
                preedit_attr.ptr,
                ptr::null_mut::<()>(),
            )
        };

        (!ic.is_null()).then_some(ic)
    }

    unsafe fn create_nothing_ic(
        xconn: &Arc<XConnection>,
        im: ffi::XIM,
        style: XIMStyle,
        window: ffi::Window,
    ) -> Option<ffi::XIC> {
        let ic = unsafe {
            (xconn.xlib.XCreateIC)(
                im,
                ffi::XNInputStyle_0.as_ptr() as *const _,
                style,
                ffi::XNClientWindow_0.as_ptr() as *const _,
                window,
                ptr::null_mut::<()>(),
            )
        };

        (!ic.is_null()).then_some(ic)
    }

    pub(crate) fn focus(&self, xconn: &Arc<XConnection>) -> Result<(), XError> {
        unsafe {
            (xconn.xlib.XSetICFocus)(self.ic);
        }
        xconn.check_errors()
    }

    pub(crate) fn unfocus(&self, xconn: &Arc<XConnection>) -> Result<(), XError> {
        unsafe {
            (xconn.xlib.XUnsetICFocus)(self.ic);
        }
        xconn.check_errors()
    }

    pub fn is_allowed(&self) -> bool {
        self.allowed
    }

    /// Set the spot and area for preedit text.
    ///
    /// This functionality depends on the libx11 version.
    ///  - Until libx11 1.8.2, XNSpotLocation was blocked by libx11 in On-The-Spot mode.
    ///  - Until libx11 1.8.11, XNArea was blocked by libx11 in On-The-Spot mode.
    ///
    /// Use of this information is discretionary by input method servers,
    /// and some may not use it by default, even if they have support.
    pub(crate) fn set_area(
        &mut self,
        xconn: &Arc<XConnection>,
        x: i16,
        y: i16,
        width: u16,
        height: u16,
    ) {
        let ic_area = ffi::XRectangle { x, y, width, height };

        if !self.is_allowed() || self.ic_area == ic_area {
            return;
        }

        self.ic_area = ic_area;
        let ic_spot =
            ffi::XPoint { x: x.saturating_add(width as i16), y: y.saturating_add(height as i16) };

        unsafe {
            let preedit_attr = util::memory::XSmartPointer::new(
                xconn,
                (xconn.xlib.XVaCreateNestedList)(
                    0,
                    ffi::XNSpotLocation_0.as_ptr(),
                    &ic_spot,
                    ffi::XNArea_0.as_ptr(),
                    &self.ic_area,
                    ptr::null_mut::<()>(),
                ),
            )
            .expect("XVaCreateNestedList returned NULL");

            (xconn.xlib.XSetICValues)(
                self.ic,
                ffi::XNPreeditAttributes_0.as_ptr() as *const _,
                preedit_attr.ptr,
                ptr::null_mut::<()>(),
            );
        }
    }
}
