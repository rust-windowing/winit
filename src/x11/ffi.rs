#![allow(dead_code)]
#![allow(non_snake_case)]
#![allow(non_camel_case_types)]
#![allow(non_upper_case_globals)]

pub use self::glx::types::*;
use libc;

/// GLX bindings
pub mod glx {
    include!(concat!(env!("OUT_DIR"), "/glx_bindings.rs"));
}

/// Functions that are not necessarly always available
pub mod glx_extra {
    include!(concat!(env!("OUT_DIR"), "/glx_extra_bindings.rs"));
}

pub type Atom = libc::c_ulong;
pub type Colormap = XID;
pub type Cursor = XID;
pub type Drawable = XID;    // TODO: not sure
pub type KeyCode = libc::c_ulong;
pub type KeySym = XID;
pub type OSMesaContext = *const ();
pub type Status = libc::c_int;  // TODO: not sure
pub type Time = libc::c_ulong;
pub type XrmDatabase = *const ();       // TODO: not sure
pub type XIC = *mut ();
pub type XIM = *mut ();
pub type Screen = ();

pub const AllocNone: libc::c_int = 0;
pub const AllocAll: libc::c_int = 1;

pub const Button1: libc::c_uint = 1;
pub const Button2: libc::c_uint = 2;
pub const Button3: libc::c_uint = 3;
pub const Button4: libc::c_uint = 4;
pub const Button5: libc::c_uint = 5;

pub const InputOutput: libc::c_uint = 1;
pub const InputOnly: libc::c_uint = 2;

pub const CWBackPixmap: libc::c_ulong = (1<<0);
pub const CWBackPixel: libc::c_ulong = (1<<1);
pub const CWBorderPixmap: libc::c_ulong = (1<<2);
pub const CWBorderPixel: libc::c_ulong = (1<<3);
pub const CWBitGravity: libc::c_ulong = (1<<4);
pub const CWWinGravity: libc::c_ulong = (1<<5);
pub const CWBackingStore: libc::c_ulong = (1<<6);
pub const CWBackingPlanes: libc::c_ulong = (1<<7);
pub const CWBackingPixel: libc::c_ulong = (1<<8);
pub const CWOverrideRedirect: libc::c_ulong = (1<<9);
pub const CWSaveUnder: libc::c_ulong = (1<<10);
pub const CWEventMask: libc::c_ulong = (1<<11);
pub const CWDontPropagate: libc::c_ulong = (1<<12);
pub const CWColormap: libc::c_ulong = (1<<13);
pub const CWCursor: libc::c_ulong = (1<<14);

pub const NoEventMask: libc::c_long = 0;
pub const KeyPressMask: libc::c_long = (1<<0);
pub const KeyReleaseMask: libc::c_long = (1<<1);
pub const ButtonPressMask: libc::c_long = (1<<2);
pub const ButtonReleaseMask: libc::c_long = (1<<3);
pub const EnterWindowMask: libc::c_long = (1<<4);
pub const LeaveWindowMask: libc::c_long = (1<<5);
pub const PointerMotionMask: libc::c_long = (1<<6);
pub const PointerMotionHintMask: libc::c_long = (1<<7);
pub const Button1MotionMask: libc::c_long = (1<<8);
pub const Button2MotionMask: libc::c_long = (1<<9);
pub const Button3MotionMask: libc::c_long = (1<<10);
pub const Button4MotionMask: libc::c_long = (1<<11);
pub const Button5MotionMask: libc::c_long = (1<<12);
pub const ButtonMotionMask: libc::c_long = (1<<13);
pub const KeymapStateMask: libc::c_long = (1<<14);
pub const ExposureMask: libc::c_long = (1<<15);
pub const VisibilityChangeMask: libc::c_long = (1<<16);
pub const StructureNotifyMask: libc::c_long = (1<<17);
pub const ResizeRedirectMask: libc::c_long = (1<<18);
pub const SubstructureNotifyMask: libc::c_long = (1<<19);
pub const SubstructureRedirectMask: libc::c_long = (1<<20);
pub const FocusChangeMask: libc::c_long = (1<<21);
pub const PropertyChangeMask: libc::c_long = (1<<22);
pub const ColormapChangeMask: libc::c_long = (1<<23);
pub const OwnerGrabButtonMask: libc::c_long = (1<<24);

pub const KeyPress: libc::c_int = 2;
pub const KeyRelease: libc::c_int = 3;
pub const ButtonPress: libc::c_int = 4;
pub const ButtonRelease: libc::c_int = 5;
pub const MotionNotify: libc::c_int = 6;
pub const EnterNotify: libc::c_int = 7;
pub const LeaveNotify: libc::c_int = 8;
pub const FocusIn: libc::c_int = 9;
pub const FocusOut: libc::c_int = 10;
pub const KeymapNotify: libc::c_int = 11;
pub const Expose: libc::c_int = 12;
pub const GraphicsExpose: libc::c_int = 13;
pub const NoExpose: libc::c_int = 14;
pub const VisibilityNotify: libc::c_int = 15;
pub const CreateNotify: libc::c_int = 16;
pub const DestroyNotify: libc::c_int = 17;
pub const UnmapNotify: libc::c_int = 18;
pub const MapNotify: libc::c_int = 19;
pub const MapRequest: libc::c_int = 20;
pub const ReparentNotify: libc::c_int = 21;
pub const ConfigureNotify: libc::c_int = 22;
pub const ConfigureRequest: libc::c_int = 23;
pub const GravityNotify: libc::c_int = 24;
pub const ResizeRequest: libc::c_int = 25;
pub const CirculateNotify: libc::c_int = 26;
pub const CirculateRequest: libc::c_int = 27;
pub const PropertyNotify: libc::c_int = 28;
pub const SelectionClear: libc::c_int = 29;
pub const SelectionRequest: libc::c_int = 30;
pub const SelectionNotify: libc::c_int = 31;
pub const ColormapNotify: libc::c_int = 32;
pub const ClientMessage: libc::c_int = 33;
pub const MappingNotify: libc::c_int = 34;

pub const GLX_USE_GL: libc::c_int = 1;
pub const GLX_BUFFER_SIZE: libc::c_int = 2;
pub const GLX_LEVEL: libc::c_int = 3;
pub const GLX_RGBA: libc::c_int = 4;
pub const GLX_DOUBLEBUFFER: libc::c_int = 5;
pub const GLX_STEREO: libc::c_int = 6;
pub const GLX_AUX_BUFFERS: libc::c_int = 7;
pub const GLX_RED_SIZE: libc::c_int = 8;
pub const GLX_GREEN_SIZE: libc::c_int = 9;
pub const GLX_BLUE_SIZE: libc::c_int = 10;
pub const GLX_ALPHA_SIZE: libc::c_int = 11;
pub const GLX_DEPTH_SIZE: libc::c_int = 12;
pub const GLX_STENCIL_SIZE: libc::c_int = 13;
pub const GLX_ACCUM_RED_SIZE: libc::c_int = 14;
pub const GLX_ACCUM_GREEN_SIZE: libc::c_int = 15;
pub const GLX_ACCUM_BLUE_SIZE: libc::c_int = 16;
pub const GLX_ACCUM_ALPHA_SIZE: libc::c_int = 17;
pub const GLX_BAD_SCREEN: libc::c_int = 1;
pub const GLX_BAD_ATTRIBUTE: libc::c_int = 2;
pub const GLX_NO_EXTENSION: libc::c_int = 3;
pub const GLX_BAD_VISUAL: libc::c_int = 4;
pub const GLX_BAD_CONTEXT: libc::c_int = 5;
pub const GLX_BAD_VALUE: libc::c_int = 6;
pub const GLX_BAD_ENUM: libc::c_int = 7;
pub const GLX_VENDOR: libc::c_int = 1;
pub const GLX_VERSION: libc::c_int = 2;
pub const GLX_EXTENSIONS: libc::c_int = 3;
pub const GLX_WINDOW_BIT: libc::c_int = 0x00000001;
pub const GLX_PIXMAP_BIT: libc::c_int = 0x00000002;
pub const GLX_PBUFFER_BIT: libc::c_int = 0x00000004;
pub const GLX_RGBA_BIT: libc::c_int = 0x00000001;
pub const GLX_COLOR_INDEX_BIT: libc::c_int = 0x00000002;
pub const GLX_PBUFFER_CLOBBER_MASK: libc::c_int = 0x08000000;
pub const GLX_FRONT_LEFT_BUFFER_BIT: libc::c_int = 0x00000001;
pub const GLX_FRONT_RIGHT_BUFFER_BIT: libc::c_int = 0x00000002;
pub const GLX_BACK_LEFT_BUFFER_BIT: libc::c_int = 0x00000004;
pub const GLX_BACK_RIGHT_BUFFER_BIT: libc::c_int = 0x00000008;
pub const GLX_AUX_BUFFERS_BIT: libc::c_int = 0x00000010;
pub const GLX_DEPTH_BUFFER_BIT: libc::c_int = 0x00000020;
pub const GLX_STENCIL_BUFFER_BIT: libc::c_int = 0x00000040;
pub const GLX_ACCUM_BUFFER_BIT: libc::c_int = 0x00000080;
pub const GLX_CONFIG_CAVEAT: libc::c_int = 0x20;
pub const GLX_X_VISUAL_TYPE: libc::c_int = 0x22;
pub const GLX_TRANSPARENT_TYPE: libc::c_int = 0x23;
pub const GLX_TRANSPARENT_INDEX_VALUE: libc::c_int = 0x24;
pub const GLX_TRANSPARENT_RED_VALUE: libc::c_int = 0x25;
pub const GLX_TRANSPARENT_GREEN_VALUE: libc::c_int = 0x26;
pub const GLX_TRANSPARENT_BLUE_VALUE: libc::c_int = 0x27;
pub const GLX_TRANSPARENT_ALPHA_VALUE: libc::c_int = 0x28;
#[allow(overflowing_literals)]
pub const GLX_DONT_CARE: libc::c_int = 0xFFFFFFFF;
pub const GLX_NONE: libc::c_int = 0x8000;
pub const GLX_SLOW_CONFIG: libc::c_int = 0x8001;
pub const GLX_TRUE_COLOR: libc::c_int = 0x8002;
pub const GLX_DIRECT_COLOR: libc::c_int = 0x8003;
pub const GLX_PSEUDO_COLOR: libc::c_int = 0x8004;
pub const GLX_const_COLOR: libc::c_int = 0x8005;
pub const GLX_GRAY_SCALE: libc::c_int = 0x8006;
pub const GLX_const_GRAY: libc::c_int = 0x8007;
pub const GLX_TRANSPARENT_RGB: libc::c_int = 0x8008;
pub const GLX_TRANSPARENT_INDEX: libc::c_int = 0x8009;
pub const GLX_VISUAL_ID: libc::c_int = 0x800B;
pub const GLX_SCREEN: libc::c_int = 0x800C;
pub const GLX_NON_CONFORMANT_CONFIG: libc::c_int = 0x800D;
pub const GLX_DRAWABLE_TYPE: libc::c_int = 0x8010;
pub const GLX_RENDER_TYPE: libc::c_int = 0x8011;
pub const GLX_X_RENDERABLE: libc::c_int = 0x8012;
pub const GLX_FBCONFIG_ID: libc::c_int = 0x8013;
pub const GLX_RGBA_TYPE: libc::c_int = 0x8014;
pub const GLX_COLOR_INDEX_TYPE: libc::c_int = 0x8015;
pub const GLX_MAX_PBUFFER_WIDTH: libc::c_int = 0x8016;
pub const GLX_MAX_PBUFFER_HEIGHT: libc::c_int = 0x8017;
pub const GLX_MAX_PBUFFER_PIXELS: libc::c_int = 0x8018;
pub const GLX_PRESERVED_CONTENTS: libc::c_int = 0x801B;
pub const GLX_LARGEST_PBUFFER: libc::c_int = 0x801C;
pub const GLX_WIDTH: libc::c_int = 0x801D;
pub const GLX_HEIGHT: libc::c_int = 0x801E;
pub const GLX_EVENT_MASK: libc::c_int = 0x801F;
pub const GLX_DAMAGED: libc::c_int = 0x8020;
pub const GLX_SAVED: libc::c_int = 0x8021;
pub const GLX_WINDOW: libc::c_int = 0x8022;
pub const GLX_PBUFFER: libc::c_int = 0x8023;
pub const GLX_PBUFFER_HEIGHT: libc::c_int = 0x8040;
pub const GLX_PBUFFER_WIDTH: libc::c_int = 0x8041;

