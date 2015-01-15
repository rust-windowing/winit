use CreationError;
use CreationError::OsError;
use BuilderAttribs;
use libc;
use std::ptr;

use core_foundation::base::TCFType;
use core_foundation::string::CFString;
use core_foundation::bundle::{CFBundleGetBundleWithIdentifier, CFBundleGetFunctionPointerForName};
use cocoa::base::{id, nil};
use cocoa::appkit::*;

mod gl {
    generate_gl_bindings! {
        api: "gl",
        profile: "core",
        version: "3.2",
        generator: "global",
        extensions: ["GL_EXT_framebuffer_object"],
    }
}

static mut framebuffer: u32 = 0;
static mut texture: u32 = 0;

pub struct HeadlessContext {
    width: usize,
    height: usize,
    context: id,
}

impl HeadlessContext {
    pub fn new(builder: BuilderAttribs) -> Result<HeadlessContext, CreationError> {
        let (width, height) = builder.dimensions;
        let context = unsafe {
            let attributes = [
                NSOpenGLPFADoubleBuffer as usize,
                NSOpenGLPFAClosestPolicy as usize,
                NSOpenGLPFAColorSize as usize, 24,
                NSOpenGLPFAAlphaSize as usize, 8,
                NSOpenGLPFADepthSize as usize, 24,
                NSOpenGLPFAStencilSize as usize, 8,
                NSOpenGLPFAOffScreen as usize,
                0
            ];

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
            width: width,
            height: height,
            context: context,
        };

        // Load the function pointers as we need them to create the FBO
        gl::load_with(|s| headless.get_proc_address(s) as *const libc::c_void);

        Ok(headless)
    }

    pub unsafe fn make_current(&self) {
        self.context.makeCurrentContext();

        gl::GenFramebuffersEXT(1, &mut framebuffer);
        gl::BindFramebufferEXT(gl::FRAMEBUFFER_EXT, framebuffer);
        gl::GenTextures(1, &mut texture);
        gl::BindTexture(gl::TEXTURE_2D, texture);
        gl::TexParameteri(gl::TEXTURE_2D, gl::TEXTURE_MAG_FILTER, gl::LINEAR as i32);
        gl::TexParameteri(gl::TEXTURE_2D, gl::TEXTURE_MIN_FILTER, gl::LINEAR as i32);
        gl::TexImage2D(gl::TEXTURE_2D, 0, gl::RGBA8 as i32, self.width as i32, self.height as i32,
                       0, gl::RGBA, gl::UNSIGNED_BYTE, ptr::null());
        gl::FramebufferTexture2DEXT(gl::FRAMEBUFFER_EXT, gl::COLOR_ATTACHMENT0_EXT,
                                    gl::TEXTURE_2D, texture, 0);
        let status = gl::CheckFramebufferStatusEXT(gl::FRAMEBUFFER_EXT);
        if status != gl::FRAMEBUFFER_COMPLETE_EXT {
            panic!("Error while creating the framebuffer");
        }
    }

    pub fn get_proc_address(&self, _addr: &str) -> *const () {
        let symbol_name: CFString = from_str(_addr).unwrap();
        let framework_name: CFString = from_str("com.apple.opengl").unwrap();
        let framework = unsafe {
            CFBundleGetBundleWithIdentifier(framework_name.as_concrete_TypeRef())
        };
        let symbol = unsafe {
            CFBundleGetFunctionPointerForName(framework, symbol_name.as_concrete_TypeRef())
        };
        symbol as *const ()
    }
}

unsafe impl Send for HeadlessContext {}
unsafe impl Sync for HeadlessContext {}

impl Drop for HeadlessContext {
    fn drop(&mut self) {
        unsafe {
            gl::DeleteTextures(1, &texture);
            gl::DeleteFramebuffersEXT(1, &framebuffer);
        }
    }
}
