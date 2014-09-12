#![allow(dead_code)]
#![allow(non_snake_case_functions)]
#![allow(non_camel_case_types)]
#![allow(uppercase_variables)]

use libc;

/// WGL bindings
pub mod wgl {
    generate_gl_bindings!("wgl", "core", "1.0", "static")
}

/// Functions that are not necessarly always available
pub mod wgl_extra {
    generate_gl_bindings!("wgl", "core", "1.0", "struct", [ "WGL_ARB_create_context" ])
}

// http://msdn.microsoft.com/en-us/library/windows/desktop/aa383751(v=vs.85).aspx
// we don't define the T types to ensure that A/W functions are used
pub type ATOM = WORD;
pub type BOOL = libc::c_int;
pub type BOOLEAN = BYTE;
pub type BYTE = libc::c_uchar;
pub type DWORD = libc::c_ulong;
pub type HANDLE = PVOID;
pub type HBRUSH = HANDLE;
pub type HCURSOR = HICON;
pub type HDC = HANDLE;
pub type HICON = HANDLE;
pub type HINSTANCE = HANDLE;
pub type HLOCAL = HANDLE;
pub type HMENU = HANDLE;
pub type HMODULE = HINSTANCE;
pub type HWND = HANDLE;
pub type LONG = libc::c_long;
pub type LONG_PTR = int;
pub type LPARAM = LONG_PTR;
pub type LPCSTR = *const libc::c_char;
pub type LPCWSTR = *const WCHAR;
pub type LPCVOID = *const libc::c_void;
pub type LPSTR = *mut libc::c_char;
pub type LPVOID = *mut libc::c_void;
pub type LPWSTR = *mut WCHAR;
pub type LRESULT = LONG_PTR;
pub type PVOID = *const libc::c_void;
pub type UINT = libc::c_uint;
pub type UINT_PTR = int;
pub type WCHAR = libc::wchar_t;
pub type WORD = libc::c_ushort;
pub type WPARAM = UINT_PTR;

// macros
pub fn LOWORD(l: DWORD) -> WORD {
    (l & 0xFFFF) as WORD
}

pub fn HIWORD(l: DWORD) -> WORD {
    (l >> 16) as WORD
}

pub fn GET_X_LPARAM(lp: LONG_PTR) -> libc::c_int {
    LOWORD(lp as DWORD) as libc::c_int
}

pub fn GET_Y_LPARAM(lp: LONG_PTR) -> libc::c_int {
    HIWORD(lp as DWORD) as libc::c_int
}

// http://msdn.microsoft.com/en-us/library/windows/desktop/ff485887(v=vs.85).aspx
pub static BN_CLICKED: WORD = 0;
pub static BN_DBLCLK: WORD = 5;
pub static BN_DISABLE: WORD = 4;
pub static BN_DOUBLECLICKED: WORD = 5;
pub static BN_HILITE: WORD = 2;
pub static BN_KILLFOCUS: WORD = 7;
pub static BN_PAINT: WORD = 1;
pub static BN_PUSHED: WORD = 2;
pub static BN_SETFOCUS: WORD = 6;
pub static BN_UNHILITE: WORD = 3;
pub static BN_UNPUSHED: WORD = 3;

// ?
pub static BS_3STATE: DWORD = 5;
pub static BS_AUTO3STATE: DWORD = 6;
pub static BS_AUTOCHECKBOX: DWORD = 3;
pub static BS_AUTORADIOBUTTON: DWORD =  9;
pub static BS_BITMAP: DWORD = 128;
pub static BS_BOTTOM: DWORD = 0x800;
pub static BS_CENTER: DWORD = 0x300;
pub static BS_CHECKBOX: DWORD = 2;
pub static BS_DEFPUSHBUTTON: DWORD = 1;
pub static BS_GROUPBOX: DWORD = 7;
pub static BS_ICON: DWORD = 64;
pub static BS_LEFT: DWORD = 256;
pub static BS_LEFTTEXT: DWORD = 32;
pub static BS_MULTILINE: DWORD = 0x2000;
pub static BS_NOTIFY: DWORD = 0x4000;
pub static BS_OWNERDRAW: DWORD = 0xb;
pub static BS_PUSHBUTTON: DWORD = 0;
pub static BS_PUSHLIKE: DWORD = 4096;
pub static BS_RADIOBUTTON: DWORD = 4;
pub static BS_RIGHT: DWORD = 512;
pub static BS_RIGHTBUTTON: DWORD = 32;
pub static BS_TEXT: DWORD = 0;
pub static BS_TOP: DWORD = 0x400;
pub static BS_USERBUTTON: DWORD = 8;
pub static BS_VCENTER: DWORD =  0xc00;
pub static BS_FLAT: DWORD = 0x8000;

// ?
pub static CDS_UPDATEREGISTRY: DWORD = 0x1;
pub static CDS_TEST: DWORD = 0x2;
pub static CDS_FULLSCREEN: DWORD = 0x4;
pub static CDS_GLOBAL: DWORD = 0x8;
pub static CDS_SET_PRIMARY: DWORD = 0x10;
pub static CDS_VIDEOPARAMETERS: DWORD = 0x20;
pub static CDS_NORESET: DWORD = 0x10000000;
pub static CDS_SETRECT: DWORD = 0x20000000;
pub static CDS_RESET: DWORD = 0x40000000;

