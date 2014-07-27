#![allow(dead_code)]
#![allow(non_snake_case_functions)]
#![allow(non_camel_case_types)]

use libc;

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
pub type PVOID = *mut libc::c_void;
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

// messages
pub static WM_COMMAND: UINT = 0x0111;
pub static WM_DESTROY: UINT = 0x0002;
pub static WM_MOUSEMOVE: UINT = 0x0200;
pub static WM_PAINT: UINT = 0x000F;
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
    // http://msdn.microsoft.com/en-us/library/windows/desktop/dd183362(v=vs.85).aspx
    pub fn BeginPaint(hwnd: HWND, lpPaint: *mut PAINTSTRUCT) -> HDC;

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

    // http://msdn.microsoft.com/en-us/library/windows/desktop/dd162719(v=vs.85).aspx
    pub fn FillRect(hDC: HDC, lprc: *const RECT, hbr: HBRUSH) -> libc::c_int;

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

    // http://msdn.microsoft.com/en-us/library/windows/desktop/ms684175(v=vs.85).aspx
    pub fn LoadLibraryW(lpFileName: LPCWSTR) -> HMODULE;

    // http://msdn.microsoft.com/en-us/library/windows/desktop/aa366730(v=vs.85).aspx
    pub fn LocalFree(hMem: HLOCAL) -> HLOCAL;

    // http://msdn.microsoft.com/en-us/library/windows/desktop/ms644945(v=vs.85).aspx
    pub fn PostQuitMessage(nExitCode: libc::c_int);

    // http://msdn.microsoft.com/en-us/library/windows/desktop/ms633586(v=vs.85).aspx
    pub fn RegisterClassExW(lpWndClass: *const WNDCLASSEX) -> ATOM;

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

    // http://msdn.microsoft.com/en-us/library/windows/desktop/dd374379(v=vs.85).aspx
    pub fn wglCreateContext(hdc: HDC) -> HGLRC;

    // http://msdn.microsoft.com/en-us/library/windows/desktop/dd374381(v=vs.85).aspx
    pub fn wglDeleteContext(hglrc: HGLRC);

    // http://msdn.microsoft.com/en-us/library/windows/desktop/dd374386(v=vs.85).aspx
    pub fn wglGetProcAddress(lpszProc: LPCSTR) -> *const libc::c_void;

    // http://msdn.microsoft.com/en-us/library/windows/desktop/dd374387(v=vs.85).aspx
    pub fn wglMakeCurrent(hdc: HDC, hglrc: HGLRC);
}
