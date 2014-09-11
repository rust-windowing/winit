#![allow(dead_code)]
#![allow(non_snake_case)]
#![allow(non_camel_case_types)]

use libc;

pub type EGLBoolean = libc::c_int;
pub type EGLint = i32;
pub type EGLDisplay = *const libc::c_void;
pub type EGLConfig = *const libc::c_void;
pub type EGLSurface = *const libc::c_void;
pub type EGLContext = *const libc::c_void;

pub type NativePixmapType = *const libc::c_void;     // FIXME: egl_native_pixmap_t instead
pub type NativeWindowType = *const ANativeWindow;

pub static EGL_DEFAULT_DISPLAY: EGLint = 0;
/*pub static EGL_NO_CONTEXT: EGLContext = { use std::ptr; ptr::null() };
pub static EGL_NO_DISPLAY: EGLDisplay = { use std::ptr; ptr::null() };
pub static EGL_NO_SURFACE: EGLSurface = { use std::ptr; ptr::null() };*/

pub static EGL_VERSION_1_0: EGLint = 1;
pub static EGL_VERSION_1_1: EGLint = 1;

pub static EGL_FALSE: EGLint = 0;
pub static EGL_TRUE: EGLint = 1;

pub static EGL_SUCCESS: EGLint = 0x3000;
pub static EGL_NOT_INITIALIZED: EGLint = 0x3001;
pub static EGL_BAD_ACCESS: EGLint = 0x3002;
pub static EGL_BAD_ALLOC: EGLint = 0x3003;
pub static EGL_BAD_ATTRIBUTE: EGLint = 0x3004;
pub static EGL_BAD_CONFIG: EGLint = 0x3005;
pub static EGL_BAD_CONTEXT: EGLint = 0x3006;
pub static EGL_BAD_CURRENT_SURFACE: EGLint = 0x3007;
pub static EGL_BAD_DISPLAY: EGLint = 0x3008;
pub static EGL_BAD_MATCH: EGLint = 0x3009;
pub static EGL_BAD_NATIVE_PIXMAP: EGLint = 0x300A;
pub static EGL_BAD_NATIVE_WINDOW: EGLint = 0x300B;
pub static EGL_BAD_PARAMETER: EGLint = 0x300C;
pub static EGL_BAD_SURFACE: EGLint = 0x300D;
pub static EGL_CONTEXT_LOST: EGLint = 0x300E;

pub static EGL_BUFFER_SIZE: EGLint = 0x3020;
pub static EGL_ALPHA_SIZE: EGLint = 0x3021;
pub static EGL_BLUE_SIZE: EGLint = 0x3022;
pub static EGL_GREEN_SIZE: EGLint = 0x3023;
pub static EGL_RED_SIZE: EGLint = 0x3024;
pub static EGL_DEPTH_SIZE: EGLint = 0x3025;
pub static EGL_STENCIL_SIZE: EGLint = 0x3026;
pub static EGL_CONFIG_CAVEAT: EGLint = 0x3027;
pub static EGL_CONFIG_ID: EGLint = 0x3028;
pub static EGL_LEVEL: EGLint = 0x3029;
pub static EGL_MAX_PBUFFER_HEIGHT: EGLint = 0x302A;
pub static EGL_MAX_PBUFFER_PIXELS: EGLint = 0x302B;
pub static EGL_MAX_PBUFFER_WIDTH: EGLint = 0x302C;
pub static EGL_NATIVE_RENDERABLE: EGLint = 0x302D;
pub static EGL_NATIVE_VISUAL_ID: EGLint = 0x302E;
pub static EGL_NATIVE_VISUAL_TYPE: EGLint = 0x302F;
/*pub static EGL_PRESERVED_RESOURCES: EGLint = 0x3030;*/
pub static EGL_SAMPLES: EGLint = 0x3031;
pub static EGL_SAMPLE_BUFFERS: EGLint = 0x3032;
pub static EGL_SURFACE_TYPE: EGLint = 0x3033;
pub static EGL_TRANSPARENT_TYPE: EGLint = 0x3034;
pub static EGL_TRANSPARENT_BLUE_VALUE: EGLint = 0x3035;
pub static EGL_TRANSPARENT_GREEN_VALUE: EGLint = 0x3036;
pub static EGL_TRANSPARENT_RED_VALUE: EGLint = 0x3037;
pub static EGL_NONE: EGLint = 0x3038   /* Also a config value */;
pub static EGL_BIND_TO_TEXTURE_RGB: EGLint = 0x3039;
pub static EGL_BIND_TO_TEXTURE_RGBA: EGLint = 0x303A;
pub static EGL_MIN_SWAP_INTERVAL: EGLint = 0x303B;
pub static EGL_MAX_SWAP_INTERVAL: EGLint = 0x303C;