pub const GLX_CONTEXT_MAJOR_VERSION: libc::c_int = 0x2091;
pub const GLX_CONTEXT_MINOR_VERSION: libc::c_int = 0x2092;
pub const GLX_CONTEXT_FLAGS: libc::c_int = 0x2094;
pub const GLX_CONTEXT_PROFILE_MASK: libc::c_int = 0x9126;
pub const GLX_CONTEXT_DEBUG_BIT: libc::c_int = 0x0001;
pub const GLX_CONTEXT_FORWARD_COMPATIBLE_BIT: libc::c_int = 0x0002;
pub const GLX_CONTEXT_CORE_PROFILE_BIT: libc::c_int = 0x00000001;
pub const GLX_CONTEXT_COMPATIBILITY_PROFILE_BIT: libc::c_int = 0x00000002;

pub const XIMPreeditArea: libc::c_long = 0x0001;
pub const XIMPreeditCallbacks: libc::c_long = 0x0002;
pub const XIMPreeditPosition: libc::c_long = 0x0004;
pub const XIMPreeditNothing: libc::c_long = 0x0008;
pub const XIMPreeditNone: libc::c_long = 0x0010;
pub const XIMStatusArea: libc::c_long = 0x0100;
pub const XIMStatusCallbacks: libc::c_long = 0x0200;
pub const XIMStatusNothing: libc::c_long = 0x0400;
pub const XIMStatusNone: libc::c_long = 0x0800;

