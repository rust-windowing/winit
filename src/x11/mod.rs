use {Event, Hints};
use libc;
use std::{mem, ptr};

mod ffi;

pub struct Window {
    display: *mut ffi::Display,
    window: ffi::Window,
    context: ffi::GLXContext,
}

impl Window {
    pub fn new(dimensions: Option<(uint, uint)>, title: &str, hints: &Hints)
        -> Result<Window, String>
    {
        // calling XOpenDisplay
        let display = unsafe {
            let display = ffi::XOpenDisplay(ptr::null());
            if display.is_null() {
                return Err(format!("XOpenDisplay failed"));
            }
            display
        };

        // TODO: set error handler

        static VISUAL_ATTRIBUTES: [libc::c_int, ..5] = [
            ffi::GLX_RGBA,
            ffi::GLX_DEPTH_SIZE,
            24,
            ffi::GLX_DOUBLEBUFFER,
            0
        ];

        // getting the visual infos
        let visual_infos = unsafe {
            let vi = ffi::glXChooseVisual(display, 0, VISUAL_ATTRIBUTES.as_ptr());
            if vi.is_null() {
                return Err(format!("glXChooseVisual failed"));
            }
            vi
        };

        // getting the root window
        let root = unsafe { ffi::XDefaultRootWindow(display) };

        // creating the color map
        let cmap = unsafe {
            let cmap = ffi::XCreateColormap(display, root,
                (*visual_infos).visual, ffi::AllocNone);
            // TODO: error checking?
            cmap
        };

        // creating
        let mut set_win_attr = {
            let mut swa: ffi::XSetWindowAttributes = unsafe { mem::zeroed() };
            swa.colormap = cmap;
            //swa.event_mask = ExposureMask | KeyPressMask;
            swa
        };

        // finally creating the window
        let window = unsafe {
            let win = ffi::XCreateWindow(display, root, 10, 10, 800, 600,
                0, (*visual_infos).depth, ffi::InputOutput, (*visual_infos).visual,
                ffi::CWColormap/* | ffi::CWEventMask*/, &mut set_win_attr);
            win
        };

        // showing window
        unsafe { ffi::XMapWindow(display, window) };
        unsafe { ffi::XStoreName(display, window, mem::transmute(title.as_slice().as_ptr())); }
        unsafe { ffi::XFlush(display); }

        // creating GL context
        let context = unsafe {
            ffi::glXCreateContext(display, visual_infos, ptr::null(), 1)
        };

        // returning
        Ok(Window{
            display: display,
            window: window,
            context: context,
        })
    }

    pub fn should_close(&self) -> bool {
        // TODO: 
        false
    }

    pub fn set_title(&self, title: &str) {
        unsafe {
            ffi::XStoreName(self.display, self.window,
                mem::transmute(title.as_slice().as_ptr()));
        }
    }

    pub fn get_position(&self) -> (uint, uint) {
        unimplemented!()
    }

    pub fn set_position(&self, x: uint, y: uint) {
        unimplemented!()
    }

    pub fn get_size(&self) -> (uint, uint) {
        unimplemented!()
    }

    pub fn set_size(&self, x: uint, y: uint) {
        unimplemented!()
    }

    pub fn poll_events(&self) -> Vec<Event> {
        unimplemented!()
    }

    pub fn wait_events(&self) -> Vec<Event> {
        // TODO: 
        Vec::new()
    }

    pub fn make_current(&self) {
        let res = unsafe { ffi::glXMakeCurrent(self.display, self.window, self.context) };
        if res == 0 {
            fail!("glXMakeCurrent failed");
        }
    }

    pub fn get_proc_address(&self, addr: &str) -> *const () {
        use std::c_str::ToCStr;
        use std::mem;

        unsafe {
            addr.with_c_str(|s| {
                let p = ffi::glXGetProcAddress(mem::transmute(s)) as *const ();
                if !p.is_null() { return p; }
                println!("{}", p);
                p
            })
        }
    }

    pub fn swap_buffers(&self) {
        unsafe { ffi::glXSwapBuffers(self.display, self.window) }
    }
}

impl Drop for Window {
    fn drop(&mut self) {
        unsafe { ffi::XCloseDisplay(self.display) }
    }
}



