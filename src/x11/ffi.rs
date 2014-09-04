#![allow(dead_code)]
#![allow(non_snake_case_functions)]
#![allow(non_camel_case_types)]

use libc;

pub type Atom = libc::c_ulong;
pub type Bool = libc::c_int;
pub type Colormap = XID;
pub type Cursor = XID;
pub type Display = ();
pub type Drawable = XID;    // TODO: not sure
pub type GLXContext = *const ();
pub type GLXContextID = XID;
pub type GLXDrawable = XID;
pub type GLXFBConfig = *const ();
pub type GLXPbuffer = XID;
pub type GLXPixmap = XID;
pub type GLXWindow = XID;
pub type KeyCode = libc::c_ulong;
pub type KeySym = XID;
pub type Pixmap = XID;
pub type Status = libc::c_int;  // TODO: not sure
pub type Time = libc::c_ulong;
pub type Visual = ();   // TODO: not sure
pub type VisualID = libc::c_ulong;   // TODO: not sure
pub type Window = XID;
pub type XrmDatabase = *const ();       // TODO: not sure
pub type XIC = *mut ();
pub type XID = uint;
pub type XIM = *mut ();

pub static AllocNone: libc::c_int = 0;
pub static AllocAll: libc::c_int = 1;

pub static Button1: libc::c_uint = 1;
pub static Button2: libc::c_uint = 2;
pub static Button3: libc::c_uint = 3;
pub static Button4: libc::c_uint = 4;
pub static Button5: libc::c_uint = 5;

pub static InputOutput: libc::c_uint = 1;
pub static InputOnly: libc::c_uint = 2;

pub static CWBackPixmap: libc::c_ulong = (1<<0);
pub static CWBackPixel: libc::c_ulong = (1<<1);
pub static CWBorderPixmap: libc::c_ulong = (1<<2);
pub static CWBorderPixel: libc::c_ulong = (1<<3);
pub static CWBitGravity: libc::c_ulong = (1<<4);
pub static CWWinGravity: libc::c_ulong = (1<<5);
pub static CWBackingStore: libc::c_ulong = (1<<6);
pub static CWBackingPlanes: libc::c_ulong = (1<<7);
pub static CWBackingPixel: libc::c_ulong = (1<<8);
pub static CWOverrideRedirect: libc::c_ulong = (1<<9);
pub static CWSaveUnder: libc::c_ulong = (1<<10);
pub static CWEventMask: libc::c_ulong = (1<<11);
pub static CWDontPropagate: libc::c_ulong = (1<<12);
pub static CWColormap: libc::c_ulong = (1<<13);
pub static CWCursor: libc::c_ulong = (1<<14);

pub static NoEventMask: libc::c_long = 0;
pub static KeyPressMask: libc::c_long = (1<<0);
pub static KeyReleaseMask: libc::c_long = (1<<1);
pub static ButtonPressMask: libc::c_long = (1<<2);
pub static ButtonReleaseMask: libc::c_long = (1<<3);
pub static EnterWindowMask: libc::c_long = (1<<4);
pub static LeaveWindowMask: libc::c_long = (1<<5);
pub static PointerMotionMask: libc::c_long = (1<<6);
pub static PointerMotionHintMask: libc::c_long = (1<<7);
pub static Button1MotionMask: libc::c_long = (1<<8);
pub static Button2MotionMask: libc::c_long = (1<<9);
pub static Button3MotionMask: libc::c_long = (1<<10);
pub static Button4MotionMask: libc::c_long = (1<<11);
pub static Button5MotionMask: libc::c_long = (1<<12);
pub static ButtonMotionMask: libc::c_long = (1<<13);
pub static KeymapStateMask: libc::c_long = (1<<14);
pub static ExposureMask: libc::c_long = (1<<15);
pub static VisibilityChangeMask: libc::c_long = (1<<16);
pub static StructureNotifyMask: libc::c_long = (1<<17);
pub static ResizeRedirectMask: libc::c_long = (1<<18);
pub static SubstructureNotifyMask: libc::c_long = (1<<19);
pub static SubstructureRedirectMask: libc::c_long = (1<<20);
pub static FocusChangeMask: libc::c_long = (1<<21);
pub static PropertyChangeMask: libc::c_long = (1<<22);
pub static ColormapChangeMask: libc::c_long = (1<<23);
pub static OwnerGrabButtonMask: libc::c_long = (1<<24);

pub static KeyPress: libc::c_int = 2;
pub static KeyRelease: libc::c_int = 3;
pub static ButtonPress: libc::c_int = 4;
pub static ButtonRelease: libc::c_int = 5;
pub static MotionNotify: libc::c_int = 6;
pub static EnterNotify: libc::c_int = 7;
pub static LeaveNotify: libc::c_int = 8;
pub static FocusIn: libc::c_int = 9;
pub static FocusOut: libc::c_int = 10;
pub static KeymapNotify: libc::c_int = 11;
pub static Expose: libc::c_int = 12;
pub static GraphicsExpose: libc::c_int = 13;
pub static NoExpose: libc::c_int = 14;
pub static VisibilityNotify: libc::c_int = 15;
pub static CreateNotify: libc::c_int = 16;
pub static DestroyNotify: libc::c_int = 17;
pub static UnmapNotify: libc::c_int = 18;
pub static MapNotify: libc::c_int = 19;
pub static MapRequest: libc::c_int = 20;
pub static ReparentNotify: libc::c_int = 21;
pub static ConfigureNotify: libc::c_int = 22;
pub static ConfigureRequest: libc::c_int = 23;
pub static GravityNotify: libc::c_int = 24;
pub static ResizeRequest: libc::c_int = 25;
pub static CirculateNotify: libc::c_int = 26;
pub static CirculateRequest: libc::c_int = 27;
pub static PropertyNotify: libc::c_int = 28;
pub static SelectionClear: libc::c_int = 29;
pub static SelectionRequest: libc::c_int = 30;
pub static SelectionNotify: libc::c_int = 31;
pub static ColormapNotify: libc::c_int = 32;
pub static ClientMessage: libc::c_int = 33;
pub static MappingNotify: libc::c_int = 34;

pub static GLX_USE_GL: libc::c_int = 1;
pub static GLX_BUFFER_SIZE: libc::c_int = 2;
pub static GLX_LEVEL: libc::c_int = 3;
pub static GLX_RGBA: libc::c_int = 4;
pub static GLX_DOUBLEBUFFER: libc::c_int = 5;
pub static GLX_STEREO: libc::c_int = 6;
pub static GLX_AUX_BUFFERS: libc::c_int = 7;
pub static GLX_RED_SIZE: libc::c_int = 8;
pub static GLX_GREEN_SIZE: libc::c_int = 9;
pub static GLX_BLUE_SIZE: libc::c_int = 10;
pub static GLX_ALPHA_SIZE: libc::c_int = 11;
pub static GLX_DEPTH_SIZE: libc::c_int = 12;
pub static GLX_STENCIL_SIZE: libc::c_int = 13;
pub static GLX_ACCUM_RED_SIZE: libc::c_int = 14;
pub static GLX_ACCUM_GREEN_SIZE: libc::c_int = 15;
pub static GLX_ACCUM_BLUE_SIZE: libc::c_int = 16;
pub static GLX_ACCUM_ALPHA_SIZE: libc::c_int = 17;
pub static GLX_BAD_SCREEN: libc::c_int = 1;
pub static GLX_BAD_ATTRIBUTE: libc::c_int = 2;
pub static GLX_NO_EXTENSION: libc::c_int = 3;
pub static GLX_BAD_VISUAL: libc::c_int = 4;
pub static GLX_BAD_CONTEXT: libc::c_int = 5;
pub static GLX_BAD_VALUE: libc::c_int = 6;
pub static GLX_BAD_ENUM: libc::c_int = 7;
pub static GLX_VENDOR: libc::c_int = 1;
pub static GLX_VERSION: libc::c_int = 2;
pub static GLX_EXTENSIONS: libc::c_int = 3;
pub static GLX_WINDOW_BIT: libc::c_int = 0x00000001;
pub static GLX_PIXMAP_BIT: libc::c_int = 0x00000002;
pub static GLX_PBUFFER_BIT: libc::c_int = 0x00000004;
pub static GLX_RGBA_BIT: libc::c_int = 0x00000001;
pub static GLX_COLOR_INDEX_BIT: libc::c_int = 0x00000002;
pub static GLX_PBUFFER_CLOBBER_MASK: libc::c_int = 0x08000000;
pub static GLX_FRONT_LEFT_BUFFER_BIT: libc::c_int = 0x00000001;
pub static GLX_FRONT_RIGHT_BUFFER_BIT: libc::c_int = 0x00000002;
pub static GLX_BACK_LEFT_BUFFER_BIT: libc::c_int = 0x00000004;
pub static GLX_BACK_RIGHT_BUFFER_BIT: libc::c_int = 0x00000008;
pub static GLX_AUX_BUFFERS_BIT: libc::c_int = 0x00000010;
pub static GLX_DEPTH_BUFFER_BIT: libc::c_int = 0x00000020;
pub static GLX_STENCIL_BUFFER_BIT: libc::c_int = 0x00000040;
pub static GLX_ACCUM_BUFFER_BIT: libc::c_int = 0x00000080;
pub static GLX_CONFIG_CAVEAT: libc::c_int = 0x20;
pub static GLX_X_VISUAL_TYPE: libc::c_int = 0x22;
pub static GLX_TRANSPARENT_TYPE: libc::c_int = 0x23;
pub static GLX_TRANSPARENT_INDEX_VALUE: libc::c_int = 0x24;
pub static GLX_TRANSPARENT_RED_VALUE: libc::c_int = 0x25;
pub static GLX_TRANSPARENT_GREEN_VALUE: libc::c_int = 0x26;
pub static GLX_TRANSPARENT_BLUE_VALUE: libc::c_int = 0x27;
pub static GLX_TRANSPARENT_ALPHA_VALUE: libc::c_int = 0x28;
#[allow(type_overflow)]
pub static GLX_DONT_CARE: libc::c_int = 0xFFFFFFFF;
pub static GLX_NONE: libc::c_int = 0x8000;
pub static GLX_SLOW_CONFIG: libc::c_int = 0x8001;
pub static GLX_TRUE_COLOR: libc::c_int = 0x8002;
pub static GLX_DIRECT_COLOR: libc::c_int = 0x8003;
pub static GLX_PSEUDO_COLOR: libc::c_int = 0x8004;
pub static GLX_STATIC_COLOR: libc::c_int = 0x8005;
pub static GLX_GRAY_SCALE: libc::c_int = 0x8006;
pub static GLX_STATIC_GRAY: libc::c_int = 0x8007;
pub static GLX_TRANSPARENT_RGB: libc::c_int = 0x8008;
pub static GLX_TRANSPARENT_INDEX: libc::c_int = 0x8009;
pub static GLX_VISUAL_ID: libc::c_int = 0x800B;
pub static GLX_SCREEN: libc::c_int = 0x800C;
pub static GLX_NON_CONFORMANT_CONFIG: libc::c_int = 0x800D;
pub static GLX_DRAWABLE_TYPE: libc::c_int = 0x8010;
pub static GLX_RENDER_TYPE: libc::c_int = 0x8011;
pub static GLX_X_RENDERABLE: libc::c_int = 0x8012;
pub static GLX_FBCONFIG_ID: libc::c_int = 0x8013;
pub static GLX_RGBA_TYPE: libc::c_int = 0x8014;
pub static GLX_COLOR_INDEX_TYPE: libc::c_int = 0x8015;
pub static GLX_MAX_PBUFFER_WIDTH: libc::c_int = 0x8016;
pub static GLX_MAX_PBUFFER_HEIGHT: libc::c_int = 0x8017;
pub static GLX_MAX_PBUFFER_PIXELS: libc::c_int = 0x8018;
pub static GLX_PRESERVED_CONTENTS: libc::c_int = 0x801B;
pub static GLX_LARGEST_PBUFFER: libc::c_int = 0x801C;
pub static GLX_WIDTH: libc::c_int = 0x801D;
pub static GLX_HEIGHT: libc::c_int = 0x801E;
pub static GLX_EVENT_MASK: libc::c_int = 0x801F;
pub static GLX_DAMAGED: libc::c_int = 0x8020;
pub static GLX_SAVED: libc::c_int = 0x8021;
pub static GLX_WINDOW: libc::c_int = 0x8022;
pub static GLX_PBUFFER: libc::c_int = 0x8023;
pub static GLX_PBUFFER_HEIGHT: libc::c_int = 0x8040;
pub static GLX_PBUFFER_WIDTH: libc::c_int = 0x8041;

