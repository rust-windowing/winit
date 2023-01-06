use std::{
    ffi::{c_void, OsStr, OsString},
    io,
    iter::once,
    mem,
    ops::BitAnd,
    os::windows::prelude::{OsStrExt, OsStringExt},
    ptr,
    sync::atomic::{AtomicBool, Ordering},
};

use once_cell::sync::Lazy;
use windows_sys::{
    core::{HRESULT, PCWSTR},
    Win32::{
        Foundation::{BOOL, HINSTANCE, HWND, RECT},
        Graphics::Gdi::{ClientToScreen, HMONITOR},
        System::{
            LibraryLoader::{GetProcAddress, LoadLibraryA},
            SystemServices::IMAGE_DOS_HEADER,
        },
        UI::{
            HiDpi::{DPI_AWARENESS_CONTEXT, MONITOR_DPI_TYPE, PROCESS_DPI_AWARENESS},
            Input::KeyboardAndMouse::GetActiveWindow,
            WindowsAndMessaging::{
                self as wam, ClipCursor, GetClientRect, GetClipCursor, GetSystemMetrics,
                GetWindowRect, ShowCursor, IDC_APPSTARTING, IDC_ARROW, IDC_CROSS, IDC_HAND,
                IDC_HELP, IDC_IBEAM, IDC_NO, IDC_SIZEALL, IDC_SIZENESW, IDC_SIZENS, IDC_SIZENWSE,
                IDC_SIZEWE, IDC_WAIT, SM_CXVIRTUALSCREEN, SM_CYVIRTUALSCREEN, SM_XVIRTUALSCREEN,
                SM_YVIRTUALSCREEN,
            },
        },
    },
};

use crate::window::CursorIcon;