pub static EGL_DONT_CARE: EGLint = -1;
pub static EGL_SLOW_CONFIG: EGLint = 0x3050   /* EGL_CONFIG_CAVEAT value */;
pub static EGL_NON_CONFORMANT_CONFIG: EGLint = 0x3051   /* " */;
pub static EGL_TRANSPARENT_RGB: EGLint = 0x3052   /* EGL_TRANSPARENT_TYPE value */;
pub static EGL_NO_TEXTURE: EGLint = 0x305C   /* EGL_TEXTURE_FORMAT/TARGET value */;
pub static EGL_TEXTURE_RGB: EGLint = 0x305D   /* EGL_TEXTURE_FORMAT value */;
pub static EGL_TEXTURE_RGBA: EGLint = 0x305E   /* " */;
pub static EGL_TEXTURE_2D: EGLint = 0x305F   /* EGL_TEXTURE_TARGET value */;

pub static EGL_PBUFFER_BIT: EGLint = 0x01 /* EGL_SURFACE_TYPE mask bit */;
pub static EGL_PIXMAP_BIT: EGLint = 0x02 /* " */;
pub static EGL_WINDOW_BIT: EGLint = 0x04 /* " */;

pub static EGL_VENDOR: EGLint = 0x3053   /* eglQueryString target */;
pub static EGL_VERSION: EGLint = 0x3054   /* " */;
pub static EGL_EXTENSIONS: EGLint = 0x3055   /* " */;

pub static EGL_HEIGHT: EGLint = 0x3056;
pub static EGL_WIDTH: EGLint = 0x3057;
pub static EGL_LARGEST_PBUFFER: EGLint = 0x3058;
pub static EGL_TEXTURE_FORMAT: EGLint = 0x3080   /* For pbuffers bound as textures */;
pub static EGL_TEXTURE_TARGET: EGLint = 0x3081   /* " */;
pub static EGL_MIPMAP_TEXTURE: EGLint = 0x3082   /* " */;
pub static EGL_MIPMAP_LEVEL: EGLint = 0x3083   /* " */;

pub static EGL_BACK_BUFFER: EGLint = 0x3084;

pub static EGL_DRAW: EGLint = 0x3059;
pub static EGL_READ: EGLint = 0x305A;

pub static EGL_CORE_NATIVE_ENGINE: EGLint = 0x305B;

