use ContextError;
use CreationError;
use CreationError::OsError;
use GlAttributes;
use GlContext;
use PixelFormatRequirements;

use core_foundation::base::TCFType;
use core_foundation::string::CFString;
use core_foundation::bundle::{CFBundleGetBundleWithIdentifier, CFBundleGetFunctionPointerForName};
use cocoa::base::{id, nil};
use cocoa::appkit::*;
use PixelFormat;
use api::cocoa::helpers;

#[derive(Clone, Default)]
pub struct PlatformSpecificHeadlessBuilderAttributes;

pub struct HeadlessContext {
    context: id,
}

impl HeadlessContext {
    pub fn new((width, height): (u32, u32), pf_reqs: &PixelFormatRequirements,
               opengl: &GlAttributes<&HeadlessContext>,
               _: &PlatformSpecificHeadlessBuilderAttributes)
               -> Result<HeadlessContext, CreationError>
    {
        let context = unsafe {

            let attributes = try!(helpers::build_nsattributes(pf_reqs, opengl));

            let pixelformat = NSOpenGLPixelFormat::alloc(nil).initWithAttributes_(&attributes);
            if pixelformat == nil {
                return Err(OsError(format!("Could not create the pixel format")));
            }
            let context = NSOpenGLContext::alloc(nil).initWithFormat_shareContext_(pixelformat, nil);
            if context == nil {
                return Err(OsError(format!("Could not create the rendering context")));
            }
            context
        };

        let headless = HeadlessContext {
            context: context,
        };

        Ok(headless)
    }
}

impl GlContext for HeadlessContext {
    unsafe fn make_current(&self) -> Result<(), ContextError> {
        self.context.makeCurrentContext();
        Ok(())
    }

    #[inline]
    fn is_current(&self) -> bool {
        unimplemented!()
    }

    #[inline]
    fn get_proc_address(&self, _addr: &str) -> *const () {
        let symbol_name: CFString = _addr.parse().unwrap();
        let framework_name: CFString = "com.apple.opengl".parse().unwrap();
        let framework = unsafe {
            CFBundleGetBundleWithIdentifier(framework_name.as_concrete_TypeRef())
        };
        let symbol = unsafe {
            CFBundleGetFunctionPointerForName(framework, symbol_name.as_concrete_TypeRef())
        };
        symbol as *const ()
    }

    #[inline]
    fn swap_buffers(&self) -> Result<(), ContextError> {
        unsafe { self.context.flushBuffer(); }
        Ok(())
    }

    #[inline]
    fn get_api(&self) -> ::Api {
        ::Api::OpenGl
    }

    #[inline]
    fn get_pixel_format(&self) -> PixelFormat {
        unimplemented!();
    }
}

unsafe impl Send for HeadlessContext {}
unsafe impl Sync for HeadlessContext {}