pub const XK_BackSpace: libc::c_uint = 0xFF08;
pub const XK_Tab: libc::c_uint = 0xFF09;
pub const XK_Linefeed: libc::c_uint = 0xFF0A;
pub const XK_Clear: libc::c_uint = 0xFF0B;
pub const XK_Return: libc::c_uint = 0xFF0D;
pub const XK_Pause: libc::c_uint = 0xFF13;
pub const XK_Scroll_Lock: libc::c_uint = 0xFF14;
pub const XK_Sys_Req: libc::c_uint = 0xFF15;
pub const XK_Escape: libc::c_uint = 0xFF1B;
pub const XK_Delete: libc::c_uint = 0xFFFF;
pub const XK_Multi_key: libc::c_uint = 0xFF20;
pub const XK_Kanji: libc::c_uint = 0xFF21;
pub const XK_Muhenkan: libc::c_uint = 0xFF22;
pub const XK_Henkan_Mode: libc::c_uint = 0xFF23;
pub const XK_Henkan: libc::c_uint = 0xFF23;
pub const XK_Romaji: libc::c_uint = 0xFF24;
pub const XK_Hiragana: libc::c_uint = 0xFF25;
pub const XK_Katakana: libc::c_uint = 0xFF26;
pub const XK_Hiragana_Katakana: libc::c_uint = 0xFF27;
pub const XK_Zenkaku: libc::c_uint = 0xFF28;
pub const XK_Hankaku: libc::c_uint = 0xFF29;
pub const XK_Zenkaku_Hankaku: libc::c_uint = 0xFF2A;
pub const XK_Touroku: libc::c_uint = 0xFF2B;
pub const XK_Massyo: libc::c_uint = 0xFF2C;
pub const XK_Kana_Lock: libc::c_uint = 0xFF2D;
pub const XK_Kana_Shift: libc::c_uint = 0xFF2E;
pub const XK_Eisu_Shift: libc::c_uint = 0xFF2F;
pub const XK_Eisu_toggle: libc::c_uint = 0xFF30;
pub const XK_Home: libc::c_uint = 0xFF50;
pub const XK_Left: libc::c_uint = 0xFF51;
pub const XK_Up: libc::c_uint = 0xFF52;
pub const XK_Right: libc::c_uint = 0xFF53;
pub const XK_Down: libc::c_uint = 0xFF54;
pub const XK_Prior: libc::c_uint = 0xFF55;
pub const XK_Page_Up: libc::c_uint = 0xFF55;
pub const XK_Next: libc::c_uint = 0xFF56;
pub const XK_Page_Down: libc::c_uint = 0xFF56;
pub const XK_End: libc::c_uint = 0xFF57;
pub const XK_Begin: libc::c_uint = 0xFF58;
pub const XK_Win_L: libc::c_uint = 0xFF5B;
pub const XK_Win_R: libc::c_uint = 0xFF5C;
pub const XK_App: libc::c_uint = 0xFF5D;
pub const XK_Select: libc::c_uint = 0xFF60;
pub const XK_Print: libc::c_uint = 0xFF61;
pub const XK_Execute: libc::c_uint = 0xFF62;
pub const XK_Insert: libc::c_uint = 0xFF63;
pub const XK_Undo: libc::c_uint = 0xFF65;
pub const XK_Redo: libc::c_uint = 0xFF66;
pub const XK_Menu: libc::c_uint = 0xFF67;
pub const XK_Find: libc::c_uint = 0xFF68;
pub const XK_Cancel: libc::c_uint = 0xFF69;
pub const XK_Help: libc::c_uint = 0xFF6A;
pub const XK_Break: libc::c_uint = 0xFF6B;
pub const XK_Mode_switch: libc::c_uint = 0xFF7E;
pub const XK_script_switch: libc::c_uint = 0xFF7E;
pub const XK_Num_Lock: libc::c_uint = 0xFF7F;
pub const XK_KP_Space: libc::c_uint = 0xFF80;
pub const XK_KP_Tab: libc::c_uint = 0xFF89;
pub const XK_KP_Enter: libc::c_uint = 0xFF8D;
pub const XK_KP_F1: libc::c_uint = 0xFF91;
pub const XK_KP_F2: libc::c_uint = 0xFF92;
pub const XK_KP_F3: libc::c_uint = 0xFF93;
pub const XK_KP_F4: libc::c_uint = 0xFF94;
pub const XK_KP_Home: libc::c_uint = 0xFF95;
pub const XK_KP_Left: libc::c_uint = 0xFF96;
pub const XK_KP_Up: libc::c_uint = 0xFF97;
pub const XK_KP_Right: libc::c_uint = 0xFF98;
pub const XK_KP_Down: libc::c_uint = 0xFF99;
pub const XK_KP_Prior: libc::c_uint = 0xFF9A;
pub const XK_KP_Page_Up: libc::c_uint = 0xFF9A;
pub const XK_KP_Next: libc::c_uint = 0xFF9B;
pub const XK_KP_Page_Down: libc::c_uint = 0xFF9B;
pub const XK_KP_End: libc::c_uint = 0xFF9C;
pub const XK_KP_Begin: libc::c_uint = 0xFF9D;
pub const XK_KP_Insert: libc::c_uint = 0xFF9E;
pub const XK_KP_Delete: libc::c_uint = 0xFF9F;
pub const XK_KP_Equal: libc::c_uint = 0xFFBD;
pub const XK_KP_Multiply: libc::c_uint = 0xFFAA;
pub const XK_KP_Add: libc::c_uint = 0xFFAB;
pub const XK_KP_Separator: libc::c_uint = 0xFFAC;
pub const XK_KP_Subtract: libc::c_uint = 0xFFAD;
pub const XK_KP_Decimal: libc::c_uint = 0xFFAE;
pub const XK_KP_Divide: libc::c_uint = 0xFFAF;
pub const XK_KP_0: libc::c_uint = 0xFFB0;
pub const XK_KP_1: libc::c_uint = 0xFFB1;
pub const XK_KP_2: libc::c_uint = 0xFFB2;
pub const XK_KP_3: libc::c_uint = 0xFFB3;
pub const XK_KP_4: libc::c_uint = 0xFFB4;
pub const XK_KP_5: libc::c_uint = 0xFFB5;
pub const XK_KP_6: libc::c_uint = 0xFFB6;
pub const XK_KP_7: libc::c_uint = 0xFFB7;
pub const XK_KP_8: libc::c_uint = 0xFFB8;
pub const XK_KP_9: libc::c_uint = 0xFFB9;
pub const XK_F1: libc::c_uint = 0xFFBE;
pub const XK_F2: libc::c_uint = 0xFFBF;
pub const XK_F3: libc::c_uint = 0xFFC0;
pub const XK_F4: libc::c_uint = 0xFFC1;
pub const XK_F5: libc::c_uint = 0xFFC2;
pub const XK_F6: libc::c_uint = 0xFFC3;
pub const XK_F7: libc::c_uint = 0xFFC4;
pub const XK_F8: libc::c_uint = 0xFFC5;
pub const XK_F9: libc::c_uint = 0xFFC6;
pub const XK_F10: libc::c_uint = 0xFFC7;
pub const XK_F11: libc::c_uint = 0xFFC8;
pub const XK_L1: libc::c_uint = 0xFFC8;
pub const XK_F12: libc::c_uint = 0xFFC9;
pub const XK_L2: libc::c_uint = 0xFFC9;
pub const XK_F13: libc::c_uint = 0xFFCA;
pub const XK_L3: libc::c_uint = 0xFFCA;
pub const XK_F14: libc::c_uint = 0xFFCB;
pub const XK_L4: libc::c_uint = 0xFFCB;
pub const XK_F15: libc::c_uint = 0xFFCC;
pub const XK_L5: libc::c_uint = 0xFFCC;
pub const XK_F16: libc::c_uint = 0xFFCD;
pub const XK_L6: libc::c_uint = 0xFFCD;
pub const XK_F17: libc::c_uint = 0xFFCE;
pub const XK_L7: libc::c_uint = 0xFFCE;
pub const XK_F18: libc::c_uint = 0xFFCF;
pub const XK_L8: libc::c_uint = 0xFFCF;
pub const XK_F19: libc::c_uint = 0xFFD0;
pub const XK_L9: libc::c_uint = 0xFFD0;
pub const XK_F20: libc::c_uint = 0xFFD1;
pub const XK_L10: libc::c_uint = 0xFFD1;
pub const XK_F21: libc::c_uint = 0xFFD2;
pub const XK_R1: libc::c_uint = 0xFFD2;
pub const XK_F22: libc::c_uint = 0xFFD3;
pub const XK_R2: libc::c_uint = 0xFFD3;
pub const XK_F23: libc::c_uint = 0xFFD4;
pub const XK_R3: libc::c_uint = 0xFFD4;
pub const XK_F24: libc::c_uint = 0xFFD5;
pub const XK_R4: libc::c_uint = 0xFFD5;
pub const XK_F25: libc::c_uint = 0xFFD6;
pub const XK_R5: libc::c_uint = 0xFFD6;
pub const XK_F26: libc::c_uint = 0xFFD7;
pub const XK_R6: libc::c_uint = 0xFFD7;
pub const XK_F27: libc::c_uint = 0xFFD8;
pub const XK_R7: libc::c_uint = 0xFFD8;
pub const XK_F28: libc::c_uint = 0xFFD9;
pub const XK_R8: libc::c_uint = 0xFFD9;
pub const XK_F29: libc::c_uint = 0xFFDA;
pub const XK_R9: libc::c_uint = 0xFFDA;
pub const XK_F30: libc::c_uint = 0xFFDB;
pub const XK_R10: libc::c_uint = 0xFFDB;
pub const XK_F31: libc::c_uint = 0xFFDC;
pub const XK_R11: libc::c_uint = 0xFFDC;
pub const XK_F32: libc::c_uint = 0xFFDD;
pub const XK_R12: libc::c_uint = 0xFFDD;
pub const XK_F33: libc::c_uint = 0xFFDE;
pub const XK_R13: libc::c_uint = 0xFFDE;
pub const XK_F34: libc::c_uint = 0xFFDF;
pub const XK_R14: libc::c_uint = 0xFFDF;
pub const XK_F35: libc::c_uint = 0xFFE0;
pub const XK_R15: libc::c_uint = 0xFFE0;
pub const XK_Shift_L: libc::c_uint = 0xFFE1;
pub const XK_Shift_R: libc::c_uint = 0xFFE2;
pub const XK_Control_L: libc::c_uint = 0xFFE3;
pub const XK_Control_R: libc::c_uint = 0xFFE4;
pub const XK_Caps_Lock: libc::c_uint = 0xFFE5;
pub const XK_Shift_Lock: libc::c_uint = 0xFFE6;
pub const XK_Meta_L: libc::c_uint = 0xFFE7;
pub const XK_Meta_R: libc::c_uint = 0xFFE8;
pub const XK_Alt_L: libc::c_uint = 0xFFE9;
pub const XK_Alt_R: libc::c_uint = 0xFFEA;
pub const XK_Super_L: libc::c_uint = 0xFFEB;
pub const XK_Super_R: libc::c_uint = 0xFFEC;
pub const XK_Hyper_L: libc::c_uint = 0xFFED;
pub const XK_Hyper_R: libc::c_uint = 0xFFEE;
pub const XK_space: libc::c_uint = 0x020;
pub const XK_exclam: libc::c_uint = 0x021;
pub const XK_quotedbl: libc::c_uint = 0x022;
pub const XK_numbersign: libc::c_uint = 0x023;
pub const XK_dollar: libc::c_uint = 0x024;
pub const XK_percent: libc::c_uint = 0x025;
pub const XK_ampersand: libc::c_uint = 0x026;
pub const XK_apostrophe: libc::c_uint = 0x027;
pub const XK_quoteright: libc::c_uint = 0x027;
pub const XK_parenleft: libc::c_uint = 0x028;
pub const XK_parenright: libc::c_uint = 0x029;
pub const XK_asterisk: libc::c_uint = 0x02a;
pub const XK_plus: libc::c_uint = 0x02b;
pub const XK_comma: libc::c_uint = 0x02c;
pub const XK_minus: libc::c_uint = 0x02d;
pub const XK_period: libc::c_uint = 0x02e;
pub const XK_slash: libc::c_uint = 0x02f;
pub const XK_0: libc::c_uint = 0x030;
pub const XK_1: libc::c_uint = 0x031;
pub const XK_2: libc::c_uint = 0x032;
pub const XK_3: libc::c_uint = 0x033;
pub const XK_4: libc::c_uint = 0x034;
pub const XK_5: libc::c_uint = 0x035;
pub const XK_6: libc::c_uint = 0x036;
pub const XK_7: libc::c_uint = 0x037;
pub const XK_8: libc::c_uint = 0x038;
pub const XK_9: libc::c_uint = 0x039;
pub const XK_colon: libc::c_uint = 0x03a;
pub const XK_semicolon: libc::c_uint = 0x03b;
pub const XK_less: libc::c_uint = 0x03c;
pub const XK_equal: libc::c_uint = 0x03d;
pub const XK_greater: libc::c_uint = 0x03e;
pub const XK_question: libc::c_uint = 0x03f;
pub const XK_at: libc::c_uint = 0x040;
pub const XK_A: libc::c_uint = 0x041;
pub const XK_B: libc::c_uint = 0x042;
pub const XK_C: libc::c_uint = 0x043;
pub const XK_D: libc::c_uint = 0x044;
pub const XK_E: libc::c_uint = 0x045;
pub const XK_F: libc::c_uint = 0x046;
pub const XK_G: libc::c_uint = 0x047;
pub const XK_H: libc::c_uint = 0x048;
pub const XK_I: libc::c_uint = 0x049;
pub const XK_J: libc::c_uint = 0x04a;
pub const XK_K: libc::c_uint = 0x04b;
pub const XK_L: libc::c_uint = 0x04c;
pub const XK_M: libc::c_uint = 0x04d;
pub const XK_N: libc::c_uint = 0x04e;
pub const XK_O: libc::c_uint = 0x04f;
pub const XK_P: libc::c_uint = 0x050;
pub const XK_Q: libc::c_uint = 0x051;
pub const XK_R: libc::c_uint = 0x052;
pub const XK_S: libc::c_uint = 0x053;
pub const XK_T: libc::c_uint = 0x054;
pub const XK_U: libc::c_uint = 0x055;
pub const XK_V: libc::c_uint = 0x056;
pub const XK_W: libc::c_uint = 0x057;
pub const XK_X: libc::c_uint = 0x058;
pub const XK_Y: libc::c_uint = 0x059;
pub const XK_Z: libc::c_uint = 0x05a;
pub const XK_bracketleft: libc::c_uint = 0x05b;
pub const XK_backslash: libc::c_uint = 0x05c;
pub const XK_bracketright: libc::c_uint = 0x05d;
pub const XK_asciicircum: libc::c_uint = 0x05e;
pub const XK_underscore: libc::c_uint = 0x05f;
pub const XK_grave: libc::c_uint = 0x060;
pub const XK_quoteleft: libc::c_uint = 0x060;
pub const XK_a: libc::c_uint = 0x061;
pub const XK_b: libc::c_uint = 0x062;
pub const XK_c: libc::c_uint = 0x063;
pub const XK_d: libc::c_uint = 0x064;
pub const XK_e: libc::c_uint = 0x065;
pub const XK_f: libc::c_uint = 0x066;
pub const XK_g: libc::c_uint = 0x067;
pub const XK_h: libc::c_uint = 0x068;
pub const XK_i: libc::c_uint = 0x069;
pub const XK_j: libc::c_uint = 0x06a;
pub const XK_k: libc::c_uint = 0x06b;
pub const XK_l: libc::c_uint = 0x06c;
pub const XK_m: libc::c_uint = 0x06d;
pub const XK_n: libc::c_uint = 0x06e;
pub const XK_o: libc::c_uint = 0x06f;
pub const XK_p: libc::c_uint = 0x070;
pub const XK_q: libc::c_uint = 0x071;
pub const XK_r: libc::c_uint = 0x072;
pub const XK_s: libc::c_uint = 0x073;
pub const XK_t: libc::c_uint = 0x074;
pub const XK_u: libc::c_uint = 0x075;
pub const XK_v: libc::c_uint = 0x076;
pub const XK_w: libc::c_uint = 0x077;
pub const XK_x: libc::c_uint = 0x078;
pub const XK_y: libc::c_uint = 0x079;
pub const XK_z: libc::c_uint = 0x07a;
pub const XK_braceleft: libc::c_uint = 0x07b;
pub const XK_bar: libc::c_uint = 0x07c;
pub const XK_braceright: libc::c_uint = 0x07d;
pub const XK_asciitilde: libc::c_uint = 0x07e;
pub const XK_nobreakspace: libc::c_uint = 0x0a0;
pub const XK_exclamdown: libc::c_uint = 0x0a1;
pub const XK_cent: libc::c_uint = 0x0a2;
pub const XK_sterling: libc::c_uint = 0x0a3;
pub const XK_currency: libc::c_uint = 0x0a4;
pub const XK_yen: libc::c_uint = 0x0a5;
pub const XK_brokenbar: libc::c_uint = 0x0a6;
pub const XK_section: libc::c_uint = 0x0a7;
pub const XK_diaeresis: libc::c_uint = 0x0a8;
pub const XK_copyright: libc::c_uint = 0x0a9;
pub const XK_ordfeminine: libc::c_uint = 0x0aa;
pub const XK_guillemotleft: libc::c_uint = 0x0ab;
pub const XK_notsign: libc::c_uint = 0x0ac;
pub const XK_hyphen: libc::c_uint = 0x0ad;
pub const XK_registered: libc::c_uint = 0x0ae;
pub const XK_macron: libc::c_uint = 0x0af;
pub const XK_degree: libc::c_uint = 0x0b0;
pub const XK_plusminus: libc::c_uint = 0x0b1;
pub const XK_twosuperior: libc::c_uint = 0x0b2;
pub const XK_threesuperior: libc::c_uint = 0x0b3;
pub const XK_acute: libc::c_uint = 0x0b4;
pub const XK_mu: libc::c_uint = 0x0b5;
pub const XK_paragraph: libc::c_uint = 0x0b6;
pub const XK_periodcentered: libc::c_uint = 0x0b7;
pub const XK_cedilla: libc::c_uint = 0x0b8;
pub const XK_onesuperior: libc::c_uint = 0x0b9;
pub const XK_masculine: libc::c_uint = 0x0ba;
pub const XK_guillemotright: libc::c_uint = 0x0bb;
pub const XK_onequarter: libc::c_uint = 0x0bc;
pub const XK_onehalf: libc::c_uint = 0x0bd;
pub const XK_threequarters: libc::c_uint = 0x0be;
pub const XK_questiondown: libc::c_uint = 0x0bf;
pub const XK_Agrave: libc::c_uint = 0x0c0;
pub const XK_Aacute: libc::c_uint = 0x0c1;
pub const XK_Acircumflex: libc::c_uint = 0x0c2;
pub const XK_Atilde: libc::c_uint = 0x0c3;
pub const XK_Adiaeresis: libc::c_uint = 0x0c4;
pub const XK_Aring: libc::c_uint = 0x0c5;
pub const XK_AE: libc::c_uint = 0x0c6;
pub const XK_Ccedilla: libc::c_uint = 0x0c7;
pub const XK_Egrave: libc::c_uint = 0x0c8;
pub const XK_Eacute: libc::c_uint = 0x0c9;
pub const XK_Ecircumflex: libc::c_uint = 0x0ca;
pub const XK_Ediaeresis: libc::c_uint = 0x0cb;
pub const XK_Igrave: libc::c_uint = 0x0cc;
pub const XK_Iacute: libc::c_uint = 0x0cd;
pub const XK_Icircumflex: libc::c_uint = 0x0ce;
pub const XK_Idiaeresis: libc::c_uint = 0x0cf;
pub const XK_ETH: libc::c_uint = 0x0d0;
pub const XK_Eth: libc::c_uint = 0x0d0;
pub const XK_Ntilde: libc::c_uint = 0x0d1;
pub const XK_Ograve: libc::c_uint = 0x0d2;
pub const XK_Oacute: libc::c_uint = 0x0d3;
pub const XK_Ocircumflex: libc::c_uint = 0x0d4;
pub const XK_Otilde: libc::c_uint = 0x0d5;
pub const XK_Odiaeresis: libc::c_uint = 0x0d6;
pub const XK_multiply: libc::c_uint = 0x0d7;
pub const XK_Ooblique: libc::c_uint = 0x0d8;
pub const XK_Ugrave: libc::c_uint = 0x0d9;
pub const XK_Uacute: libc::c_uint = 0x0da;
pub const XK_Ucircumflex: libc::c_uint = 0x0db;
pub const XK_Udiaeresis: libc::c_uint = 0x0dc;
pub const XK_Yacute: libc::c_uint = 0x0dd;
pub const XK_THORN: libc::c_uint = 0x0de;
pub const XK_Thorn: libc::c_uint = 0x0de;
pub const XK_ssharp: libc::c_uint = 0x0df;
pub const XK_agrave: libc::c_uint = 0x0e0;
pub const XK_aacute: libc::c_uint = 0x0e1;
pub const XK_acircumflex: libc::c_uint = 0x0e2;
pub const XK_atilde: libc::c_uint = 0x0e3;
pub const XK_adiaeresis: libc::c_uint = 0x0e4;
pub const XK_aring: libc::c_uint = 0x0e5;
pub const XK_ae: libc::c_uint = 0x0e6;
pub const XK_ccedilla: libc::c_uint = 0x0e7;
pub const XK_egrave: libc::c_uint = 0x0e8;
pub const XK_eacute: libc::c_uint = 0x0e9;
pub const XK_ecircumflex: libc::c_uint = 0x0ea;
pub const XK_ediaeresis: libc::c_uint = 0x0eb;
pub const XK_igrave: libc::c_uint = 0x0ec;
pub const XK_iacute: libc::c_uint = 0x0ed;
pub const XK_icircumflex: libc::c_uint = 0x0ee;
pub const XK_idiaeresis: libc::c_uint = 0x0ef;
pub const XK_eth: libc::c_uint = 0x0f0;
pub const XK_ntilde: libc::c_uint = 0x0f1;
pub const XK_ograve: libc::c_uint = 0x0f2;
pub const XK_oacute: libc::c_uint = 0x0f3;
pub const XK_ocircumflex: libc::c_uint = 0x0f4;
pub const XK_otilde: libc::c_uint = 0x0f5;
pub const XK_odiaeresis: libc::c_uint = 0x0f6;
pub const XK_division: libc::c_uint = 0x0f7;
pub const XK_oslash: libc::c_uint = 0x0f8;
pub const XK_ugrave: libc::c_uint = 0x0f9;
pub const XK_uacute: libc::c_uint = 0x0fa;
pub const XK_ucircumflex: libc::c_uint = 0x0fb;
pub const XK_udiaeresis: libc::c_uint = 0x0fc;
pub const XK_yacute: libc::c_uint = 0x0fd;
pub const XK_thorn: libc::c_uint = 0x0fe;
pub const XK_ydiaeresis: libc::c_uint = 0x0ff;
pub const XK_Aogonek: libc::c_uint = 0x1a1;
pub const XK_breve: libc::c_uint = 0x1a2;
pub const XK_Lstroke: libc::c_uint = 0x1a3;
pub const XK_Lcaron: libc::c_uint = 0x1a5;
pub const XK_Sacute: libc::c_uint = 0x1a6;
pub const XK_Scaron: libc::c_uint = 0x1a9;
pub const XK_Scedilla: libc::c_uint = 0x1aa;
pub const XK_Tcaron: libc::c_uint = 0x1ab;
pub const XK_Zacute: libc::c_uint = 0x1ac;
pub const XK_Zcaron: libc::c_uint = 0x1ae;
pub const XK_Zabovedot: libc::c_uint = 0x1af;
pub const XK_aogonek: libc::c_uint = 0x1b1;
pub const XK_ogonek: libc::c_uint = 0x1b2;
pub const XK_lstroke: libc::c_uint = 0x1b3;
pub const XK_lcaron: libc::c_uint = 0x1b5;
pub const XK_sacute: libc::c_uint = 0x1b6;
pub const XK_caron: libc::c_uint = 0x1b7;
pub const XK_scaron: libc::c_uint = 0x1b9;
pub const XK_scedilla: libc::c_uint = 0x1ba;
pub const XK_tcaron: libc::c_uint = 0x1bb;
pub const XK_zacute: libc::c_uint = 0x1bc;
pub const XK_doubleacute: libc::c_uint = 0x1bd;
pub const XK_zcaron: libc::c_uint = 0x1be;
pub const XK_zabovedot: libc::c_uint = 0x1bf;
pub const XK_Racute: libc::c_uint = 0x1c0;
pub const XK_Abreve: libc::c_uint = 0x1c3;
pub const XK_Lacute: libc::c_uint = 0x1c5;
pub const XK_Cacute: libc::c_uint = 0x1c6;
pub const XK_Ccaron: libc::c_uint = 0x1c8;
pub const XK_Eogonek: libc::c_uint = 0x1ca;
pub const XK_Ecaron: libc::c_uint = 0x1cc;
pub const XK_Dcaron: libc::c_uint = 0x1cf;
pub const XK_Dstroke: libc::c_uint = 0x1d0;
pub const XK_Nacute: libc::c_uint = 0x1d1;
pub const XK_Ncaron: libc::c_uint = 0x1d2;
pub const XK_Odoubleacute: libc::c_uint = 0x1d5;
pub const XK_Rcaron: libc::c_uint = 0x1d8;
pub const XK_Uring: libc::c_uint = 0x1d9;
pub const XK_Udoubleacute: libc::c_uint = 0x1db;
pub const XK_Tcedilla: libc::c_uint = 0x1de;
pub const XK_racute: libc::c_uint = 0x1e0;
pub const XK_abreve: libc::c_uint = 0x1e3;
pub const XK_lacute: libc::c_uint = 0x1e5;
pub const XK_cacute: libc::c_uint = 0x1e6;
pub const XK_ccaron: libc::c_uint = 0x1e8;
pub const XK_eogonek: libc::c_uint = 0x1ea;
pub const XK_ecaron: libc::c_uint = 0x1ec;
pub const XK_dcaron: libc::c_uint = 0x1ef;
pub const XK_dstroke: libc::c_uint = 0x1f0;
pub const XK_nacute: libc::c_uint = 0x1f1;
pub const XK_ncaron: libc::c_uint = 0x1f2;
pub const XK_odoubleacute: libc::c_uint = 0x1f5;
pub const XK_udoubleacute: libc::c_uint = 0x1fb;
pub const XK_rcaron: libc::c_uint = 0x1f8;
pub const XK_uring: libc::c_uint = 0x1f9;
pub const XK_tcedilla: libc::c_uint = 0x1fe;
pub const XK_abovedot: libc::c_uint = 0x1ff;
pub const XK_Hstroke: libc::c_uint = 0x2a1;
pub const XK_Hcircumflex: libc::c_uint = 0x2a6;
pub const XK_Iabovedot: libc::c_uint = 0x2a9;
pub const XK_Gbreve: libc::c_uint = 0x2ab;
pub const XK_Jcircumflex: libc::c_uint = 0x2ac;
pub const XK_hstroke: libc::c_uint = 0x2b1;
pub const XK_hcircumflex: libc::c_uint = 0x2b6;
pub const XK_idotless: libc::c_uint = 0x2b9;
pub const XK_gbreve: libc::c_uint = 0x2bb;
pub const XK_jcircumflex: libc::c_uint = 0x2bc;
pub const XK_Cabovedot: libc::c_uint = 0x2c5;
pub const XK_Ccircumflex: libc::c_uint = 0x2c6;
pub const XK_Gabovedot: libc::c_uint = 0x2d5;
pub const XK_Gcircumflex: libc::c_uint = 0x2d8;
pub const XK_Ubreve: libc::c_uint = 0x2dd;
pub const XK_Scircumflex: libc::c_uint = 0x2de;
pub const XK_cabovedot: libc::c_uint = 0x2e5;
pub const XK_ccircumflex: libc::c_uint = 0x2e6;
pub const XK_gabovedot: libc::c_uint = 0x2f5;
pub const XK_gcircumflex: libc::c_uint = 0x2f8;
pub const XK_ubreve: libc::c_uint = 0x2fd;
pub const XK_scircumflex: libc::c_uint = 0x2fe;
pub const XK_kra: libc::c_uint = 0x3a2;
pub const XK_kappa: libc::c_uint = 0x3a2;
pub const XK_Rcedilla: libc::c_uint = 0x3a3;
pub const XK_Itilde: libc::c_uint = 0x3a5;
pub const XK_Lcedilla: libc::c_uint = 0x3a6;
pub const XK_Emacron: libc::c_uint = 0x3aa;
pub const XK_Gcedilla: libc::c_uint = 0x3ab;
pub const XK_Tslash: libc::c_uint = 0x3ac;
pub const XK_rcedilla: libc::c_uint = 0x3b3;
pub const XK_itilde: libc::c_uint = 0x3b5;
pub const XK_lcedilla: libc::c_uint = 0x3b6;
pub const XK_emacron: libc::c_uint = 0x3ba;
pub const XK_gcedilla: libc::c_uint = 0x3bb;
pub const XK_tslash: libc::c_uint = 0x3bc;
pub const XK_ENG: libc::c_uint = 0x3bd;
pub const XK_eng: libc::c_uint = 0x3bf;
pub const XK_Amacron: libc::c_uint = 0x3c0;
pub const XK_Iogonek: libc::c_uint = 0x3c7;
pub const XK_Eabovedot: libc::c_uint = 0x3cc;
pub const XK_Imacron: libc::c_uint = 0x3cf;
pub const XK_Ncedilla: libc::c_uint = 0x3d1;
pub const XK_Omacron: libc::c_uint = 0x3d2;
pub const XK_Kcedilla: libc::c_uint = 0x3d3;
pub const XK_Uogonek: libc::c_uint = 0x3d9;
pub const XK_Utilde: libc::c_uint = 0x3dd;
pub const XK_Umacron: libc::c_uint = 0x3de;
pub const XK_amacron: libc::c_uint = 0x3e0;
pub const XK_iogonek: libc::c_uint = 0x3e7;
pub const XK_eabovedot: libc::c_uint = 0x3ec;
pub const XK_imacron: libc::c_uint = 0x3ef;
pub const XK_ncedilla: libc::c_uint = 0x3f1;
pub const XK_omacron: libc::c_uint = 0x3f2;
pub const XK_kcedilla: libc::c_uint = 0x3f3;
pub const XK_uogonek: libc::c_uint = 0x3f9;
pub const XK_utilde: libc::c_uint = 0x3fd;
pub const XK_umacron: libc::c_uint = 0x3fe;
pub const XK_overline: libc::c_uint = 0x47e;
pub const XK_kana_fullstop: libc::c_uint = 0x4a1;
pub const XK_kana_openingbracket: libc::c_uint = 0x4a2;
pub const XK_kana_closingbracket: libc::c_uint = 0x4a3;
pub const XK_kana_comma: libc::c_uint = 0x4a4;
pub const XK_kana_conjunctive: libc::c_uint = 0x4a5;
pub const XK_kana_middledot: libc::c_uint = 0x4a5;
pub const XK_kana_WO: libc::c_uint = 0x4a6;
pub const XK_kana_a: libc::c_uint = 0x4a7;
pub const XK_kana_i: libc::c_uint = 0x4a8;
pub const XK_kana_u: libc::c_uint = 0x4a9;
pub const XK_kana_e: libc::c_uint = 0x4aa;
pub const XK_kana_o: libc::c_uint = 0x4ab;
pub const XK_kana_ya: libc::c_uint = 0x4ac;
pub const XK_kana_yu: libc::c_uint = 0x4ad;
pub const XK_kana_yo: libc::c_uint = 0x4ae;
pub const XK_kana_tsu: libc::c_uint = 0x4af;
pub const XK_kana_tu: libc::c_uint = 0x4af;
pub const XK_prolongedsound: libc::c_uint = 0x4b0;
pub const XK_kana_A: libc::c_uint = 0x4b1;
pub const XK_kana_I: libc::c_uint = 0x4b2;
pub const XK_kana_U: libc::c_uint = 0x4b3;
pub const XK_kana_E: libc::c_uint = 0x4b4;
pub const XK_kana_O: libc::c_uint = 0x4b5;
pub const XK_kana_KA: libc::c_uint = 0x4b6;
pub const XK_kana_KI: libc::c_uint = 0x4b7;
pub const XK_kana_KU: libc::c_uint = 0x4b8;
pub const XK_kana_KE: libc::c_uint = 0x4b9;
pub const XK_kana_KO: libc::c_uint = 0x4ba;
pub const XK_kana_SA: libc::c_uint = 0x4bb;
pub const XK_kana_SHI: libc::c_uint = 0x4bc;
pub const XK_kana_SU: libc::c_uint = 0x4bd;
pub const XK_kana_SE: libc::c_uint = 0x4be;
pub const XK_kana_SO: libc::c_uint = 0x4bf;
pub const XK_kana_TA: libc::c_uint = 0x4c0;
pub const XK_kana_CHI: libc::c_uint = 0x4c1;
pub const XK_kana_TI: libc::c_uint = 0x4c1;
pub const XK_kana_TSU: libc::c_uint = 0x4c2;
pub const XK_kana_TU: libc::c_uint = 0x4c2;
pub const XK_kana_TE: libc::c_uint = 0x4c3;
pub const XK_kana_TO: libc::c_uint = 0x4c4;
pub const XK_kana_NA: libc::c_uint = 0x4c5;
pub const XK_kana_NI: libc::c_uint = 0x4c6;
pub const XK_kana_NU: libc::c_uint = 0x4c7;
pub const XK_kana_NE: libc::c_uint = 0x4c8;
pub const XK_kana_NO: libc::c_uint = 0x4c9;
pub const XK_kana_HA: libc::c_uint = 0x4ca;
pub const XK_kana_HI: libc::c_uint = 0x4cb;
pub const XK_kana_FU: libc::c_uint = 0x4cc;
pub const XK_kana_HU: libc::c_uint = 0x4cc;
pub const XK_kana_HE: libc::c_uint = 0x4cd;
pub const XK_kana_HO: libc::c_uint = 0x4ce;
pub const XK_kana_MA: libc::c_uint = 0x4cf;
pub const XK_kana_MI: libc::c_uint = 0x4d0;
pub const XK_kana_MU: libc::c_uint = 0x4d1;
pub const XK_kana_ME: libc::c_uint = 0x4d2;
pub const XK_kana_MO: libc::c_uint = 0x4d3;
pub const XK_kana_YA: libc::c_uint = 0x4d4;
pub const XK_kana_YU: libc::c_uint = 0x4d5;
pub const XK_kana_YO: libc::c_uint = 0x4d6;
pub const XK_kana_RA: libc::c_uint = 0x4d7;
pub const XK_kana_RI: libc::c_uint = 0x4d8;
pub const XK_kana_RU: libc::c_uint = 0x4d9;
pub const XK_kana_RE: libc::c_uint = 0x4da;
pub const XK_kana_RO: libc::c_uint = 0x4db;
pub const XK_kana_WA: libc::c_uint = 0x4dc;
pub const XK_kana_N: libc::c_uint = 0x4dd;
pub const XK_voicedsound: libc::c_uint = 0x4de;
pub const XK_semivoicedsound: libc::c_uint = 0x4df;
pub const XK_kana_switch: libc::c_uint = 0xFF7E;
pub const XK_Arabic_comma: libc::c_uint = 0x5ac;
pub const XK_Arabic_semicolon: libc::c_uint = 0x5bb;
pub const XK_Arabic_question_mark: libc::c_uint = 0x5bf;
pub const XK_Arabic_hamza: libc::c_uint = 0x5c1;
pub const XK_Arabic_maddaonalef: libc::c_uint = 0x5c2;
pub const XK_Arabic_hamzaonalef: libc::c_uint = 0x5c3;
pub const XK_Arabic_hamzaonwaw: libc::c_uint = 0x5c4;
pub const XK_Arabic_hamzaunderalef: libc::c_uint = 0x5c5;
pub const XK_Arabic_hamzaonyeh: libc::c_uint = 0x5c6;
pub const XK_Arabic_alef: libc::c_uint = 0x5c7;
pub const XK_Arabic_beh: libc::c_uint = 0x5c8;
pub const XK_Arabic_tehmarbuta: libc::c_uint = 0x5c9;
pub const XK_Arabic_teh: libc::c_uint = 0x5ca;
pub const XK_Arabic_theh: libc::c_uint = 0x5cb;
pub const XK_Arabic_jeem: libc::c_uint = 0x5cc;
pub const XK_Arabic_hah: libc::c_uint = 0x5cd;
pub const XK_Arabic_khah: libc::c_uint = 0x5ce;
pub const XK_Arabic_dal: libc::c_uint = 0x5cf;
pub const XK_Arabic_thal: libc::c_uint = 0x5d0;
pub const XK_Arabic_ra: libc::c_uint = 0x5d1;
pub const XK_Arabic_zain: libc::c_uint = 0x5d2;
pub const XK_Arabic_seen: libc::c_uint = 0x5d3;
pub const XK_Arabic_sheen: libc::c_uint = 0x5d4;
pub const XK_Arabic_sad: libc::c_uint = 0x5d5;
pub const XK_Arabic_dad: libc::c_uint = 0x5d6;
pub const XK_Arabic_tah: libc::c_uint = 0x5d7;
pub const XK_Arabic_zah: libc::c_uint = 0x5d8;
pub const XK_Arabic_ain: libc::c_uint = 0x5d9;
pub const XK_Arabic_ghain: libc::c_uint = 0x5da;
pub const XK_Arabic_tatweel: libc::c_uint = 0x5e0;
pub const XK_Arabic_feh: libc::c_uint = 0x5e1;
pub const XK_Arabic_qaf: libc::c_uint = 0x5e2;
pub const XK_Arabic_kaf: libc::c_uint = 0x5e3;
pub const XK_Arabic_lam: libc::c_uint = 0x5e4;
pub const XK_Arabic_meem: libc::c_uint = 0x5e5;
pub const XK_Arabic_noon: libc::c_uint = 0x5e6;
pub const XK_Arabic_ha: libc::c_uint = 0x5e7;
pub const XK_Arabic_heh: libc::c_uint = 0x5e7;
pub const XK_Arabic_waw: libc::c_uint = 0x5e8;
pub const XK_Arabic_alefmaksura: libc::c_uint = 0x5e9;
pub const XK_Arabic_yeh: libc::c_uint = 0x5ea;
pub const XK_Arabic_fathatan: libc::c_uint = 0x5eb;
pub const XK_Arabic_dammatan: libc::c_uint = 0x5ec;
pub const XK_Arabic_kasratan: libc::c_uint = 0x5ed;
pub const XK_Arabic_fatha: libc::c_uint = 0x5ee;
pub const XK_Arabic_damma: libc::c_uint = 0x5ef;
pub const XK_Arabic_kasra: libc::c_uint = 0x5f0;
pub const XK_Arabic_shadda: libc::c_uint = 0x5f1;
pub const XK_Arabic_sukun: libc::c_uint = 0x5f2;
pub const XK_Arabic_switch: libc::c_uint = 0xFF7E;
pub const XK_Serbian_dje: libc::c_uint = 0x6a1;
pub const XK_Macedonia_gje: libc::c_uint = 0x6a2;
pub const XK_Cyrillic_io: libc::c_uint = 0x6a3;
pub const XK_Ukrainian_ie: libc::c_uint = 0x6a4;
pub const XK_Ukranian_je: libc::c_uint = 0x6a4;
pub const XK_Macedonia_dse: libc::c_uint = 0x6a5;
pub const XK_Ukrainian_i: libc::c_uint = 0x6a6;
pub const XK_Ukranian_i: libc::c_uint = 0x6a6;
pub const XK_Ukrainian_yi: libc::c_uint = 0x6a7;
pub const XK_Ukranian_yi: libc::c_uint = 0x6a7;
pub const XK_Cyrillic_je: libc::c_uint = 0x6a8;
pub const XK_Serbian_je: libc::c_uint = 0x6a8;
pub const XK_Cyrillic_lje: libc::c_uint = 0x6a9;
pub const XK_Serbian_lje: libc::c_uint = 0x6a9;
pub const XK_Cyrillic_nje: libc::c_uint = 0x6aa;
pub const XK_Serbian_nje: libc::c_uint = 0x6aa;
pub const XK_Serbian_tshe: libc::c_uint = 0x6ab;
pub const XK_Macedonia_kje: libc::c_uint = 0x6ac;
pub const XK_Byelorussian_shortu: libc::c_uint = 0x6ae;
pub const XK_Cyrillic_dzhe: libc::c_uint = 0x6af;
pub const XK_Serbian_dze: libc::c_uint = 0x6af;
pub const XK_numerosign: libc::c_uint = 0x6b0;
pub const XK_Serbian_DJE: libc::c_uint = 0x6b1;
pub const XK_Macedonia_GJE: libc::c_uint = 0x6b2;
pub const XK_Cyrillic_IO: libc::c_uint = 0x6b3;
pub const XK_Ukrainian_IE: libc::c_uint = 0x6b4;
pub const XK_Ukranian_JE: libc::c_uint = 0x6b4;
pub const XK_Macedonia_DSE: libc::c_uint = 0x6b5;
pub const XK_Ukrainian_I: libc::c_uint = 0x6b6;
pub const XK_Ukranian_I: libc::c_uint = 0x6b6;
pub const XK_Ukrainian_YI: libc::c_uint = 0x6b7;
pub const XK_Ukranian_YI: libc::c_uint = 0x6b7;
pub const XK_Cyrillic_JE: libc::c_uint = 0x6b8;
pub const XK_Serbian_JE: libc::c_uint = 0x6b8;
pub const XK_Cyrillic_LJE: libc::c_uint = 0x6b9;
pub const XK_Serbian_LJE: libc::c_uint = 0x6b9;
pub const XK_Cyrillic_NJE: libc::c_uint = 0x6ba;
pub const XK_Serbian_NJE: libc::c_uint = 0x6ba;
pub const XK_Serbian_TSHE: libc::c_uint = 0x6bb;
pub const XK_Macedonia_KJE: libc::c_uint = 0x6bc;
pub const XK_Byelorussian_SHORTU: libc::c_uint = 0x6be;
pub const XK_Cyrillic_DZHE: libc::c_uint = 0x6bf;
pub const XK_Serbian_DZE: libc::c_uint = 0x6bf;
pub const XK_Cyrillic_yu: libc::c_uint = 0x6c0;
pub const XK_Cyrillic_a: libc::c_uint = 0x6c1;
pub const XK_Cyrillic_be: libc::c_uint = 0x6c2;
pub const XK_Cyrillic_tse: libc::c_uint = 0x6c3;
pub const XK_Cyrillic_de: libc::c_uint = 0x6c4;
pub const XK_Cyrillic_ie: libc::c_uint = 0x6c5;
pub const XK_Cyrillic_ef: libc::c_uint = 0x6c6;
pub const XK_Cyrillic_ghe: libc::c_uint = 0x6c7;
pub const XK_Cyrillic_ha: libc::c_uint = 0x6c8;
pub const XK_Cyrillic_i: libc::c_uint = 0x6c9;
pub const XK_Cyrillic_shorti: libc::c_uint = 0x6ca;
pub const XK_Cyrillic_ka: libc::c_uint = 0x6cb;
pub const XK_Cyrillic_el: libc::c_uint = 0x6cc;
pub const XK_Cyrillic_em: libc::c_uint = 0x6cd;
pub const XK_Cyrillic_en: libc::c_uint = 0x6ce;
pub const XK_Cyrillic_o: libc::c_uint = 0x6cf;
pub const XK_Cyrillic_pe: libc::c_uint = 0x6d0;
pub const XK_Cyrillic_ya: libc::c_uint = 0x6d1;
pub const XK_Cyrillic_er: libc::c_uint = 0x6d2;
pub const XK_Cyrillic_es: libc::c_uint = 0x6d3;
pub const XK_Cyrillic_te: libc::c_uint = 0x6d4;
pub const XK_Cyrillic_u: libc::c_uint = 0x6d5;
pub const XK_Cyrillic_zhe: libc::c_uint = 0x6d6;
pub const XK_Cyrillic_ve: libc::c_uint = 0x6d7;
pub const XK_Cyrillic_softsign: libc::c_uint = 0x6d8;
pub const XK_Cyrillic_yeru: libc::c_uint = 0x6d9;
pub const XK_Cyrillic_ze: libc::c_uint = 0x6da;
pub const XK_Cyrillic_sha: libc::c_uint = 0x6db;
pub const XK_Cyrillic_e: libc::c_uint = 0x6dc;
pub const XK_Cyrillic_shcha: libc::c_uint = 0x6dd;
pub const XK_Cyrillic_che: libc::c_uint = 0x6de;
pub const XK_Cyrillic_hardsign: libc::c_uint = 0x6df;
pub const XK_Cyrillic_YU: libc::c_uint = 0x6e0;
pub const XK_Cyrillic_A: libc::c_uint = 0x6e1;
pub const XK_Cyrillic_BE: libc::c_uint = 0x6e2;
pub const XK_Cyrillic_TSE: libc::c_uint = 0x6e3;
pub const XK_Cyrillic_DE: libc::c_uint = 0x6e4;
pub const XK_Cyrillic_IE: libc::c_uint = 0x6e5;
pub const XK_Cyrillic_EF: libc::c_uint = 0x6e6;
pub const XK_Cyrillic_GHE: libc::c_uint = 0x6e7;
pub const XK_Cyrillic_HA: libc::c_uint = 0x6e8;
pub const XK_Cyrillic_I: libc::c_uint = 0x6e9;
pub const XK_Cyrillic_SHORTI: libc::c_uint = 0x6ea;
pub const XK_Cyrillic_KA: libc::c_uint = 0x6eb;
pub const XK_Cyrillic_EL: libc::c_uint = 0x6ec;
pub const XK_Cyrillic_EM: libc::c_uint = 0x6ed;
pub const XK_Cyrillic_EN: libc::c_uint = 0x6ee;
pub const XK_Cyrillic_O: libc::c_uint = 0x6ef;
pub const XK_Cyrillic_PE: libc::c_uint = 0x6f0;
pub const XK_Cyrillic_YA: libc::c_uint = 0x6f1;
pub const XK_Cyrillic_ER: libc::c_uint = 0x6f2;
pub const XK_Cyrillic_ES: libc::c_uint = 0x6f3;
pub const XK_Cyrillic_TE: libc::c_uint = 0x6f4;
pub const XK_Cyrillic_U: libc::c_uint = 0x6f5;
pub const XK_Cyrillic_ZHE: libc::c_uint = 0x6f6;
pub const XK_Cyrillic_VE: libc::c_uint = 0x6f7;
pub const XK_Cyrillic_SOFTSIGN: libc::c_uint = 0x6f8;
pub const XK_Cyrillic_YERU: libc::c_uint = 0x6f9;
pub const XK_Cyrillic_ZE: libc::c_uint = 0x6fa;
pub const XK_Cyrillic_SHA: libc::c_uint = 0x6fb;
pub const XK_Cyrillic_E: libc::c_uint = 0x6fc;
pub const XK_Cyrillic_SHCHA: libc::c_uint = 0x6fd;
pub const XK_Cyrillic_CHE: libc::c_uint = 0x6fe;
pub const XK_Cyrillic_HARDSIGN: libc::c_uint = 0x6ff;
pub const XK_Greek_ALPHAaccent: libc::c_uint = 0x7a1;
pub const XK_Greek_EPSILONaccent: libc::c_uint = 0x7a2;
pub const XK_Greek_ETAaccent: libc::c_uint = 0x7a3;
pub const XK_Greek_IOTAaccent: libc::c_uint = 0x7a4;
pub const XK_Greek_IOTAdiaeresis: libc::c_uint = 0x7a5;
pub const XK_Greek_OMICRONaccent: libc::c_uint = 0x7a7;
pub const XK_Greek_UPSILONaccent: libc::c_uint = 0x7a8;
pub const XK_Greek_UPSILONdieresis: libc::c_uint = 0x7a9;
pub const XK_Greek_OMEGAaccent: libc::c_uint = 0x7ab;
pub const XK_Greek_accentdieresis: libc::c_uint = 0x7ae;
pub const XK_Greek_horizbar: libc::c_uint = 0x7af;
pub const XK_Greek_alphaaccent: libc::c_uint = 0x7b1;
pub const XK_Greek_epsilonaccent: libc::c_uint = 0x7b2;
pub const XK_Greek_etaaccent: libc::c_uint = 0x7b3;
pub const XK_Greek_iotaaccent: libc::c_uint = 0x7b4;
pub const XK_Greek_iotadieresis: libc::c_uint = 0x7b5;
pub const XK_Greek_iotaaccentdieresis: libc::c_uint = 0x7b6;
pub const XK_Greek_omicronaccent: libc::c_uint = 0x7b7;
pub const XK_Greek_upsilonaccent: libc::c_uint = 0x7b8;
pub const XK_Greek_upsilondieresis: libc::c_uint = 0x7b9;
pub const XK_Greek_upsilonaccentdieresis: libc::c_uint = 0x7ba;
pub const XK_Greek_omegaaccent: libc::c_uint = 0x7bb;
pub const XK_Greek_ALPHA: libc::c_uint = 0x7c1;
pub const XK_Greek_BETA: libc::c_uint = 0x7c2;
pub const XK_Greek_GAMMA: libc::c_uint = 0x7c3;
pub const XK_Greek_DELTA: libc::c_uint = 0x7c4;
pub const XK_Greek_EPSILON: libc::c_uint = 0x7c5;
pub const XK_Greek_ZETA: libc::c_uint = 0x7c6;
pub const XK_Greek_ETA: libc::c_uint = 0x7c7;
pub const XK_Greek_THETA: libc::c_uint = 0x7c8;
pub const XK_Greek_IOTA: libc::c_uint = 0x7c9;
pub const XK_Greek_KAPPA: libc::c_uint = 0x7ca;
pub const XK_Greek_LAMDA: libc::c_uint = 0x7cb;
pub const XK_Greek_LAMBDA: libc::c_uint = 0x7cb;
pub const XK_Greek_MU: libc::c_uint = 0x7cc;
pub const XK_Greek_NU: libc::c_uint = 0x7cd;
pub const XK_Greek_XI: libc::c_uint = 0x7ce;
pub const XK_Greek_OMICRON: libc::c_uint = 0x7cf;
pub const XK_Greek_PI: libc::c_uint = 0x7d0;
pub const XK_Greek_RHO: libc::c_uint = 0x7d1;
pub const XK_Greek_SIGMA: libc::c_uint = 0x7d2;
pub const XK_Greek_TAU: libc::c_uint = 0x7d4;
pub const XK_Greek_UPSILON: libc::c_uint = 0x7d5;
pub const XK_Greek_PHI: libc::c_uint = 0x7d6;
pub const XK_Greek_CHI: libc::c_uint = 0x7d7;
pub const XK_Greek_PSI: libc::c_uint = 0x7d8;
pub const XK_Greek_OMEGA: libc::c_uint = 0x7d9;
pub const XK_Greek_alpha: libc::c_uint = 0x7e1;
pub const XK_Greek_beta: libc::c_uint = 0x7e2;
pub const XK_Greek_gamma: libc::c_uint = 0x7e3;
pub const XK_Greek_delta: libc::c_uint = 0x7e4;
pub const XK_Greek_epsilon: libc::c_uint = 0x7e5;
pub const XK_Greek_zeta: libc::c_uint = 0x7e6;
pub const XK_Greek_eta: libc::c_uint = 0x7e7;
pub const XK_Greek_theta: libc::c_uint = 0x7e8;
pub const XK_Greek_iota: libc::c_uint = 0x7e9;
pub const XK_Greek_kappa: libc::c_uint = 0x7ea;
pub const XK_Greek_lamda: libc::c_uint = 0x7eb;
pub const XK_Greek_lambda: libc::c_uint = 0x7eb;
pub const XK_Greek_mu: libc::c_uint = 0x7ec;
pub const XK_Greek_nu: libc::c_uint = 0x7ed;
pub const XK_Greek_xi: libc::c_uint = 0x7ee;
pub const XK_Greek_omicron: libc::c_uint = 0x7ef;
pub const XK_Greek_pi: libc::c_uint = 0x7f0;
pub const XK_Greek_rho: libc::c_uint = 0x7f1;
pub const XK_Greek_sigma: libc::c_uint = 0x7f2;
pub const XK_Greek_finalsmallsigma: libc::c_uint = 0x7f3;
pub const XK_Greek_tau: libc::c_uint = 0x7f4;
pub const XK_Greek_upsilon: libc::c_uint = 0x7f5;
pub const XK_Greek_phi: libc::c_uint = 0x7f6;
pub const XK_Greek_chi: libc::c_uint = 0x7f7;
pub const XK_Greek_psi: libc::c_uint = 0x7f8;
pub const XK_Greek_omega: libc::c_uint = 0x7f9;
pub const XK_Greek_switch: libc::c_uint = 0xFF7E;
pub const XK_leftradical: libc::c_uint = 0x8a1;
pub const XK_topleftradical: libc::c_uint = 0x8a2;
pub const XK_horizconnector: libc::c_uint = 0x8a3;
pub const XK_topintegral: libc::c_uint = 0x8a4;
pub const XK_botintegral: libc::c_uint = 0x8a5;
pub const XK_vertconnector: libc::c_uint = 0x8a6;
pub const XK_topleftsqbracket: libc::c_uint = 0x8a7;
pub const XK_botleftsqbracket: libc::c_uint = 0x8a8;
pub const XK_toprightsqbracket: libc::c_uint = 0x8a9;
pub const XK_botrightsqbracket: libc::c_uint = 0x8aa;
pub const XK_topleftparens: libc::c_uint = 0x8ab;
pub const XK_botleftparens: libc::c_uint = 0x8ac;
pub const XK_toprightparens: libc::c_uint = 0x8ad;
pub const XK_botrightparens: libc::c_uint = 0x8ae;
pub const XK_leftmiddlecurlybrace: libc::c_uint = 0x8af;
pub const XK_rightmiddlecurlybrace: libc::c_uint = 0x8b0;
pub const XK_topleftsummation: libc::c_uint = 0x8b1;
pub const XK_botleftsummation: libc::c_uint = 0x8b2;
pub const XK_topvertsummationconnector: libc::c_uint = 0x8b3;
pub const XK_botvertsummationconnector: libc::c_uint = 0x8b4;
pub const XK_toprightsummation: libc::c_uint = 0x8b5;
pub const XK_botrightsummation: libc::c_uint = 0x8b6;
pub const XK_rightmiddlesummation: libc::c_uint = 0x8b7;
pub const XK_lessthanequal: libc::c_uint = 0x8bc;
pub const XK_notequal: libc::c_uint = 0x8bd;
pub const XK_greaterthanequal: libc::c_uint = 0x8be;
pub const XK_integral: libc::c_uint = 0x8bf;
pub const XK_therefore: libc::c_uint = 0x8c0;
pub const XK_variation: libc::c_uint = 0x8c1;
pub const XK_infinity: libc::c_uint = 0x8c2;
pub const XK_nabla: libc::c_uint = 0x8c5;
pub const XK_approximate: libc::c_uint = 0x8c8;
pub const XK_similarequal: libc::c_uint = 0x8c9;
pub const XK_ifonlyif: libc::c_uint = 0x8cd;
pub const XK_implies: libc::c_uint = 0x8ce;
pub const XK_identical: libc::c_uint = 0x8cf;
pub const XK_radical: libc::c_uint = 0x8d6;
pub const XK_includedin: libc::c_uint = 0x8da;
pub const XK_includes: libc::c_uint = 0x8db;
pub const XK_intersection: libc::c_uint = 0x8dc;
pub const XK_union: libc::c_uint = 0x8dd;
pub const XK_logicaland: libc::c_uint = 0x8de;
pub const XK_logicalor: libc::c_uint = 0x8df;
pub const XK_partialderivative: libc::c_uint = 0x8ef;
pub const XK_function: libc::c_uint = 0x8f6;
pub const XK_leftarrow: libc::c_uint = 0x8fb;
pub const XK_uparrow: libc::c_uint = 0x8fc;
pub const XK_rightarrow: libc::c_uint = 0x8fd;
pub const XK_downarrow: libc::c_uint = 0x8fe;
pub const XK_blank: libc::c_uint = 0x9df;
pub const XK_soliddiamond: libc::c_uint = 0x9e0;
pub const XK_checkerboard: libc::c_uint = 0x9e1;
pub const XK_ht: libc::c_uint = 0x9e2;
pub const XK_ff: libc::c_uint = 0x9e3;
pub const XK_cr: libc::c_uint = 0x9e4;
pub const XK_lf: libc::c_uint = 0x9e5;
pub const XK_nl: libc::c_uint = 0x9e8;
pub const XK_vt: libc::c_uint = 0x9e9;
pub const XK_lowrightcorner: libc::c_uint = 0x9ea;
pub const XK_uprightcorner: libc::c_uint = 0x9eb;
pub const XK_upleftcorner: libc::c_uint = 0x9ec;
pub const XK_lowleftcorner: libc::c_uint = 0x9ed;
pub const XK_crossinglines: libc::c_uint = 0x9ee;
pub const XK_horizlinescan1: libc::c_uint = 0x9ef;
pub const XK_horizlinescan3: libc::c_uint = 0x9f0;
pub const XK_horizlinescan5: libc::c_uint = 0x9f1;
pub const XK_horizlinescan7: libc::c_uint = 0x9f2;
pub const XK_horizlinescan9: libc::c_uint = 0x9f3;
pub const XK_leftt: libc::c_uint = 0x9f4;
pub const XK_rightt: libc::c_uint = 0x9f5;
pub const XK_bott: libc::c_uint = 0x9f6;
pub const XK_topt: libc::c_uint = 0x9f7;
pub const XK_vertbar: libc::c_uint = 0x9f8;
pub const XK_emspace: libc::c_uint = 0xaa1;
pub const XK_enspace: libc::c_uint = 0xaa2;
pub const XK_em3space: libc::c_uint = 0xaa3;
pub const XK_em4space: libc::c_uint = 0xaa4;
pub const XK_digitspace: libc::c_uint = 0xaa5;
pub const XK_punctspace: libc::c_uint = 0xaa6;
pub const XK_thinspace: libc::c_uint = 0xaa7;
pub const XK_hairspace: libc::c_uint = 0xaa8;
pub const XK_emdash: libc::c_uint = 0xaa9;
pub const XK_endash: libc::c_uint = 0xaaa;
pub const XK_signifblank: libc::c_uint = 0xaac;
pub const XK_ellipsis: libc::c_uint = 0xaae;
pub const XK_doubbaselinedot: libc::c_uint = 0xaaf;
pub const XK_onethird: libc::c_uint = 0xab0;
pub const XK_twothirds: libc::c_uint = 0xab1;
pub const XK_onefifth: libc::c_uint = 0xab2;
pub const XK_twofifths: libc::c_uint = 0xab3;
pub const XK_threefifths: libc::c_uint = 0xab4;
pub const XK_fourfifths: libc::c_uint = 0xab5;
pub const XK_onesixth: libc::c_uint = 0xab6;
pub const XK_fivesixths: libc::c_uint = 0xab7;
pub const XK_careof: libc::c_uint = 0xab8;
pub const XK_figdash: libc::c_uint = 0xabb;
pub const XK_leftanglebracket: libc::c_uint = 0xabc;
pub const XK_decimalpoint: libc::c_uint = 0xabd;
pub const XK_rightanglebracket: libc::c_uint = 0xabe;
pub const XK_marker: libc::c_uint = 0xabf;
pub const XK_oneeighth: libc::c_uint = 0xac3;
pub const XK_threeeighths: libc::c_uint = 0xac4;
pub const XK_fiveeighths: libc::c_uint = 0xac5;
pub const XK_seveneighths: libc::c_uint = 0xac6;
pub const XK_trademark: libc::c_uint = 0xac9;
pub const XK_signaturemark: libc::c_uint = 0xaca;
pub const XK_trademarkincircle: libc::c_uint = 0xacb;
pub const XK_leftopentriangle: libc::c_uint = 0xacc;
pub const XK_rightopentriangle: libc::c_uint = 0xacd;
pub const XK_emopencircle: libc::c_uint = 0xace;
pub const XK_emopenrectangle: libc::c_uint = 0xacf;
pub const XK_leftsinglequotemark: libc::c_uint = 0xad0;
pub const XK_rightsinglequotemark: libc::c_uint = 0xad1;
pub const XK_leftdoublequotemark: libc::c_uint = 0xad2;
pub const XK_rightdoublequotemark: libc::c_uint = 0xad3;
pub const XK_prescription: libc::c_uint = 0xad4;
pub const XK_minutes: libc::c_uint = 0xad6;
pub const XK_seconds: libc::c_uint = 0xad7;
pub const XK_latincross: libc::c_uint = 0xad9;
pub const XK_hexagram: libc::c_uint = 0xada;
pub const XK_filledrectbullet: libc::c_uint = 0xadb;
pub const XK_filledlefttribullet: libc::c_uint = 0xadc;
pub const XK_filledrighttribullet: libc::c_uint = 0xadd;
pub const XK_emfilledcircle: libc::c_uint = 0xade;
pub const XK_emfilledrect: libc::c_uint = 0xadf;
pub const XK_enopencircbullet: libc::c_uint = 0xae0;
pub const XK_enopensquarebullet: libc::c_uint = 0xae1;
pub const XK_openrectbullet: libc::c_uint = 0xae2;
pub const XK_opentribulletup: libc::c_uint = 0xae3;
pub const XK_opentribulletdown: libc::c_uint = 0xae4;
pub const XK_openstar: libc::c_uint = 0xae5;
pub const XK_enfilledcircbullet: libc::c_uint = 0xae6;
pub const XK_enfilledsqbullet: libc::c_uint = 0xae7;
pub const XK_filledtribulletup: libc::c_uint = 0xae8;
pub const XK_filledtribulletdown: libc::c_uint = 0xae9;
pub const XK_leftpointer: libc::c_uint = 0xaea;
pub const XK_rightpointer: libc::c_uint = 0xaeb;
pub const XK_club: libc::c_uint = 0xaec;
pub const XK_diamond: libc::c_uint = 0xaed;
pub const XK_heart: libc::c_uint = 0xaee;
pub const XK_maltesecross: libc::c_uint = 0xaf0;
pub const XK_dagger: libc::c_uint = 0xaf1;
pub const XK_doubledagger: libc::c_uint = 0xaf2;
pub const XK_checkmark: libc::c_uint = 0xaf3;
pub const XK_ballotcross: libc::c_uint = 0xaf4;
pub const XK_musicalsharp: libc::c_uint = 0xaf5;
pub const XK_musicalflat: libc::c_uint = 0xaf6;
pub const XK_malesymbol: libc::c_uint = 0xaf7;
pub const XK_femalesymbol: libc::c_uint = 0xaf8;
pub const XK_telephone: libc::c_uint = 0xaf9;
pub const XK_telephonerecorder: libc::c_uint = 0xafa;
pub const XK_phonographcopyright: libc::c_uint = 0xafb;
pub const XK_caret: libc::c_uint = 0xafc;
pub const XK_singlelowquotemark: libc::c_uint = 0xafd;
pub const XK_doublelowquotemark: libc::c_uint = 0xafe;
pub const XK_cursor: libc::c_uint = 0xaff;
pub const XK_leftcaret: libc::c_uint = 0xba3;
pub const XK_rightcaret: libc::c_uint = 0xba6;
pub const XK_downcaret: libc::c_uint = 0xba8;
pub const XK_upcaret: libc::c_uint = 0xba9;
pub const XK_overbar: libc::c_uint = 0xbc0;
pub const XK_downtack: libc::c_uint = 0xbc2;
pub const XK_upshoe: libc::c_uint = 0xbc3;
pub const XK_downstile: libc::c_uint = 0xbc4;
pub const XK_underbar: libc::c_uint = 0xbc6;
pub const XK_jot: libc::c_uint = 0xbca;
pub const XK_quad: libc::c_uint = 0xbcc;
pub const XK_uptack: libc::c_uint = 0xbce;
pub const XK_circle: libc::c_uint = 0xbcf;
pub const XK_upstile: libc::c_uint = 0xbd3;
pub const XK_downshoe: libc::c_uint = 0xbd6;
pub const XK_rightshoe: libc::c_uint = 0xbd8;
pub const XK_leftshoe: libc::c_uint = 0xbda;
pub const XK_lefttack: libc::c_uint = 0xbdc;
pub const XK_righttack: libc::c_uint = 0xbfc;
pub const XK_hebrew_doublelowline: libc::c_uint = 0xcdf;
pub const XK_hebrew_aleph: libc::c_uint = 0xce0;
pub const XK_hebrew_bet: libc::c_uint = 0xce1;
pub const XK_hebrew_beth: libc::c_uint = 0xce1;
pub const XK_hebrew_gimel: libc::c_uint = 0xce2;
pub const XK_hebrew_gimmel: libc::c_uint = 0xce2;
pub const XK_hebrew_dalet: libc::c_uint = 0xce3;
pub const XK_hebrew_daleth: libc::c_uint = 0xce3;
pub const XK_hebrew_he: libc::c_uint = 0xce4;
pub const XK_hebrew_waw: libc::c_uint = 0xce5;
pub const XK_hebrew_zain: libc::c_uint = 0xce6;
pub const XK_hebrew_zayin: libc::c_uint = 0xce6;
pub const XK_hebrew_chet: libc::c_uint = 0xce7;
pub const XK_hebrew_het: libc::c_uint = 0xce7;
pub const XK_hebrew_tet: libc::c_uint = 0xce8;
pub const XK_hebrew_teth: libc::c_uint = 0xce8;
pub const XK_hebrew_yod: libc::c_uint = 0xce9;
pub const XK_hebrew_finalkaph: libc::c_uint = 0xcea;
pub const XK_hebrew_kaph: libc::c_uint = 0xceb;
pub const XK_hebrew_lamed: libc::c_uint = 0xcec;
pub const XK_hebrew_finalmem: libc::c_uint = 0xced;
pub const XK_hebrew_mem: libc::c_uint = 0xcee;
pub const XK_hebrew_finalnun: libc::c_uint = 0xcef;
pub const XK_hebrew_nun: libc::c_uint = 0xcf0;
pub const XK_hebrew_samech: libc::c_uint = 0xcf1;
pub const XK_hebrew_samekh: libc::c_uint = 0xcf1;
pub const XK_hebrew_ayin: libc::c_uint = 0xcf2;
pub const XK_hebrew_finalpe: libc::c_uint = 0xcf3;
pub const XK_hebrew_pe: libc::c_uint = 0xcf4;
pub const XK_hebrew_finalzade: libc::c_uint = 0xcf5;
pub const XK_hebrew_finalzadi: libc::c_uint = 0xcf5;
pub const XK_hebrew_zade: libc::c_uint = 0xcf6;
pub const XK_hebrew_zadi: libc::c_uint = 0xcf6;
pub const XK_hebrew_qoph: libc::c_uint = 0xcf7;
pub const XK_hebrew_kuf: libc::c_uint = 0xcf7;
pub const XK_hebrew_resh: libc::c_uint = 0xcf8;
pub const XK_hebrew_shin: libc::c_uint = 0xcf9;
pub const XK_hebrew_taw: libc::c_uint = 0xcfa;
pub const XK_hebrew_taf: libc::c_uint = 0xcfa;
pub const XK_Hebrew_switch: libc::c_uint = 0xFF7E;



