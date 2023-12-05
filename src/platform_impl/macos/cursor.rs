use icrate::AppKit::{NSBitmapImageRep, NSDeviceRGBColorSpace, NSImage};
use icrate::Foundation::{
    ns_string, NSData, NSDictionary, NSNumber, NSObject, NSObjectProtocol, NSPoint, NSSize,
    NSString,
};
use objc2::rc::Id;
use objc2::runtime::Sel;
use objc2::{msg_send_id, sel, ClassType};
use once_cell::sync::Lazy;
use std::ffi::c_uchar;
use std::slice;

use super::appkit::NSCursor;
use super::EventLoopWindowTarget;
use crate::cursor::CursorImage;
use crate::cursor::OnlyCursorImageBuilder;
use crate::window::CursorIcon;

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct CustomCursor(pub(crate) Id<NSCursor>);

impl CustomCursor {
    pub(crate) fn build<T>(
        cursor: OnlyCursorImageBuilder,
        _: &EventLoopWindowTarget<T>,
    ) -> CustomCursor {
        Self(cursor_from_image(&cursor.0))
    }
}

pub(crate) fn cursor_from_image(cursor: &CursorImage) -> Id<NSCursor> {
    let width = cursor.width;
    let height = cursor.height;

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
        ).unwrap()
    };
    let bitmap_data = unsafe { slice::from_raw_parts_mut(bitmap.bitmapData(), cursor.rgba.len()) };
    bitmap_data.copy_from_slice(&cursor.rgba);

    let image = unsafe {
        NSImage::initWithSize(NSImage::alloc(), NSSize::new(width.into(), height.into()))
    };
    unsafe { image.addRepresentation(&bitmap) };

    let hotspot = NSPoint::new(cursor.hotspot_x as f64, cursor.hotspot_y as f64);

    NSCursor::new(&image, hotspot)
}

pub(crate) fn default_cursor() -> Id<NSCursor> {
    NSCursor::arrowCursor()
}