pub(crate) fn msg_name(msg: u32) -> String {
    let msg_str = match msg {
        wam::WM_NULL => Some("WM_NULL"),
        wam::WM_CREATE => Some("WM_CREATE"),
        wam::WM_DESTROY => Some("WM_DESTROY"),
        wam::WM_MOVE => Some("WM_MOVE"),
        wam::WM_SIZE => Some("WM_SIZE"),
        wam::WM_ACTIVATE => Some("WM_ACTIVATE"),
        wam::WM_SETFOCUS => Some("WM_SETFOCUS"),
        wam::WM_KILLFOCUS => Some("WM_KILLFOCUS"),
        wam::WM_ENABLE => Some("WM_ENABLE"),
        wam::WM_SETREDRAW => Some("WM_SETREDRAW"),
        wam::WM_SETTEXT => Some("WM_SETTEXT"),
        wam::WM_GETTEXT => Some("WM_GETTEXT"),
        wam::WM_GETTEXTLENGTH => Some("WM_GETTEXTLENGTH"),
        wam::WM_PAINT => Some("WM_PAINT"),
        wam::WM_CLOSE => Some("WM_CLOSE"),
        wam::WM_QUERYENDSESSION => Some("WM_QUERYENDSESSION"),
        wam::WM_QUIT => Some("WM_QUIT"),
        wam::WM_QUERYOPEN => Some("WM_QUERYOPEN"),
        wam::WM_ERASEBKGND => Some("WM_ERASEBKGND"),
        wam::WM_SYSCOLORCHANGE => Some("WM_SYSCOLORCHANGE"),
        wam::WM_ENDSESSION => Some("WM_ENDSESSION"),
        wam::WM_SHOWWINDOW => Some("WM_SHOWWINDOW"),
        // wam::WM_CTLCOLOR => Some("WM_CTLCOLOR"),
        wam::WM_WININICHANGE => Some("WM_WININICHANGE"),
        wam::WM_DEVMODECHANGE => Some("WM_DEVMODECHANGE"),
        wam::WM_ACTIVATEAPP => Some("WM_ACTIVATEAPP"),
        wam::WM_FONTCHANGE => Some("WM_FONTCHANGE"),
        wam::WM_TIMECHANGE => Some("WM_TIMECHANGE"),
        wam::WM_CANCELMODE => Some("WM_CANCELMODE"),
        wam::WM_SETCURSOR => Some("WM_SETCURSOR"),
        wam::WM_MOUSEACTIVATE => Some("WM_MOUSEACTIVATE"),
        wam::WM_CHILDACTIVATE => Some("WM_CHILDACTIVATE"),
        wam::WM_QUEUESYNC => Some("WM_QUEUESYNC"),
        wam::WM_GETMINMAXINFO => Some("WM_GETMINMAXINFO"),
        wam::WM_PAINTICON => Some("WM_PAINTICON"),
        wam::WM_ICONERASEBKGND => Some("WM_ICONERASEBKGND"),
        wam::WM_NEXTDLGCTL => Some("WM_NEXTDLGCTL"),
        wam::WM_SPOOLERSTATUS => Some("WM_SPOOLERSTATUS"),
        wam::WM_DRAWITEM => Some("WM_DRAWITEM"),
        wam::WM_MEASUREITEM => Some("WM_MEASUREITEM"),
        wam::WM_DELETEITEM => Some("WM_DELETEITEM"),
        wam::WM_VKEYTOITEM => Some("WM_VKEYTOITEM"),
        wam::WM_CHARTOITEM => Some("WM_CHARTOITEM"),
        wam::WM_SETFONT => Some("WM_SETFONT"),
        wam::WM_GETFONT => Some("WM_GETFONT"),
        wam::WM_SETHOTKEY => Some("WM_SETHOTKEY"),
        wam::WM_GETHOTKEY => Some("WM_GETHOTKEY"),
        wam::WM_QUERYDRAGICON => Some("WM_QUERYDRAGICON"),
        wam::WM_COMPAREITEM => Some("WM_COMPAREITEM"),
        wam::WM_GETOBJECT => Some("WM_GETOBJECT"),
        wam::WM_COMPACTING => Some("WM_COMPACTING"),
        wam::WM_COMMNOTIFY => Some("WM_COMMNOTIFY"),
        wam::WM_WINDOWPOSCHANGING => Some("WM_WINDOWPOSCHANGING"),
        wam::WM_WINDOWPOSCHANGED => Some("WM_WINDOWPOSCHANGED"),
        wam::WM_POWER => Some("WM_POWER"),
        // wam::WM_COPYGLOBALDATA => Some("WM_COPYGLOBALDATA"),
        wam::WM_COPYDATA => Some("WM_COPYDATA"),
        wam::WM_CANCELJOURNAL => Some("WM_CANCELJOURNAL"),
        wam::WM_NOTIFY => Some("WM_NOTIFY"),
        wam::WM_INPUTLANGCHANGEREQUEST => Some("WM_INPUTLANGCHANGEREQUEST"),
        wam::WM_INPUTLANGCHANGE => Some("WM_INPUTLANGCHANGE"),
        wam::WM_TCARD => Some("WM_TCARD"),
        wam::WM_HELP => Some("WM_HELP"),
        wam::WM_USERCHANGED => Some("WM_USERCHANGED"),
        wam::WM_NOTIFYFORMAT => Some("WM_NOTIFYFORMAT"),
        wam::WM_CONTEXTMENU => Some("WM_CONTEXTMENU"),
        wam::WM_STYLECHANGING => Some("WM_STYLECHANGING"),
        wam::WM_STYLECHANGED => Some("WM_STYLECHANGED"),
        wam::WM_DISPLAYCHANGE => Some("WM_DISPLAYCHANGE"),
        wam::WM_GETICON => Some("WM_GETICON"),
        wam::WM_SETICON => Some("WM_SETICON"),
        wam::WM_NCCREATE => Some("WM_NCCREATE"),
        wam::WM_NCDESTROY => Some("WM_NCDESTROY"),
        wam::WM_NCCALCSIZE => Some("WM_NCCALCSIZE"),
        wam::WM_NCHITTEST => Some("WM_NCHITTEST"),
        wam::WM_NCPAINT => Some("WM_NCPAINT"),
        wam::WM_NCACTIVATE => Some("WM_NCACTIVATE"),
        wam::WM_GETDLGCODE => Some("WM_GETDLGCODE"),
        wam::WM_SYNCPAINT => Some("WM_SYNCPAINT"),
        wam::WM_NCMOUSEMOVE => Some("WM_NCMOUSEMOVE"),
        wam::WM_NCLBUTTONDOWN => Some("WM_NCLBUTTONDOWN"),
        wam::WM_NCLBUTTONUP => Some("WM_NCLBUTTONUP"),
        wam::WM_NCLBUTTONDBLCLK => Some("WM_NCLBUTTONDBLCLK"),
        wam::WM_NCRBUTTONDOWN => Some("WM_NCRBUTTONDOWN"),
        wam::WM_NCRBUTTONUP => Some("WM_NCRBUTTONUP"),
        wam::WM_NCRBUTTONDBLCLK => Some("WM_NCRBUTTONDBLCLK"),
        wam::WM_NCMBUTTONDOWN => Some("WM_NCMBUTTONDOWN"),
        wam::WM_NCMBUTTONUP => Some("WM_NCMBUTTONUP"),
        wam::WM_NCMBUTTONDBLCLK => Some("WM_NCMBUTTONDBLCLK"),
        wam::WM_NCXBUTTONDOWN => Some("WM_NCXBUTTONDOWN"),
        wam::WM_NCXBUTTONUP => Some("WM_NCXBUTTONUP"),
        wam::WM_NCXBUTTONDBLCLK => Some("WM_NCXBUTTONDBLCLK"),
        // wam::EM_GETSEL => Some("EM_GETSEL"),
        // wam::EM_SETSEL => Some("EM_SETSEL"),
        // wam::EM_GETRECT => Some("EM_GETRECT"),
        // wam::EM_SETRECT => Some("EM_SETRECT"),
        // wam::EM_SETRECTNP => Some("EM_SETRECTNP"),
        // wam::EM_SCROLL => Some("EM_SCROLL"),
        // wam::EM_LINESCROLL => Some("EM_LINESCROLL"),
        // wam::EM_SCROLLCARET => Some("EM_SCROLLCARET"),
        // wam::EM_GETMODIFY => Some("EM_GETMODIFY"),
        // wam::EM_SETMODIFY => Some("EM_SETMODIFY"),
        // wam::EM_GETLINECOUNT => Some("EM_GETLINECOUNT"),
        // wam::EM_LINEINDEX => Some("EM_LINEINDEX"),
        // wam::EM_SETHANDLE => Some("EM_SETHANDLE"),
        // wam::EM_GETHANDLE => Some("EM_GETHANDLE"),
        // wam::EM_GETTHUMB => Some("EM_GETTHUMB"),
        // wam::EM_LINELENGTH => Some("EM_LINELENGTH"),
        // wam::EM_REPLACESEL => Some("EM_REPLACESEL"),
        // wam::EM_SETFONT => Some("EM_SETFONT"),
        // wam::EM_GETLINE => Some("EM_GETLINE"),
        // wam::EM_LIMITTEXT => Some("EM_LIMITTEXT"),
        // wam::EM_SETLIMITTEXT => Some("EM_SETLIMITTEXT"),
        // wam::EM_CANUNDO => Some("EM_CANUNDO"),
        // wam::EM_UNDO => Some("EM_UNDO"),
        // wam::EM_FMTLINES => Some("EM_FMTLINES"),
        // wam::EM_LINEFROMCHAR => Some("EM_LINEFROMCHAR"),
        // wam::EM_SETWORDBREAK => Some("EM_SETWORDBREAK"),
        // wam::EM_SETTABSTOPS => Some("EM_SETTABSTOPS"),
        // wam::EM_SETPASSWORDCHAR => Some("EM_SETPASSWORDCHAR"),
        // wam::EM_EMPTYUNDOBUFFER => Some("EM_EMPTYUNDOBUFFER"),
        // wam::EM_GETFIRSTVISIBLELINE => Some("EM_GETFIRSTVISIBLELINE"),
        // wam::EM_SETREADONLY => Some("EM_SETREADONLY"),
        // wam::EM_SETWORDBREAKPROC => Some("EM_SETWORDBREAKPROC"),
        // wam::EM_GETWORDBREAKPROC => Some("EM_GETWORDBREAKPROC"),
        // wam::EM_GETPASSWORDCHAR => Some("EM_GETPASSWORDCHAR"),
        // wam::EM_SETMARGINS => Some("EM_SETMARGINS"),
        // wam::EM_GETMARGINS => Some("EM_GETMARGINS"),
        // wam::EM_GETLIMITTEXT => Some("EM_GETLIMITTEXT"),
        // wam::EM_POSFROMCHAR => Some("EM_POSFROMCHAR"),
        // wam::EM_CHARFROMPOS => Some("EM_CHARFROMPOS"),
        // wam::EM_SETIMESTATUS => Some("EM_SETIMESTATUS"),
        // wam::EM_GETIMESTATUS => Some("EM_GETIMESTATUS"),
        wam::SBM_SETPOS => Some("SBM_SETPOS"),
        wam::SBM_GETPOS => Some("SBM_GETPOS"),
        wam::SBM_SETRANGE => Some("SBM_SETRANGE"),
        wam::SBM_GETRANGE => Some("SBM_GETRANGE"),
        wam::SBM_ENABLE_ARROWS => Some("SBM_ENABLE_ARROWS"),
        wam::SBM_SETRANGEREDRAW => Some("SBM_SETRANGEREDRAW"),
        wam::SBM_SETSCROLLINFO => Some("SBM_SETSCROLLINFO"),
        wam::SBM_GETSCROLLINFO => Some("SBM_GETSCROLLINFO"),
        wam::SBM_GETSCROLLBARINFO => Some("SBM_GETSCROLLBARINFO"),
        wam::BM_GETCHECK => Some("BM_GETCHECK"),
        wam::BM_SETCHECK => Some("BM_SETCHECK"),
        wam::BM_GETSTATE => Some("BM_GETSTATE"),
        wam::BM_SETSTATE => Some("BM_SETSTATE"),
        wam::BM_SETSTYLE => Some("BM_SETSTYLE"),
        wam::BM_CLICK => Some("BM_CLICK"),
        wam::BM_GETIMAGE => Some("BM_GETIMAGE"),
        wam::BM_SETIMAGE => Some("BM_SETIMAGE"),
        wam::BM_SETDONTCLICK => Some("BM_SETDONTCLICK"),
        wam::WM_INPUT => Some("WM_INPUT"),
        wam::WM_KEYDOWN => Some("WM_KEYDOWN"),
        wam::WM_KEYUP => Some("WM_KEYUP"),
        wam::WM_CHAR => Some("WM_CHAR"),
        wam::WM_DEADCHAR => Some("WM_DEADCHAR"),
        wam::WM_SYSKEYDOWN => Some("WM_SYSKEYDOWN"),
        wam::WM_SYSKEYUP => Some("WM_SYSKEYUP"),
        wam::WM_SYSCHAR => Some("WM_SYSCHAR"),
        wam::WM_SYSDEADCHAR => Some("WM_SYSDEADCHAR"),
        wam::WM_UNICHAR => Some("WM_UNICHAR"),
        // wam::WM_WNT_CONVERTREQUESTEX => Some("WM_WNT_CONVERTREQUESTEX"),
        // wam::WM_CONVERTREQUEST => Some("WM_CONVERTREQUEST"),
        // wam::WM_CONVERTRESULT => Some("WM_CONVERTRESULT"),
        // wam::WM_INTERIM => Some("WM_INTERIM"),
        wam::WM_IME_STARTCOMPOSITION => Some("WM_IME_STARTCOMPOSITION"),
        wam::WM_IME_ENDCOMPOSITION => Some("WM_IME_ENDCOMPOSITION"),
        wam::WM_IME_COMPOSITION => Some("WM_IME_COMPOSITION"),
        wam::WM_INITDIALOG => Some("WM_INITDIALOG"),
        wam::WM_COMMAND => Some("WM_COMMAND"),
        wam::WM_SYSCOMMAND => Some("WM_SYSCOMMAND"),
        wam::WM_TIMER => Some("WM_TIMER"),
        wam::WM_HSCROLL => Some("WM_HSCROLL"),
        wam::WM_VSCROLL => Some("WM_VSCROLL"),
        wam::WM_INITMENU => Some("WM_INITMENU"),
        wam::WM_INITMENUPOPUP => Some("WM_INITMENUPOPUP"),
        // wam::WM_SYSTIMER => Some("WM_SYSTIMER"),
        wam::WM_MENUSELECT => Some("WM_MENUSELECT"),
        wam::WM_MENUCHAR => Some("WM_MENUCHAR"),
        wam::WM_ENTERIDLE => Some("WM_ENTERIDLE"),
        wam::WM_MENURBUTTONUP => Some("WM_MENURBUTTONUP"),
        wam::WM_MENUDRAG => Some("WM_MENUDRAG"),
        wam::WM_MENUGETOBJECT => Some("WM_MENUGETOBJECT"),
        wam::WM_UNINITMENUPOPUP => Some("WM_UNINITMENUPOPUP"),
        wam::WM_MENUCOMMAND => Some("WM_MENUCOMMAND"),
        wam::WM_CHANGEUISTATE => Some("WM_CHANGEUISTATE"),
        wam::WM_UPDATEUISTATE => Some("WM_UPDATEUISTATE"),
        wam::WM_QUERYUISTATE => Some("WM_QUERYUISTATE"),
        wam::WM_CTLCOLORMSGBOX => Some("WM_CTLCOLORMSGBOX"),
        wam::WM_CTLCOLOREDIT => Some("WM_CTLCOLOREDIT"),
        wam::WM_CTLCOLORLISTBOX => Some("WM_CTLCOLORLISTBOX"),
        wam::WM_CTLCOLORBTN => Some("WM_CTLCOLORBTN"),
        wam::WM_CTLCOLORDLG => Some("WM_CTLCOLORDLG"),
        wam::WM_CTLCOLORSCROLLBAR => Some("WM_CTLCOLORSCROLLBAR"),
        wam::WM_CTLCOLORSTATIC => Some("WM_CTLCOLORSTATIC"),
        wam::WM_MOUSEMOVE => Some("WM_MOUSEMOVE"),
        wam::WM_LBUTTONDOWN => Some("WM_LBUTTONDOWN"),
        wam::WM_LBUTTONUP => Some("WM_LBUTTONUP"),
        wam::WM_LBUTTONDBLCLK => Some("WM_LBUTTONDBLCLK"),
        wam::WM_RBUTTONDOWN => Some("WM_RBUTTONDOWN"),
        wam::WM_RBUTTONUP => Some("WM_RBUTTONUP"),
        wam::WM_RBUTTONDBLCLK => Some("WM_RBUTTONDBLCLK"),
        wam::WM_MBUTTONDOWN => Some("WM_MBUTTONDOWN"),
        wam::WM_MBUTTONUP => Some("WM_MBUTTONUP"),
        wam::WM_MBUTTONDBLCLK => Some("WM_MBUTTONDBLCLK"),
        wam::WM_MOUSEWHEEL => Some("WM_MOUSEWHEEL"),
        wam::WM_XBUTTONDOWN => Some("WM_XBUTTONDOWN"),
        wam::WM_XBUTTONUP => Some("WM_XBUTTONUP"),
        wam::WM_XBUTTONDBLCLK => Some("WM_XBUTTONDBLCLK"),
        wam::WM_MOUSEHWHEEL => Some("WM_MOUSEHWHEEL"),
        wam::WM_PARENTNOTIFY => Some("WM_PARENTNOTIFY"),
        wam::WM_ENTERMENULOOP => Some("WM_ENTERMENULOOP"),
        wam::WM_EXITMENULOOP => Some("WM_EXITMENULOOP"),
        wam::WM_NEXTMENU => Some("WM_NEXTMENU"),
        wam::WM_SIZING => Some("WM_SIZING"),
        wam::WM_CAPTURECHANGED => Some("WM_CAPTURECHANGED"),
        wam::WM_MOVING => Some("WM_MOVING"),
        wam::WM_POWERBROADCAST => Some("WM_POWERBROADCAST"),
        wam::WM_DEVICECHANGE => Some("WM_DEVICECHANGE"),
        wam::WM_MDICREATE => Some("WM_MDICREATE"),
        wam::WM_MDIDESTROY => Some("WM_MDIDESTROY"),
        wam::WM_MDIACTIVATE => Some("WM_MDIACTIVATE"),
        wam::WM_MDIRESTORE => Some("WM_MDIRESTORE"),
        wam::WM_MDINEXT => Some("WM_MDINEXT"),
        wam::WM_MDIMAXIMIZE => Some("WM_MDIMAXIMIZE"),
        wam::WM_MDITILE => Some("WM_MDITILE"),
        wam::WM_MDICASCADE => Some("WM_MDICASCADE"),
        wam::WM_MDIICONARRANGE => Some("WM_MDIICONARRANGE"),
        wam::WM_MDIGETACTIVE => Some("WM_MDIGETACTIVE"),
        wam::WM_MDISETMENU => Some("WM_MDISETMENU"),
        wam::WM_ENTERSIZEMOVE => Some("WM_ENTERSIZEMOVE"),
        wam::WM_EXITSIZEMOVE => Some("WM_EXITSIZEMOVE"),
        wam::WM_DROPFILES => Some("WM_DROPFILES"),
        wam::WM_MDIREFRESHMENU => Some("WM_MDIREFRESHMENU"),
        // wam::WM_IME_REPORT => Some("WM_IME_REPORT"),
        wam::WM_IME_SETCONTEXT => Some("WM_IME_SETCONTEXT"),
        wam::WM_IME_NOTIFY => Some("WM_IME_NOTIFY"),
        wam::WM_IME_CONTROL => Some("WM_IME_CONTROL"),
        wam::WM_IME_COMPOSITIONFULL => Some("WM_IME_COMPOSITIONFULL"),
        wam::WM_IME_SELECT => Some("WM_IME_SELECT"),
        wam::WM_IME_CHAR => Some("WM_IME_CHAR"),
        wam::WM_IME_REQUEST => Some("WM_IME_REQUEST"),
        // wam::WM_IMEKEYDOWN => Some("WM_IMEKEYDOWN"),
        wam::WM_IME_KEYDOWN => Some("WM_IME_KEYDOWN"),
        // wam::WM_IMEKEYUP => Some("WM_IMEKEYUP"),
        wam::WM_IME_KEYUP => Some("WM_IME_KEYUP"),
        wam::WM_NCMOUSEHOVER => Some("WM_NCMOUSEHOVER"),
        // FIXME: Missing definition in windows-sys
        // wam::WM_MOUSEHOVER => Some("WM_MOUSEHOVER"),
        0x02a1 => Some("WM_MOUSEHOVER"),
        wam::WM_NCMOUSELEAVE => Some("WM_NCMOUSELEAVE"),
        // FIXME: Missing definition in windows-sys
        // wam::WM_MOUSELEAVE => Some("WM_MOUSELEAVE"),
        0x02a3 => Some("WM_MOUSELEAVE"),
        wam::WM_CUT => Some("WM_CUT"),
        wam::WM_COPY => Some("WM_COPY"),
        wam::WM_PASTE => Some("WM_PASTE"),
        wam::WM_CLEAR => Some("WM_CLEAR"),
        wam::WM_UNDO => Some("WM_UNDO"),
        wam::WM_RENDERFORMAT => Some("WM_RENDERFORMAT"),
        wam::WM_RENDERALLFORMATS => Some("WM_RENDERALLFORMATS"),
        wam::WM_DESTROYCLIPBOARD => Some("WM_DESTROYCLIPBOARD"),
        wam::WM_DRAWCLIPBOARD => Some("WM_DRAWCLIPBOARD"),
        wam::WM_PAINTCLIPBOARD => Some("WM_PAINTCLIPBOARD"),
        wam::WM_VSCROLLCLIPBOARD => Some("WM_VSCROLLCLIPBOARD"),
        wam::WM_SIZECLIPBOARD => Some("WM_SIZECLIPBOARD"),
        wam::WM_ASKCBFORMATNAME => Some("WM_ASKCBFORMATNAME"),
        wam::WM_CHANGECBCHAIN => Some("WM_CHANGECBCHAIN"),
        wam::WM_HSCROLLCLIPBOARD => Some("WM_HSCROLLCLIPBOARD"),
        wam::WM_QUERYNEWPALETTE => Some("WM_QUERYNEWPALETTE"),
        wam::WM_PALETTEISCHANGING => Some("WM_PALETTEISCHANGING"),
        wam::WM_PALETTECHANGED => Some("WM_PALETTECHANGED"),
        wam::WM_HOTKEY => Some("WM_HOTKEY"),
        wam::WM_PRINT => Some("WM_PRINT"),
        wam::WM_PRINTCLIENT => Some("WM_PRINTCLIENT"),
        wam::WM_APPCOMMAND => Some("WM_APPCOMMAND"),
        wam::WM_HANDHELDFIRST => Some("WM_HANDHELDFIRST"),
        wam::WM_HANDHELDLAST => Some("WM_HANDHELDLAST"),
        wam::WM_AFXFIRST => Some("WM_AFXFIRST"),
        wam::WM_AFXLAST => Some("WM_AFXLAST"),
        wam::WM_PENWINFIRST => Some("WM_PENWINFIRST"),
        // wam::WM_RCRESULT => Some("WM_RCRESULT"),
        // wam::WM_HOOKRCRESULT => Some("WM_HOOKRCRESULT"),
        // wam::WM_GLOBALRCCHANGE => Some("WM_GLOBALRCCHANGE"),
        // wam::WM_PENMISCINFO => Some("WM_PENMISCINFO"),
        // wam::WM_SKB => Some("WM_SKB"),
        // wam::WM_HEDITCTL => Some("WM_HEDITCTL"),
        // wam::WM_PENCTL => Some("WM_PENCTL"),
        // wam::WM_PENMISC => Some("WM_PENMISC"),
        // wam::WM_CTLINIT => Some("WM_CTLINIT"),
        // wam::WM_PENEVENT => Some("WM_PENEVENT"),
        wam::WM_PENWINLAST => Some("WM_PENWINLAST"),
        wam::WM_APP => Some("WM_APP"),
        // wam::WM_RASDIALEVENT => Some("WM_RASDIALEVENT"),
        _ => None,
    };

    if let Some(msg_str) = msg_str {
        msg_str.to_string()
    } else {
        format!("{:#X}", msg)
    }
}