pub static GLX_CONTEXT_MAJOR_VERSION: libc::c_int = 0x2091;
pub static GLX_CONTEXT_MINOR_VERSION: libc::c_int = 0x2092;
pub static GLX_CONTEXT_FLAGS: libc::c_int = 0x2094;
pub static GLX_CONTEXT_PROFILE_MASK: libc::c_int = 0x9126;
pub static GLX_CONTEXT_DEBUG_BIT: libc::c_int = 0x0001;
pub static GLX_CONTEXT_FORWARD_COMPATIBLE_BIT: libc::c_int = 0x0002;
pub static GLX_CONTEXT_CORE_PROFILE_BIT: libc::c_int = 0x00000001;
pub static GLX_CONTEXT_COMPATIBILITY_PROFILE_BIT: libc::c_int = 0x00000002;

pub static XIMPreeditArea: libc::c_long = 0x0001;
pub static XIMPreeditCallbacks: libc::c_long = 0x0002;
pub static XIMPreeditPosition: libc::c_long = 0x0004;
pub static XIMPreeditNothing: libc::c_long = 0x0008;
pub static XIMPreeditNone: libc::c_long = 0x0010;
pub static XIMStatusArea: libc::c_long = 0x0100;
pub static XIMStatusCallbacks: libc::c_long = 0x0200;
pub static XIMStatusNothing: libc::c_long = 0x0400;
pub static XIMStatusNone: libc::c_long = 0x0800;