#[repr(C)]
pub struct XSetWindowAttributes {
    pub background_pixmap: Pixmap,
    pub background_pixel: libc::c_ulong,
    pub border_pixmap: Pixmap,
    pub border_pixel: libc::c_ulong,
    pub bit_gravity: libc::c_int,
    pub win_gravity: libc::c_int,
    pub backing_store: libc::c_int,
    pub backing_planes: libc::c_ulong,
    pub backing_pixel: libc::c_long,
    pub save_under: Bool,
    pub event_mask: libc::c_long,
    pub do_not_propagate_mask: libc::c_long,
    pub override_redirect: Bool,
    pub colormap: Colormap,
    pub cursor: Cursor,
}

#[repr(C)]
pub struct XEvent {
    pub type_: libc::c_int,
    pad: [libc::c_long; 24],
}

#[repr(C)]
pub struct XClientMessageEvent {
    pub type_: libc::c_int,
    pub serial: libc::c_ulong,
    pub send_event: Bool,
    pub display: *mut Display,
    pub window: Window,
    pub message_type: Atom,
    pub format: libc::c_int,
    pub l: [libc::c_long; 5],
}

#[repr(C)]
pub struct XResizeRequestEvent {
    pub type_: libc::c_int,
    pub serial: libc::c_ulong,
    pub send_event: Bool,
    pub display: *mut Display,
    pub window: Window,
    pub width: libc::c_int,
    pub height: libc::c_int,
}