pub fn encode_wide(string: impl AsRef<OsStr>) -> Vec<u16> {
    string.as_ref().encode_wide().chain(once(0)).collect()
}

pub fn decode_wide(mut wide_c_string: &[u16]) -> OsString {
    if let Some(null_pos) = wide_c_string.iter().position(|c| *c == 0) {
        wide_c_string = &wide_c_string[..null_pos];
    }

    OsString::from_wide(wide_c_string)
}

pub fn has_flag<T>(bitset: T, flag: T) -> bool
where
    T: Copy + PartialEq + BitAnd<T, Output = T>,
{
    bitset & flag == flag
}

pub(crate) fn win_to_err(result: BOOL) -> Result<(), io::Error> {
    if result != false.into() {
        Ok(())
    } else {
        Err(io::Error::last_os_error())
    }
}

pub enum WindowArea {
    Outer,
    Inner,
}

impl WindowArea {
    pub fn get_rect(self, hwnd: HWND) -> Result<RECT, io::Error> {
        let mut rect = unsafe { mem::zeroed() };

        match self {
            WindowArea::Outer => {
                win_to_err(unsafe { GetWindowRect(hwnd, &mut rect) })?;
            }
            WindowArea::Inner => unsafe {
                let mut top_left = mem::zeroed();

                win_to_err(ClientToScreen(hwnd, &mut top_left))?;
                win_to_err(GetClientRect(hwnd, &mut rect))?;
                rect.left += top_left.x;
                rect.top += top_left.y;
                rect.right += top_left.x;
                rect.bottom += top_left.y;
            },
        }

        Ok(rect)
    }
}

