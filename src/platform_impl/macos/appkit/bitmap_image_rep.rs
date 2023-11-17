use std::ffi::c_uchar;

use icrate::Foundation::{NSInteger, NSObject, NSString};
use objc2::rc::Id;
use objc2::runtime::Bool;
use objc2::{extern_class, extern_methods, msg_send, msg_send_id, mutability, ClassType};

extern_class!(
    #[derive(Debug, PartialEq, Eq, Hash)]
    pub struct NSImageRep;

    unsafe impl ClassType for NSImageRep {
        type Super = NSObject;
        type Mutability = mutability::InteriorMutable;
    }
);

extern "C" {
    static NSDeviceRGBColorSpace: &'static NSString;
}

extern_class!(
    // <https://developer.apple.com/documentation/appkit/nsbitmapimagerep?language=objc>
    #[derive(Debug, PartialEq, Eq, Hash)]
    pub(crate) struct NSBitmapImageRep;

    unsafe impl ClassType for NSBitmapImageRep {
        type Super = NSImageRep;
        type Mutability = mutability::InteriorMutable;
    }
);

extern_methods!(
    unsafe impl NSBitmapImageRep {
        pub fn init_rgba(width: NSInteger, height: NSInteger) -> Id<Self> {
            unsafe {
                msg_send_id![Self::alloc(),
                    initWithBitmapDataPlanes: std::ptr::null_mut() as *mut *mut c_uchar,
                    pixelsWide: width,
                    pixelsHigh: height,
                    bitsPerSample: 8 as NSInteger,
                    samplesPerPixel: 4 as NSInteger,
                    hasAlpha: Bool::new(true),
                    isPlanar: Bool::new(false),
                    colorSpaceName: NSDeviceRGBColorSpace,
                    bytesPerRow: width * 4,
                    bitsPerPixel: 32 as NSInteger,
                ]
            }
        }

        pub fn bitmap_data(&self) -> *mut u8 {
            unsafe { msg_send![self, bitmapData] }
        }
    }
);
