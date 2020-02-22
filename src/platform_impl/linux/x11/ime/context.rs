use std::{
    mem::transmute,
    os::raw::{c_short, c_void},
    ptr,
    sync::Arc,
};

use super::{ffi, util, XConnection, XError};
use crate::platform_impl::platform::x11::ime::{ImeEvent, ImeEventSender};
use std::ffi::CStr;
use x11_dl::xlib::{XIMCallback, XIMPreeditCaretCallbackStruct, XIMPreeditDrawCallbackStruct};

#[derive(Debug)]
pub enum ImeContextCreationError {
    XError(XError),
    Null,
}
type XIMProcNonnull = unsafe extern "C" fn(ffi::XIM, ffi::XPointer, ffi::XPointer);
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
        .expect("failed to send composition start event");
    -1
}

extern "C" fn preedit_done_callback(
    _xim: ffi::XIM,
    client_data: ffi::XPointer,
    _call_data: ffi::XPointer,
) {
    let client_data = unsafe { &mut *(client_data as *mut ImeContextClientData) };

    client_data
        .event_sender
        .send((client_data.window, ImeEvent::End))
        .expect("failed to send composition end event");
}

fn calc_byte_position(text: &Vec<char>, pos: usize) -> usize {
    let mut byte_pos = 0;
    for i in 0..pos {
        byte_pos += text[i].len_utf8();
    }
    byte_pos
}

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
        warn!(
            "invalid chg range: buffer length={}, but chg_first={} chg_lengthg={}",
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
        let new_text = unsafe { CStr::from_ptr(xim_text.string.multi_byte) };

        String::from(new_text.to_str().expect("Invalid UTF-8 String from IME"))
            .chars()
            .collect()
    };
    let mut old_text_tail = client_data.text.split_off(chg_range.end);
    client_data.text.split_off(chg_range.start);
    client_data.text.append(&mut new_chars);
    client_data.text.append(&mut old_text_tail);
    let cursor_byte_pos = calc_byte_position(&client_data.text, client_data.cursor_pos);

    client_data
        .event_sender
        .send((
            client_data.window,
            ImeEvent::Update(client_data.text.iter().collect(), cursor_byte_pos),
        ))
        .expect("failed to send composition update event");
}

extern "C" fn preedit_caret_callback(
    _xim: ffi::XIM,
    client_data: ffi::XPointer,
    call_data: ffi::XPointer,
) {
    let client_data = unsafe { &mut *(client_data as *mut ImeContextClientData) };
    let call_data = unsafe { &mut *(call_data as *mut XIMPreeditCaretCallbackStruct) };
    client_data.cursor_pos = call_data.position as usize;
    let cursor_byte_pos = calc_byte_position(&client_data.text, client_data.cursor_pos);

    client_data
        .event_sender
        .send((
            client_data.window,
            ImeEvent::Update(client_data.text.iter().collect(), cursor_byte_pos),
        ))
        .expect("failed to send composition update event");
}

unsafe fn create_pre_edit_attr<'a>(
    xconn: &'a Arc<XConnection>,
    preedit_callbacks: &'a PreeditCallbacks,
) -> util::XSmartPointer<'a, c_void> {
    util::XSmartPointer::new(
        xconn,
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
        ),
    )
    .expect("XVaCreateNestedList returned NULL")
}

unsafe fn create_pre_edit_attr_with_spot<'a>(
    xconn: &'a Arc<XConnection>,
    ic_spot: &'a ffi::XPoint,
    preedit_callbacks: &'a PreeditCallbacks,
) -> util::XSmartPointer<'a, c_void> {
    util::XSmartPointer::new(
        xconn,
        (xconn.xlib.XVaCreateNestedList)(
            0,
            ffi::XNSpotLocation_0.as_ptr() as *const _,
            ic_spot,
            ffi::XNPreeditStartCallback_0.as_ptr() as *const _,
            &preedit_callbacks.start_callback as *const _,
            ffi::XNPreeditDoneCallback_0.as_ptr() as *const _,
            &preedit_callbacks.done_callback as *const _,
            ffi::XNPreeditCaretCallback_0.as_ptr() as *const _,
            &preedit_callbacks.caret_callback as *const _,
            ffi::XNPreeditDrawCallback_0.as_ptr() as *const _,
            &preedit_callbacks.draw_callback as *const _,
            ptr::null_mut::<()>(),
        ),
    )
    .expect("XVaCreateNestedList returned NULL")
}

fn create_xim_callback(client_data: ffi::XPointer, callback: XIMProcNonnull) -> ffi::XIMCallback {
    XIMCallback {
        client_data,
        callback: Some(callback),
    }
}

pub struct PreeditCallbacks {
    pub start_callback: ffi::XIMCallback,
    pub done_callback: ffi::XIMCallback,
    pub draw_callback: ffi::XIMCallback,
    pub caret_callback: ffi::XIMCallback,
}