// http://msdn.microsoft.com/en-us/library/windows/desktop/ff729176(v=vs.85).aspx
pub static CS_BYTEALIGNCLIENT: DWORD = 0x1000;
pub static CS_BYTEALIGNWINDOW: DWORD = 0x2000;
pub static CS_CLASSDC: DWORD = 0x0040;
pub static CS_DBLCLKS: DWORD = 0x0008;
pub static CS_DROPSHADOW: DWORD = 0x00020000;
pub static CS_GLOBALCLASS: DWORD = 0x4000;
pub static CS_HREDRAW: DWORD = 0x0002;
pub static CS_NOCLOSE: DWORD = 0x0200;
pub static CS_OWNDC: DWORD = 0x0020;
pub static CS_PARENTDC: DWORD = 0x0080;
pub static CS_SAVEBITS: DWORD = 0x0800;
pub static CS_VREDRAW: DWORD = 0x0001;

// ?
#[allow(type_overflow)]
pub static CW_USEDEFAULT: libc::c_int = 0x80000000;

// ?
pub static DISP_CHANGE_SUCCESSFUL: LONG = 0;
pub static DISP_CHANGE_RESTART: LONG = 1;
pub static DISP_CHANGE_FAILED: LONG = -1;
pub static DISP_CHANGE_BADMODE: LONG = -2;
pub static DISP_CHANGE_NOTUPDATED: LONG = -3;
pub static DISP_CHANGE_BADFLAGS: LONG = -4;
pub static DISP_CHANGE_BADPARAM: LONG = -5;
pub static DISP_CHANGE_BADDUALVIEW: LONG = -6;

// ?
pub static DISPLAY_DEVICE_ACTIVE: DWORD = 0x00000001;
pub static DISPLAY_DEVICE_MULTI_DRIVER: DWORD = 0x00000002;
pub static DISPLAY_DEVICE_PRIMARY_DEVICE: DWORD = 0x00000004;
pub static DISPLAY_DEVICE_MIRRORING_DRIVER: DWORD = 0x00000008;
pub static DISPLAY_DEVICE_VGA_COMPATIBLE: DWORD = 0x00000010;

// ?
pub static DM_ORIENTATION: DWORD = 0x00000001;
pub static DM_PAPERSIZE: DWORD = 0x00000002;
pub static DM_PAPERLENGTH: DWORD = 0x00000004;
pub static DM_PAPERWIDTH: DWORD = 0x00000008;
pub static DM_SCALE: DWORD = 0x00000010;
pub static DM_POSITION: DWORD = 0x00000020;
pub static DM_NUP: DWORD = 0x00000040;
pub static DM_DISPLAYORIENTATION: DWORD = 0x00000080;
pub static DM_COPIES: DWORD = 0x00000100;
pub static DM_DEFAULTSOURCE: DWORD = 0x00000200;
pub static DM_PRINTQUALITY: DWORD = 0x00000400;
pub static DM_COLOR: DWORD = 0x00000800;
pub static DM_DUPLEX: DWORD = 0x00001000;
pub static DM_YRESOLUTION: DWORD = 0x00002000;
pub static DM_TTOPTION: DWORD = 0x00004000;
pub static DM_COLLATE: DWORD = 0x00008000;
pub static DM_FORMNAME: DWORD = 0x00010000;
pub static DM_LOGPIXELS: DWORD = 0x00020000;
pub static DM_BITSPERPEL: DWORD = 0x00040000;
pub static DM_PELSWIDTH: DWORD = 0x00080000;
pub static DM_PELSHEIGHT: DWORD = 0x00100000;
pub static DM_DISPLAYFLAGS: DWORD = 0x00200000;
pub static DM_DISPLAYFREQUENCY: DWORD = 0x00400000;
pub static DM_ICMMETHOD: DWORD = 0x00800000;
pub static DM_ICMINTENT: DWORD = 0x01000000;
pub static DM_MEDIATYPE: DWORD = 0x02000000;
pub static DM_DITHERTYPE: DWORD = 0x04000000;
pub static DM_PANNINGWIDTH: DWORD = 0x08000000;
pub static DM_PANNINGHEIGHT: DWORD = 0x10000000;
pub static DM_DISPLAYFIXEDOUTPUT: DWORD = 0x20000000;

// http://msdn.microsoft.com/en-us/library/windows/desktop/dd162609(v=vs.85).aspx
pub static EDD_GET_DEVICE_INTERFACE_NAME: DWORD = 0x00000001;

// ?
pub static ENUM_CURRENT_SETTINGS: DWORD = -1;
pub static ENUM_REGISTRY_SETTINGS: DWORD = -2;

// http://msdn.microsoft.com/en-us/library/windows/desktop/ms679351(v=vs.85).aspx
pub static FORMAT_MESSAGE_ALLOCATE_BUFFER: DWORD = 0x00000100;
pub static FORMAT_MESSAGE_ARGUMENT_ARRAY: DWORD = 0x00002000;
pub static FORMAT_MESSAGE_FROM_HMODULE: DWORD = 0x00000800;
pub static FORMAT_MESSAGE_FROM_STRING: DWORD = 0x00000400;
pub static FORMAT_MESSAGE_FROM_SYSTEM: DWORD = 0x00001000;
pub static FORMAT_MESSAGE_IGNORE_INSERTS: DWORD = 0x00000200;