unsafe fn try_cursor_from_selector(sel: Sel) -> Option<Id<NSCursor>> {
    let cls = NSCursor::class();
    if cls.responds_to(sel) {
        let cursor: Id<NSCursor> = unsafe { msg_send_id![cls, performSelector: sel] };
        Some(cursor)
    } else {
        warn!("cursor `{sel}` appears to be invalid");
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
        fn $name() -> Id<NSCursor> {
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
unsafe fn load_webkit_cursor(name: &NSString) -> Id<NSCursor> {
    // Snatch a cursor from WebKit; They fit the style of the native
    // cursors, and will seem completely standard to macOS users.
    //
    // https://stackoverflow.com/a/21786835/5435443
    let root = ns_string!("/System/Library/Frameworks/ApplicationServices.framework/Versions/A/Frameworks/HIServices.framework/Versions/A/Resources/cursors");
    let cursor_path = root.stringByAppendingPathComponent(name);

    let pdf_path = cursor_path.stringByAppendingPathComponent(ns_string!("cursor.pdf"));
    let image = NSImage::initByReferencingFile(NSImage::alloc(), &pdf_path).unwrap();

    // TODO: Handle PLists better
    let info_path = cursor_path.stringByAppendingPathComponent(ns_string!("info.plist"));
    let info: Id<NSDictionary<NSObject, NSObject>> = unsafe {
        msg_send_id![
            <NSDictionary<NSObject, NSObject>>::class(),
            dictionaryWithContentsOfFile: &*info_path,
        ]
    };
    let mut x = 0.0;
    if let Some(n) = info.get(&*ns_string!("hotx")) {
        if n.is_kind_of::<NSNumber>() {
            let ptr: *const NSObject = n;
            let ptr: *const NSNumber = ptr.cast();
            x = unsafe { &*ptr }.as_cgfloat()
        }
    }
    let mut y = 0.0;
    if let Some(n) = info.get(&*ns_string!("hotx")) {
        if n.is_kind_of::<NSNumber>() {
            let ptr: *const NSObject = n;
            let ptr: *const NSNumber = ptr.cast();
            y = unsafe { &*ptr }.as_cgfloat()
        }
    }

    let hotspot = NSPoint::new(x, y);
    NSCursor::new(&image, hotspot)
}

fn webkit_move() -> Id<NSCursor> {
    unsafe { load_webkit_cursor(ns_string!("move")) }
}

fn webkit_cell() -> Id<NSCursor> {
    unsafe { load_webkit_cursor(ns_string!("cell")) }
}

pub(crate) fn invisible_cursor() -> Id<NSCursor> {
    // 16x16 GIF data for invisible cursor
    // You can reproduce this via ImageMagick.
    // $ convert -size 16x16 xc:none cursor.gif
    static CURSOR_BYTES: &[u8] = &[
        0x47, 0x49, 0x46, 0x38, 0x39, 0x61, 0x10, 0x00, 0x10, 0x00, 0xF0, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x00, 0x21, 0xF9, 0x04, 0x01, 0x00, 0x00, 0x00, 0x00, 0x2C, 0x00, 0x00,
        0x00, 0x00, 0x10, 0x00, 0x10, 0x00, 0x00, 0x02, 0x0E, 0x84, 0x8F, 0xA9, 0xCB, 0xED, 0x0F,
        0xA3, 0x9C, 0xB4, 0xDA, 0x8B, 0xB3, 0x3E, 0x05, 0x00, 0x3B,
    ];

    struct Wrapper(Id<NSCursor>);

    // SAFETY: NSCursor is immutable and thread-safe
    // TODO(madsmtm): Put this logic in icrate itself
    unsafe impl Send for Wrapper {}
    unsafe impl Sync for Wrapper {}

    static CURSOR: Lazy<Wrapper> = Lazy::new(|| {
        // TODO: Consider using `dataWithBytesNoCopy:`
        let data = NSData::with_bytes(CURSOR_BYTES);
        let image = NSImage::initWithData(NSImage::alloc(), &data).unwrap();
        let hotspot = NSPoint::new(0.0, 0.0);
        Wrapper(NSCursor::new(&image, hotspot))
    });

    CURSOR.0.clone()
}

pub(crate) fn cursor_from_icon(icon: CursorIcon) -> Id<NSCursor> {
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
        CursorIcon::EResize => NSCursor::resizeRightCursor(),
        CursorIcon::NResize => NSCursor::resizeUpCursor(),
        CursorIcon::WResize => NSCursor::resizeLeftCursor(),
        CursorIcon::SResize => NSCursor::resizeDownCursor(),
        CursorIcon::EwResize | CursorIcon::ColResize => NSCursor::resizeLeftRightCursor(),
        CursorIcon::NsResize | CursorIcon::RowResize => NSCursor::resizeUpDownCursor(),
        CursorIcon::Help => _helpCursor(),
        CursorIcon::ZoomIn => _zoomInCursor(),
        CursorIcon::ZoomOut => _zoomOutCursor(),
        CursorIcon::NeResize => _windowResizeNorthEastCursor(),
        CursorIcon::NwResize => _windowResizeNorthWestCursor(),
        CursorIcon::SeResize => _windowResizeSouthEastCursor(),
        CursorIcon::SwResize => _windowResizeSouthWestCursor(),
        CursorIcon::NeswResize => _windowResizeNorthEastSouthWestCursor(),
        CursorIcon::NwseResize => _windowResizeNorthWestSouthEastCursor(),
        // This is the wrong semantics for `Wait`, but it's the same as
        // what's used in Safari and Chrome.
        CursorIcon::Wait | CursorIcon::Progress => busyButClickableCursor(),
        CursorIcon::Move | CursorIcon::AllScroll => webkit_move(),
        CursorIcon::Cell => webkit_cell(),
        _ => default_cursor(),
    }
}