pub fn set_cursor_hidden(hidden: bool) {
    static HIDDEN: AtomicBool = AtomicBool::new(false);
    let changed = HIDDEN.swap(hidden, Ordering::SeqCst) ^ hidden;
    if changed {
        unsafe { ShowCursor(BOOL::from(!hidden)) };
    }
}

pub fn get_cursor_clip() -> Result<RECT, io::Error> {
    unsafe {
        let mut rect: RECT = mem::zeroed();
        win_to_err(GetClipCursor(&mut rect)).map(|_| rect)
    }
}

/// Sets the cursor's clip rect.
///
/// Note that calling this will automatically dispatch a `WM_MOUSEMOVE` event.
pub fn set_cursor_clip(rect: Option<RECT>) -> Result<(), io::Error> {
    unsafe {
        let rect_ptr = rect
            .as_ref()
            .map(|r| r as *const RECT)
            .unwrap_or(ptr::null());
        win_to_err(ClipCursor(rect_ptr))
    }
}

pub fn get_desktop_rect() -> RECT {
    unsafe {
        let left = GetSystemMetrics(SM_XVIRTUALSCREEN);
        let top = GetSystemMetrics(SM_YVIRTUALSCREEN);
        RECT {
            left,
            top,
            right: left + GetSystemMetrics(SM_CXVIRTUALSCREEN),
            bottom: top + GetSystemMetrics(SM_CYVIRTUALSCREEN),
        }
    }
}