/*
  printf( "Getting matching framebuffer configs\n" );
  int fbcount;
  GLXFBConfig* fbc = glXChooseFBConfig(display, DefaultScreen(display), visual_attribs, &fbcount);
  if (!fbc)
  {
    printf( "Failed to retrieve a framebuffer config\n" );
    exit(1);
  }
  printf( "Found %d matching FB configs.\n", fbcount );
 
  // Pick the FB config/visual with the most samples per pixel
  printf( "Getting XVisualInfos\n" );
  int best_fbc = -1, worst_fbc = -1, best_num_samp = -1, worst_num_samp = 999;
 
  int i;
  for (i=0; i<fbcount; ++i)
  {
    XVisualInfo *vi = glXGetVisualFromFBConfig( display, fbc[i] );
    if ( vi )
    {
      int samp_buf, samples;
      glXGetFBConfigAttrib( display, fbc[i], GLX_SAMPLE_BUFFERS, &samp_buf );
      glXGetFBConfigAttrib( display, fbc[i], GLX_SAMPLES       , &samples  );
 
      printf( "  Matching fbconfig %d, visual ID 0x%2x: SAMPLE_BUFFERS = %d,"
              " SAMPLES = %d\n", 
              i, vi -> visualid, samp_buf, samples );
 
      if ( best_fbc < 0 || samp_buf && samples > best_num_samp )
        best_fbc = i, best_num_samp = samples;
      if ( worst_fbc < 0 || !samp_buf || samples < worst_num_samp )
        worst_fbc = i, worst_num_samp = samples;
    }
    XFree( vi );
  }
 
  GLXFBConfig bestFbc = fbc[ best_fbc ];
 
  // Be sure to free the FBConfig list allocated by glXChooseFBConfig()
  XFree( fbc );
 
  // Get a visual
  XVisualInfo *vi = glXGetVisualFromFBConfig( display, bestFbc );
  printf( "Chosen visual ID = 0x%x\n", vi->visualid );
 
  printf( "Creating colormap\n" );
  XSetWindowAttributes swa;
  Colormap cmap;
  swa.colormap = cmap = XCreateColormap( display,
                                         RootWindow( display, vi->screen ), 
                                         vi->visual, AllocNone );
  swa.background_pixmap = None ;
  swa.border_pixel      = 0;
  swa.event_mask        = StructureNotifyMask;
 
  printf( "Creating window\n" );
  Window win = XCreateWindow( display, RootWindow( display, vi->screen ), 
                              0, 0, 100, 100, 0, vi->depth, InputOutput, 
                              vi->visual, 
                              CWBorderPixel|CWColormap|CWEventMask, &swa );
  if ( !win )
  {
    printf( "Failed to create window.\n" );
    exit(1);
  }
 
  // Done with the visual info data
  XFree( vi );
 
  XStoreName( display, win, "GL 3.0 Window" );
 
  printf( "Mapping window\n" );
  XMapWindow( display, win );
 
  // Get the default screen's GLX extension list
  const char *glxExts = glXQueryExtensionsString( display,
                                                  DefaultScreen( display ) );
 
  // NOTE: It is not necessary to create or make current to a context before
  // calling glXGetProcAddressARB
  glXCreateContextAttribsARBProc glXCreateContextAttribsARB = 0;
  glXCreateContextAttribsARB = (glXCreateContextAttribsARBProc)
           glXGetProcAddressARB( (const GLubyte *) "glXCreateContextAttribsARB" );
 
  GLXContext ctx = 0;
 
  // Install an X error handler so the application won't exit if GL 3.0
  // context allocation fails.
  //
  // Note this error handler is global.  All display connections in all threads
  // of a process use the same error handler, so be sure to guard against other
  // threads issuing X commands while this code is running.
  ctxErrorOccurred = false;
  int (*oldHandler)(Display*, XErrorEvent*) =
      XSetErrorHandler(&ctxErrorHandler);
 
  // Check for the GLX_ARB_create_context extension string and the function.
  // If either is not present, use GLX 1.3 context creation method.
  if ( !isExtensionSupported( glxExts, "GLX_ARB_create_context" ) ||
       !glXCreateContextAttribsARB )
  {
    printf( "glXCreateContextAttribsARB() not found"
            " ... using old-style GLX context\n" );
    ctx = glXCreateNewContext( display, bestFbc, GLX_RGBA_TYPE, 0, True );
  }
 
  // If it does, try to get a GL 3.0 context!
  else
  {
    int context_attribs[] =
      {
        GLX_CONTEXT_MAJOR_VERSION_ARB, 3,
        GLX_CONTEXT_MINOR_VERSION_ARB, 0,
        //GLX_CONTEXT_FLAGS_ARB        , GLX_CONTEXT_FORWARD_COMPATIBLE_BIT_ARB,
        None
      };
 
    printf( "Creating context\n" );
    ctx = glXCreateContextAttribsARB( display, bestFbc, 0,
                                      True, context_attribs );
 
    // Sync to ensure any errors generated are processed.
    XSync( display, False );
    if ( !ctxErrorOccurred && ctx )
      printf( "Created GL 3.0 context\n" );
    else
    {
      // Couldn't create GL 3.0 context.  Fall back to old-style 2.x context.
      // When a context version below 3.0 is requested, implementations will
      // return the newest context version compatible with OpenGL versions less
      // than version 3.0.
      // GLX_CONTEXT_MAJOR_VERSION_ARB = 1
      context_attribs[1] = 1;
      // GLX_CONTEXT_MINOR_VERSION_ARB = 0
      context_attribs[3] = 0;
 
      ctxErrorOccurred = false;
 
      printf( "Failed to create GL 3.0 context"
              " ... using old-style GLX context\n" );
      ctx = glXCreateContextAttribsARB( display, bestFbc, 0, 
                                        True, context_attribs );
    }
  }
 
  // Sync to ensure any errors generated are processed.
  XSync( display, False );
 
  // Restore the original error handler
  XSetErrorHandler( oldHandler );
 
  if ( ctxErrorOccurred || !ctx )
  {
    printf( "Failed to create an OpenGL context\n" );
    exit(1);
  }
 
  // Verifying that context is a direct context
  if ( ! glXIsDirect ( display, ctx ) )
  {
    printf( "Indirect GLX rendering context obtained\n" );
  }
  else
  {
    printf( "Direct GLX rendering context obtained\n" );
  }
 
  printf( "Making context current\n" );
  glXMakeCurrent( display, win, ctx );
 
  glClearColor( 0, 0.5, 1, 1 );
  glClear( GL_COLOR_BUFFER_BIT );
  glXSwapBuffers ( display, win );

*/