// ?
pub static PFD_TYPE_RGBA: BYTE = 0;
pub static PFD_TYPE_COLORINDEX: BYTE = 1;
pub static PFD_MAIN_PLANE: BYTE = 0;
pub static PFD_OVERLAY_PLANE: BYTE = 1;
pub static PFD_UNDERLAY_PLANE: BYTE = (-1);
pub static PFD_DOUBLEBUFFER: DWORD = 0x00000001;
pub static PFD_STEREO: DWORD = 0x00000002;
pub static PFD_DRAW_TO_WINDOW: DWORD = 0x00000004;
pub static PFD_DRAW_TO_BITMAP: DWORD = 0x00000008;
pub static PFD_SUPPORT_GDI: DWORD = 0x00000010;
pub static PFD_SUPPORT_OPENGL: DWORD = 0x00000020;
pub static PFD_GENERIC_FORMAT: DWORD = 0x00000040;
pub static PFD_NEED_PALETTE: DWORD = 0x00000080;
pub static PFD_NEED_SYSTEM_PALETTE: DWORD = 0x00000100;
pub static PFD_SWAP_EXCHANGE: DWORD = 0x00000200;
pub static PFD_SWAP_COPY: DWORD = 0x00000400;
pub static PFD_SWAP_LAYER_BUFFERS: DWORD = 0x00000800;
pub static PFD_GENERIC_ACCELERATED: DWORD = 0x00001000;
pub static PFD_SUPPORT_COMPOSITION: DWORD = 0x00008000;
pub static PFD_DEPTH_DONTCARE: DWORD = 0x20000000;
pub static PFD_DOUBLEBUFFER_DONTCARE: DWORD = 0x40000000;
pub static PFD_STEREO_DONTCARE: DWORD = 0x80000000;

// http://msdn.microsoft.com/en-us/library/windows/desktop/ms633548(v=vs.85).aspx
pub static SW_FORCEMINIMIZE: libc::c_int = 11;
pub static SW_HIDE: libc::c_int = 0;
pub static SW_MAXIMIZE: libc::c_int = 3;
pub static SW_MINIMIZE: libc::c_int = 6;
pub static SW_RESTORE: libc::c_int = 9;
pub static SW_SHOW: libc::c_int = 5;
pub static SW_SHOWDEFAULT: libc::c_int = 10;
pub static SW_SHOWMAXIMIZED: libc::c_int = 3;
pub static SW_SHOWMINIMIZED: libc::c_int = 2;
pub static SW_SHOWMINNOACTIVE: libc::c_int = 7;
pub static SW_SHOWNA: libc::c_int = 8;
pub static SW_SHOWNOACTIVATE: libc::c_int = 4;
pub static SW_SHOWNORMAL: libc::c_int = 1;

// http://msdn.microsoft.com/en-us/library/windows/desktop/ms633545(v=vs.85).aspx
pub static SWP_ASYNCWINDOWPOS: UINT = 0x4000;
pub static SWP_DEFERERASE: UINT = 0x2000;
pub static SWP_DRAWFRAME: UINT = 0x0020;
pub static SWP_FRAMECHANGED: UINT = 0x0020;
pub static SWP_HIDEWINDOW: UINT = 0x0080;
pub static SWP_NOACTIVATE: UINT = 0x0010;
pub static SWP_NOCOPYBITS: UINT = 0x0100;
pub static SWP_NOMOVE: UINT = 0x0002;
pub static SWP_NOOWNERZORDER: UINT = 0x0200;
pub static SWP_NOREDRAW: UINT = 0x0008;
pub static SWP_NOREPOSITION: UINT = 0x0200;
pub static SWP_NOSENDCHANGING: UINT = 0x0400;
pub static SWP_NOSIZE: UINT = 0x0001;
pub static SWP_NOZORDER: UINT = 0x0004;
pub static SWP_SHOWWINDOW: UINT = 0x0040;