#[repr(C)]
pub struct XMotionEvent {
    pub type_: libc::c_int,
    pub serial: libc::c_ulong,
    pub send_event: Bool,
    pub display: *mut Display,
    pub window: Window,
    pub root: Window,
    pub subwindow: Window,
    pub time: Time,
    pub x: libc::c_int,
    pub y: libc::c_int,
    pub x_root: libc::c_int,
    pub y_root: libc::c_int,
    pub state: libc::c_uint,
    pub is_hint: libc::c_char,
    pub same_screen: Bool,
}

#[repr(C)]
pub struct XKeyEvent {
    pub type_: libc::c_int,
    pub serial: libc::c_ulong,
    pub send_event: Bool,
    pub display: *mut Display,
    pub window: Window,
    pub root: Window,
    pub subwindow: Window,
    pub time: Time,
    pub x: libc::c_int,
    pub y: libc::c_int,
    pub x_root: libc::c_int,
    pub y_root: libc::c_int,
    pub state: libc::c_uint,
    pub keycode: libc::c_uint,
    pub same_screen: Bool,
}

#[repr(C)]
pub struct XButtonEvent {
    pub type_: libc::c_int,
    pub serial: libc::c_ulong,
    pub send_event: Bool,
    pub display: *mut Display,
    pub window: Window,
    pub root: Window,
    pub subwindow: Window,
    pub time: Time,
    pub x: libc::c_int,
    pub y: libc::c_int,
    pub x_root: libc::c_int,
    pub y_root: libc::c_int,
    pub state: libc::c_uint,
    pub button: libc::c_uint,
    pub same_screen: Bool,
}