#[link(name = "android")]
#[link(name = "EGL")]
#[link(name = "GLESv2")]
extern {
    pub fn eglGetError() -> EGLint;

    pub fn eglGetDisplay(display: *const ()/*NativeDisplayType*/) -> EGLDisplay;
    pub fn eglInitialize(dpy: EGLDisplay, major: *mut EGLint, minor: *mut EGLint) -> EGLBoolean;
    pub fn eglTerminate(dpy: EGLDisplay) -> EGLBoolean;
    pub fn eglQueryString(dpy: EGLDisplay, name: EGLint) -> *const libc::c_char;
    pub fn eglGetProcAddress(procname: *const libc::c_char) -> *const ();

    pub fn eglGetConfigs(dpy: EGLDisplay, configs: *mut EGLConfig, config_size: EGLint,
        num_config: *mut EGLint) -> EGLBoolean;
    pub fn eglChooseConfig(dpy: EGLDisplay, attrib_list: *const EGLint, configs: *mut EGLConfig,
        config_size: EGLint, num_config: *mut EGLint) -> EGLBoolean;
    pub fn eglGetConfigAttrib(dpy: EGLDisplay, config: EGLConfig, attribute: EGLint,
        value: *mut EGLint) -> EGLBoolean;

    pub fn eglCreateWindowSurface(dpy: EGLDisplay, config: EGLConfig, window: NativeWindowType, attrib_list: *const EGLint) -> EGLSurface;
    pub fn eglCreatePixmapSurface(dpy: EGLDisplay, config: EGLConfig, pixmap: NativePixmapType, attrib_list: *const EGLint) -> EGLSurface;
    pub fn eglCreatePbufferSurface(dpy: EGLDisplay, config: EGLConfig,
        attrib_list: *const EGLint) -> EGLSurface;
    pub fn eglDestroySurface(dpy: EGLDisplay, surface: EGLSurface) -> EGLBoolean;
    pub fn eglQuerySurface(dpy: EGLDisplay, surface: EGLSurface, attribute: EGLint,
        value: *mut EGLint) -> EGLBoolean;

    pub fn eglSurfaceAttrib(dpy: EGLDisplay, surface: EGLSurface, attribute: EGLint,
        value: EGLint) -> EGLBoolean;
    pub fn eglBindTexImage(dpy: EGLDisplay, surface: EGLSurface, buffer: EGLint) -> EGLBoolean;
    pub fn eglReleaseTexImage(dpy: EGLDisplay, surface: EGLSurface, buffer: EGLint) -> EGLBoolean;

    pub fn eglSwapInterval(dpy: EGLDisplay, interval: EGLint) -> EGLBoolean;

    pub fn eglCreateContext(dpy: EGLDisplay, config: EGLConfig, share_list: EGLContext,
        attrib_list: *const EGLint) -> EGLContext;
    pub fn eglDestroyContext(dpy: EGLDisplay, ctx: EGLContext) -> EGLBoolean;
    pub fn eglMakeCurrent(dpy: EGLDisplay, draw: EGLSurface, read: EGLSurface,
        ctx: EGLContext) -> EGLBoolean;
    pub fn eglGetCurrentContext() -> EGLContext;
    pub fn eglGetCurrentSurface(readdraw: EGLint) -> EGLSurface;
    pub fn eglGetCurrentDisplay() -> EGLDisplay;
    pub fn eglQueryContext(dpy: EGLDisplay, ctx: EGLContext, attribute: EGLint,
        value: *mut EGLint) -> EGLBoolean;

    pub fn eglWaitGL() -> EGLBoolean;
    pub fn eglWaitNative(engine: EGLint) -> EGLBoolean;
    pub fn eglSwapBuffers(dpy: EGLDisplay, draw: EGLSurface) -> EGLBoolean;
    //pub fn eglCopyBuffers(dpy: EGLDisplay, surface: EGLSurface, target: NativePixmapType) -> EGLBoolean;
}

/**
 * asset_manager.h
 */
pub type AAssetManager = ();

/**
 * native_window.h
 */
pub type ANativeWindow = ();

extern {
    pub fn ANativeWindow_getHeight(window: *const ANativeWindow) -> libc::int32_t;
    pub fn ANativeWindow_getWidth(window: *const ANativeWindow) -> libc::int32_t;
}

/**
 * native_activity.h
 */
pub type JavaVM = ();
pub type JNIEnv = ();
pub type jobject = *const libc::c_void;

pub type AInputQueue = ();  // FIXME: wrong
pub type ARect = ();  // FIXME: wrong