pub static XK_BackSpace: libc::c_uint = 0xFF08;
pub static XK_Tab: libc::c_uint = 0xFF09;
pub static XK_Linefeed: libc::c_uint = 0xFF0A;
pub static XK_Clear: libc::c_uint = 0xFF0B;
pub static XK_Return: libc::c_uint = 0xFF0D;
pub static XK_Pause: libc::c_uint = 0xFF13;
pub static XK_Scroll_Lock: libc::c_uint = 0xFF14;
pub static XK_Sys_Req: libc::c_uint = 0xFF15;
pub static XK_Escape: libc::c_uint = 0xFF1B;
pub static XK_Delete: libc::c_uint = 0xFFFF;
pub static XK_Multi_key: libc::c_uint = 0xFF20;
pub static XK_Kanji: libc::c_uint = 0xFF21;
pub static XK_Muhenkan: libc::c_uint = 0xFF22;
pub static XK_Henkan_Mode: libc::c_uint = 0xFF23;
pub static XK_Henkan: libc::c_uint = 0xFF23;
pub static XK_Romaji: libc::c_uint = 0xFF24;
pub static XK_Hiragana: libc::c_uint = 0xFF25;
pub static XK_Katakana: libc::c_uint = 0xFF26;
pub static XK_Hiragana_Katakana: libc::c_uint = 0xFF27;
pub static XK_Zenkaku: libc::c_uint = 0xFF28;
pub static XK_Hankaku: libc::c_uint = 0xFF29;
pub static XK_Zenkaku_Hankaku: libc::c_uint = 0xFF2A;
pub static XK_Touroku: libc::c_uint = 0xFF2B;
pub static XK_Massyo: libc::c_uint = 0xFF2C;
pub static XK_Kana_Lock: libc::c_uint = 0xFF2D;
pub static XK_Kana_Shift: libc::c_uint = 0xFF2E;
pub static XK_Eisu_Shift: libc::c_uint = 0xFF2F;
pub static XK_Eisu_toggle: libc::c_uint = 0xFF30;
pub static XK_Home: libc::c_uint = 0xFF50;
pub static XK_Left: libc::c_uint = 0xFF51;
pub static XK_Up: libc::c_uint = 0xFF52;
pub static XK_Right: libc::c_uint = 0xFF53;
pub static XK_Down: libc::c_uint = 0xFF54;
pub static XK_Prior: libc::c_uint = 0xFF55;
pub static XK_Page_Up: libc::c_uint = 0xFF55;
pub static XK_Next: libc::c_uint = 0xFF56;
pub static XK_Page_Down: libc::c_uint = 0xFF56;
pub static XK_End: libc::c_uint = 0xFF57;
pub static XK_Begin: libc::c_uint = 0xFF58;
pub static XK_Win_L: libc::c_uint = 0xFF5B;
pub static XK_Win_R: libc::c_uint = 0xFF5C;
pub static XK_App: libc::c_uint = 0xFF5D;
pub static XK_Select: libc::c_uint = 0xFF60;
pub static XK_Print: libc::c_uint = 0xFF61;
pub static XK_Execute: libc::c_uint = 0xFF62;
pub static XK_Insert: libc::c_uint = 0xFF63;
pub static XK_Undo: libc::c_uint = 0xFF65;
pub static XK_Redo: libc::c_uint = 0xFF66;
pub static XK_Menu: libc::c_uint = 0xFF67;
pub static XK_Find: libc::c_uint = 0xFF68;
pub static XK_Cancel: libc::c_uint = 0xFF69;
pub static XK_Help: libc::c_uint = 0xFF6A;
pub static XK_Break: libc::c_uint = 0xFF6B;
pub static XK_Mode_switch: libc::c_uint = 0xFF7E;
pub static XK_script_switch: libc::c_uint = 0xFF7E;
pub static XK_Num_Lock: libc::c_uint = 0xFF7F;
pub static XK_KP_Space: libc::c_uint = 0xFF80;
pub static XK_KP_Tab: libc::c_uint = 0xFF89;
pub static XK_KP_Enter: libc::c_uint = 0xFF8D;
pub static XK_KP_F1: libc::c_uint = 0xFF91;
pub static XK_KP_F2: libc::c_uint = 0xFF92;
pub static XK_KP_F3: libc::c_uint = 0xFF93;
pub static XK_KP_F4: libc::c_uint = 0xFF94;
pub static XK_KP_Home: libc::c_uint = 0xFF95;
pub static XK_KP_Left: libc::c_uint = 0xFF96;
pub static XK_KP_Up: libc::c_uint = 0xFF97;
pub static XK_KP_Right: libc::c_uint = 0xFF98;
pub static XK_KP_Down: libc::c_uint = 0xFF99;
pub static XK_KP_Prior: libc::c_uint = 0xFF9A;
pub static XK_KP_Page_Up: libc::c_uint = 0xFF9A;
pub static XK_KP_Next: libc::c_uint = 0xFF9B;
pub static XK_KP_Page_Down: libc::c_uint = 0xFF9B;
pub static XK_KP_End: libc::c_uint = 0xFF9C;
pub static XK_KP_Begin: libc::c_uint = 0xFF9D;
pub static XK_KP_Insert: libc::c_uint = 0xFF9E;
pub static XK_KP_Delete: libc::c_uint = 0xFF9F;
pub static XK_KP_Equal: libc::c_uint = 0xFFBD;
pub static XK_KP_Multiply: libc::c_uint = 0xFFAA;
pub static XK_KP_Add: libc::c_uint = 0xFFAB;
pub static XK_KP_Separator: libc::c_uint = 0xFFAC;
pub static XK_KP_Subtract: libc::c_uint = 0xFFAD;
pub static XK_KP_Decimal: libc::c_uint = 0xFFAE;
pub static XK_KP_Divide: libc::c_uint = 0xFFAF;
pub static XK_KP_0: libc::c_uint = 0xFFB0;
pub static XK_KP_1: libc::c_uint = 0xFFB1;
pub static XK_KP_2: libc::c_uint = 0xFFB2;
pub static XK_KP_3: libc::c_uint = 0xFFB3;
pub static XK_KP_4: libc::c_uint = 0xFFB4;
pub static XK_KP_5: libc::c_uint = 0xFFB5;
pub static XK_KP_6: libc::c_uint = 0xFFB6;
pub static XK_KP_7: libc::c_uint = 0xFFB7;
pub static XK_KP_8: libc::c_uint = 0xFFB8;
pub static XK_KP_9: libc::c_uint = 0xFFB9;
pub static XK_F1: libc::c_uint = 0xFFBE;
pub static XK_F2: libc::c_uint = 0xFFBF;
pub static XK_F3: libc::c_uint = 0xFFC0;
pub static XK_F4: libc::c_uint = 0xFFC1;
pub static XK_F5: libc::c_uint = 0xFFC2;
pub static XK_F6: libc::c_uint = 0xFFC3;
pub static XK_F7: libc::c_uint = 0xFFC4;
pub static XK_F8: libc::c_uint = 0xFFC5;
pub static XK_F9: libc::c_uint = 0xFFC6;
pub static XK_F10: libc::c_uint = 0xFFC7;
pub static XK_F11: libc::c_uint = 0xFFC8;
pub static XK_L1: libc::c_uint = 0xFFC8;
pub static XK_F12: libc::c_uint = 0xFFC9;
pub static XK_L2: libc::c_uint = 0xFFC9;
pub static XK_F13: libc::c_uint = 0xFFCA;
pub static XK_L3: libc::c_uint = 0xFFCA;
pub static XK_F14: libc::c_uint = 0xFFCB;
pub static XK_L4: libc::c_uint = 0xFFCB;
pub static XK_F15: libc::c_uint = 0xFFCC;
pub static XK_L5: libc::c_uint = 0xFFCC;
pub static XK_F16: libc::c_uint = 0xFFCD;
pub static XK_L6: libc::c_uint = 0xFFCD;
pub static XK_F17: libc::c_uint = 0xFFCE;
pub static XK_L7: libc::c_uint = 0xFFCE;
pub static XK_F18: libc::c_uint = 0xFFCF;
pub static XK_L8: libc::c_uint = 0xFFCF;
pub static XK_F19: libc::c_uint = 0xFFD0;
pub static XK_L9: libc::c_uint = 0xFFD0;
pub static XK_F20: libc::c_uint = 0xFFD1;
pub static XK_L10: libc::c_uint = 0xFFD1;
pub static XK_F21: libc::c_uint = 0xFFD2;
pub static XK_R1: libc::c_uint = 0xFFD2;
pub static XK_F22: libc::c_uint = 0xFFD3;
pub static XK_R2: libc::c_uint = 0xFFD3;
pub static XK_F23: libc::c_uint = 0xFFD4;
pub static XK_R3: libc::c_uint = 0xFFD4;
pub static XK_F24: libc::c_uint = 0xFFD5;
pub static XK_R4: libc::c_uint = 0xFFD5;
pub static XK_F25: libc::c_uint = 0xFFD6;
pub static XK_R5: libc::c_uint = 0xFFD6;
pub static XK_F26: libc::c_uint = 0xFFD7;
pub static XK_R6: libc::c_uint = 0xFFD7;
pub static XK_F27: libc::c_uint = 0xFFD8;
pub static XK_R7: libc::c_uint = 0xFFD8;
pub static XK_F28: libc::c_uint = 0xFFD9;
pub static XK_R8: libc::c_uint = 0xFFD9;
pub static XK_F29: libc::c_uint = 0xFFDA;
pub static XK_R9: libc::c_uint = 0xFFDA;
pub static XK_F30: libc::c_uint = 0xFFDB;
pub static XK_R10: libc::c_uint = 0xFFDB;
pub static XK_F31: libc::c_uint = 0xFFDC;
pub static XK_R11: libc::c_uint = 0xFFDC;
pub static XK_F32: libc::c_uint = 0xFFDD;
pub static XK_R12: libc::c_uint = 0xFFDD;
pub static XK_F33: libc::c_uint = 0xFFDE;
pub static XK_R13: libc::c_uint = 0xFFDE;
pub static XK_F34: libc::c_uint = 0xFFDF;
pub static XK_R14: libc::c_uint = 0xFFDF;
pub static XK_F35: libc::c_uint = 0xFFE0;
pub static XK_R15: libc::c_uint = 0xFFE0;
pub static XK_Shift_L: libc::c_uint = 0xFFE1;
pub static XK_Shift_R: libc::c_uint = 0xFFE2;
pub static XK_Control_L: libc::c_uint = 0xFFE3;
pub static XK_Control_R: libc::c_uint = 0xFFE4;
pub static XK_Caps_Lock: libc::c_uint = 0xFFE5;
pub static XK_Shift_Lock: libc::c_uint = 0xFFE6;
pub static XK_Meta_L: libc::c_uint = 0xFFE7;
pub static XK_Meta_R: libc::c_uint = 0xFFE8;
pub static XK_Alt_L: libc::c_uint = 0xFFE9;
pub static XK_Alt_R: libc::c_uint = 0xFFEA;
pub static XK_Super_L: libc::c_uint = 0xFFEB;
pub static XK_Super_R: libc::c_uint = 0xFFEC;
pub static XK_Hyper_L: libc::c_uint = 0xFFED;
pub static XK_Hyper_R: libc::c_uint = 0xFFEE;
pub static XK_space: libc::c_uint = 0x020;
pub static XK_exclam: libc::c_uint = 0x021;
pub static XK_quotedbl: libc::c_uint = 0x022;
pub static XK_numbersign: libc::c_uint = 0x023;
pub static XK_dollar: libc::c_uint = 0x024;
pub static XK_percent: libc::c_uint = 0x025;
pub static XK_ampersand: libc::c_uint = 0x026;
pub static XK_apostrophe: libc::c_uint = 0x027;
pub static XK_quoteright: libc::c_uint = 0x027;
pub static XK_parenleft: libc::c_uint = 0x028;
pub static XK_parenright: libc::c_uint = 0x029;
pub static XK_asterisk: libc::c_uint = 0x02a;
pub static XK_plus: libc::c_uint = 0x02b;
pub static XK_comma: libc::c_uint = 0x02c;
pub static XK_minus: libc::c_uint = 0x02d;
pub static XK_period: libc::c_uint = 0x02e;
pub static XK_slash: libc::c_uint = 0x02f;
pub static XK_0: libc::c_uint = 0x030;
pub static XK_1: libc::c_uint = 0x031;
pub static XK_2: libc::c_uint = 0x032;
pub static XK_3: libc::c_uint = 0x033;
pub static XK_4: libc::c_uint = 0x034;
pub static XK_5: libc::c_uint = 0x035;
pub static XK_6: libc::c_uint = 0x036;
pub static XK_7: libc::c_uint = 0x037;
pub static XK_8: libc::c_uint = 0x038;
pub static XK_9: libc::c_uint = 0x039;
pub static XK_colon: libc::c_uint = 0x03a;
pub static XK_semicolon: libc::c_uint = 0x03b;
pub static XK_less: libc::c_uint = 0x03c;
pub static XK_equal: libc::c_uint = 0x03d;
pub static XK_greater: libc::c_uint = 0x03e;
pub static XK_question: libc::c_uint = 0x03f;
pub static XK_at: libc::c_uint = 0x040;
pub static XK_A: libc::c_uint = 0x041;
pub static XK_B: libc::c_uint = 0x042;
pub static XK_C: libc::c_uint = 0x043;
pub static XK_D: libc::c_uint = 0x044;
pub static XK_E: libc::c_uint = 0x045;
pub static XK_F: libc::c_uint = 0x046;
pub static XK_G: libc::c_uint = 0x047;
pub static XK_H: libc::c_uint = 0x048;
pub static XK_I: libc::c_uint = 0x049;
pub static XK_J: libc::c_uint = 0x04a;
pub static XK_K: libc::c_uint = 0x04b;
pub static XK_L: libc::c_uint = 0x04c;
pub static XK_M: libc::c_uint = 0x04d;
pub static XK_N: libc::c_uint = 0x04e;
pub static XK_O: libc::c_uint = 0x04f;
pub static XK_P: libc::c_uint = 0x050;
pub static XK_Q: libc::c_uint = 0x051;
pub static XK_R: libc::c_uint = 0x052;
pub static XK_S: libc::c_uint = 0x053;
pub static XK_T: libc::c_uint = 0x054;
pub static XK_U: libc::c_uint = 0x055;
pub static XK_V: libc::c_uint = 0x056;
pub static XK_W: libc::c_uint = 0x057;
pub static XK_X: libc::c_uint = 0x058;
pub static XK_Y: libc::c_uint = 0x059;
pub static XK_Z: libc::c_uint = 0x05a;
pub static XK_bracketleft: libc::c_uint = 0x05b;
pub static XK_backslash: libc::c_uint = 0x05c;
pub static XK_bracketright: libc::c_uint = 0x05d;
pub static XK_asciicircum: libc::c_uint = 0x05e;
pub static XK_underscore: libc::c_uint = 0x05f;
pub static XK_grave: libc::c_uint = 0x060;
pub static XK_quoteleft: libc::c_uint = 0x060;
pub static XK_a: libc::c_uint = 0x061;
pub static XK_b: libc::c_uint = 0x062;
pub static XK_c: libc::c_uint = 0x063;
pub static XK_d: libc::c_uint = 0x064;
pub static XK_e: libc::c_uint = 0x065;
pub static XK_f: libc::c_uint = 0x066;
pub static XK_g: libc::c_uint = 0x067;
pub static XK_h: libc::c_uint = 0x068;
pub static XK_i: libc::c_uint = 0x069;
pub static XK_j: libc::c_uint = 0x06a;
pub static XK_k: libc::c_uint = 0x06b;
pub static XK_l: libc::c_uint = 0x06c;
pub static XK_m: libc::c_uint = 0x06d;
pub static XK_n: libc::c_uint = 0x06e;
pub static XK_o: libc::c_uint = 0x06f;
pub static XK_p: libc::c_uint = 0x070;
pub static XK_q: libc::c_uint = 0x071;
pub static XK_r: libc::c_uint = 0x072;
pub static XK_s: libc::c_uint = 0x073;
pub static XK_t: libc::c_uint = 0x074;
pub static XK_u: libc::c_uint = 0x075;
pub static XK_v: libc::c_uint = 0x076;
pub static XK_w: libc::c_uint = 0x077;
pub static XK_x: libc::c_uint = 0x078;
pub static XK_y: libc::c_uint = 0x079;
pub static XK_z: libc::c_uint = 0x07a;
pub static XK_braceleft: libc::c_uint = 0x07b;
pub static XK_bar: libc::c_uint = 0x07c;
pub static XK_braceright: libc::c_uint = 0x07d;
pub static XK_asciitilde: libc::c_uint = 0x07e;
pub static XK_nobreakspace: libc::c_uint = 0x0a0;
pub static XK_exclamdown: libc::c_uint = 0x0a1;
pub static XK_cent: libc::c_uint = 0x0a2;
pub static XK_sterling: libc::c_uint = 0x0a3;
pub static XK_currency: libc::c_uint = 0x0a4;
pub static XK_yen: libc::c_uint = 0x0a5;
pub static XK_brokenbar: libc::c_uint = 0x0a6;
pub static XK_section: libc::c_uint = 0x0a7;
pub static XK_diaeresis: libc::c_uint = 0x0a8;
pub static XK_copyright: libc::c_uint = 0x0a9;
pub static XK_ordfeminine: libc::c_uint = 0x0aa;
pub static XK_guillemotleft: libc::c_uint = 0x0ab;
pub static XK_notsign: libc::c_uint = 0x0ac;
pub static XK_hyphen: libc::c_uint = 0x0ad;
pub static XK_registered: libc::c_uint = 0x0ae;
pub static XK_macron: libc::c_uint = 0x0af;
pub static XK_degree: libc::c_uint = 0x0b0;
pub static XK_plusminus: libc::c_uint = 0x0b1;
pub static XK_twosuperior: libc::c_uint = 0x0b2;
pub static XK_threesuperior: libc::c_uint = 0x0b3;
pub static XK_acute: libc::c_uint = 0x0b4;
pub static XK_mu: libc::c_uint = 0x0b5;
pub static XK_paragraph: libc::c_uint = 0x0b6;
pub static XK_periodcentered: libc::c_uint = 0x0b7;
pub static XK_cedilla: libc::c_uint = 0x0b8;
pub static XK_onesuperior: libc::c_uint = 0x0b9;
pub static XK_masculine: libc::c_uint = 0x0ba;
pub static XK_guillemotright: libc::c_uint = 0x0bb;
pub static XK_onequarter: libc::c_uint = 0x0bc;
pub static XK_onehalf: libc::c_uint = 0x0bd;
pub static XK_threequarters: libc::c_uint = 0x0be;
pub static XK_questiondown: libc::c_uint = 0x0bf;
pub static XK_Agrave: libc::c_uint = 0x0c0;
pub static XK_Aacute: libc::c_uint = 0x0c1;
pub static XK_Acircumflex: libc::c_uint = 0x0c2;
pub static XK_Atilde: libc::c_uint = 0x0c3;
pub static XK_Adiaeresis: libc::c_uint = 0x0c4;
pub static XK_Aring: libc::c_uint = 0x0c5;
pub static XK_AE: libc::c_uint = 0x0c6;
pub static XK_Ccedilla: libc::c_uint = 0x0c7;
pub static XK_Egrave: libc::c_uint = 0x0c8;
pub static XK_Eacute: libc::c_uint = 0x0c9;
pub static XK_Ecircumflex: libc::c_uint = 0x0ca;
pub static XK_Ediaeresis: libc::c_uint = 0x0cb;
pub static XK_Igrave: libc::c_uint = 0x0cc;
pub static XK_Iacute: libc::c_uint = 0x0cd;
pub static XK_Icircumflex: libc::c_uint = 0x0ce;
pub static XK_Idiaeresis: libc::c_uint = 0x0cf;
pub static XK_ETH: libc::c_uint = 0x0d0;
pub static XK_Eth: libc::c_uint = 0x0d0;
pub static XK_Ntilde: libc::c_uint = 0x0d1;
pub static XK_Ograve: libc::c_uint = 0x0d2;
pub static XK_Oacute: libc::c_uint = 0x0d3;
pub static XK_Ocircumflex: libc::c_uint = 0x0d4;
pub static XK_Otilde: libc::c_uint = 0x0d5;
pub static XK_Odiaeresis: libc::c_uint = 0x0d6;
pub static XK_multiply: libc::c_uint = 0x0d7;
pub static XK_Ooblique: libc::c_uint = 0x0d8;
pub static XK_Ugrave: libc::c_uint = 0x0d9;
pub static XK_Uacute: libc::c_uint = 0x0da;
pub static XK_Ucircumflex: libc::c_uint = 0x0db;
pub static XK_Udiaeresis: libc::c_uint = 0x0dc;
pub static XK_Yacute: libc::c_uint = 0x0dd;
pub static XK_THORN: libc::c_uint = 0x0de;
pub static XK_Thorn: libc::c_uint = 0x0de;
pub static XK_ssharp: libc::c_uint = 0x0df;
pub static XK_agrave: libc::c_uint = 0x0e0;
pub static XK_aacute: libc::c_uint = 0x0e1;
pub static XK_acircumflex: libc::c_uint = 0x0e2;
pub static XK_atilde: libc::c_uint = 0x0e3;
pub static XK_adiaeresis: libc::c_uint = 0x0e4;
pub static XK_aring: libc::c_uint = 0x0e5;
pub static XK_ae: libc::c_uint = 0x0e6;
pub static XK_ccedilla: libc::c_uint = 0x0e7;
pub static XK_egrave: libc::c_uint = 0x0e8;
pub static XK_eacute: libc::c_uint = 0x0e9;
pub static XK_ecircumflex: libc::c_uint = 0x0ea;
pub static XK_ediaeresis: libc::c_uint = 0x0eb;
pub static XK_igrave: libc::c_uint = 0x0ec;
pub static XK_iacute: libc::c_uint = 0x0ed;
pub static XK_icircumflex: libc::c_uint = 0x0ee;
pub static XK_idiaeresis: libc::c_uint = 0x0ef;
pub static XK_eth: libc::c_uint = 0x0f0;
pub static XK_ntilde: libc::c_uint = 0x0f1;
pub static XK_ograve: libc::c_uint = 0x0f2;
pub static XK_oacute: libc::c_uint = 0x0f3;
pub static XK_ocircumflex: libc::c_uint = 0x0f4;
pub static XK_otilde: libc::c_uint = 0x0f5;
pub static XK_odiaeresis: libc::c_uint = 0x0f6;
pub static XK_division: libc::c_uint = 0x0f7;
pub static XK_oslash: libc::c_uint = 0x0f8;
pub static XK_ugrave: libc::c_uint = 0x0f9;
pub static XK_uacute: libc::c_uint = 0x0fa;
pub static XK_ucircumflex: libc::c_uint = 0x0fb;
pub static XK_udiaeresis: libc::c_uint = 0x0fc;
pub static XK_yacute: libc::c_uint = 0x0fd;
pub static XK_thorn: libc::c_uint = 0x0fe;
pub static XK_ydiaeresis: libc::c_uint = 0x0ff;
pub static XK_Aogonek: libc::c_uint = 0x1a1;
pub static XK_breve: libc::c_uint = 0x1a2;
pub static XK_Lstroke: libc::c_uint = 0x1a3;
pub static XK_Lcaron: libc::c_uint = 0x1a5;
pub static XK_Sacute: libc::c_uint = 0x1a6;
pub static XK_Scaron: libc::c_uint = 0x1a9;
pub static XK_Scedilla: libc::c_uint = 0x1aa;
pub static XK_Tcaron: libc::c_uint = 0x1ab;
pub static XK_Zacute: libc::c_uint = 0x1ac;
pub static XK_Zcaron: libc::c_uint = 0x1ae;
pub static XK_Zabovedot: libc::c_uint = 0x1af;
pub static XK_aogonek: libc::c_uint = 0x1b1;
pub static XK_ogonek: libc::c_uint = 0x1b2;
pub static XK_lstroke: libc::c_uint = 0x1b3;
pub static XK_lcaron: libc::c_uint = 0x1b5;
pub static XK_sacute: libc::c_uint = 0x1b6;
pub static XK_caron: libc::c_uint = 0x1b7;
pub static XK_scaron: libc::c_uint = 0x1b9;
pub static XK_scedilla: libc::c_uint = 0x1ba;
pub static XK_tcaron: libc::c_uint = 0x1bb;
pub static XK_zacute: libc::c_uint = 0x1bc;
pub static XK_doubleacute: libc::c_uint = 0x1bd;
pub static XK_zcaron: libc::c_uint = 0x1be;
pub static XK_zabovedot: libc::c_uint = 0x1bf;
pub static XK_Racute: libc::c_uint = 0x1c0;
pub static XK_Abreve: libc::c_uint = 0x1c3;
pub static XK_Lacute: libc::c_uint = 0x1c5;
pub static XK_Cacute: libc::c_uint = 0x1c6;
pub static XK_Ccaron: libc::c_uint = 0x1c8;
pub static XK_Eogonek: libc::c_uint = 0x1ca;
pub static XK_Ecaron: libc::c_uint = 0x1cc;
pub static XK_Dcaron: libc::c_uint = 0x1cf;
pub static XK_Dstroke: libc::c_uint = 0x1d0;
pub static XK_Nacute: libc::c_uint = 0x1d1;
pub static XK_Ncaron: libc::c_uint = 0x1d2;
pub static XK_Odoubleacute: libc::c_uint = 0x1d5;
pub static XK_Rcaron: libc::c_uint = 0x1d8;
pub static XK_Uring: libc::c_uint = 0x1d9;
pub static XK_Udoubleacute: libc::c_uint = 0x1db;
pub static XK_Tcedilla: libc::c_uint = 0x1de;
pub static XK_racute: libc::c_uint = 0x1e0;
pub static XK_abreve: libc::c_uint = 0x1e3;
pub static XK_lacute: libc::c_uint = 0x1e5;
pub static XK_cacute: libc::c_uint = 0x1e6;
pub static XK_ccaron: libc::c_uint = 0x1e8;
pub static XK_eogonek: libc::c_uint = 0x1ea;
pub static XK_ecaron: libc::c_uint = 0x1ec;
pub static XK_dcaron: libc::c_uint = 0x1ef;
pub static XK_dstroke: libc::c_uint = 0x1f0;
pub static XK_nacute: libc::c_uint = 0x1f1;
pub static XK_ncaron: libc::c_uint = 0x1f2;
pub static XK_odoubleacute: libc::c_uint = 0x1f5;
pub static XK_udoubleacute: libc::c_uint = 0x1fb;
pub static XK_rcaron: libc::c_uint = 0x1f8;
pub static XK_uring: libc::c_uint = 0x1f9;
pub static XK_tcedilla: libc::c_uint = 0x1fe;
pub static XK_abovedot: libc::c_uint = 0x1ff;
pub static XK_Hstroke: libc::c_uint = 0x2a1;
pub static XK_Hcircumflex: libc::c_uint = 0x2a6;
pub static XK_Iabovedot: libc::c_uint = 0x2a9;
pub static XK_Gbreve: libc::c_uint = 0x2ab;
pub static XK_Jcircumflex: libc::c_uint = 0x2ac;
pub static XK_hstroke: libc::c_uint = 0x2b1;
pub static XK_hcircumflex: libc::c_uint = 0x2b6;
pub static XK_idotless: libc::c_uint = 0x2b9;
pub static XK_gbreve: libc::c_uint = 0x2bb;
pub static XK_jcircumflex: libc::c_uint = 0x2bc;
pub static XK_Cabovedot: libc::c_uint = 0x2c5;
pub static XK_Ccircumflex: libc::c_uint = 0x2c6;
pub static XK_Gabovedot: libc::c_uint = 0x2d5;
pub static XK_Gcircumflex: libc::c_uint = 0x2d8;
pub static XK_Ubreve: libc::c_uint = 0x2dd;
pub static XK_Scircumflex: libc::c_uint = 0x2de;
pub static XK_cabovedot: libc::c_uint = 0x2e5;
pub static XK_ccircumflex: libc::c_uint = 0x2e6;
pub static XK_gabovedot: libc::c_uint = 0x2f5;
pub static XK_gcircumflex: libc::c_uint = 0x2f8;
pub static XK_ubreve: libc::c_uint = 0x2fd;
pub static XK_scircumflex: libc::c_uint = 0x2fe;
pub static XK_kra: libc::c_uint = 0x3a2;
pub static XK_kappa: libc::c_uint = 0x3a2;
pub static XK_Rcedilla: libc::c_uint = 0x3a3;
pub static XK_Itilde: libc::c_uint = 0x3a5;
pub static XK_Lcedilla: libc::c_uint = 0x3a6;
pub static XK_Emacron: libc::c_uint = 0x3aa;
pub static XK_Gcedilla: libc::c_uint = 0x3ab;
pub static XK_Tslash: libc::c_uint = 0x3ac;
pub static XK_rcedilla: libc::c_uint = 0x3b3;
pub static XK_itilde: libc::c_uint = 0x3b5;
pub static XK_lcedilla: libc::c_uint = 0x3b6;
pub static XK_emacron: libc::c_uint = 0x3ba;
pub static XK_gcedilla: libc::c_uint = 0x3bb;
pub static XK_tslash: libc::c_uint = 0x3bc;
pub static XK_ENG: libc::c_uint = 0x3bd;
pub static XK_eng: libc::c_uint = 0x3bf;
pub static XK_Amacron: libc::c_uint = 0x3c0;
pub static XK_Iogonek: libc::c_uint = 0x3c7;
pub static XK_Eabovedot: libc::c_uint = 0x3cc;
pub static XK_Imacron: libc::c_uint = 0x3cf;
pub static XK_Ncedilla: libc::c_uint = 0x3d1;
pub static XK_Omacron: libc::c_uint = 0x3d2;
pub static XK_Kcedilla: libc::c_uint = 0x3d3;
pub static XK_Uogonek: libc::c_uint = 0x3d9;
pub static XK_Utilde: libc::c_uint = 0x3dd;
pub static XK_Umacron: libc::c_uint = 0x3de;
pub static XK_amacron: libc::c_uint = 0x3e0;
pub static XK_iogonek: libc::c_uint = 0x3e7;
pub static XK_eabovedot: libc::c_uint = 0x3ec;
pub static XK_imacron: libc::c_uint = 0x3ef;
pub static XK_ncedilla: libc::c_uint = 0x3f1;
pub static XK_omacron: libc::c_uint = 0x3f2;
pub static XK_kcedilla: libc::c_uint = 0x3f3;
pub static XK_uogonek: libc::c_uint = 0x3f9;
pub static XK_utilde: libc::c_uint = 0x3fd;
pub static XK_umacron: libc::c_uint = 0x3fe;
pub static XK_overline: libc::c_uint = 0x47e;
pub static XK_kana_fullstop: libc::c_uint = 0x4a1;
pub static XK_kana_openingbracket: libc::c_uint = 0x4a2;
pub static XK_kana_closingbracket: libc::c_uint = 0x4a3;
pub static XK_kana_comma: libc::c_uint = 0x4a4;
pub static XK_kana_conjunctive: libc::c_uint = 0x4a5;
pub static XK_kana_middledot: libc::c_uint = 0x4a5;
pub static XK_kana_WO: libc::c_uint = 0x4a6;
pub static XK_kana_a: libc::c_uint = 0x4a7;
pub static XK_kana_i: libc::c_uint = 0x4a8;
pub static XK_kana_u: libc::c_uint = 0x4a9;
pub static XK_kana_e: libc::c_uint = 0x4aa;
pub static XK_kana_o: libc::c_uint = 0x4ab;
pub static XK_kana_ya: libc::c_uint = 0x4ac;
pub static XK_kana_yu: libc::c_uint = 0x4ad;
pub static XK_kana_yo: libc::c_uint = 0x4ae;
pub static XK_kana_tsu: libc::c_uint = 0x4af;
pub static XK_kana_tu: libc::c_uint = 0x4af;
pub static XK_prolongedsound: libc::c_uint = 0x4b0;
pub static XK_kana_A: libc::c_uint = 0x4b1;
pub static XK_kana_I: libc::c_uint = 0x4b2;
pub static XK_kana_U: libc::c_uint = 0x4b3;
pub static XK_kana_E: libc::c_uint = 0x4b4;
pub static XK_kana_O: libc::c_uint = 0x4b5;
pub static XK_kana_KA: libc::c_uint = 0x4b6;
pub static XK_kana_KI: libc::c_uint = 0x4b7;
pub static XK_kana_KU: libc::c_uint = 0x4b8;
pub static XK_kana_KE: libc::c_uint = 0x4b9;
pub static XK_kana_KO: libc::c_uint = 0x4ba;
pub static XK_kana_SA: libc::c_uint = 0x4bb;
pub static XK_kana_SHI: libc::c_uint = 0x4bc;
pub static XK_kana_SU: libc::c_uint = 0x4bd;
pub static XK_kana_SE: libc::c_uint = 0x4be;
pub static XK_kana_SO: libc::c_uint = 0x4bf;
pub static XK_kana_TA: libc::c_uint = 0x4c0;
pub static XK_kana_CHI: libc::c_uint = 0x4c1;
pub static XK_kana_TI: libc::c_uint = 0x4c1;
pub static XK_kana_TSU: libc::c_uint = 0x4c2;
pub static XK_kana_TU: libc::c_uint = 0x4c2;
pub static XK_kana_TE: libc::c_uint = 0x4c3;
pub static XK_kana_TO: libc::c_uint = 0x4c4;
pub static XK_kana_NA: libc::c_uint = 0x4c5;
pub static XK_kana_NI: libc::c_uint = 0x4c6;
pub static XK_kana_NU: libc::c_uint = 0x4c7;
pub static XK_kana_NE: libc::c_uint = 0x4c8;
pub static XK_kana_NO: libc::c_uint = 0x4c9;
pub static XK_kana_HA: libc::c_uint = 0x4ca;
pub static XK_kana_HI: libc::c_uint = 0x4cb;
pub static XK_kana_FU: libc::c_uint = 0x4cc;
pub static XK_kana_HU: libc::c_uint = 0x4cc;
pub static XK_kana_HE: libc::c_uint = 0x4cd;
pub static XK_kana_HO: libc::c_uint = 0x4ce;
pub static XK_kana_MA: libc::c_uint = 0x4cf;
pub static XK_kana_MI: libc::c_uint = 0x4d0;
pub static XK_kana_MU: libc::c_uint = 0x4d1;
pub static XK_kana_ME: libc::c_uint = 0x4d2;
pub static XK_kana_MO: libc::c_uint = 0x4d3;
pub static XK_kana_YA: libc::c_uint = 0x4d4;
pub static XK_kana_YU: libc::c_uint = 0x4d5;
pub static XK_kana_YO: libc::c_uint = 0x4d6;
pub static XK_kana_RA: libc::c_uint = 0x4d7;
pub static XK_kana_RI: libc::c_uint = 0x4d8;
pub static XK_kana_RU: libc::c_uint = 0x4d9;
pub static XK_kana_RE: libc::c_uint = 0x4da;
pub static XK_kana_RO: libc::c_uint = 0x4db;
pub static XK_kana_WA: libc::c_uint = 0x4dc;
pub static XK_kana_N: libc::c_uint = 0x4dd;
pub static XK_voicedsound: libc::c_uint = 0x4de;
pub static XK_semivoicedsound: libc::c_uint = 0x4df;
pub static XK_kana_switch: libc::c_uint = 0xFF7E;
pub static XK_Arabic_comma: libc::c_uint = 0x5ac;
pub static XK_Arabic_semicolon: libc::c_uint = 0x5bb;
pub static XK_Arabic_question_mark: libc::c_uint = 0x5bf;
pub static XK_Arabic_hamza: libc::c_uint = 0x5c1;
pub static XK_Arabic_maddaonalef: libc::c_uint = 0x5c2;
pub static XK_Arabic_hamzaonalef: libc::c_uint = 0x5c3;
pub static XK_Arabic_hamzaonwaw: libc::c_uint = 0x5c4;
pub static XK_Arabic_hamzaunderalef: libc::c_uint = 0x5c5;
pub static XK_Arabic_hamzaonyeh: libc::c_uint = 0x5c6;
pub static XK_Arabic_alef: libc::c_uint = 0x5c7;
pub static XK_Arabic_beh: libc::c_uint = 0x5c8;
pub static XK_Arabic_tehmarbuta: libc::c_uint = 0x5c9;
pub static XK_Arabic_teh: libc::c_uint = 0x5ca;
pub static XK_Arabic_theh: libc::c_uint = 0x5cb;
pub static XK_Arabic_jeem: libc::c_uint = 0x5cc;
pub static XK_Arabic_hah: libc::c_uint = 0x5cd;
pub static XK_Arabic_khah: libc::c_uint = 0x5ce;
pub static XK_Arabic_dal: libc::c_uint = 0x5cf;
pub static XK_Arabic_thal: libc::c_uint = 0x5d0;
pub static XK_Arabic_ra: libc::c_uint = 0x5d1;
pub static XK_Arabic_zain: libc::c_uint = 0x5d2;
pub static XK_Arabic_seen: libc::c_uint = 0x5d3;
pub static XK_Arabic_sheen: libc::c_uint = 0x5d4;
pub static XK_Arabic_sad: libc::c_uint = 0x5d5;
pub static XK_Arabic_dad: libc::c_uint = 0x5d6;
pub static XK_Arabic_tah: libc::c_uint = 0x5d7;
pub static XK_Arabic_zah: libc::c_uint = 0x5d8;
pub static XK_Arabic_ain: libc::c_uint = 0x5d9;
pub static XK_Arabic_ghain: libc::c_uint = 0x5da;
pub static XK_Arabic_tatweel: libc::c_uint = 0x5e0;
pub static XK_Arabic_feh: libc::c_uint = 0x5e1;
pub static XK_Arabic_qaf: libc::c_uint = 0x5e2;
pub static XK_Arabic_kaf: libc::c_uint = 0x5e3;
pub static XK_Arabic_lam: libc::c_uint = 0x5e4;
pub static XK_Arabic_meem: libc::c_uint = 0x5e5;
pub static XK_Arabic_noon: libc::c_uint = 0x5e6;
pub static XK_Arabic_ha: libc::c_uint = 0x5e7;
pub static XK_Arabic_heh: libc::c_uint = 0x5e7;
pub static XK_Arabic_waw: libc::c_uint = 0x5e8;
pub static XK_Arabic_alefmaksura: libc::c_uint = 0x5e9;
pub static XK_Arabic_yeh: libc::c_uint = 0x5ea;
pub static XK_Arabic_fathatan: libc::c_uint = 0x5eb;
pub static XK_Arabic_dammatan: libc::c_uint = 0x5ec;
pub static XK_Arabic_kasratan: libc::c_uint = 0x5ed;
pub static XK_Arabic_fatha: libc::c_uint = 0x5ee;
pub static XK_Arabic_damma: libc::c_uint = 0x5ef;
pub static XK_Arabic_kasra: libc::c_uint = 0x5f0;
pub static XK_Arabic_shadda: libc::c_uint = 0x5f1;
pub static XK_Arabic_sukun: libc::c_uint = 0x5f2;
pub static XK_Arabic_switch: libc::c_uint = 0xFF7E;
pub static XK_Serbian_dje: libc::c_uint = 0x6a1;
pub static XK_Macedonia_gje: libc::c_uint = 0x6a2;
pub static XK_Cyrillic_io: libc::c_uint = 0x6a3;
pub static XK_Ukrainian_ie: libc::c_uint = 0x6a4;
pub static XK_Ukranian_je: libc::c_uint = 0x6a4;
pub static XK_Macedonia_dse: libc::c_uint = 0x6a5;
pub static XK_Ukrainian_i: libc::c_uint = 0x6a6;
pub static XK_Ukranian_i: libc::c_uint = 0x6a6;
pub static XK_Ukrainian_yi: libc::c_uint = 0x6a7;
pub static XK_Ukranian_yi: libc::c_uint = 0x6a7;
pub static XK_Cyrillic_je: libc::c_uint = 0x6a8;
pub static XK_Serbian_je: libc::c_uint = 0x6a8;
pub static XK_Cyrillic_lje: libc::c_uint = 0x6a9;
pub static XK_Serbian_lje: libc::c_uint = 0x6a9;
pub static XK_Cyrillic_nje: libc::c_uint = 0x6aa;
pub static XK_Serbian_nje: libc::c_uint = 0x6aa;
pub static XK_Serbian_tshe: libc::c_uint = 0x6ab;
pub static XK_Macedonia_kje: libc::c_uint = 0x6ac;
pub static XK_Byelorussian_shortu: libc::c_uint = 0x6ae;
pub static XK_Cyrillic_dzhe: libc::c_uint = 0x6af;
pub static XK_Serbian_dze: libc::c_uint = 0x6af;
pub static XK_numerosign: libc::c_uint = 0x6b0;
pub static XK_Serbian_DJE: libc::c_uint = 0x6b1;
pub static XK_Macedonia_GJE: libc::c_uint = 0x6b2;
pub static XK_Cyrillic_IO: libc::c_uint = 0x6b3;
pub static XK_Ukrainian_IE: libc::c_uint = 0x6b4;
pub static XK_Ukranian_JE: libc::c_uint = 0x6b4;
pub static XK_Macedonia_DSE: libc::c_uint = 0x6b5;
pub static XK_Ukrainian_I: libc::c_uint = 0x6b6;
pub static XK_Ukranian_I: libc::c_uint = 0x6b6;
pub static XK_Ukrainian_YI: libc::c_uint = 0x6b7;
pub static XK_Ukranian_YI: libc::c_uint = 0x6b7;
pub static XK_Cyrillic_JE: libc::c_uint = 0x6b8;
pub static XK_Serbian_JE: libc::c_uint = 0x6b8;
pub static XK_Cyrillic_LJE: libc::c_uint = 0x6b9;
pub static XK_Serbian_LJE: libc::c_uint = 0x6b9;
pub static XK_Cyrillic_NJE: libc::c_uint = 0x6ba;
pub static XK_Serbian_NJE: libc::c_uint = 0x6ba;
pub static XK_Serbian_TSHE: libc::c_uint = 0x6bb;
pub static XK_Macedonia_KJE: libc::c_uint = 0x6bc;
pub static XK_Byelorussian_SHORTU: libc::c_uint = 0x6be;
pub static XK_Cyrillic_DZHE: libc::c_uint = 0x6bf;
pub static XK_Serbian_DZE: libc::c_uint = 0x6bf;
pub static XK_Cyrillic_yu: libc::c_uint = 0x6c0;
pub static XK_Cyrillic_a: libc::c_uint = 0x6c1;
pub static XK_Cyrillic_be: libc::c_uint = 0x6c2;
pub static XK_Cyrillic_tse: libc::c_uint = 0x6c3;
pub static XK_Cyrillic_de: libc::c_uint = 0x6c4;
pub static XK_Cyrillic_ie: libc::c_uint = 0x6c5;
pub static XK_Cyrillic_ef: libc::c_uint = 0x6c6;
pub static XK_Cyrillic_ghe: libc::c_uint = 0x6c7;
pub static XK_Cyrillic_ha: libc::c_uint = 0x6c8;
pub static XK_Cyrillic_i: libc::c_uint = 0x6c9;
pub static XK_Cyrillic_shorti: libc::c_uint = 0x6ca;
pub static XK_Cyrillic_ka: libc::c_uint = 0x6cb;
pub static XK_Cyrillic_el: libc::c_uint = 0x6cc;
pub static XK_Cyrillic_em: libc::c_uint = 0x6cd;
pub static XK_Cyrillic_en: libc::c_uint = 0x6ce;
pub static XK_Cyrillic_o: libc::c_uint = 0x6cf;
pub static XK_Cyrillic_pe: libc::c_uint = 0x6d0;
pub static XK_Cyrillic_ya: libc::c_uint = 0x6d1;
pub static XK_Cyrillic_er: libc::c_uint = 0x6d2;
pub static XK_Cyrillic_es: libc::c_uint = 0x6d3;
pub static XK_Cyrillic_te: libc::c_uint = 0x6d4;
pub static XK_Cyrillic_u: libc::c_uint = 0x6d5;
pub static XK_Cyrillic_zhe: libc::c_uint = 0x6d6;
pub static XK_Cyrillic_ve: libc::c_uint = 0x6d7;
pub static XK_Cyrillic_softsign: libc::c_uint = 0x6d8;
pub static XK_Cyrillic_yeru: libc::c_uint = 0x6d9;
pub static XK_Cyrillic_ze: libc::c_uint = 0x6da;
pub static XK_Cyrillic_sha: libc::c_uint = 0x6db;
pub static XK_Cyrillic_e: libc::c_uint = 0x6dc;
pub static XK_Cyrillic_shcha: libc::c_uint = 0x6dd;
pub static XK_Cyrillic_che: libc::c_uint = 0x6de;
pub static XK_Cyrillic_hardsign: libc::c_uint = 0x6df;
pub static XK_Cyrillic_YU: libc::c_uint = 0x6e0;
pub static XK_Cyrillic_A: libc::c_uint = 0x6e1;
pub static XK_Cyrillic_BE: libc::c_uint = 0x6e2;
pub static XK_Cyrillic_TSE: libc::c_uint = 0x6e3;
pub static XK_Cyrillic_DE: libc::c_uint = 0x6e4;
pub static XK_Cyrillic_IE: libc::c_uint = 0x6e5;
pub static XK_Cyrillic_EF: libc::c_uint = 0x6e6;
pub static XK_Cyrillic_GHE: libc::c_uint = 0x6e7;
pub static XK_Cyrillic_HA: libc::c_uint = 0x6e8;
pub static XK_Cyrillic_I: libc::c_uint = 0x6e9;
pub static XK_Cyrillic_SHORTI: libc::c_uint = 0x6ea;
pub static XK_Cyrillic_KA: libc::c_uint = 0x6eb;
pub static XK_Cyrillic_EL: libc::c_uint = 0x6ec;
pub static XK_Cyrillic_EM: libc::c_uint = 0x6ed;
pub static XK_Cyrillic_EN: libc::c_uint = 0x6ee;
pub static XK_Cyrillic_O: libc::c_uint = 0x6ef;
pub static XK_Cyrillic_PE: libc::c_uint = 0x6f0;
pub static XK_Cyrillic_YA: libc::c_uint = 0x6f1;
pub static XK_Cyrillic_ER: libc::c_uint = 0x6f2;
pub static XK_Cyrillic_ES: libc::c_uint = 0x6f3;
pub static XK_Cyrillic_TE: libc::c_uint = 0x6f4;
pub static XK_Cyrillic_U: libc::c_uint = 0x6f5;
pub static XK_Cyrillic_ZHE: libc::c_uint = 0x6f6;
pub static XK_Cyrillic_VE: libc::c_uint = 0x6f7;
pub static XK_Cyrillic_SOFTSIGN: libc::c_uint = 0x6f8;
pub static XK_Cyrillic_YERU: libc::c_uint = 0x6f9;
pub static XK_Cyrillic_ZE: libc::c_uint = 0x6fa;
pub static XK_Cyrillic_SHA: libc::c_uint = 0x6fb;
pub static XK_Cyrillic_E: libc::c_uint = 0x6fc;
pub static XK_Cyrillic_SHCHA: libc::c_uint = 0x6fd;
pub static XK_Cyrillic_CHE: libc::c_uint = 0x6fe;
pub static XK_Cyrillic_HARDSIGN: libc::c_uint = 0x6ff;
pub static XK_Greek_ALPHAaccent: libc::c_uint = 0x7a1;
pub static XK_Greek_EPSILONaccent: libc::c_uint = 0x7a2;
pub static XK_Greek_ETAaccent: libc::c_uint = 0x7a3;
pub static XK_Greek_IOTAaccent: libc::c_uint = 0x7a4;
pub static XK_Greek_IOTAdiaeresis: libc::c_uint = 0x7a5;
pub static XK_Greek_OMICRONaccent: libc::c_uint = 0x7a7;
pub static XK_Greek_UPSILONaccent: libc::c_uint = 0x7a8;
pub static XK_Greek_UPSILONdieresis: libc::c_uint = 0x7a9;
pub static XK_Greek_OMEGAaccent: libc::c_uint = 0x7ab;
pub static XK_Greek_accentdieresis: libc::c_uint = 0x7ae;
pub static XK_Greek_horizbar: libc::c_uint = 0x7af;
pub static XK_Greek_alphaaccent: libc::c_uint = 0x7b1;
pub static XK_Greek_epsilonaccent: libc::c_uint = 0x7b2;
pub static XK_Greek_etaaccent: libc::c_uint = 0x7b3;
pub static XK_Greek_iotaaccent: libc::c_uint = 0x7b4;
pub static XK_Greek_iotadieresis: libc::c_uint = 0x7b5;
pub static XK_Greek_iotaaccentdieresis: libc::c_uint = 0x7b6;
pub static XK_Greek_omicronaccent: libc::c_uint = 0x7b7;
pub static XK_Greek_upsilonaccent: libc::c_uint = 0x7b8;
pub static XK_Greek_upsilondieresis: libc::c_uint = 0x7b9;
pub static XK_Greek_upsilonaccentdieresis: libc::c_uint = 0x7ba;
pub static XK_Greek_omegaaccent: libc::c_uint = 0x7bb;
pub static XK_Greek_ALPHA: libc::c_uint = 0x7c1;
pub static XK_Greek_BETA: libc::c_uint = 0x7c2;
pub static XK_Greek_GAMMA: libc::c_uint = 0x7c3;
pub static XK_Greek_DELTA: libc::c_uint = 0x7c4;
pub static XK_Greek_EPSILON: libc::c_uint = 0x7c5;
pub static XK_Greek_ZETA: libc::c_uint = 0x7c6;
pub static XK_Greek_ETA: libc::c_uint = 0x7c7;
pub static XK_Greek_THETA: libc::c_uint = 0x7c8;
pub static XK_Greek_IOTA: libc::c_uint = 0x7c9;
pub static XK_Greek_KAPPA: libc::c_uint = 0x7ca;
pub static XK_Greek_LAMDA: libc::c_uint = 0x7cb;
pub static XK_Greek_LAMBDA: libc::c_uint = 0x7cb;
pub static XK_Greek_MU: libc::c_uint = 0x7cc;
pub static XK_Greek_NU: libc::c_uint = 0x7cd;
pub static XK_Greek_XI: libc::c_uint = 0x7ce;
pub static XK_Greek_OMICRON: libc::c_uint = 0x7cf;
pub static XK_Greek_PI: libc::c_uint = 0x7d0;
pub static XK_Greek_RHO: libc::c_uint = 0x7d1;
pub static XK_Greek_SIGMA: libc::c_uint = 0x7d2;
pub static XK_Greek_TAU: libc::c_uint = 0x7d4;
pub static XK_Greek_UPSILON: libc::c_uint = 0x7d5;
pub static XK_Greek_PHI: libc::c_uint = 0x7d6;
pub static XK_Greek_CHI: libc::c_uint = 0x7d7;
pub static XK_Greek_PSI: libc::c_uint = 0x7d8;
pub static XK_Greek_OMEGA: libc::c_uint = 0x7d9;
pub static XK_Greek_alpha: libc::c_uint = 0x7e1;
pub static XK_Greek_beta: libc::c_uint = 0x7e2;
pub static XK_Greek_gamma: libc::c_uint = 0x7e3;
pub static XK_Greek_delta: libc::c_uint = 0x7e4;
pub static XK_Greek_epsilon: libc::c_uint = 0x7e5;
pub static XK_Greek_zeta: libc::c_uint = 0x7e6;
pub static XK_Greek_eta: libc::c_uint = 0x7e7;
pub static XK_Greek_theta: libc::c_uint = 0x7e8;
pub static XK_Greek_iota: libc::c_uint = 0x7e9;
pub static XK_Greek_kappa: libc::c_uint = 0x7ea;
pub static XK_Greek_lamda: libc::c_uint = 0x7eb;
pub static XK_Greek_lambda: libc::c_uint = 0x7eb;
pub static XK_Greek_mu: libc::c_uint = 0x7ec;
pub static XK_Greek_nu: libc::c_uint = 0x7ed;
pub static XK_Greek_xi: libc::c_uint = 0x7ee;
pub static XK_Greek_omicron: libc::c_uint = 0x7ef;
pub static XK_Greek_pi: libc::c_uint = 0x7f0;
pub static XK_Greek_rho: libc::c_uint = 0x7f1;
pub static XK_Greek_sigma: libc::c_uint = 0x7f2;
pub static XK_Greek_finalsmallsigma: libc::c_uint = 0x7f3;
pub static XK_Greek_tau: libc::c_uint = 0x7f4;
pub static XK_Greek_upsilon: libc::c_uint = 0x7f5;
pub static XK_Greek_phi: libc::c_uint = 0x7f6;
pub static XK_Greek_chi: libc::c_uint = 0x7f7;
pub static XK_Greek_psi: libc::c_uint = 0x7f8;
pub static XK_Greek_omega: libc::c_uint = 0x7f9;
pub static XK_Greek_switch: libc::c_uint = 0xFF7E;
pub static XK_leftradical: libc::c_uint = 0x8a1;
pub static XK_topleftradical: libc::c_uint = 0x8a2;
pub static XK_horizconnector: libc::c_uint = 0x8a3;
pub static XK_topintegral: libc::c_uint = 0x8a4;
pub static XK_botintegral: libc::c_uint = 0x8a5;
pub static XK_vertconnector: libc::c_uint = 0x8a6;
pub static XK_topleftsqbracket: libc::c_uint = 0x8a7;
pub static XK_botleftsqbracket: libc::c_uint = 0x8a8;
pub static XK_toprightsqbracket: libc::c_uint = 0x8a9;
pub static XK_botrightsqbracket: libc::c_uint = 0x8aa;
pub static XK_topleftparens: libc::c_uint = 0x8ab;
pub static XK_botleftparens: libc::c_uint = 0x8ac;
pub static XK_toprightparens: libc::c_uint = 0x8ad;
pub static XK_botrightparens: libc::c_uint = 0x8ae;
pub static XK_leftmiddlecurlybrace: libc::c_uint = 0x8af;
pub static XK_rightmiddlecurlybrace: libc::c_uint = 0x8b0;
pub static XK_topleftsummation: libc::c_uint = 0x8b1;
pub static XK_botleftsummation: libc::c_uint = 0x8b2;
pub static XK_topvertsummationconnector: libc::c_uint = 0x8b3;
pub static XK_botvertsummationconnector: libc::c_uint = 0x8b4;
pub static XK_toprightsummation: libc::c_uint = 0x8b5;
pub static XK_botrightsummation: libc::c_uint = 0x8b6;
pub static XK_rightmiddlesummation: libc::c_uint = 0x8b7;
pub static XK_lessthanequal: libc::c_uint = 0x8bc;
pub static XK_notequal: libc::c_uint = 0x8bd;
pub static XK_greaterthanequal: libc::c_uint = 0x8be;
pub static XK_integral: libc::c_uint = 0x8bf;
pub static XK_therefore: libc::c_uint = 0x8c0;
pub static XK_variation: libc::c_uint = 0x8c1;
pub static XK_infinity: libc::c_uint = 0x8c2;
pub static XK_nabla: libc::c_uint = 0x8c5;
pub static XK_approximate: libc::c_uint = 0x8c8;
pub static XK_similarequal: libc::c_uint = 0x8c9;
pub static XK_ifonlyif: libc::c_uint = 0x8cd;
pub static XK_implies: libc::c_uint = 0x8ce;
pub static XK_identical: libc::c_uint = 0x8cf;
pub static XK_radical: libc::c_uint = 0x8d6;
pub static XK_includedin: libc::c_uint = 0x8da;
pub static XK_includes: libc::c_uint = 0x8db;
pub static XK_intersection: libc::c_uint = 0x8dc;
pub static XK_union: libc::c_uint = 0x8dd;
pub static XK_logicaland: libc::c_uint = 0x8de;
pub static XK_logicalor: libc::c_uint = 0x8df;
pub static XK_partialderivative: libc::c_uint = 0x8ef;
pub static XK_function: libc::c_uint = 0x8f6;
pub static XK_leftarrow: libc::c_uint = 0x8fb;
pub static XK_uparrow: libc::c_uint = 0x8fc;
pub static XK_rightarrow: libc::c_uint = 0x8fd;
pub static XK_downarrow: libc::c_uint = 0x8fe;
pub static XK_blank: libc::c_uint = 0x9df;
pub static XK_soliddiamond: libc::c_uint = 0x9e0;
pub static XK_checkerboard: libc::c_uint = 0x9e1;
pub static XK_ht: libc::c_uint = 0x9e2;
pub static XK_ff: libc::c_uint = 0x9e3;
pub static XK_cr: libc::c_uint = 0x9e4;
pub static XK_lf: libc::c_uint = 0x9e5;
pub static XK_nl: libc::c_uint = 0x9e8;
pub static XK_vt: libc::c_uint = 0x9e9;
pub static XK_lowrightcorner: libc::c_uint = 0x9ea;
pub static XK_uprightcorner: libc::c_uint = 0x9eb;
pub static XK_upleftcorner: libc::c_uint = 0x9ec;
pub static XK_lowleftcorner: libc::c_uint = 0x9ed;
pub static XK_crossinglines: libc::c_uint = 0x9ee;
pub static XK_horizlinescan1: libc::c_uint = 0x9ef;
pub static XK_horizlinescan3: libc::c_uint = 0x9f0;
pub static XK_horizlinescan5: libc::c_uint = 0x9f1;
pub static XK_horizlinescan7: libc::c_uint = 0x9f2;
pub static XK_horizlinescan9: libc::c_uint = 0x9f3;
pub static XK_leftt: libc::c_uint = 0x9f4;
pub static XK_rightt: libc::c_uint = 0x9f5;
pub static XK_bott: libc::c_uint = 0x9f6;
pub static XK_topt: libc::c_uint = 0x9f7;
pub static XK_vertbar: libc::c_uint = 0x9f8;
pub static XK_emspace: libc::c_uint = 0xaa1;
pub static XK_enspace: libc::c_uint = 0xaa2;
pub static XK_em3space: libc::c_uint = 0xaa3;
pub static XK_em4space: libc::c_uint = 0xaa4;
pub static XK_digitspace: libc::c_uint = 0xaa5;
pub static XK_punctspace: libc::c_uint = 0xaa6;
pub static XK_thinspace: libc::c_uint = 0xaa7;
pub static XK_hairspace: libc::c_uint = 0xaa8;
pub static XK_emdash: libc::c_uint = 0xaa9;
pub static XK_endash: libc::c_uint = 0xaaa;
pub static XK_signifblank: libc::c_uint = 0xaac;
pub static XK_ellipsis: libc::c_uint = 0xaae;
pub static XK_doubbaselinedot: libc::c_uint = 0xaaf;
pub static XK_onethird: libc::c_uint = 0xab0;
pub static XK_twothirds: libc::c_uint = 0xab1;
pub static XK_onefifth: libc::c_uint = 0xab2;
pub static XK_twofifths: libc::c_uint = 0xab3;
pub static XK_threefifths: libc::c_uint = 0xab4;
pub static XK_fourfifths: libc::c_uint = 0xab5;
pub static XK_onesixth: libc::c_uint = 0xab6;
pub static XK_fivesixths: libc::c_uint = 0xab7;
pub static XK_careof: libc::c_uint = 0xab8;
pub static XK_figdash: libc::c_uint = 0xabb;
pub static XK_leftanglebracket: libc::c_uint = 0xabc;
pub static XK_decimalpoint: libc::c_uint = 0xabd;
pub static XK_rightanglebracket: libc::c_uint = 0xabe;
pub static XK_marker: libc::c_uint = 0xabf;
pub static XK_oneeighth: libc::c_uint = 0xac3;
pub static XK_threeeighths: libc::c_uint = 0xac4;
pub static XK_fiveeighths: libc::c_uint = 0xac5;
pub static XK_seveneighths: libc::c_uint = 0xac6;
pub static XK_trademark: libc::c_uint = 0xac9;
pub static XK_signaturemark: libc::c_uint = 0xaca;
pub static XK_trademarkincircle: libc::c_uint = 0xacb;
pub static XK_leftopentriangle: libc::c_uint = 0xacc;
pub static XK_rightopentriangle: libc::c_uint = 0xacd;
pub static XK_emopencircle: libc::c_uint = 0xace;
pub static XK_emopenrectangle: libc::c_uint = 0xacf;
pub static XK_leftsinglequotemark: libc::c_uint = 0xad0;
pub static XK_rightsinglequotemark: libc::c_uint = 0xad1;
pub static XK_leftdoublequotemark: libc::c_uint = 0xad2;
pub static XK_rightdoublequotemark: libc::c_uint = 0xad3;
pub static XK_prescription: libc::c_uint = 0xad4;
pub static XK_minutes: libc::c_uint = 0xad6;
pub static XK_seconds: libc::c_uint = 0xad7;
pub static XK_latincross: libc::c_uint = 0xad9;
pub static XK_hexagram: libc::c_uint = 0xada;
pub static XK_filledrectbullet: libc::c_uint = 0xadb;
pub static XK_filledlefttribullet: libc::c_uint = 0xadc;
pub static XK_filledrighttribullet: libc::c_uint = 0xadd;
pub static XK_emfilledcircle: libc::c_uint = 0xade;
pub static XK_emfilledrect: libc::c_uint = 0xadf;
pub static XK_enopencircbullet: libc::c_uint = 0xae0;
pub static XK_enopensquarebullet: libc::c_uint = 0xae1;
pub static XK_openrectbullet: libc::c_uint = 0xae2;
pub static XK_opentribulletup: libc::c_uint = 0xae3;
pub static XK_opentribulletdown: libc::c_uint = 0xae4;
pub static XK_openstar: libc::c_uint = 0xae5;
pub static XK_enfilledcircbullet: libc::c_uint = 0xae6;
pub static XK_enfilledsqbullet: libc::c_uint = 0xae7;
pub static XK_filledtribulletup: libc::c_uint = 0xae8;
pub static XK_filledtribulletdown: libc::c_uint = 0xae9;
pub static XK_leftpointer: libc::c_uint = 0xaea;
pub static XK_rightpointer: libc::c_uint = 0xaeb;
pub static XK_club: libc::c_uint = 0xaec;
pub static XK_diamond: libc::c_uint = 0xaed;
pub static XK_heart: libc::c_uint = 0xaee;
pub static XK_maltesecross: libc::c_uint = 0xaf0;
pub static XK_dagger: libc::c_uint = 0xaf1;
pub static XK_doubledagger: libc::c_uint = 0xaf2;
pub static XK_checkmark: libc::c_uint = 0xaf3;
pub static XK_ballotcross: libc::c_uint = 0xaf4;
pub static XK_musicalsharp: libc::c_uint = 0xaf5;
pub static XK_musicalflat: libc::c_uint = 0xaf6;
pub static XK_malesymbol: libc::c_uint = 0xaf7;
pub static XK_femalesymbol: libc::c_uint = 0xaf8;
pub static XK_telephone: libc::c_uint = 0xaf9;
pub static XK_telephonerecorder: libc::c_uint = 0xafa;
pub static XK_phonographcopyright: libc::c_uint = 0xafb;
pub static XK_caret: libc::c_uint = 0xafc;
pub static XK_singlelowquotemark: libc::c_uint = 0xafd;
pub static XK_doublelowquotemark: libc::c_uint = 0xafe;
pub static XK_cursor: libc::c_uint = 0xaff;
pub static XK_leftcaret: libc::c_uint = 0xba3;
pub static XK_rightcaret: libc::c_uint = 0xba6;
pub static XK_downcaret: libc::c_uint = 0xba8;
pub static XK_upcaret: libc::c_uint = 0xba9;
pub static XK_overbar: libc::c_uint = 0xbc0;
pub static XK_downtack: libc::c_uint = 0xbc2;
pub static XK_upshoe: libc::c_uint = 0xbc3;
pub static XK_downstile: libc::c_uint = 0xbc4;
pub static XK_underbar: libc::c_uint = 0xbc6;
pub static XK_jot: libc::c_uint = 0xbca;
pub static XK_quad: libc::c_uint = 0xbcc;
pub static XK_uptack: libc::c_uint = 0xbce;
pub static XK_circle: libc::c_uint = 0xbcf;
pub static XK_upstile: libc::c_uint = 0xbd3;
pub static XK_downshoe: libc::c_uint = 0xbd6;
pub static XK_rightshoe: libc::c_uint = 0xbd8;
pub static XK_leftshoe: libc::c_uint = 0xbda;
pub static XK_lefttack: libc::c_uint = 0xbdc;
pub static XK_righttack: libc::c_uint = 0xbfc;
pub static XK_hebrew_doublelowline: libc::c_uint = 0xcdf;
pub static XK_hebrew_aleph: libc::c_uint = 0xce0;
pub static XK_hebrew_bet: libc::c_uint = 0xce1;
pub static XK_hebrew_beth: libc::c_uint = 0xce1;
pub static XK_hebrew_gimel: libc::c_uint = 0xce2;
pub static XK_hebrew_gimmel: libc::c_uint = 0xce2;
pub static XK_hebrew_dalet: libc::c_uint = 0xce3;
pub static XK_hebrew_daleth: libc::c_uint = 0xce3;
pub static XK_hebrew_he: libc::c_uint = 0xce4;
pub static XK_hebrew_waw: libc::c_uint = 0xce5;
pub static XK_hebrew_zain: libc::c_uint = 0xce6;
pub static XK_hebrew_zayin: libc::c_uint = 0xce6;
pub static XK_hebrew_chet: libc::c_uint = 0xce7;
pub static XK_hebrew_het: libc::c_uint = 0xce7;
pub static XK_hebrew_tet: libc::c_uint = 0xce8;
pub static XK_hebrew_teth: libc::c_uint = 0xce8;
pub static XK_hebrew_yod: libc::c_uint = 0xce9;
pub static XK_hebrew_finalkaph: libc::c_uint = 0xcea;
pub static XK_hebrew_kaph: libc::c_uint = 0xceb;
pub static XK_hebrew_lamed: libc::c_uint = 0xcec;
pub static XK_hebrew_finalmem: libc::c_uint = 0xced;
pub static XK_hebrew_mem: libc::c_uint = 0xcee;
pub static XK_hebrew_finalnun: libc::c_uint = 0xcef;
pub static XK_hebrew_nun: libc::c_uint = 0xcf0;
pub static XK_hebrew_samech: libc::c_uint = 0xcf1;
pub static XK_hebrew_samekh: libc::c_uint = 0xcf1;
pub static XK_hebrew_ayin: libc::c_uint = 0xcf2;
pub static XK_hebrew_finalpe: libc::c_uint = 0xcf3;
pub static XK_hebrew_pe: libc::c_uint = 0xcf4;
pub static XK_hebrew_finalzade: libc::c_uint = 0xcf5;
pub static XK_hebrew_finalzadi: libc::c_uint = 0xcf5;
pub static XK_hebrew_zade: libc::c_uint = 0xcf6;
pub static XK_hebrew_zadi: libc::c_uint = 0xcf6;
pub static XK_hebrew_qoph: libc::c_uint = 0xcf7;
pub static XK_hebrew_kuf: libc::c_uint = 0xcf7;
pub static XK_hebrew_resh: libc::c_uint = 0xcf8;
pub static XK_hebrew_shin: libc::c_uint = 0xcf9;
pub static XK_hebrew_taw: libc::c_uint = 0xcfa;
pub static XK_hebrew_taf: libc::c_uint = 0xcfa;
pub static XK_Hebrew_switch: libc::c_uint = 0xFF7E;