// http://msdn.microsoft.com/en-us/library/windows/desktop/dd375731(v=vs.85).aspx
pub static VK_LBUTTON: WPARAM = 0x01;
pub static VK_RBUTTON: WPARAM = 0x02;
pub static VK_CANCEL: WPARAM = 0x03;
pub static VK_MBUTTON: WPARAM = 0x04;
pub static VK_XBUTTON1: WPARAM = 0x05;
pub static VK_XBUTTON2: WPARAM = 0x06;
pub static VK_BACK: WPARAM = 0x08;
pub static VK_TAB: WPARAM = 0x09;
pub static VK_CLEAR: WPARAM = 0x0C;
pub static VK_RETURN: WPARAM = 0x0D;
pub static VK_SHIFT: WPARAM = 0x10;
pub static VK_CONTROL: WPARAM = 0x11;
pub static VK_MENU: WPARAM = 0x12;
pub static VK_PAUSE: WPARAM = 0x13;
pub static VK_CAPITAL: WPARAM = 0x14;
pub static VK_KANA: WPARAM = 0x15;
pub static VK_HANGUEL: WPARAM = 0x15;
pub static VK_HANGUL: WPARAM = 0x15;
pub static VK_JUNJA: WPARAM = 0x17;
pub static VK_FINAL: WPARAM = 0x18;
pub static VK_HANJA: WPARAM = 0x19;
pub static VK_KANJI: WPARAM = 0x19;
pub static VK_ESCAPE: WPARAM = 0x1B;
pub static VK_CONVERT: WPARAM = 0x1C;
pub static VK_NONCONVERT: WPARAM = 0x1D;
pub static VK_ACCEPT: WPARAM = 0x1E;
pub static VK_MODECHANGE: WPARAM = 0x1F;
pub static VK_SPACE: WPARAM = 0x20;
pub static VK_PRIOR: WPARAM = 0x21;
pub static VK_NEXT: WPARAM = 0x22;
pub static VK_END: WPARAM = 0x23;
pub static VK_HOME: WPARAM = 0x24;
pub static VK_LEFT: WPARAM = 0x25;
pub static VK_UP: WPARAM = 0x26;
pub static VK_RIGHT: WPARAM = 0x27;
pub static VK_DOWN: WPARAM = 0x28;
pub static VK_SELECT: WPARAM = 0x29;
pub static VK_PRINT: WPARAM = 0x2A;
pub static VK_EXECUTE: WPARAM = 0x2B;
pub static VK_SNAPSHOT: WPARAM = 0x2C;
pub static VK_INSERT: WPARAM = 0x2D;
pub static VK_DELETE: WPARAM = 0x2E;
pub static VK_HELP: WPARAM = 0x2F;
pub static VK_LWIN: WPARAM = 0x5B;
pub static VK_RWIN: WPARAM = 0x5C;
pub static VK_APPS: WPARAM = 0x5D;
pub static VK_SLEEP: WPARAM = 0x5F;
pub static VK_NUMPAD0: WPARAM = 0x60;
pub static VK_NUMPAD1: WPARAM = 0x61;
pub static VK_NUMPAD2: WPARAM = 0x62;
pub static VK_NUMPAD3: WPARAM = 0x63;
pub static VK_NUMPAD4: WPARAM = 0x64;
pub static VK_NUMPAD5: WPARAM = 0x65;
pub static VK_NUMPAD6: WPARAM = 0x66;
pub static VK_NUMPAD7: WPARAM = 0x67;
pub static VK_NUMPAD8: WPARAM = 0x68;
pub static VK_NUMPAD9: WPARAM = 0x69;
pub static VK_MULTIPLY: WPARAM = 0x6A;
pub static VK_ADD: WPARAM = 0x6B;
pub static VK_SEPARATOR: WPARAM = 0x6C;
pub static VK_SUBTRACT: WPARAM = 0x6D;
pub static VK_DECIMAL: WPARAM = 0x6E;
pub static VK_DIVIDE: WPARAM = 0x6F;
pub static VK_F1: WPARAM = 0x70;
pub static VK_F2: WPARAM = 0x71;
pub static VK_F3: WPARAM = 0x72;
pub static VK_F4: WPARAM = 0x73;
pub static VK_F5: WPARAM = 0x74;
pub static VK_F6: WPARAM = 0x75;
pub static VK_F7: WPARAM = 0x76;
pub static VK_F8: WPARAM = 0x77;
pub static VK_F9: WPARAM = 0x78;
pub static VK_F10: WPARAM = 0x79;
pub static VK_F11: WPARAM = 0x7A;
pub static VK_F12: WPARAM = 0x7B;
pub static VK_F13: WPARAM = 0x7C;
pub static VK_F14: WPARAM = 0x7D;
pub static VK_F15: WPARAM = 0x7E;
pub static VK_F16: WPARAM = 0x7F;
pub static VK_F17: WPARAM = 0x80;
pub static VK_F18: WPARAM = 0x81;
pub static VK_F19: WPARAM = 0x82;
pub static VK_F20: WPARAM = 0x83;
pub static VK_F21: WPARAM = 0x84;
pub static VK_F22: WPARAM = 0x85;
pub static VK_F23: WPARAM = 0x86;
pub static VK_F24: WPARAM = 0x87;
pub static VK_NUMLOCK: WPARAM = 0x90;
pub static VK_SCROLL: WPARAM = 0x91;
pub static VK_LSHIFT: WPARAM = 0xA0;
pub static VK_RSHIFT: WPARAM = 0xA1;
pub static VK_LCONTROL: WPARAM = 0xA2;
pub static VK_RCONTROL: WPARAM = 0xA3;
pub static VK_LMENU: WPARAM = 0xA4;
pub static VK_RMENU: WPARAM = 0xA5;
pub static VK_BROWSER_BACK: WPARAM = 0xA6;
pub static VK_BROWSER_FORWARD: WPARAM = 0xA7;
pub static VK_BROWSER_REFRESH: WPARAM = 0xA8;
pub static VK_BROWSER_STOP: WPARAM = 0xA9;
pub static VK_BROWSER_SEARCH: WPARAM = 0xAA;
pub static VK_BROWSER_FAVORITES: WPARAM = 0xAB;
pub static VK_BROWSER_HOME: WPARAM = 0xAC;
pub static VK_VOLUME_MUTE: WPARAM = 0xAD;
pub static VK_VOLUME_DOWN: WPARAM = 0xAE;
pub static VK_VOLUME_UP: WPARAM = 0xAF;
pub static VK_MEDIA_NEXT_TRACK: WPARAM = 0xB0;
pub static VK_MEDIA_PREV_TRACK: WPARAM = 0xB1;
pub static VK_MEDIA_STOP: WPARAM = 0xB2;
pub static VK_MEDIA_PLAY_PAUSE: WPARAM = 0xB3;
pub static VK_LAUNCH_MAIL: WPARAM = 0xB4;
pub static VK_LAUNCH_MEDIA_SELECT: WPARAM = 0xB5;
pub static VK_LAUNCH_APP1: WPARAM = 0xB6;
pub static VK_LAUNCH_APP2: WPARAM = 0xB7;
pub static VK_OEM_1: WPARAM = 0xBA;
pub static VK_OEM_PLUS: WPARAM = 0xBB;
pub static VK_OEM_COMMA: WPARAM = 0xBC;
pub static VK_OEM_MINUS: WPARAM = 0xBD;
pub static VK_OEM_PERIOD: WPARAM = 0xBE;
pub static VK_OEM_2: WPARAM = 0xBF;
pub static VK_OEM_3: WPARAM = 0xC0;
pub static VK_OEM_4: WPARAM = 0xDB;
pub static VK_OEM_5: WPARAM = 0xDC;
pub static VK_OEM_6: WPARAM = 0xDD;
pub static VK_OEM_7: WPARAM = 0xDE;
pub static VK_OEM_8: WPARAM = 0xDF;
pub static VK_OEM_102: WPARAM = 0xE2;
pub static VK_PROCESSKEY: WPARAM = 0xE5;
pub static VK_PACKET: WPARAM = 0xE7;
pub static VK_ATTN: WPARAM = 0xF6;
pub static VK_CRSEL: WPARAM = 0xF7;
pub static VK_EXSEL: WPARAM = 0xF8;
pub static VK_EREOF: WPARAM = 0xF9;
pub static VK_PLAY: WPARAM = 0xFA;
pub static VK_ZOOM: WPARAM = 0xFB;
pub static VK_NONAME: WPARAM = 0xFC;
pub static VK_PA1: WPARAM = 0xFD;
pub static VK_OEM_CLEAR: WPARAM = 0xFE;