pub fn is_focused(window: HWND) -> bool {
    window == unsafe { GetActiveWindow() }
}

pub fn get_instance_handle() -> HINSTANCE {
    // Gets the instance handle by taking the address of the
    // pseudo-variable created by the microsoft linker:
    // https://devblogs.microsoft.com/oldnewthing/20041025-00/?p=37483

    // This is preferred over GetModuleHandle(NULL) because it also works in DLLs:
    // https://stackoverflow.com/questions/21718027/getmodulehandlenull-vs-hinstance

    extern "C" {
        static __ImageBase: IMAGE_DOS_HEADER;
    }

    unsafe { &__ImageBase as *const _ as _ }
}

impl CursorIcon {
    pub(crate) fn to_windows_cursor(self) -> PCWSTR {
        match self {
            CursorIcon::Arrow | CursorIcon::Default => IDC_ARROW,
            CursorIcon::Hand => IDC_HAND,
            CursorIcon::Crosshair => IDC_CROSS,
            CursorIcon::Text | CursorIcon::VerticalText => IDC_IBEAM,
            CursorIcon::NotAllowed | CursorIcon::NoDrop => IDC_NO,
            CursorIcon::Grab | CursorIcon::Grabbing | CursorIcon::Move | CursorIcon::AllScroll => {
                IDC_SIZEALL
            }
            CursorIcon::EResize
            | CursorIcon::WResize
            | CursorIcon::EwResize
            | CursorIcon::ColResize => IDC_SIZEWE,
            CursorIcon::NResize
            | CursorIcon::SResize
            | CursorIcon::NsResize
            | CursorIcon::RowResize => IDC_SIZENS,
            CursorIcon::NeResize | CursorIcon::SwResize | CursorIcon::NeswResize => IDC_SIZENESW,
            CursorIcon::NwResize | CursorIcon::SeResize | CursorIcon::NwseResize => IDC_SIZENWSE,
            CursorIcon::Wait => IDC_WAIT,
            CursorIcon::Progress => IDC_APPSTARTING,
            CursorIcon::Help => IDC_HELP,
            _ => IDC_ARROW, // use arrow for the missing cases.
        }
    }
}

