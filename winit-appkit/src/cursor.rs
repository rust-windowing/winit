use std::ffi::c_uchar;
use std::slice;
use std::sync::OnceLock;

use objc2::rc::Retained;
use objc2::runtime::Sel;
use objc2::{AllocAnyThread, ClassType, available, msg_send, sel};
use objc2_app_kit::{
    NSBitmapImageRep, NSCursor, NSCursorFrameResizeDirections, NSCursorFrameResizePosition,
    NSDeviceRGBColorSpace, NSImage,
};
use objc2_foundation::{
    NSData, NSDictionary, NSNumber, NSObject, NSPoint, NSSize, NSString, ns_string,
};
use winit_core::cursor::{CursorIcon, CursorImage, CustomCursorProvider, CustomCursorSource};
use winit_core::error::{NotSupportedError, RequestError};

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct CustomCursor(pub(crate) Retained<NSCursor>);

impl CustomCursorProvider for CustomCursor {
    fn is_animated(&self) -> bool {
        false
    }
}

// SAFETY: NSCursor is immutable and thread-safe
// TODO(madsmtm): Put this logic in objc2-app-kit itself
unsafe impl Send for CustomCursor {}
unsafe impl Sync for CustomCursor {}

impl CustomCursor {
    pub(crate) fn new(cursor: CustomCursorSource) -> Result<CustomCursor, RequestError> {
        let cursor = match cursor {
            CustomCursorSource::Image(cursor_image) => cursor_image,
            CustomCursorSource::Animation { .. } | CustomCursorSource::Url { .. } => {
                return Err(NotSupportedError::new("unsupported cursor kind").into());
            },
        };

        cursor_from_image(&cursor).map(Self)
    }
}

pub(crate) fn cursor_from_image(cursor: &CursorImage) -> Result<Retained<NSCursor>, RequestError> {
    let width = cursor.width();
    let height = cursor.height();

    let bitmap = unsafe {
        NSBitmapImageRep::initWithBitmapDataPlanes_pixelsWide_pixelsHigh_bitsPerSample_samplesPerPixel_hasAlpha_isPlanar_colorSpaceName_bytesPerRow_bitsPerPixel(
            NSBitmapImageRep::alloc(),
            std::ptr::null_mut::<*mut c_uchar>(),
            width as isize,
            height as isize,
            8,
            4,
            true,
            false,
            NSDeviceRGBColorSpace,
            width as isize * 4,
            32,
        )
    }.ok_or_else(|| os_error!("parent view should be installed in a window"))?;
    let bitmap_data =
        unsafe { slice::from_raw_parts_mut(bitmap.bitmapData(), cursor.buffer().len()) };
    bitmap_data.copy_from_slice(cursor.buffer());

    let image = NSImage::initWithSize(NSImage::alloc(), NSSize::new(width.into(), height.into()));
    image.addRepresentation(&bitmap);

    let hotspot = NSPoint::new(cursor.hotspot_x() as f64, cursor.hotspot_y() as f64);

    Ok(NSCursor::initWithImage_hotSpot(NSCursor::alloc(), &image, hotspot))
}

pub(crate) fn default_cursor() -> Retained<NSCursor> {
    NSCursor::arrowCursor()
}

unsafe fn try_cursor_from_selector(sel: Sel) -> Option<Retained<NSCursor>> {
    let cls = NSCursor::class();
    if unsafe { msg_send![cls, respondsToSelector: sel] } {
        let cursor: Retained<NSCursor> = unsafe { msg_send![cls, performSelector: sel] };
        Some(cursor)
    } else {
        tracing::warn!("cursor `{sel}` appears to be invalid");
        None
    }
}