// messages
pub static WM_LBUTTONDOWN: UINT = 0x0201;
pub static WM_LBUTTONUP: UINT = 0x0202;
pub static WM_CHAR: UINT = 0x0102;
pub static WM_COMMAND: UINT = 0x0111;
pub static WM_DESTROY: UINT = 0x0002;
pub static WM_ERASEBKGND: UINT = 0x0014;
pub static WM_KEYDOWN: UINT = 0x0100;
pub static WM_KEYUP: UINT = 0x0101;
pub static WM_KILLFOCUS: UINT = 0x0008;
pub static WM_MBUTTONDOWN: UINT = 0x0207;
pub static WM_MBUTTONUP: UINT = 0x0208;
pub static WM_MOUSEMOVE: UINT = 0x0200;
pub static WM_MOUSEWHEEL: UINT = 0x020A;
pub static WM_MOVE: UINT = 0x0003;
pub static WM_PAINT: UINT = 0x000F;
pub static WM_RBUTTONDOWN: UINT = 0x0204;
pub static WM_RBUTTONUP: UINT = 0x0205;
pub static WM_SETFOCUS: UINT = 0x0007;
pub static WM_SIZE: UINT = 0x0005;
pub static WM_SIZING: UINT = 0x0214;

// http://msdn.microsoft.com/en-us/library/windows/desktop/ms632600(v=vs.85).aspx
pub static WS_BORDER: DWORD = 0x00800000;
pub static WS_CAPTION: DWORD = 0x00C00000;
pub static WS_CHILD: DWORD = 0x40000000;
pub static WS_CHILDWINDOW: DWORD = 0x40000000;
pub static WS_CLIPCHILDREN: DWORD = 0x02000000;
pub static WS_CLIPSIBLINGS: DWORD = 0x04000000;
pub static WS_DISABLED: DWORD = 0x08000000;
pub static WS_DLGFRAME: DWORD = 0x00400000;
pub static WS_GROUP: DWORD = 0x00020000;
pub static WS_HSCROLL: DWORD = 0x00100000;
pub static WS_ICONIC: DWORD = 0x20000000;
pub static WS_MAXIMIZE: DWORD = 0x01000000;
pub static WS_MAXIMIZEBOX: DWORD = 0x00010000;
pub static WS_MINIMIZE: DWORD = 0x20000000;
pub static WS_MINIMIZEBOX: DWORD = 0x00020000;
pub static WS_OVERLAPPED: DWORD = 0x00000000;
pub static WS_OVERLAPPEDWINDOW: DWORD = (WS_OVERLAPPED | WS_CAPTION | WS_SYSMENU | WS_THICKFRAME | WS_MINIMIZEBOX | WS_MAXIMIZEBOX);
pub static WS_POPUP: DWORD = 0x80000000;
pub static WS_POPUPWINDOW: DWORD = (WS_POPUP | WS_BORDER | WS_SYSMENU);
pub static WS_SIZEBOX: DWORD = 0x00040000;
pub static WS_SYSMENU: DWORD = 0x00080000;
pub static WS_TABSTOP: DWORD = 0x00010000;
pub static WS_THICKFRAME: DWORD = 0x00040000;
pub static WS_TILED: DWORD = 0x00000000;
pub static WS_TILEDWINDOW: DWORD = (WS_OVERLAPPED | WS_CAPTION | WS_SYSMENU | WS_THICKFRAME | WS_MINIMIZEBOX | WS_MAXIMIZEBOX);
pub static WS_VISIBLE: DWORD = 0x10000000;
pub static WS_VSCROLL: DWORD = 0x00200000;