#[repr(C)]
pub struct ANativeActivity {
    pub callbacks: *mut ANativeActivityCallbacks,
    pub vm: *mut JavaVM,
    pub env: *mut JNIEnv,
    pub clazz: jobject,
    pub internalDataPath: *const libc::c_char,
    pub externalDataPath: *const libc::c_char,
    pub sdkVersion: libc::int32_t,
    pub instance: *mut libc::c_void,
    pub assetManager: *mut AAssetManager,
    pub obbPath: *const libc::c_char,
}

#[repr(C)]
pub struct ANativeActivityCallbacks {
    pub onStart: extern fn(*mut ANativeActivity),
    pub onResume: extern fn(*mut ANativeActivity),
    pub onSaveInstanceState: extern fn(*mut ANativeActivity, *mut libc::size_t),
    pub onPause: extern fn(*mut ANativeActivity),
    pub onStop: extern fn(*mut ANativeActivity),
    pub onDestroy: extern fn(*mut ANativeActivity),
    pub onWindowFocusChanged: extern fn(*mut ANativeActivity, libc::c_int),
    pub onNativeWindowCreated: extern fn(*mut ANativeActivity, *const ANativeWindow),
    pub onNativeWindowResized: extern fn(*mut ANativeActivity, *const ANativeWindow),
    pub onNativeWindowRedrawNeeded: extern fn(*mut ANativeActivity, *const ANativeWindow),
    pub onNativeWindowDestroyed: extern fn(*mut ANativeActivity, *const ANativeWindow),
    pub onInputQueueCreated: extern fn(*mut ANativeActivity, *mut AInputQueue),
    pub onInputQueueDestroyed: extern fn(*mut ANativeActivity, *mut AInputQueue),
    pub onContentRectChanged: extern fn(*mut ANativeActivity, *const ARect),
    pub onConfigurationChanged: extern fn(*mut ANativeActivity),
    pub onLowMemory: extern fn(*mut ANativeActivity),
}

/**
 * looper.h
 */
pub type ALooper = ();

#[link(name = "android")]
extern {
    pub fn ALooper_forThread() -> *const ALooper;
    pub fn ALooper_acquire(looper: *const ALooper);
    pub fn ALooper_release(looper: *const ALooper);
    pub fn ALooper_prepare(opts: libc::c_int) -> *const ALooper;
    pub fn ALooper_pollOnce(timeoutMillis: libc::c_int, outFd: *mut libc::c_int,
        outEvents: *mut libc::c_int, outData: *mut *mut libc::c_void) -> libc::c_int;
    pub fn ALooper_pollAll(timeoutMillis: libc::c_int, outFd: *mut libc::c_int,
        outEvents: *mut libc::c_int, outData: *mut *mut libc::c_void) -> libc::c_int;
    pub fn ALooper_wake(looper: *const ALooper);
    pub fn ALooper_addFd(looper: *const ALooper, fd: libc::c_int, ident: libc::c_int,
        events: libc::c_int, callback: ALooper_callbackFunc, data: *mut libc::c_void)
        -> libc::c_int;
    pub fn ALooper_removeFd(looper: *const ALooper, fd: libc::c_int) -> libc::c_int;
}

pub static ALOOPER_PREPARE_ALLOW_NON_CALLBACKS: libc::c_int = 1 << 0;

pub static ALOOPER_POLL_WAKE: libc::c_int = -1;
pub static ALOOPER_POLL_CALLBACK: libc::c_int = -2;
pub static ALOOPER_POLL_TIMEOUT: libc::c_int = -3;
pub static ALOOPER_POLL_ERROR: libc::c_int = -4;

pub static ALOOPER_EVENT_INPUT: libc::c_int = 1 << 0;
pub static ALOOPER_EVENT_OUTPUT: libc::c_int = 1 << 1;
pub static ALOOPER_EVENT_ERROR: libc::c_int = 1 << 2;
pub static ALOOPER_EVENT_HANGUP: libc::c_int = 1 << 3;
pub static ALOOPER_EVENT_INVALID: libc::c_int = 1 << 4;

pub type ALooper_callbackFunc = extern fn(libc::c_int, libc::c_int, *mut libc::c_void) -> libc::c_int;
