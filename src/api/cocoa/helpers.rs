
use CreationError;
use GlAttributes;
use GlProfile;
use GlRequest;
use PixelFormatRequirements;
use ReleaseBehavior;
use cocoa::appkit::*;

pub fn build_nsattributes<T>(pf_reqs: &PixelFormatRequirements, opengl: &GlAttributes<&T>)
    -> Result<Vec<u32>, CreationError> {

    let profile = match (opengl.version, opengl.version.to_gl_version(), opengl.profile) {

        // Note: we are not using ranges because of a rust bug that should be fixed here:
        // https://github.com/rust-lang/rust/pull/27050

        (GlRequest::Latest, _, Some(GlProfile::Compatibility)) => NSOpenGLProfileVersionLegacy as u32,
        (GlRequest::Latest, _, _) => {
            if NSAppKitVersionNumber.floor() >= NSAppKitVersionNumber10_9 {
                NSOpenGLProfileVersion4_1Core as u32
            } else if NSAppKitVersionNumber.floor() >= NSAppKitVersionNumber10_7 {
                NSOpenGLProfileVersion3_2Core as u32
            } else {
                NSOpenGLProfileVersionLegacy as u32
            }
        },

        (_, Some((1, _)), _) => NSOpenGLProfileVersionLegacy as u32,
        (_, Some((2, _)), _) => NSOpenGLProfileVersionLegacy as u32,
        (_, Some((3, 0)), _) => NSOpenGLProfileVersionLegacy as u32,
        (_, Some((3, 1)), _) => NSOpenGLProfileVersionLegacy as u32,
        (_, Some((3, 2)), _) => NSOpenGLProfileVersion3_2Core as u32,
        (_, Some((3, _)), Some(GlProfile::Compatibility)) => return Err(CreationError::OpenGlVersionNotSupported),
        (_, Some((3, _)), _) => NSOpenGLProfileVersion4_1Core as u32,
        (_, Some((4, _)), Some(GlProfile::Compatibility)) => return Err(CreationError::OpenGlVersionNotSupported),
        (_, Some((4, _)), _) => NSOpenGLProfileVersion4_1Core as u32,
        _ => return Err(CreationError::OpenGlVersionNotSupported),
    };

    // NOTE: OS X no longer has the concept of setting individual
    // color component's bit size. Instead we can only specify the
    // full color size and hope for the best. Another hiccup is that
    // `NSOpenGLPFAColorSize` also includes `NSOpenGLPFAAlphaSize`,
    // so we have to account for that as well.
    let alpha_depth = pf_reqs.alpha_bits.unwrap_or(8);
    let color_depth = pf_reqs.color_bits.unwrap_or(24) + alpha_depth;

    // TODO: handle hardware_accelerated parameter of pf_reqs

    let mut attributes = vec![
        NSOpenGLPFADoubleBuffer as u32,
        NSOpenGLPFAClosestPolicy as u32,
        NSOpenGLPFAColorSize as u32, color_depth as u32,
        NSOpenGLPFAAlphaSize as u32, alpha_depth as u32,
        NSOpenGLPFADepthSize as u32, pf_reqs.depth_bits.unwrap_or(24) as u32,
        NSOpenGLPFAStencilSize as u32, pf_reqs.stencil_bits.unwrap_or(8) as u32,
        NSOpenGLPFAOpenGLProfile as u32, profile,
    ];

    if pf_reqs.release_behavior != ReleaseBehavior::Flush {
        return Err(CreationError::NoAvailablePixelFormat);
    }

    if pf_reqs.stereoscopy {
        unimplemented!();   // TODO:
    }

    if pf_reqs.double_buffer == Some(false) {
        unimplemented!();   // TODO:
    }

    if pf_reqs.float_color_buffer {
        attributes.push(NSOpenGLPFAColorFloat as u32);
    }

    pf_reqs.multisampling.map(|samples| {
        attributes.push(NSOpenGLPFAMultisample as u32);
        attributes.push(NSOpenGLPFASampleBuffers as u32); attributes.push(1);
        attributes.push(NSOpenGLPFASamples as u32); attributes.push(samples as u32);
    });

    // attribute list must be null terminated.
    attributes.push(0);

    Ok(attributes)
}