// http://msdn.microsoft.com/en-us/library/windows/desktop/ff700543(v=vs.85).aspx
pub static WS_EX_ACCEPTFILES: DWORD = 0x00000010;
pub static WS_EX_APPWINDOW: DWORD = 0x00040000;
pub static WS_EX_CLIENTEDGE: DWORD = 0x00000200;
pub static WS_EX_COMPOSITED: DWORD = 0x02000000;
pub static WS_EX_CONTEXTHELP: DWORD = 0x00000400;
pub static WS_EX_CONTROLPARENT: DWORD = 0x00010000;
pub static WS_EX_DLGMODALFRAME: DWORD = 0x00000001;
pub static WS_EX_LAYERED: DWORD = 0x00080000;
pub static WS_EX_LAYOUTRTL: DWORD = 0x00400000;
pub static WS_EX_LEFT: DWORD = 0x00000000;
pub static WS_EX_LEFTSCROLLBAR: DWORD = 0x00004000;
pub static WS_EX_LTRREADING: DWORD = 0x00000000;
pub static WS_EX_MDICHILD: DWORD = 0x00000040;
pub static WS_EX_NOACTIVATE: DWORD = 0x08000000;
pub static WS_EX_NOINHERITLAYOUT: DWORD = 0x00100000;
pub static WS_EX_NOPARENTNOTIFY: DWORD = 0x00000004;
pub static WS_EX_NOREDIRECTIONBITMAP: DWORD = 0x00200000;
pub static WS_EX_OVERLAPPEDWINDOW: DWORD = (WS_EX_WINDOWEDGE | WS_EX_CLIENTEDGE);
pub static WS_EX_PALETTEWINDOW: DWORD = (WS_EX_WINDOWEDGE | WS_EX_TOOLWINDOW | WS_EX_TOPMOST);
pub static WS_EX_RIGHT: DWORD = 0x00001000;
pub static WS_EX_RIGHTSCROLLBAR: DWORD = 0x00000000;
pub static WS_EX_RTLREADING: DWORD = 0x00002000;
pub static WS_EX_STATICEDGE: DWORD = 0x00020000;
pub static WS_EX_TOOLWINDOW: DWORD = 0x00000080;
pub static WS_EX_TOPMOST: DWORD = 0x00000008;
pub static WS_EX_TRANSPARENT: DWORD = 0x00000020;
pub static WS_EX_WINDOWEDGE: DWORD = 0x00000100;

// http://msdn.microsoft.com/en-us/library/windows/desktop/ms633573(v=vs.85).aspx
pub type WNDPROC = extern "stdcall" fn(HWND, UINT, WPARAM, LPARAM) -> LRESULT;

// ?
pub type HGLRC = HANDLE;

// http://msdn.microsoft.com/en-us/library/windows/desktop/ms633577(v=vs.85).aspx
#[repr(C)]
pub struct WNDCLASSEX {
    pub cbSize: UINT,
    pub style: UINT,
    pub lpfnWndProc: WNDPROC,
    pub cbClsExtra: libc::c_int,
    pub cbWndExtra: libc::c_int,
    pub hInstance: HINSTANCE,
    pub hIcon: HICON,
    pub hCursor: HCURSOR,
    pub hbrBackground: HBRUSH,
    pub lpszMenuName: LPCWSTR,
    pub lpszClassName: LPCWSTR,
    pub hIconSm: HICON,
}

// http://msdn.microsoft.com/en-us/library/windows/desktop/dd162805(v=vs.85).aspxtag
#[repr(C)]
pub struct POINT {
    pub x: LONG,
    pub y: LONG,
}

// http://msdn.microsoft.com/en-us/library/windows/desktop/ms644958(v=vs.85).aspx
#[repr(C)]
pub struct MSG {
    pub hwnd: HWND,
    pub message: UINT,
    pub wParam: WPARAM,
    pub lParam: LPARAM,
    pub time: DWORD,
    pub pt: POINT,
}

// http://msdn.microsoft.com/en-us/library/windows/desktop/dd162768(v=vs.85).aspx
#[repr(C)]
pub struct PAINTSTRUCT {
    pub hdc: HDC,
    pub fErase: BOOL,
    pub rcPaint: RECT,
    pub fRestore: BOOL,
    pub fIncUpdate: BOOL,
    pub rgbReserved: [BYTE, ..32],
}

// http://msdn.microsoft.com/en-us/library/windows/desktop/dd162897(v=vs.85).aspx
#[repr(C)]
pub struct RECT {
    pub left: LONG,
    pub top: LONG,
    pub right: LONG,
    pub bottom: LONG,
}

// http://msdn.microsoft.com/en-us/library/windows/desktop/dd368826(v=vs.85).aspx
#[repr(C)]
pub struct PIXELFORMATDESCRIPTOR {
    pub nSize: WORD,
    pub nVersion: WORD,
    pub dwFlags: DWORD,
    pub iPixelType: BYTE,
    pub cColorBits: BYTE,
    pub cRedBits: BYTE,
    pub cRedShift: BYTE,
    pub cGreenBits: BYTE,
    pub cGreenShift: BYTE,
    pub cBlueBits: BYTE,
    pub cBlueShift: BYTE,
    pub cAlphaBits: BYTE,
    pub cAlphaShift: BYTE,
    pub cAccumBits: BYTE,
    pub cAccumRedBits: BYTE,
    pub cAccumGreenBits: BYTE,
    pub cAccumBlueBits: BYTE,
    pub cAccumAlphaBits: BYTE,
    pub cDepthBits: BYTE,
    pub cStencilBits: BYTE,
    pub cAuxBuffers: BYTE,
    pub iLayerType: BYTE,
    pub bReserved: BYTE,
    pub dwLayerMask: DWORD,
    pub dwVisibleMask: DWORD,
    pub dwDamageMask: DWORD,
}

// http://msdn.microsoft.com/en-us/library/dd162807(v=vs.85).aspx
#[repr(C)]
pub struct POINTL {
    pub x: LONG,
    pub y: LONG,
}

