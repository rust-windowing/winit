use std::sync::atomic::AtomicBool;
use std::ptr;
use std::mem;
use std::os;
use super::callback;
use super::Window;
use super::MonitorID;

use Api;
use BuilderAttribs;
use CreationError;
use CreationError::OsError;
use GlRequest;
use PixelFormat;

use std::ffi::CString;
use std::sync::mpsc::channel;

use libc;
use super::gl;
use winapi;
use kernel32;
use user32;
use gdi32;

/// Work-around the fact that HGLRC doesn't implement Send
pub struct ContextHack(pub winapi::HGLRC);
unsafe impl Send for ContextHack {}

pub fn new_window(builder: BuilderAttribs<'static>, builder_sharelists: Option<ContextHack>)
                  -> Result<Window, CreationError>
{
    // initializing variables to be sent to the task
    let title = builder.title.as_slice().utf16_units()
        .chain(Some(0).into_iter()).collect::<Vec<u16>>();    // title to utf16
    //let hints = hints.clone();
    let (tx, rx) = channel();

    // GetMessage must be called in the same thread as CreateWindow,
    //  so we create a new thread dedicated to this window.
    // This is the only safe method. Using `nosend` wouldn't work for non-native runtime.
    ::std::thread::Thread::spawn(move || {
        unsafe {
            // sending
            match init(title, builder, builder_sharelists) {
                Ok(w) => tx.send(Ok(w)).ok(),
                Err(e) => {
                    tx.send(Err(e)).ok();
                    return;
                }
            };

            // now that the `Window` struct is initialized, the main `Window::new()` function will
            //  return and this events loop will run in parallel
            loop {
                let mut msg = mem::uninitialized();

                if user32::GetMessageW(&mut msg, ptr::null_mut(), 0, 0) == 0 {
                    break;
                }

                user32::TranslateMessage(&msg);
                user32::DispatchMessageW(&msg);     // calls `callback` (see below)
            }
        }
    });

    rx.recv().unwrap()
}

unsafe fn init(title: Vec<u16>, builder: BuilderAttribs<'static>,
               builder_sharelists: Option<ContextHack>) -> Result<Window, CreationError>
{
    let builder_sharelists = builder_sharelists.map(|s| s.0);

    // registering the window class
    let class_name = register_window_class();

    // building a RECT object with coordinates
    let mut rect = winapi::RECT {
        left: 0, right: builder.dimensions.unwrap_or((1024, 768)).0 as winapi::LONG,
        top: 0, bottom: builder.dimensions.unwrap_or((1024, 768)).1 as winapi::LONG,
    };

    // switching to fullscreen if necessary
    // this means adjusting the window's position so that it overlaps the right monitor,
    //  and change the monitor's resolution if necessary
    if builder.monitor.is_some() {
        let monitor = builder.monitor.as_ref().unwrap();
        try!(switch_to_fullscreen(&mut rect, monitor));
    }

    // computing the style and extended style of the window
    let (ex_style, style) = if builder.monitor.is_some() {
        (winapi::WS_EX_APPWINDOW, winapi::WS_POPUP | winapi::WS_CLIPSIBLINGS | winapi::WS_CLIPCHILDREN)
    } else {
        (winapi::WS_EX_APPWINDOW | winapi::WS_EX_WINDOWEDGE,
            winapi::WS_OVERLAPPEDWINDOW | winapi::WS_CLIPSIBLINGS | winapi::WS_CLIPCHILDREN)
    };

    // adjusting the window coordinates using the style
    user32::AdjustWindowRectEx(&mut rect, style, 0, ex_style);

    // getting the address of wglCreateContextAttribsARB
    let extra_functions = {
        // creating a dummy invisible window for GL initialization
        let dummy_window = {
            let handle = user32::CreateWindowExW(ex_style, class_name.as_ptr(),
                title.as_ptr() as winapi::LPCWSTR,
                style | winapi::WS_CLIPSIBLINGS | winapi::WS_CLIPCHILDREN,
                winapi::CW_USEDEFAULT, winapi::CW_USEDEFAULT,
                rect.right - rect.left, rect.bottom - rect.top,
                ptr::null_mut(), ptr::null_mut(), kernel32::GetModuleHandleW(ptr::null()),
                ptr::null_mut());

            if handle.is_null() {
                return Err(OsError(format!("CreateWindowEx function failed: {}",
                    os::error_string(os::errno()))));
            }

            handle
        };

        // getting the HDC of the dummy window
        let dummy_hdc = {
            let hdc = user32::GetDC(dummy_window);
            if hdc.is_null() {
                let err = Err(OsError(format!("GetDC function failed: {}",
                    os::error_string(os::errno()))));
                user32::DestroyWindow(dummy_window);
                return err;
            }
            hdc
        };

        // getting the pixel format that we will use and setting it
        {
            let formats = enumerate_native_pixel_formats(dummy_hdc);
            let (id, _) = builder.choose_pixel_format(formats.into_iter().map(|(a, b)| (b, a)));
            try!(set_pixel_format(dummy_hdc, id));
        }

        // creating the dummy OpenGL context
        let dummy_context = try!(create_context(None, dummy_hdc, None));

        // making context current
        gl::wgl::MakeCurrent(dummy_hdc as *const libc::c_void,
                             dummy_context as *const libc::c_void);

        // loading the extra WGL functions
        let extra_functions = gl::wgl_extra::Wgl::load_with(|addr| {
            use libc;

            let addr = CString::from_slice(addr.as_bytes());
            let addr = addr.as_ptr();

            gl::wgl::GetProcAddress(addr) as *const libc::c_void
        });

        // removing current context
        gl::wgl::MakeCurrent(ptr::null(), ptr::null());

        // destroying the context and the window
        gl::wgl::DeleteContext(dummy_context as *const libc::c_void);
        user32::DestroyWindow(dummy_window);

        // returning the address
        extra_functions
    };

    // creating the real window this time
    let real_window = {
        let (width, height) = if builder.monitor.is_some() || builder.dimensions.is_some() {
            (Some(rect.right - rect.left), Some(rect.bottom - rect.top))
        } else {
            (None, None)
        };

        let style = if !builder.visible || builder.headless {
            style
        } else {
            style | winapi::WS_VISIBLE
        };

        let handle = user32::CreateWindowExW(ex_style, class_name.as_ptr(),
            title.as_ptr() as winapi::LPCWSTR,
            style | winapi::WS_CLIPSIBLINGS | winapi::WS_CLIPCHILDREN,
            if builder.monitor.is_some() { 0 } else { winapi::CW_USEDEFAULT },
            if builder.monitor.is_some() { 0 } else { winapi::CW_USEDEFAULT },
            width.unwrap_or(winapi::CW_USEDEFAULT), height.unwrap_or(winapi::CW_USEDEFAULT),
            ptr::null_mut(), ptr::null_mut(), kernel32::GetModuleHandleW(ptr::null()),
            ptr::null_mut());

        if handle.is_null() {
            return Err(OsError(format!("CreateWindowEx function failed: {}",
                                       os::error_string(os::errno()))));
        }

        handle
    };

    // getting the HDC of the window
    let hdc = {
        let hdc = user32::GetDC(real_window);
        if hdc.is_null() {
            let err = Err(OsError(format!("GetDC function failed: {}",
                                          os::error_string(os::errno()))));
            user32::DestroyWindow(real_window);
            return err;
        }
        hdc
    };

    // calling SetPixelFormat
    {
        let formats = if extra_functions.GetPixelFormatAttribivARB.is_loaded() {
            enumerate_arb_pixel_formats(&extra_functions, hdc)
        } else {
            enumerate_native_pixel_formats(hdc)
        };

        let (id, _) = builder.choose_pixel_format(formats.into_iter().map(|(a, b)| (b, a)));
        try!(set_pixel_format(hdc, id));
    }

    // creating the OpenGL context
    let context = try!(create_context(Some((&extra_functions, &builder)), hdc, builder_sharelists));

    // calling SetForegroundWindow if fullscreen
    if builder.monitor.is_some() {
        user32::SetForegroundWindow(real_window);
    }

    // filling the WINDOW task-local storage
    let events_receiver = {
        let (tx, rx) = channel();
        let mut tx = Some(tx);
        callback::WINDOW.with(|window| {
            (*window.borrow_mut()) = Some((real_window, tx.take().unwrap()));
        });
        rx
    };

    // loading the opengl32 module
    let gl_library = try!(load_opengl32_dll());

    // handling vsync
    if builder.vsync {
        if extra_functions.SwapIntervalEXT.is_loaded() {
            gl::wgl::MakeCurrent(hdc as *const libc::c_void, context as *const libc::c_void);
            if extra_functions.SwapIntervalEXT(1) == 0 {
                gl::wgl::DeleteContext(context as *const libc::c_void);
                user32::DestroyWindow(real_window);
                return Err(OsError(format!("wglSwapIntervalEXT failed")));
            }

            // it is important to remove the current context, otherwise you get very weird
            // errors
            gl::wgl::MakeCurrent(ptr::null(), ptr::null());
        }
    }

    // building the struct
    Ok(Window {
        window: real_window,
        hdc: hdc as winapi::HDC,
        context: context,
        gl_library: gl_library,
        events_receiver: events_receiver,
        is_closed: AtomicBool::new(false),
    })
}

unsafe fn register_window_class() -> Vec<u16> {
    let class_name: Vec<u16> = "Window Class".utf16_units().chain(Some(0).into_iter())
                                             .collect::<Vec<u16>>();
    
    let class = winapi::WNDCLASSEXW {
        cbSize: mem::size_of::<winapi::WNDCLASSEXW>() as winapi::UINT,
        style: winapi::CS_HREDRAW | winapi::CS_VREDRAW | winapi::CS_OWNDC,
        lpfnWndProc: Some(callback::callback),
        cbClsExtra: 0,
        cbWndExtra: 0,
        hInstance: kernel32::GetModuleHandleW(ptr::null()),
        hIcon: ptr::null_mut(),
        hCursor: ptr::null_mut(),
        hbrBackground: ptr::null_mut(),
        lpszMenuName: ptr::null(),
        lpszClassName: class_name.as_ptr(),
        hIconSm: ptr::null_mut(),
    };

    // We ignore errors because registering the same window class twice would trigger
    //  an error, and because errors here are detected during CreateWindowEx anyway.
    // Also since there is no weird element in the struct, there is no reason for this
    //  call to fail.
    user32::RegisterClassExW(&class);

    class_name
}

unsafe fn switch_to_fullscreen(rect: &mut winapi::RECT, monitor: &MonitorID)
                               -> Result<(), CreationError>
{
    // adjusting the rect
    {
        let pos = monitor.get_position();
        rect.left += pos.0 as winapi::LONG;
        rect.right += pos.0 as winapi::LONG;
        rect.top += pos.1 as winapi::LONG;
        rect.bottom += pos.1 as winapi::LONG;
    }

    // changing device settings
    let mut screen_settings: winapi::DEVMODEW = mem::zeroed();
    screen_settings.dmSize = mem::size_of::<winapi::DEVMODEW>() as winapi::WORD;
    screen_settings.dmPelsWidth = (rect.right - rect.left) as winapi::DWORD;
    screen_settings.dmPelsHeight = (rect.bottom - rect.top) as winapi::DWORD;
    screen_settings.dmBitsPerPel = 32;      // TODO: ?
    screen_settings.dmFields = winapi::DM_BITSPERPEL | winapi::DM_PELSWIDTH | winapi::DM_PELSHEIGHT;

    let result = user32::ChangeDisplaySettingsExW(monitor.get_system_name().as_ptr(),
                                                  &mut screen_settings, ptr::null_mut(),
                                                  winapi::CDS_FULLSCREEN, ptr::null_mut());
    
    if result != winapi::DISP_CHANGE_SUCCESSFUL {
        return Err(OsError(format!("ChangeDisplaySettings failed: {}", result)));
    }

    Ok(())
}

unsafe fn create_context(extra: Option<(&gl::wgl_extra::Wgl, &BuilderAttribs<'static>)>,
                         hdc: winapi::HDC, share: Option<winapi::HGLRC>)
                         -> Result<winapi::HGLRC, CreationError>
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
                GlRequest::Specific(_, _) => panic!("Only OpenGL is supported"),
                GlRequest::GlThenGles { opengl_version: (major, minor), .. } => {
                    attributes.push(gl::wgl_extra::CONTEXT_MAJOR_VERSION_ARB as libc::c_int);
                    attributes.push(major as libc::c_int);
                    attributes.push(gl::wgl_extra::CONTEXT_MINOR_VERSION_ARB as libc::c_int);
                    attributes.push(minor as libc::c_int);
                },
            }

            if builder.gl_debug {
                attributes.push(gl::wgl_extra::CONTEXT_FLAGS_ARB as libc::c_int);
                attributes.push(gl::wgl_extra::CONTEXT_DEBUG_BIT_ARB as libc::c_int);
            }

            attributes.push(0);

            Some(extra_functions.CreateContextAttribsARB(hdc as *const libc::c_void,
                                                         share as *const libc::c_void,
                                                         attributes.as_slice().as_ptr()))

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
        return Err(OsError(format!("OpenGL context creation failed: {}",
                           os::error_string(os::errno()))));
    }

    Ok(ctxt as winapi::HGLRC)
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
            red_bits: output.cRedBits,
            green_bits: output.cGreenBits,
            blue_bits: output.cBlueBits,
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
        extra.GetPixelFormatAttribivARB(hdc as *const libc::c_void, index as libc::c_int,
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
            red_bits: get_info(index, gl::wgl_extra::RED_BITS_ARB) as u8,
            green_bits: get_info(index, gl::wgl_extra::GREEN_BITS_ARB) as u8,
            blue_bits: get_info(index, gl::wgl_extra::BLUE_BITS_ARB) as u8,
            alpha_bits: get_info(index, gl::wgl_extra::ALPHA_BITS_ARB) as u8,
            depth_bits: get_info(index, gl::wgl_extra::DEPTH_BITS_ARB) as u8,
            stencil_bits: get_info(index, gl::wgl_extra::STENCIL_BITS_ARB) as u8,
            stereoscopy: get_info(index, gl::wgl_extra::STEREO_ARB) != 0,
            double_buffer: get_info(index, gl::wgl_extra::DOUBLE_BUFFER_ARB) != 0,
            multisampling: None,        // FIXME: 
            srgb: false,        // FIXME: 
        }, index as libc::c_int));
    }

    result
}

unsafe fn set_pixel_format(hdc: winapi::HDC, id: libc::c_int) -> Result<(), CreationError> {
    let mut output: winapi::PIXELFORMATDESCRIPTOR = mem::zeroed();

    if gdi32::DescribePixelFormat(hdc, id, mem::size_of::<winapi::PIXELFORMATDESCRIPTOR>()
                                  as winapi::UINT, &mut output) == 0
    {
        return Err(OsError(format!("DescribePixelFormat function failed: {}",
                                   os::error_string(os::errno()))));
    }

    if gdi32::SetPixelFormat(hdc, id, &output) == 0 {
        return Err(OsError(format!("SetPixelFormat function failed: {}",
                                   os::error_string(os::errno()))));
    }

    Ok(())
}

unsafe fn load_opengl32_dll() -> Result<winapi::HMODULE, CreationError> {
    let name = "opengl32.dll".utf16_units().chain(Some(0).into_iter())
                             .collect::<Vec<u16>>().as_ptr();

    let lib = kernel32::LoadLibraryW(name);

    if lib.is_null() {
        return Err(OsError(format!("LoadLibrary function failed: {}",
                                    os::error_string(os::errno()))));
    }

    Ok(lib)
}
