use objc2::foundation::{NSInteger, NSString};
use objc2::rc::{Id, Shared};
use objc2::runtime::Bool;
use objc2::{extern_class, extern_methods, msg_send, msg_send_id, ClassType};

use super::NSImageRep;

extern "C" {
    static NSDeviceRGBColorSpace: &'static NSString;
}

extern_class!(
    /// <https://developer.apple.com/documentation/appkit/nsbitmapimagerep?language=objc>
    #[derive(Debug, PartialEq, Eq, Hash)]
    pub(crate) struct NSBitmapImageRep;

    unsafe impl ClassType for NSBitmapImageRep {
        type Super = NSImageRep;
    }
);

extern_methods!(
    unsafe impl NSBitmapImageRep {
        pub fn initAbgr(width: NSInteger, height: NSInteger) -> Id<Self, Shared> {
            unsafe {
                let this = msg_send_id![Self::class(), alloc];

                msg_send_id![this,
                    initWithBitmapDataPlanes: std::ptr::null_mut() as *mut u8,
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

        pub fn bitmapData(&self) -> *mut u8 {
            unsafe { msg_send![self, bitmapData] }
        }
    }
);