// http://msdn.microsoft.com/en-us/library/windows/desktop/dd183565(v=vs.85).aspx
#[repr(C)]
pub struct DEVMODE {
    pub dmDeviceName: [WCHAR, ..32],
    pub dmSpecVersion: WORD,
    pub dmDriverVersion: WORD,
    pub dmSize: WORD,
    pub dmDriverExtra: WORD,
    pub dmFields: DWORD,
    pub union1: [u8, ..16],
    pub dmColor: libc::c_short,
    pub dmDuplex: libc::c_short,
    pub dmYResolution: libc::c_short,
    pub dmTTOption: libc::c_short,
    pub dmCollate: libc::c_short,
    pub dmFormName: [WCHAR, ..32],
    pub dmLogPixels: WORD,
    pub dmBitsPerPel: DWORD,
    pub dmPelsWidth: DWORD,
    pub dmPelsHeight: DWORD,
    pub dmDisplayFlags: DWORD,
    pub dmDisplayFrequency: DWORD,
    pub dmICMMethod: DWORD,
    pub dmICMIntent: DWORD,
    pub dmMediaType: DWORD,
    pub dmDitherType: DWORD,
    dmReserved1: DWORD,
    dmReserved2: DWORD,
    pub dmPanningWidth: DWORD,
    pub dmPanningHeight: DWORD,
}

// http://msdn.microsoft.com/en-us/library/windows/desktop/ms632611(v=vs.85).aspx
#[repr(C)]
pub struct WINDOWPLACEMENT {
    pub length: UINT,
    pub flags: UINT,
    pub showCmd: UINT,
    pub ptMinPosition: POINT,
    pub ptMaxPosition: POINT,
    pub rcNormalPosition: RECT,
}

// http://msdn.microsoft.com/en-us/library/windows/desktop/dd183569(v=vs.85).aspx
#[repr(C)]
pub struct DISPLAY_DEVICEW {
    pub cb: DWORD,
    pub DeviceName: [WCHAR, ..32],
    pub DeviceString: [WCHAR, ..128],
    pub StateFlags: DWORD,
    pub DeviceID: [WCHAR, ..128],
    pub DeviceKey: [WCHAR, ..128],
}

pub type LPMSG = *mut MSG;

