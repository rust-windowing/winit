#![cfg(any(target_os = "windows"))]

use BuilderAttribs;
use CreationError;
use GlContext;
use GlRequest;
use GlProfile;
use PixelFormat;
use Api;

use self::make_current_guard::CurrentContextGuard;

use libc;
use std::ffi::{CStr, CString, OsStr};
use std::os::windows::ffi::OsStrExt;
use std::{mem, ptr};
use std::io;

use winapi;
use kernel32;
use user32;
use gdi32;

mod make_current_guard;
mod gl;

pub struct Context {
    context: ContextWrapper,

    hdc: winapi::HDC,

    /// Binded to `opengl32.dll`.
    ///
    /// `wglGetProcAddress` returns null for GL 1.1 functions because they are
    ///  already defined by the system. This module contains them.
    gl_library: winapi::HMODULE,

    /// The pixel format that has been used to create this context.
    pixel_format: PixelFormat,
}

/// A simple wrapper that destroys the window when it is destroyed.
struct WindowWrapper(winapi::HWND, winapi::HDC);

impl Drop for WindowWrapper {
    fn drop(&mut self) {
        unsafe {
            user32::DestroyWindow(self.0);
        }
    }
}

struct ContextWrapper(winapi::HGLRC);

impl Drop for ContextWrapper {
    fn drop(&mut self) {
        unsafe {
            gl::wgl::DeleteContext(self.0 as *const _);
        }
    }
}

impl Context {
    /// Attempt to build a new WGL context on a window.
    ///
    /// The window must **not** have had `SetPixelFormat` called on it.
    pub fn new(builder: &BuilderAttribs<'static>, window: winapi::HWND,
               builder_sharelists: Option<winapi::HGLRC>)
               -> Result<Context, CreationError>
    {
        unsafe {
            let hdc = user32::GetDC(window);
            if hdc.is_null() {
                let err = Err(CreationError::OsError(format!("GetDC function failed: {}",
                                                          format!("{}", io::Error::last_os_error()))));
                return err;
            }

            // the first step is to create a dummy window and a dummy context which we will use
            // to load the pointers to some functions in the OpenGL driver in `extra_functions`
            let extra_functions = {
                let (ex_style, style) = (winapi::WS_EX_APPWINDOW, winapi::WS_POPUP |
                                         winapi::WS_CLIPSIBLINGS | winapi::WS_CLIPCHILDREN);

                // creating a dummy invisible window
                let dummy_window = {
                    let rect = {
                        let mut placement: winapi::WINDOWPLACEMENT = unsafe { mem::zeroed() };
                        placement.length = mem::size_of::<winapi::WINDOWPLACEMENT>() as winapi::UINT;
                        if unsafe { user32::GetWindowPlacement(window, &mut placement) } == 0 {
                            panic!();
                        }
                        placement.rcNormalPosition
                    };

                    let mut class_name = [0u16; 128];
                    if user32::GetClassNameW(window, class_name.as_mut_ptr(), 128) == 0 {
                        return Err(CreationError::OsError(format!("GetClassNameW function failed: {}",
                                                          format!("{}", io::Error::last_os_error()))));
                    }

                    let handle = user32::CreateWindowExW(ex_style, class_name.as_ptr(),
                                                         b"dummy window\0".as_ptr() as *const _, style,
                                                         winapi::CW_USEDEFAULT, winapi::CW_USEDEFAULT,
                                                         rect.right - rect.left, rect.bottom - rect.top,
                                                         ptr::null_mut(), ptr::null_mut(),
                                                         kernel32::GetModuleHandleW(ptr::null()),
                                                         ptr::null_mut());

                    if handle.is_null() {
                        return Err(CreationError::OsError(format!("CreateWindowEx function failed: {}",
                                                          format!("{}", io::Error::last_os_error()))));
                    }

                    let hdc = user32::GetDC(handle);
                    if hdc.is_null() {
                        let err = Err(CreationError::OsError(format!("GetDC function failed: {}",
                                                           format!("{}", io::Error::last_os_error()))));
                        return err;
                    }

                    WindowWrapper(handle, hdc)
                };

                // getting the pixel format that we will use and setting it
                {
                    let formats = enumerate_native_pixel_formats(dummy_window.1);
                    let id = try!(choose_dummy_pixel_format(formats.into_iter()));
                    try!(set_pixel_format(dummy_window.1, id));
                }

                // creating the dummy OpenGL context and making it current
                let dummy_context = try!(create_context(None, dummy_window.0, dummy_window.1, None));
                let _current_context = try!(CurrentContextGuard::make_current(dummy_window.1,
                                                                              dummy_context.0));

                // loading the extra WGL functions
                gl::wgl_extra::Wgl::load_with(|addr| {
                    let addr = CString::new(addr.as_bytes()).unwrap();
                    let addr = addr.as_ptr();
                    gl::wgl::GetProcAddress(addr) as *const libc::c_void
                })
            };

            // calling SetPixelFormat
            let pixel_format = {
                let formats = if extra_functions.GetPixelFormatAttribivARB.is_loaded() {
                    let f = enumerate_arb_pixel_formats(&extra_functions, hdc);
                    if f.is_empty() {
                        enumerate_native_pixel_formats(hdc)
                    } else {
                        f
                    }
                } else {
                    enumerate_native_pixel_formats(hdc)
                };

                let (id, f) = try!(builder.choose_pixel_format(formats.into_iter().map(|(a, b)| (b, a))));
                try!(set_pixel_format(hdc, id));
                f
            };

            // creating the OpenGL context
            let context = try!(create_context(Some((&extra_functions, builder)), window, hdc,
                                              builder_sharelists));

            // loading the opengl32 module
            let gl_library = try!(load_opengl32_dll());

            // handling vsync
            if builder.vsync {
                if extra_functions.SwapIntervalEXT.is_loaded() {
                    let _guard = try!(CurrentContextGuard::make_current(hdc, context.0));

                    if extra_functions.SwapIntervalEXT(1) == 0 {
                        return Err(CreationError::OsError(format!("wglSwapIntervalEXT failed")));
                    }
                }
            }

            Ok(Context {
                context: context,
                hdc: hdc,
                gl_library: gl_library,
                pixel_format: pixel_format,
            })
        }
    }