#[repr(C)]
pub struct XConfigureEvent {
    pub type_: libc::c_int,
    pub serial: libc::c_ulong,
    pub send_event: Bool,
    pub display: *mut Display,
    pub event: Window,
    pub window: Window,
    pub x: libc::c_int,
    pub y: libc::c_int,
    pub width: libc::c_int,
    pub height: libc::c_int,
    pub border_width: libc::c_int,
    pub above: Window,
    pub override_redirect: Bool,
}

#[repr(C)]
pub struct XF86VidModeModeInfo {
    pub dotclock: libc::c_uint,
    pub hdisplay: libc::c_ushort,
    pub hsyncstart: libc::c_ushort,
    pub hsyncend: libc::c_ushort,
    pub htotal: libc::c_ushort,
    pub hskew: libc::c_ushort,
    pub vdisplay: libc::c_ushort,
    pub vsyncstart: libc::c_ushort,
    pub vsyncend: libc::c_ushort,
    pub vtotal: libc::c_ushort,
    pub flags: libc::c_uint,
    privsize: libc::c_int,
    private: libc::c_long,
}

#[cfg(feature = "headless")]
#[link(name = "OSMesa")]
extern "C" {
    pub fn OSMesaCreateContext(format: libc::c_uint, sharelist: OSMesaContext) -> OSMesaContext;
    pub fn OSMesaCreateContextExt(format: libc::c_uint, depthBits: libc::c_int,
    	stencilBits: libc::c_int, accumBits: libc::c_int, sharelist: OSMesaContext)
    	-> OSMesaContext;
    pub fn OSMesaDestroyContext(ctx: OSMesaContext);
    pub fn OSMesaMakeCurrent(ctx: OSMesaContext, buffer: *mut libc::c_void, type_: libc::c_uint,
    	width: libc::c_int, height: libc::c_int) -> libc::c_uchar;
    pub fn OSMesaGetCurrentContext() -> OSMesaContext;
    pub fn OSMesaPixelStore(pname: libc::c_int, value: libc::c_int);
    pub fn OSMesaGetIntegerv(pname: libc::c_int, value: *mut libc::c_int);
    pub fn OSMesaGetDepthBuffer(c: OSMesaContext, width: *mut libc::c_int,
    	height: *mut libc::c_int, bytesPerValue: *mut libc::c_int,
    	buffer: *mut *mut libc::c_void);
    pub fn OSMesaGetColorBuffer(c: OSMesaContext, width: *mut libc::c_int,
    	height: *mut libc::c_int, format: *mut libc::c_int, buffer: *mut *mut libc::c_void);
    pub fn OSMesaGetProcAddress(funcName: *const libc::c_char) -> *const libc::c_void;
    pub fn OSMesaColorClamp(enable: libc::c_uchar);
}