#[repr(C)]
pub struct XVisualInfo {
    pub visual: *mut Visual,
    pub visualid: VisualID,
    pub screen: libc::c_int,
    pub depth: libc::c_int,
    pub class: libc::c_int,
    pub red_mask: libc::c_ulong,
    pub green_mask: libc::c_ulong,
    pub blue_mask: libc::c_ulong,
    pub colormap_size: libc::c_int,
    pub bits_per_rgb: libc::c_int,
}

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
    pad: [libc::c_long, ..24],
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
    pub l: [libc::c_long, ..5],
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

#[link(name = "GL")]
#[link(name = "X11")]
#[link(name = "Xxf86vm")]
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
    pub fn XInternAtom(display: *mut Display, atom_name: *const libc::c_char,
        only_if_exists: Bool) -> Atom;
    pub fn XKeycodeToKeysym(display: *mut Display, keycode: KeyCode,
        index: libc::c_int) -> KeySym;
    pub fn XMoveWindow(display: *mut Display, w: Window, x: libc::c_int, y: libc::c_int);
    pub fn XMapWindow(display: *mut Display, w: Window);
    pub fn XNextEvent(display: *mut Display, event_return: *mut XEvent);
    pub fn XOpenDisplay(display_name: *const libc::c_char) -> *mut Display;
    pub fn XPeekEvent(display: *mut Display, event_return: *mut XEvent);
    pub fn XRefreshKeyboardMapping(event_map: *const XEvent);
    pub fn XSetWMProtocols(display: *mut Display, w: Window, protocols: *mut Atom,
        count: libc::c_int) -> Status;
    pub fn XStoreName(display: *mut Display, w: Window, window_name: *const libc::c_char);

    pub fn XCloseIM(im: XIM) -> Status;
    pub fn XOpenIM(display: *mut Display, db: XrmDatabase, res_name: *mut libc::c_char,
        res_class: *mut libc::c_char) -> XIM;

    // TODO: this is a vararg function
    //pub fn XCreateIC(im: XIM, ...) -> XIC;
    pub fn XCreateIC(im: XIM, a: *const libc::c_char, b: libc::c_long, c: *const libc::c_char,
        d: Window, e: *const ()) -> XIC;
    pub fn XDestroyIC(ic: XIC);
    pub fn XSetICFocus(ic: XIC);
    pub fn XUnsetICFocus(ic: XIC);

    pub fn Xutf8LookupString(ic: XIC, event: *mut XKeyEvent,
        buffer_return: *mut libc::c_char, bytes_buffer: libc::c_int,
        keysym_return: *mut KeySym, status_return: *mut Status) -> libc::c_int;

    pub fn glXCreateContext(dpy: *mut Display, vis: *const XVisualInfo,
        shareList: GLXContext, direct: Bool) -> GLXContext;
    pub fn glXCreateNewContext(dpy: *mut Display, config: GLXFBConfig, render_type: libc::c_int,
        shareList: GLXContext, direct: Bool) -> GLXContext;
    pub fn glXDestroyContext(dpy: *mut Display, ctx: GLXContext);
    pub fn glXChooseFBConfig(dpy: *mut Display, screen: libc::c_int,
        attrib_list: *const libc::c_int, nelements: *mut libc::c_int) -> *mut GLXFBConfig;
    pub fn glXChooseVisual(dpy: *mut Display, screen: libc::c_int,
        attribList: *const libc::c_int) -> *const XVisualInfo;
    pub fn glXGetProcAddress(procName: *const libc::c_uchar) -> *const ();
    pub fn glXGetVisualFromFBConfig(dpy: *mut Display, config: GLXFBConfig) -> *mut XVisualInfo;
    pub fn glXMakeCurrent(dpy: *mut Display, drawable: GLXDrawable,
        ctx: GLXContext) -> Bool;
    pub fn glXSwapBuffers(dpy: *mut Display, drawable: GLXDrawable);

    pub fn XkbSetDetectableAutoRepeat(dpy: *mut Display, detectable: bool, supported_rtm: *mut bool) -> bool;
    pub fn XF86VidModeSwitchToMode(dpy: *mut Display, screen: libc::c_int,
        modeline: *mut XF86VidModeModeInfo) -> Bool;
    pub fn XF86VidModeSetViewPort(dpy: *mut Display, screen: libc::c_int,
        x: libc::c_int, y: libc::c_int) -> Bool;
    pub fn XF86VidModeGetAllModeLines(dpy: *mut Display, screen: libc::c_int,
        modecount_return: *mut libc::c_int, modesinfo: *mut *mut *mut XF86VidModeModeInfo) -> Bool;
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