// Helper function to dynamically load function pointer.
// `library` and `function` must be zero-terminated.
pub(super) fn get_function_impl(library: &str, function: &str) -> Option<*const c_void> {
    assert_eq!(library.chars().last(), Some('\0'));
    assert_eq!(function.chars().last(), Some('\0'));

    // Library names we will use are ASCII so we can use the A version to avoid string conversion.
    let module = unsafe { LoadLibraryA(library.as_ptr()) };
    if module == 0 {
        return None;
    }

    unsafe { GetProcAddress(module, function.as_ptr()) }.map(|function_ptr| function_ptr as _)
}

macro_rules! get_function {
    ($lib:expr, $func:ident) => {
        crate::platform_impl::platform::util::get_function_impl(
            concat!($lib, '\0'),
            concat!(stringify!($func), '\0'),
        )
        .map(|f| unsafe { std::mem::transmute::<*const _, $func>(f) })
    };
}

pub type SetProcessDPIAware = unsafe extern "system" fn() -> BOOL;
pub type SetProcessDpiAwareness =
    unsafe extern "system" fn(value: PROCESS_DPI_AWARENESS) -> HRESULT;
pub type SetProcessDpiAwarenessContext =
    unsafe extern "system" fn(value: DPI_AWARENESS_CONTEXT) -> BOOL;