#[cfg(feature = "window")]
#[link(name = "GL")]
#[link(name = "X11")]
#[link(name = "Xxf86vm")]
#[link(name = "Xcursor")]
extern "C" {
    pub fn XCloseDisplay(display: *mut Display);
    pub fn XCheckMaskEvent(display: *mut Display, event_mask: libc::c_long,
        event_return: *mut XEvent) -> Bool;
    pub fn XCheckTypedEvent(display: *mut Display, event_type: libc::c_int,
        event_return: *mut XEvent) -> Bool;
    pub fn XCreateColormap(display: *mut Display, w: Window,
        visual: *mut Visual, alloc: libc::c_int) -> Colormap;
    pub fn XCreateWindow(display: *mut Display, parent: Window, x: libc::c_int,
        y: libc::c_int, width: libc::c_uint, height: libc::c_uint,
        border_width: libc::c_uint, depth: libc::c_int, class: libc::c_uint,
        visual: *mut Visual, valuemask: libc::c_ulong,
        attributes: *mut XSetWindowAttributes) -> Window;
    pub fn XDefaultRootWindow(display: *mut Display) -> Window;
    pub fn XDefaultScreen(display: *mut Display) -> libc::c_int;
    pub fn XDestroyWindow(display: *mut Display, w: Window);
    pub fn XFilterEvent(event: *mut XEvent, w: Window) -> Bool;
    pub fn XFlush(display: *mut Display);
    pub fn XFree(data: *const libc::c_void);
    pub fn XGetGeometry(display: *mut Display, d: Drawable, root_return: *mut Window,
        x_return: *mut libc::c_int, y_return: *mut libc::c_int,
        width_return: *mut libc::c_uint, height_return: *mut libc::c_uint,
        border_width_return: *mut libc::c_uint, depth_return: *mut libc::c_uint) -> Status;
    pub fn XSendEvent(display: *mut Display, window: Window, propagate: Bool,
                      event_mask: libc::c_long, event_send: *mut XEvent) -> Status;
    pub fn XInternAtom(display: *mut Display, atom_name: *const libc::c_char,
        only_if_exists: Bool) -> Atom;
    pub fn XKeycodeToKeysym(display: *mut Display, keycode: KeyCode,
        index: libc::c_int) -> KeySym;
    pub fn XMoveWindow(display: *mut Display, w: Window, x: libc::c_int, y: libc::c_int);
    pub fn XMapWindow(display: *mut Display, w: Window);
    pub fn XMapRaised(display: *mut Display, w: Window);
    pub fn XUnmapWindow(display: *mut Display, w: Window);
    pub fn XNextEvent(display: *mut Display, event_return: *mut XEvent);
    pub fn XInitThreads() -> Status;
    pub fn XOpenDisplay(display_name: *const libc::c_char) -> *mut Display;
    pub fn XPeekEvent(display: *mut Display, event_return: *mut XEvent);
    pub fn XRefreshKeyboardMapping(event_map: *const XEvent);
    pub fn XSetWMProtocols(display: *mut Display, w: Window, protocols: *mut Atom,
        count: libc::c_int) -> Status;
    pub fn XStoreName(display: *mut Display, w: Window, window_name: *const libc::c_char);
    pub fn XScreenCount(display: *mut Display) -> libc::c_int;
    pub fn XScreenOfDisplay(display: *mut Display, screen_number: libc::c_int) -> *const Screen;
    pub fn XWidthOfScreen(screen: *const Screen) -> libc::c_int;
    pub fn XHeightOfScreen(screen: *const Screen) -> libc::c_int;

    pub fn XCloseIM(im: XIM) -> Status;
    pub fn XOpenIM(display: *mut Display, db: XrmDatabase, res_name: *mut libc::c_char,
        res_class: *mut libc::c_char) -> XIM;

    // TODO: this is a vararg function
    //pub fn XCreateIC(im: XIM; .) -> XIC;
    pub fn XCreateIC(im: XIM, a: *const libc::c_char, b: libc::c_long, c: *const libc::c_char,
        d: Window, e: *const ()) -> XIC;
    pub fn XDestroyIC(ic: XIC);
    pub fn XSetICFocus(ic: XIC);
    pub fn XUnsetICFocus(ic: XIC);

    pub fn Xutf8LookupString(ic: XIC, event: *mut XKeyEvent,
        buffer_return: *mut libc::c_char, bytes_buffer: libc::c_int,
        keysym_return: *mut KeySym, status_return: *mut Status) -> libc::c_int;

    pub fn XkbSetDetectableAutoRepeat(dpy: *mut Display, detectable: bool, supported_rtm: *mut bool) -> bool;
    
    pub fn XF86VidModeSwitchToMode(dpy: *mut Display, screen: libc::c_int,
        modeline: *mut XF86VidModeModeInfo) -> Bool;
    pub fn XF86VidModeSetViewPort(dpy: *mut Display, screen: libc::c_int,
        x: libc::c_int, y: libc::c_int) -> Bool;
    pub fn XF86VidModeGetAllModeLines(dpy: *mut Display, screen: libc::c_int,
        modecount_return: *mut libc::c_int, modesinfo: *mut *mut *mut XF86VidModeModeInfo) -> Bool;

    pub fn XcursorLibraryLoadCursor(dpy: *mut Display, name: *const libc::c_char) -> Cursor;
    pub fn XDefineCursor(dby: *mut Display, w: Window, cursor: Cursor);
}