    /// Returns the raw HGLRC.
    pub fn get_hglrc(&self) -> winapi::HGLRC {
        self.context.0
    }
}

impl GlContext for Context {
    unsafe fn make_current(&self) {
        // TODO: check return value
        gl::wgl::MakeCurrent(self.hdc as *const _, self.context.0 as *const _);
    }

    fn is_current(&self) -> bool {
        unsafe { gl::wgl::GetCurrentContext() == self.context.0 as *const libc::c_void }
    }

    fn get_proc_address(&self, addr: &str) -> *const libc::c_void {
        let addr = CString::new(addr.as_bytes()).unwrap();
        let addr = addr.as_ptr();

        unsafe {
            let p = gl::wgl::GetProcAddress(addr) as *const _;
            if !p.is_null() { return p; }
            kernel32::GetProcAddress(self.gl_library, addr) as *const _
        }
    }

    fn swap_buffers(&self) {
        unsafe {
            gdi32::SwapBuffers(self.hdc);
        }
    }

    fn get_api(&self) -> Api {
        // FIXME: can be opengl es
        Api::OpenGl
    }

    fn get_pixel_format(&self) -> PixelFormat {
        self.pixel_format.clone()
    }
}

unsafe impl Send for Context {}
unsafe impl Sync for Context {}