impl PreeditCallbacks {
    pub fn new(client_data: ffi::XPointer) -> PreeditCallbacks {
        let start_callback = create_xim_callback(client_data, unsafe {
            transmute(preedit_start_callback as usize)
        });
        let done_callback = create_xim_callback(client_data, preedit_done_callback);
        let caret_callback = create_xim_callback(client_data, preedit_caret_callback);
        let draw_callback = create_xim_callback(client_data, preedit_draw_callback);

        PreeditCallbacks {
            start_callback,
            done_callback,
            caret_callback,
            draw_callback,
        }
    }
}

pub struct ImeContextClientData {
    pub window: ffi::Window,
    pub event_sender: ImeEventSender,
    pub text: Vec<char>,
    pub cursor_pos: usize,
}

// WARNING: this struct doesn't destroy its XIC resource when dropped.
// This is intentional, as it doesn't have enough information to know whether or not the context
// still exists on the server. Since `ImeInner` has that awareness, destruction must be handled
// through `ImeInner`.
pub struct ImeContext {
    pub ic: ffi::XIC,
    pub ic_spot: ffi::XPoint,
    pub preedit_callbacks: PreeditCallbacks,
    pub client_data: Box<ImeContextClientData>,
}

impl ImeContext {
    pub unsafe fn new(
        xconn: &Arc<XConnection>,
        im: ffi::XIM,
        window: ffi::Window,
        ic_spot: Option<ffi::XPoint>,
        event_sender: ImeEventSender,
    ) -> Result<Self, ImeContextCreationError> {
        let client_data = Box::new(ImeContextClientData {
            window,
            event_sender,
            text: Vec::new(),
            cursor_pos: 0,
        });
        let client_data_ptr = Box::into_raw(client_data);
        let preedit_callbacks = PreeditCallbacks::new(client_data_ptr as ffi::XPointer);
        let ic = if let Some(ic_spot) = ic_spot {
            ImeContext::create_ic_with_spot(xconn, im, window, ic_spot, &preedit_callbacks)
        } else {
            ImeContext::create_ic(xconn, im, window, &preedit_callbacks)
        };

        let ic = ic.ok_or(ImeContextCreationError::Null)?;
        xconn
            .check_errors()
            .map_err(ImeContextCreationError::XError)?;

        Ok(ImeContext {
            ic,
            ic_spot: ic_spot.unwrap_or_else(|| ffi::XPoint { x: 0, y: 0 }),
            preedit_callbacks,
            client_data: Box::from_raw(client_data_ptr),
        })
    }

    unsafe fn create_ic(
        xconn: &Arc<XConnection>,
        im: ffi::XIM,
        window: ffi::Window,
        preedit_callbacks: &PreeditCallbacks,
    ) -> Option<ffi::XIC> {
        let pre_edit_attr = create_pre_edit_attr(xconn, preedit_callbacks);
        let ic = (xconn.xlib.XCreateIC)(
            im,
            ffi::XNInputStyle_0.as_ptr() as *const _,
            ffi::XIMPreeditCallbacks | ffi::XIMStatusNothing,
            ffi::XNClientWindow_0.as_ptr() as *const _,
            window,
            ffi::XNPreeditAttributes_0.as_ptr(),
            pre_edit_attr.ptr,
            ptr::null_mut::<()>(),
        );
        if ic.is_null() {
            None
        } else {
            Some(ic)
        }
    }

    unsafe fn create_ic_with_spot(
        xconn: &Arc<XConnection>,
        im: ffi::XIM,
        window: ffi::Window,
        ic_spot: ffi::XPoint,
        preedit_callbacks: &PreeditCallbacks,
    ) -> Option<ffi::XIC> {
        let pre_edit_attr = create_pre_edit_attr_with_spot(xconn, &ic_spot, preedit_callbacks);
        let ic = (xconn.xlib.XCreateIC)(
            im,
            ffi::XNInputStyle_0.as_ptr() as *const _,
            ffi::XIMPreeditCallbacks | ffi::XIMStatusNothing,
            ffi::XNClientWindow_0.as_ptr() as *const _,
            window,
            ffi::XNPreeditAttributes_0.as_ptr() as *const _,
            pre_edit_attr.ptr,
            ptr::null_mut::<()>(),
        );
        if ic.is_null() {
            None
        } else {
            Some(ic)
        }
    }

    pub fn focus(&self, xconn: &Arc<XConnection>) -> Result<(), XError> {
        unsafe {
            (xconn.xlib.XSetICFocus)(self.ic);
        }
        xconn.check_errors()
    }

    pub fn unfocus(&self, xconn: &Arc<XConnection>) -> Result<(), XError> {
        unsafe {
            (xconn.xlib.XUnsetICFocus)(self.ic);
        }
        xconn.check_errors()
    }

    pub fn set_spot(&mut self, xconn: &Arc<XConnection>, x: c_short, y: c_short) {
        if self.ic_spot.x == x && self.ic_spot.y == y {
            return;
        }
        self.ic_spot = ffi::XPoint { x, y };

        unsafe {
            let pre_edit_attr =
                create_pre_edit_attr_with_spot(xconn, &self.ic_spot, &self.preedit_callbacks);
            (xconn.xlib.XSetICValues)(
                self.ic,
                ffi::XNPreeditAttributes_0.as_ptr() as *const _,
                pre_edit_attr.ptr,
                ptr::null_mut::<()>(),
            );
        }
    }
}