/*
GLXFBConfig *glXGetFBConfigs (Display *dpy, int screen, int *nelements);
int glXGetFBConfigAttrib (Display *dpy, GLXFBConfig config, int attribute, int *value);
GLXWindow glXCreateWindow (Display *dpy, GLXFBConfig config, Window win, const int *attrib_list);
void glXDestroyWindow (Display *dpy, GLXWindow win);
GLXPixmap glXCreatePixmap (Display *dpy, GLXFBConfig config, Pixmap pixmap, const int *attrib_list);
void glXDestroyPixmap (Display *dpy, GLXPixmap pixmap);
GLXPbuffer glXCreatePbuffer (Display *dpy, GLXFBConfig config, const int *attrib_list);
void glXDestroyPbuffer (Display *dpy, GLXPbuffer pbuf);
void glXQueryDrawable (Display *dpy, GLXDrawable draw, int attribute, unsigned int *value);
GLXContext glXCreateNewContext (Display *dpy, GLXFBConfig config, int render_type, GLXContext share_list, Bool direct);
Bool glXMakeContextCurrent (Display *dpy, GLXDrawable draw, GLXDrawable read, GLXContext ctx);
GLXDrawable glXGetCurrentReadDrawable (void);
int glXQueryContext (Display *dpy, GLXContext ctx, int attribute, int *value);
void glXSelectEvent (Display *dpy, GLXDrawable draw, unsigned long event_mask);
void glXGetSelectedEvent (Display *dpy, GLXDrawable draw, unsigned long *event_mask);


extern void glXCopyContext( Display *dpy, GLXContext src, GLXContext dst,
                unsigned long mask );


extern GLXPixmap glXCreateGLXPixmap( Display *dpy, XVisualInfo *visual,
                     Pixmap pixmap );

extern void glXDestroyGLXPixmap( Display *dpy, GLXPixmap pixmap );

extern Bool glXQueryExtension( Display *dpy, int *errorb, int *event );

extern Bool glXQueryVersion( Display *dpy, int *maj, int *min );

extern Bool glXIsDirect( Display *dpy, GLXContext ctx );

extern int glXGetConfig( Display *dpy, XVisualInfo *visual,
             int attrib, int *value );

extern GLXContext glXGetCurrentContext( void );

extern GLXDrawable glXGetCurrentDrawable( void );

extern void glXWaitGL( void );

extern void glXWaitX( void );

extern void glXUseXFont( Font font, int first, int count, int list );

extern const char *glXQueryExtensionsString( Display *dpy, int screen );

extern const char *glXQueryServerString( Display *dpy, int screen, int name );

extern const char *glXGetClientString( Display *dpy, int name );

extern Display *glXGetCurrentDisplay( void );

*/