unsafe fn create_context(extra: Option<(&gl::wgl_extra::Wgl, &BuilderAttribs<'static>)>,
                         _: winapi::HWND, hdc: winapi::HDC, share: Option<winapi::HGLRC>)
                         -> Result<ContextWrapper, CreationError>
{
    let share = share.unwrap_or(ptr::null_mut());

    let ctxt = if let Some((extra_functions, builder)) = extra {
        if extra_functions.CreateContextAttribsARB.is_loaded() {
            let mut attributes = Vec::new();

            match builder.gl_version {
                GlRequest::Latest => {},
                GlRequest::Specific(Api::OpenGl, (major, minor)) => {
                    attributes.push(gl::wgl_extra::CONTEXT_MAJOR_VERSION_ARB as libc::c_int);
                    attributes.push(major as libc::c_int);
                    attributes.push(gl::wgl_extra::CONTEXT_MINOR_VERSION_ARB as libc::c_int);
                    attributes.push(minor as libc::c_int);
                },
                GlRequest::Specific(Api::OpenGlEs, (major, minor)) => {
                    if is_extension_supported(extra_functions, hdc,
                                              "WGL_EXT_create_context_es2_profile")
                    {
                        attributes.push(gl::wgl_extra::CONTEXT_PROFILE_MASK_ARB as libc::c_int);
                        attributes.push(gl::wgl_extra::CONTEXT_ES2_PROFILE_BIT_EXT as libc::c_int);
                    } else {
                        return Err(CreationError::NotSupported);
                    }

                    attributes.push(gl::wgl_extra::CONTEXT_MAJOR_VERSION_ARB as libc::c_int);
                    attributes.push(major as libc::c_int);
                    attributes.push(gl::wgl_extra::CONTEXT_MINOR_VERSION_ARB as libc::c_int);
                    attributes.push(minor as libc::c_int);
                },
                GlRequest::Specific(_, _) => return Err(CreationError::NotSupported),
                GlRequest::GlThenGles { opengl_version: (major, minor), .. } => {
                    attributes.push(gl::wgl_extra::CONTEXT_MAJOR_VERSION_ARB as libc::c_int);
                    attributes.push(major as libc::c_int);
                    attributes.push(gl::wgl_extra::CONTEXT_MINOR_VERSION_ARB as libc::c_int);
                    attributes.push(minor as libc::c_int);
                },
            }

            if let Some(profile) = builder.gl_profile {
                if is_extension_supported(extra_functions, hdc,
                                          "WGL_ARB_create_context_profile")
                {
                    let flag = match profile {
                        GlProfile::Compatibility =>
                            gl::wgl_extra::CONTEXT_COMPATIBILITY_PROFILE_BIT_ARB,
                        GlProfile::Core =>
                            gl::wgl_extra::CONTEXT_CORE_PROFILE_BIT_ARB,
                    };
                    attributes.push(gl::wgl_extra::CONTEXT_PROFILE_MASK_ARB as libc::c_int);
                    attributes.push(flag as libc::c_int);
                } else {
                    return Err(CreationError::NotSupported);
                }
            }

            if builder.gl_debug {
                attributes.push(gl::wgl_extra::CONTEXT_FLAGS_ARB as libc::c_int);
                attributes.push(gl::wgl_extra::CONTEXT_DEBUG_BIT_ARB as libc::c_int);
            }

            attributes.push(0);

            Some(extra_functions.CreateContextAttribsARB(hdc as *const libc::c_void,
                                                         share as *const libc::c_void,
                                                         attributes.as_ptr()))

        } else {
            None
        }
    } else {
        None
    };

    let ctxt = match ctxt {
        Some(ctxt) => ctxt,
        None => {
            let ctxt = gl::wgl::CreateContext(hdc as *const libc::c_void);
            if !ctxt.is_null() && !share.is_null() {
                gl::wgl::ShareLists(share as *const libc::c_void, ctxt);
            };
            ctxt
        }
    };

    if ctxt.is_null() {
        return Err(CreationError::OsError(format!("OpenGL context creation failed: {}",
                                                  format!("{}", io::Error::last_os_error()))));
    }

    Ok(ContextWrapper(ctxt as winapi::HGLRC))
}

unsafe fn enumerate_native_pixel_formats(hdc: winapi::HDC) -> Vec<(PixelFormat, libc::c_int)> {
    let size_of_pxfmtdescr = mem::size_of::<winapi::PIXELFORMATDESCRIPTOR>() as u32;
    let num = gdi32::DescribePixelFormat(hdc, 1, size_of_pxfmtdescr, ptr::null_mut());

    let mut result = Vec::new();

    for index in (0 .. num) {
        let mut output: winapi::PIXELFORMATDESCRIPTOR = mem::zeroed();
        
        if gdi32::DescribePixelFormat(hdc, index, size_of_pxfmtdescr, &mut output) == 0 {
            continue;
        }

        if (output.dwFlags & winapi::PFD_DRAW_TO_WINDOW) == 0 {
            continue;
        }

        if (output.dwFlags & winapi::PFD_SUPPORT_OPENGL) == 0 {
            continue;
        }

        if output.iPixelType != winapi::PFD_TYPE_RGBA {
            continue;
        }

        result.push((PixelFormat {
            hardware_accelerated: (output.dwFlags & winapi::PFD_GENERIC_FORMAT) == 0,
            color_bits: output.cRedBits + output.cGreenBits + output.cBlueBits,
            alpha_bits: output.cAlphaBits,
            depth_bits: output.cDepthBits,
            stencil_bits: output.cStencilBits,
            stereoscopy: (output.dwFlags & winapi::PFD_STEREO) != 0,
            double_buffer: (output.dwFlags & winapi::PFD_DOUBLEBUFFER) != 0,
            multisampling: None,
            srgb: false,
        }, index));
    }

    result
}

unsafe fn enumerate_arb_pixel_formats(extra: &gl::wgl_extra::Wgl, hdc: winapi::HDC)
                                      -> Vec<(PixelFormat, libc::c_int)>
{
    let get_info = |index: u32, attrib: u32| {
        let mut value = mem::uninitialized();
        extra.GetPixelFormatAttribivARB(hdc as *const _, index as libc::c_int,
                                        0, 1, [attrib as libc::c_int].as_ptr(),
                                        &mut value);
        value as u32
    };

    // getting the number of formats
    // the `1` is ignored
    let num = get_info(1, gl::wgl_extra::NUMBER_PIXEL_FORMATS_ARB);

    let mut result = Vec::new();

    for index in (0 .. num) {
        if get_info(index, gl::wgl_extra::DRAW_TO_WINDOW_ARB) == 0 {
            continue;
        }
        if get_info(index, gl::wgl_extra::SUPPORT_OPENGL_ARB) == 0 {
            continue;
        }

        if get_info(index, gl::wgl_extra::ACCELERATION_ARB) == gl::wgl_extra::NO_ACCELERATION_ARB {
            continue;
        }

        if get_info(index, gl::wgl_extra::PIXEL_TYPE_ARB) != gl::wgl_extra::TYPE_RGBA_ARB {
            continue;
        }

        result.push((PixelFormat {
            hardware_accelerated: true,
            color_bits: get_info(index, gl::wgl_extra::RED_BITS_ARB) as u8 + 
                        get_info(index, gl::wgl_extra::GREEN_BITS_ARB) as u8 +
                        get_info(index, gl::wgl_extra::BLUE_BITS_ARB) as u8,
            alpha_bits: get_info(index, gl::wgl_extra::ALPHA_BITS_ARB) as u8,
            depth_bits: get_info(index, gl::wgl_extra::DEPTH_BITS_ARB) as u8,
            stencil_bits: get_info(index, gl::wgl_extra::STENCIL_BITS_ARB) as u8,
            stereoscopy: get_info(index, gl::wgl_extra::STEREO_ARB) != 0,
            double_buffer: get_info(index, gl::wgl_extra::DOUBLE_BUFFER_ARB) != 0,
            multisampling: {
                if is_extension_supported(extra, hdc, "WGL_ARB_multisample") {
                    match get_info(index, gl::wgl_extra::SAMPLES_ARB) {
                        0 => None,
                        a => Some(a as u16),
                    }
                } else {
                    None
                }
            },
            srgb: if is_extension_supported(extra, hdc, "WGL_ARB_framebuffer_sRGB") {
                get_info(index, gl::wgl_extra::FRAMEBUFFER_SRGB_CAPABLE_ARB) != 0
            } else if is_extension_supported(extra, hdc, "WGL_EXT_framebuffer_sRGB") {
                get_info(index, gl::wgl_extra::FRAMEBUFFER_SRGB_CAPABLE_EXT) != 0
            } else {
                false
            },
        }, index as libc::c_int));
    }

    result
}

unsafe fn set_pixel_format(hdc: winapi::HDC, id: libc::c_int) -> Result<(), CreationError> {
    let mut output: winapi::PIXELFORMATDESCRIPTOR = mem::zeroed();

    if gdi32::DescribePixelFormat(hdc, id, mem::size_of::<winapi::PIXELFORMATDESCRIPTOR>()
                                  as winapi::UINT, &mut output) == 0
    {
        return Err(CreationError::OsError(format!("DescribePixelFormat function failed: {}",
                                                  format!("{}", io::Error::last_os_error()))));
    }

    if gdi32::SetPixelFormat(hdc, id, &output) == 0 {
        return Err(CreationError::OsError(format!("SetPixelFormat function failed: {}",
                                                  format!("{}", io::Error::last_os_error()))));
    }

    Ok(())
}

unsafe fn load_opengl32_dll() -> Result<winapi::HMODULE, CreationError> {
    let name = OsStr::new("opengl32.dll").encode_wide().chain(Some(0).into_iter())
                                         .collect::<Vec<_>>();

    let lib = kernel32::LoadLibraryW(name.as_ptr());

    if lib.is_null() {
        return Err(CreationError::OsError(format!("LoadLibrary function failed: {}",
                                                  format!("{}", io::Error::last_os_error()))));
    }

    Ok(lib)
}

unsafe fn is_extension_supported(extra: &gl::wgl_extra::Wgl, hdc: winapi::HDC,
                                 extension: &str) -> bool
{
    let extensions = if extra.GetExtensionsStringARB.is_loaded() {
        let data = extra.GetExtensionsStringARB(hdc as *const _);
        let data = CStr::from_ptr(data).to_bytes().to_vec();
        String::from_utf8(data).unwrap()

    } else if extra.GetExtensionsStringEXT.is_loaded() {
        let data = extra.GetExtensionsStringEXT();
        let data = CStr::from_ptr(data).to_bytes().to_vec();
        String::from_utf8(data).unwrap()

    } else {
        return false;
    };

    extensions.split(" ").find(|&e| e == extension).is_some()
}

fn choose_dummy_pixel_format<I>(iter: I) -> Result<libc::c_int, CreationError>
                                where I: Iterator<Item=(PixelFormat, libc::c_int)>
{
    let mut backup_id = None;

    for (format, id) in iter {
        if backup_id.is_none() {
            backup_id = Some(id);
        }

        if format.hardware_accelerated {
            return Ok(id);
        }
    }

    backup_id.ok_or(CreationError::NotSupported)
}
