use Api;
use BuilderAttribs;
use CreationError;
use GlRequest;

use gl_common;
use libc;

use platform;

/// Object that allows you to build headless contexts.
pub struct HeadlessRendererBuilder {
    attribs: BuilderAttribs<'static>,
}

impl HeadlessRendererBuilder {
    /// Initializes a new `HeadlessRendererBuilder` with default values.
    pub fn new(width: u32, height: u32) -> HeadlessRendererBuilder {
        HeadlessRendererBuilder {
            attribs: BuilderAttribs {
                headless: true,
                dimensions: Some((width, height)),
                .. BuilderAttribs::new()
            },
        }
    }

    /// Sets how the backend should choose the OpenGL API and version.
    pub fn with_gl(mut self, request: GlRequest) -> HeadlessRendererBuilder {
        self.attribs.gl_version = request;
        self
    }

    /// Sets the *debug* flag for the OpenGL context.
    ///
    /// The default value for this flag is `cfg!(ndebug)`, which means that it's enabled
    /// when you run `cargo build` and disabled when you run `cargo build --release`.
    pub fn with_gl_debug_flag(mut self, flag: bool) -> HeadlessRendererBuilder {
        self.attribs.gl_debug = flag;
        self
    }

    /// Builds the headless context.
    ///
    /// Error should be very rare and only occur in case of permission denied, incompatible system,
    ///  out of memory, etc.
    pub fn build(self) -> Result<HeadlessContext, CreationError> {
        platform::HeadlessContext::new(self.attribs).map(|w| HeadlessContext { context: w })
    }

    /// Builds the headless context.
    ///
    /// The context is build in a *strict* way. That means that if the backend couldn't give
    /// you what you requested, an `Err` will be returned.
    pub fn build_strict(mut self) -> Result<HeadlessContext, CreationError> {
        self.attribs.strict = true;
        self.build()
    }
}

/// Represents a headless OpenGL context.
pub struct HeadlessContext {
    context: platform::HeadlessContext,
}

impl HeadlessContext {
    /// Creates a new OpenGL context
    /// Sets the context as the current context.
    #[inline]
    pub unsafe fn make_current(&self) {
        self.context.make_current()
    }
    
    /// Returns true if this context is the current one in this thread.
    #[inline]
    pub fn is_current(&self) -> bool {
        self.context.is_current()
    }

    /// Returns the address of an OpenGL function.
    ///
    /// Contrary to `wglGetProcAddress`, all available OpenGL functions return an address.
    #[inline]
    pub fn get_proc_address(&self, addr: &str) -> *const libc::c_void {
        self.context.get_proc_address(addr) as *const libc::c_void
    }

    /// Returns the API that is currently provided by this window.
    ///
    /// See `Window::get_api` for more infos.
    pub fn get_api(&self) -> Api {
        self.context.get_api()
    }

    pub fn set_window_resize_callback(&mut self, _: Option<fn(u32, u32)>) {
    }
}

impl gl_common::GlFunctionsSource for HeadlessContext {
    fn get_proc_addr(&self, addr: &str) -> *const libc::c_void {
        self.get_proc_address(addr)
    }
}