macro_rules! def_undocumented_cursor {
    {$(
        $(#[$($m:meta)*])*
        fn $name:ident();
    )*} => {$(
        $(#[$($m)*])*
        #[allow(non_snake_case)]
        fn $name() -> Retained<NSCursor> {
            unsafe { try_cursor_from_selector(sel!($name)).unwrap_or_else(|| default_cursor()) }
        }
    )*};
}

def_undocumented_cursor!(
    // Undocumented cursors: https://stackoverflow.com/a/46635398/5435443
    fn _helpCursor();
    fn _zoomInCursor();
    fn _zoomOutCursor();
    fn _windowResizeNorthEastCursor();
    fn _windowResizeNorthWestCursor();
    fn _windowResizeSouthEastCursor();
    fn _windowResizeSouthWestCursor();
    fn _windowResizeNorthEastSouthWestCursor();
    fn _windowResizeNorthWestSouthEastCursor();

    // While these two are available, the former just loads a white arrow,
    // and the latter loads an ugly deflated beachball!
    // pub fn _moveCursor();
    // pub fn _waitCursor();

    // An even more undocumented cursor...
    // https://bugs.eclipse.org/bugs/show_bug.cgi?id=522349
    fn busyButClickableCursor();
);

// Note that loading `busybutclickable` with this code won't animate
// the frames; instead you'll just get them all in a column.
unsafe fn load_webkit_cursor(name: &NSString) -> Retained<NSCursor> {
    // Snatch a cursor from WebKit; They fit the style of the native
    // cursors, and will seem completely standard to macOS users.
    //
    // https://stackoverflow.com/a/21786835/5435443
    let root = ns_string!(
        "/System/Library/Frameworks/ApplicationServices.framework/Versions/A/Frameworks/\
         HIServices.framework/Versions/A/Resources/cursors"
    );
    let cursor_path = root.stringByAppendingPathComponent(name);

    let pdf_path = cursor_path.stringByAppendingPathComponent(ns_string!("cursor.pdf"));
    let image = NSImage::initByReferencingFile(NSImage::alloc(), &pdf_path).unwrap();

    // TODO: Handle PLists better
    let info_path = cursor_path.stringByAppendingPathComponent(ns_string!("info.plist"));
    #[allow(deprecated)]
    let info: Retained<NSDictionary<NSObject, NSObject>> =
        unsafe { NSDictionary::dictionaryWithContentsOfFile(&info_path) }.unwrap();
    let mut x = 0.0;
    if let Some(n) = info.objectForKey(ns_string!("hotx")) {
        if let Ok(n) = n.downcast::<NSNumber>() {
            x = n.as_cgfloat();
        }
    }
    let mut y = 0.0;
    if let Some(n) = info.objectForKey(ns_string!("hoty")) {
        if let Ok(n) = n.downcast::<NSNumber>() {
            y = n.as_cgfloat();
        }
    }

    let hotspot = NSPoint::new(x, y);
    NSCursor::initWithImage_hotSpot(NSCursor::alloc(), &image, hotspot)
}

fn webkit_move() -> Retained<NSCursor> {
    unsafe { load_webkit_cursor(ns_string!("move")) }
}

fn webkit_cell() -> Retained<NSCursor> {
    unsafe { load_webkit_cursor(ns_string!("cell")) }
}

pub(crate) fn invisible_cursor() -> Retained<NSCursor> {
    // 16x16 GIF data for invisible cursor
    // You can reproduce this via ImageMagick.
    // $ convert -size 16x16 xc:none cursor.gif
    static CURSOR_BYTES: &[u8] = &[
        0x47, 0x49, 0x46, 0x38, 0x39, 0x61, 0x10, 0x00, 0x10, 0x00, 0xf0, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x00, 0x21, 0xf9, 0x04, 0x01, 0x00, 0x00, 0x00, 0x00, 0x2c, 0x00, 0x00,
        0x00, 0x00, 0x10, 0x00, 0x10, 0x00, 0x00, 0x02, 0x0e, 0x84, 0x8f, 0xa9, 0xcb, 0xed, 0x0f,
        0xa3, 0x9c, 0xb4, 0xda, 0x8b, 0xb3, 0x3e, 0x05, 0x00, 0x3b,
    ];

    fn new_invisible() -> Retained<NSCursor> {
        // TODO: Consider using `dataWithBytesNoCopy:`
        let data = NSData::with_bytes(CURSOR_BYTES);
        let image = NSImage::initWithData(NSImage::alloc(), &data).unwrap();
        let hotspot = NSPoint::new(0.0, 0.0);
        NSCursor::initWithImage_hotSpot(NSCursor::alloc(), &image, hotspot)
    }

    // Cache this for efficiency
    static CURSOR: OnceLock<CustomCursor> = OnceLock::new();
    CURSOR.get_or_init(|| CustomCursor(new_invisible())).0.clone()
}

#[allow(deprecated)]
pub(crate) fn cursor_from_icon(icon: CursorIcon) -> Retained<NSCursor> {
    match icon {
        CursorIcon::Default => default_cursor(),
        CursorIcon::Pointer => NSCursor::pointingHandCursor(),
        CursorIcon::Grab => NSCursor::openHandCursor(),
        CursorIcon::Grabbing => NSCursor::closedHandCursor(),
        CursorIcon::Text => NSCursor::IBeamCursor(),
        CursorIcon::VerticalText => NSCursor::IBeamCursorForVerticalLayout(),
        CursorIcon::Copy => NSCursor::dragCopyCursor(),
        CursorIcon::Alias => NSCursor::dragLinkCursor(),
        CursorIcon::NotAllowed | CursorIcon::NoDrop => NSCursor::operationNotAllowedCursor(),
        CursorIcon::ContextMenu => NSCursor::contextualMenuCursor(),
        CursorIcon::Crosshair => NSCursor::crosshairCursor(),
        CursorIcon::EResize if available!(macos = 15.0) => {
            NSCursor::frameResizeCursorFromPosition_inDirections(
                NSCursorFrameResizePosition::Right,
                NSCursorFrameResizeDirections::Outward,
            )
        },
        CursorIcon::EResize => NSCursor::resizeRightCursor(),
        CursorIcon::NResize if available!(macos = 15.0) => {
            NSCursor::frameResizeCursorFromPosition_inDirections(
                NSCursorFrameResizePosition::Top,
                NSCursorFrameResizeDirections::Outward,
            )
        },
        CursorIcon::NResize => NSCursor::resizeUpCursor(),
        CursorIcon::WResize if available!(macos = 15.0) => {
            NSCursor::frameResizeCursorFromPosition_inDirections(
                NSCursorFrameResizePosition::Left,
                NSCursorFrameResizeDirections::Outward,
            )
        },
        CursorIcon::WResize => NSCursor::resizeLeftCursor(),
        CursorIcon::SResize if available!(macos = 15.0) => {
            NSCursor::frameResizeCursorFromPosition_inDirections(
                NSCursorFrameResizePosition::Bottom,
                NSCursorFrameResizeDirections::Outward,
            )
        },
        CursorIcon::SResize => NSCursor::resizeDownCursor(),
        CursorIcon::EwResize if available!(macos = 15.0) => {
            NSCursor::frameResizeCursorFromPosition_inDirections(
                NSCursorFrameResizePosition::Right,
                NSCursorFrameResizeDirections::All,
            )
        },
        CursorIcon::EwResize => NSCursor::resizeLeftRightCursor(),
        CursorIcon::NsResize if available!(macos = 15.0) => {
            NSCursor::frameResizeCursorFromPosition_inDirections(
                NSCursorFrameResizePosition::Top,
                NSCursorFrameResizeDirections::All,
            )
        },
        CursorIcon::NsResize => NSCursor::resizeUpDownCursor(),
        CursorIcon::NeResize if available!(macos = 15.0) => {
            NSCursor::frameResizeCursorFromPosition_inDirections(
                NSCursorFrameResizePosition::TopRight,
                NSCursorFrameResizeDirections::Outward,
            )
        },
        CursorIcon::NeResize => _windowResizeNorthEastCursor(),
        CursorIcon::NwResize if available!(macos = 15.0) => {
            NSCursor::frameResizeCursorFromPosition_inDirections(
                NSCursorFrameResizePosition::TopLeft,
                NSCursorFrameResizeDirections::Outward,
            )
        },
        CursorIcon::NwResize => _windowResizeNorthWestCursor(),
        CursorIcon::SeResize if available!(macos = 15.0) => {
            NSCursor::frameResizeCursorFromPosition_inDirections(
                NSCursorFrameResizePosition::BottomRight,
                NSCursorFrameResizeDirections::Outward,
            )
        },
        CursorIcon::SeResize => _windowResizeSouthEastCursor(),
        CursorIcon::SwResize if available!(macos = 15.0) => {
            NSCursor::frameResizeCursorFromPosition_inDirections(
                NSCursorFrameResizePosition::BottomLeft,
                NSCursorFrameResizeDirections::Outward,
            )
        },
        CursorIcon::SwResize => _windowResizeSouthWestCursor(),
        CursorIcon::NeswResize if available!(macos = 15.0) => {
            NSCursor::frameResizeCursorFromPosition_inDirections(
                NSCursorFrameResizePosition::TopRight,
                NSCursorFrameResizeDirections::All,
            )
        },
        CursorIcon::NeswResize => _windowResizeNorthEastSouthWestCursor(),
        CursorIcon::NwseResize if available!(macos = 15.0) => {
            NSCursor::frameResizeCursorFromPosition_inDirections(
                NSCursorFrameResizePosition::TopLeft,
                NSCursorFrameResizeDirections::All,
            )
        },
        CursorIcon::NwseResize => _windowResizeNorthWestSouthEastCursor(),
        CursorIcon::ColResize if available!(macos = 15.0) => NSCursor::columnResizeCursor(),
        CursorIcon::ColResize => NSCursor::resizeLeftRightCursor(),
        CursorIcon::RowResize if available!(macos = 15.0) => NSCursor::rowResizeCursor(),
        CursorIcon::RowResize => NSCursor::resizeUpDownCursor(),
        CursorIcon::ZoomIn if available!(macos = 15.0) => NSCursor::zoomInCursor(),
        CursorIcon::ZoomIn => _zoomInCursor(),
        CursorIcon::ZoomOut if available!(macos = 15.0) => NSCursor::zoomOutCursor(),
        CursorIcon::ZoomOut => _zoomOutCursor(),
        CursorIcon::Help => _helpCursor(),
        // This is the wrong semantics for `Wait`, but it's the same as
        // what's used in Safari and Chrome.
        CursorIcon::Wait | CursorIcon::Progress => busyButClickableCursor(),
        CursorIcon::Move | CursorIcon::AllScroll => webkit_move(),
        CursorIcon::Cell => webkit_cell(),
        _ => default_cursor(),
    }
}