pub type GetDpiForWindow = unsafe extern "system" fn(hwnd: HWND) -> u32;
pub type GetDpiForMonitor = unsafe extern "system" fn(
    hmonitor: HMONITOR,
    dpi_type: MONITOR_DPI_TYPE,
    dpi_x: *mut u32,
    dpi_y: *mut u32,
) -> HRESULT;
pub type EnableNonClientDpiScaling = unsafe extern "system" fn(hwnd: HWND) -> BOOL;
pub type AdjustWindowRectExForDpi = unsafe extern "system" fn(
    rect: *mut RECT,
    dwStyle: u32,
    bMenu: BOOL,
    dwExStyle: u32,
    dpi: u32,
) -> BOOL;

pub static GET_DPI_FOR_WINDOW: Lazy<Option<GetDpiForWindow>> =
    Lazy::new(|| get_function!("user32.dll", GetDpiForWindow));
pub static ADJUST_WINDOW_RECT_EX_FOR_DPI: Lazy<Option<AdjustWindowRectExForDpi>> =
    Lazy::new(|| get_function!("user32.dll", AdjustWindowRectExForDpi));
pub static GET_DPI_FOR_MONITOR: Lazy<Option<GetDpiForMonitor>> =
    Lazy::new(|| get_function!("shcore.dll", GetDpiForMonitor));
pub static ENABLE_NON_CLIENT_DPI_SCALING: Lazy<Option<EnableNonClientDpiScaling>> =
    Lazy::new(|| get_function!("user32.dll", EnableNonClientDpiScaling));
pub static SET_PROCESS_DPI_AWARENESS_CONTEXT: Lazy<Option<SetProcessDpiAwarenessContext>> =
    Lazy::new(|| get_function!("user32.dll", SetProcessDpiAwarenessContext));
pub static SET_PROCESS_DPI_AWARENESS: Lazy<Option<SetProcessDpiAwareness>> =
    Lazy::new(|| get_function!("shcore.dll", SetProcessDpiAwareness));
pub static SET_PROCESS_DPI_AWARE: Lazy<Option<SetProcessDPIAware>> =
    Lazy::new(|| get_function!("user32.dll", SetProcessDPIAware));