#[link(name = "advapi32")]
#[link(name = "comctl32")]
#[link(name = "comdlg32")]
#[link(name = "gdi32")]
#[link(name = "kernel32")]
#[link(name = "odbc32")]
#[link(name = "odbccp32")]
#[link(name = "ole32")]
#[link(name = "oleaut32")]
#[link(name = "Opengl32")]
#[link(name = "shell32")]
#[link(name = "user32")]
#[link(name = "uuid")]
#[link(name = "winspool")]
extern "system" {
    // http://msdn.microsoft.com/en-us/library/windows/desktop/ms632667(v=vs.85).aspx
    pub fn AdjustWindowRectEx(lpRect: *mut RECT, dwStyle: DWORD, bMenu: BOOL,
        dwExStyle: DWORD) -> BOOL;

    // http://msdn.microsoft.com/en-us/library/windows/desktop/dd183362(v=vs.85).aspx
    pub fn BeginPaint(hwnd: HWND, lpPaint: *mut PAINTSTRUCT) -> HDC;

    // http://msdn.microsoft.com/en-us/library/windows/desktop/dd183411(v=vs.85).aspx
    pub fn ChangeDisplaySettingsW(lpDevMode: *mut DEVMODE, dwFlags: DWORD) -> LONG;

    // http://msdn.microsoft.com/en-us/library/windows/desktop/dd183413(v=vs.85).aspx
    pub fn ChangeDisplaySettingsExW(lpszDeviceName: LPCWSTR, lpDevMode: *mut DEVMODE, hwnd: HWND,
        dwFlags: DWORD, lParam: LPVOID) -> LONG;

    // http://msdn.microsoft.com/en-us/library/dd318284(v=vs.85).aspx
    pub fn ChoosePixelFormat(hdc: HDC, ppfd: *const PIXELFORMATDESCRIPTOR) -> libc::c_int;

    // http://msdn.microsoft.com/en-us/library/windows/desktop/ms632680(v=vs.85).aspx
    pub fn CreateWindowExW(dwExStyle: DWORD, lpClassName: LPCWSTR, lpWindowName: LPCWSTR,
        dwStyle: DWORD, x: libc::c_int, y: libc::c_int, nWidth: libc::c_int, nHeight: libc::c_int,
        hWndParent: HWND, hMenu: HMENU, hInstance: HINSTANCE, lpParam: LPVOID) -> HWND;

    // http://msdn.microsoft.com/en-us/library/windows/desktop/ms633572(v=vs.85).aspx
    pub fn DefWindowProcW(hWnd: HWND, Msg: UINT, wParam: WPARAM, lParam: LPARAM) -> LRESULT;

    // http://msdn.microsoft.com/en-us/library/windows/desktop/dd318302(v=vs.85).aspx
    pub fn DescribePixelFormat(hdc: HDC, iPixelFormat: libc::c_int, nBytes: UINT,
        ppfd: *mut PIXELFORMATDESCRIPTOR) -> libc::c_int;

    // http://msdn.microsoft.com/en-us/library/windows/desktop/ms632682(v=vs.85).aspx
    pub fn DestroyWindow(hWnd: HWND) -> BOOL;

    // http://msdn.microsoft.com/en-us/library/windows/desktop/ms644934(v=vs.85).aspx
    pub fn DispatchMessageW(lpmsg: *const MSG) -> LRESULT;

    // http://msdn.microsoft.com/en-us/library/windows/desktop/dd162598(v=vs.85).aspx
    pub fn EndPaint(hWnd: HWND, lpPaint: *const PAINTSTRUCT) -> BOOL;

    // http://msdn.microsoft.com/en-us/library/windows/desktop/dd162609(v=vs.85).aspx
    pub fn EnumDisplayDevicesW(lpDevice: LPCWSTR, iDevNum: DWORD,
        lpDisplayDevice: *mut DISPLAY_DEVICEW, dwFlags: DWORD) -> BOOL;

    // http://msdn.microsoft.com/en-us/library/dd162612(v=vs.85).aspx
    pub fn EnumDisplaySettingsExW(lpszDeviceName: LPCWSTR, iModeNum: DWORD,
        lpDevMode: *mut DEVMODE, dwFlags: DWORD) -> BOOL;

    // http://msdn.microsoft.com/en-us/library/windows/desktop/dd162719(v=vs.85).aspx
    pub fn FillRect(hDC: HDC, lprc: *const RECT, hbr: HBRUSH) -> libc::c_int;

    // http://msdn.microsoft.com/en-us/library/windows/desktop/ms633503(v=vs.85).aspx
    pub fn GetClientRect(hWnd: HWND, lpRect: *mut RECT) -> BOOL;

    // http://msdn.microsoft.com/en-us/library/dd144871(v=vs.85).aspx
    pub fn GetDC(hWnd: HWND) -> HDC;

    // http://msdn.microsoft.com/en-us/library/windows/desktop/ms679360(v=vs.85).aspx
    pub fn GetLastError() -> DWORD;

    // http://msdn.microsoft.com/en-us/library/windows/desktop/ms644936(v=vs.85).aspx
    pub fn GetMessageW(lpMsg: LPMSG, hWnd: HWND, wMsgFilterMin: UINT, wMsgFilterMax: UINT) -> BOOL;

    // http://msdn.microsoft.com/en-us/library/windows/desktop/ms683199(v=vs.85).aspx
    pub fn GetModuleHandleW(lpModuleName: LPCWSTR) -> HMODULE;

    // http://msdn.microsoft.com/en-us/library/windows/desktop/ms683212(v=vs.85).aspx
    pub fn GetProcAddress(hModule: HMODULE, lpProcName: LPCSTR) -> *const libc::c_void;

    // http://msdn.microsoft.com/en-us/library/windows/desktop/ms633518(v=vs.85).aspx
    pub fn GetWindowPlacement(hWnd: HWND, lpwndpl: *mut WINDOWPLACEMENT) -> BOOL;

    // http://msdn.microsoft.com/en-us/library/windows/desktop/ms633519(v=vs.85).aspx
    pub fn GetWindowRect(hWnd: HWND, lpRect: *mut RECT) -> BOOL;

    //
    pub fn glFlush();

    // http://msdn.microsoft.com/en-us/library/windows/desktop/ms684175(v=vs.85).aspx
    pub fn LoadLibraryW(lpFileName: LPCWSTR) -> HMODULE;

    // http://msdn.microsoft.com/en-us/library/windows/desktop/aa366730(v=vs.85).aspx
    pub fn LocalFree(hMem: HLOCAL) -> HLOCAL;

    // http://msdn.microsoft.com/en-us/library/windows/desktop/ms644943(v=vs.85).aspx
    pub fn PeekMessageW(lpMsg: *mut MSG, hWnd: HWND, wMsgFilterMin: UINT, wMsgFilterMax: UINT,
        wRemoveMsg: UINT) -> BOOL;

    // http://msdn.microsoft.com/en-us/library/windows/desktop/ms644945(v=vs.85).aspx
    pub fn PostQuitMessage(nExitCode: libc::c_int);

    // http://msdn.microsoft.com/en-us/library/windows/desktop/ms633586(v=vs.85).aspx
    pub fn RegisterClassExW(lpWndClass: *const WNDCLASSEX) -> ATOM;

    // http://msdn.microsoft.com/en-us/library/windows/desktop/ms633539(v=vs.85).aspx
    pub fn SetForegroundWindow(hWnd: HWND) -> BOOL;

    // http://msdn.microsoft.com/en-us/library/windows/desktop/dd369049(v=vs.85).aspx
    pub fn SetPixelFormat(hdc: HDC, iPixelFormat: libc::c_int,
        ppfd: *const PIXELFORMATDESCRIPTOR) -> BOOL;

    // http://msdn.microsoft.com/en-us/library/windows/desktop/ms633545(v=vs.85).aspx
    pub fn SetWindowPos(hWnd: HWND, hWndInsertAfter: HWND, X: libc::c_int, Y: libc::c_int,
        cx: libc::c_int, cy: libc::c_int, uFlags: UINT) -> BOOL;

    // http://msdn.microsoft.com/en-us/library/windows/desktop/ms633546(v=vs.85).aspx
    pub fn SetWindowTextW(hWnd: HWND, lpString: LPCWSTR) -> BOOL;

    // http://msdn.microsoft.com/en-us/library/windows/desktop/ms633548(v=vs.85).aspx
    pub fn ShowWindow(hWnd: HWND, nCmdShow: libc::c_int) -> BOOL;

    // http://msdn.microsoft.com/en-us/library/windows/desktop/dd369060(v=vs.85).aspx
    pub fn SwapBuffers(hdc: HDC) -> BOOL;

    // http://msdn.microsoft.com/en-us/library/windows/desktop/ms644934(v=vs.85).aspx
    pub fn TranslateMessage(lpmsg: *const MSG) -> BOOL;

    // http://msdn.microsoft.com/en-us/library/dd145167(v=vs.85).aspx
    pub fn UpdateWindow(hWnd: HWND) -> BOOL;

    // http://msdn.microsoft.com/en-us/library/windows/desktop/ms644956(v=vs.85).aspx
    pub fn WaitMessage() -> BOOL;
}
