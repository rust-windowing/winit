use cocoa::{
    appkit::NSImage,
    base::{id, nil},
    foundation::{NSDictionary, NSPoint, NSString},
};
use objc::runtime::Sel;

use crate::window::CursorIcon;

pub enum Cursor {
    Native(&'static str),
    Undocumented(&'static str),
    WebKit(&'static str),
}

impl From<CursorIcon> for Cursor {
    fn from(cursor: CursorIcon) -> Self {
        match cursor {
            CursorIcon::Arrow | CursorIcon::Default => Cursor::Native("arrowCursor"),
            CursorIcon::Hand => Cursor::Native("pointingHandCursor"),
            CursorIcon::Grabbing | CursorIcon::Grab => Cursor::Native("closedHandCursor"),
            CursorIcon::Text => Cursor::Native("IBeamCursor"),
            CursorIcon::VerticalText => Cursor::Native("IBeamCursorForVerticalLayout"),
            CursorIcon::Copy => Cursor::Native("dragCopyCursor"),
            CursorIcon::Alias => Cursor::Native("dragLinkCursor"),
            CursorIcon::NotAllowed | CursorIcon::NoDrop => {
                Cursor::Native("operationNotAllowedCursor")
            },
            CursorIcon::ContextMenu => Cursor::Native("contextualMenuCursor"),
            CursorIcon::Crosshair => Cursor::Native("crosshairCursor"),
            CursorIcon::EResize => Cursor::Native("resizeRightCursor"),
            CursorIcon::NResize => Cursor::Native("resizeUpCursor"),
            CursorIcon::WResize => Cursor::Native("resizeLeftCursor"),
            CursorIcon::SResize => Cursor::Native("resizeDownCursor"),
            CursorIcon::EwResize | CursorIcon::ColResize => Cursor::Native("resizeLeftRightCursor"),
            CursorIcon::NsResize | CursorIcon::RowResize => Cursor::Native("resizeUpDownCursor"),

            // Undocumented cursors: https://stackoverflow.com/a/46635398/5435443
            CursorIcon::Help => Cursor::Undocumented("_helpCursor"),
            CursorIcon::ZoomIn => Cursor::Undocumented("_zoomInCursor"),
            CursorIcon::ZoomOut => Cursor::Undocumented("_zoomOutCursor"),
            CursorIcon::NeResize => Cursor::Undocumented("_windowResizeNorthEastCursor"),
            CursorIcon::NwResize => Cursor::Undocumented("_windowResizeNorthWestCursor"),
            CursorIcon::SeResize => Cursor::Undocumented("_windowResizeSouthEastCursor"),
            CursorIcon::SwResize => Cursor::Undocumented("_windowResizeSouthWestCursor"),
            CursorIcon::NeswResize => Cursor::Undocumented("_windowResizeNorthEastSouthWestCursor"),
            CursorIcon::NwseResize => Cursor::Undocumented("_windowResizeNorthWestSouthEastCursor"),

            // While these are available, the former just loads a white arrow,
            // and the latter loads an ugly deflated beachball!
            // CursorIcon::Move => Cursor::Undocumented("_moveCursor"),
            // CursorIcon::Wait => Cursor::Undocumented("_waitCursor"),

            // An even more undocumented cursor...
            // https://bugs.eclipse.org/bugs/show_bug.cgi?id=522349
            // This is the wrong semantics for `Wait`, but it's the same as
            // what's used in Safari and Chrome.
            CursorIcon::Wait | CursorIcon::Progress => {
                Cursor::Undocumented("busyButClickableCursor")
            },

            // For the rest, we can just snatch the cursors from WebKit...
            // They fit the style of the native cursors, and will seem
            // completely standard to macOS users.
            // https://stackoverflow.com/a/21786835/5435443
            CursorIcon::Move | CursorIcon::AllScroll => Cursor::WebKit("move"),
            CursorIcon::Cell => Cursor::WebKit("cell"),
        }
    }
}

impl Default for Cursor {
    fn default() -> Self {
        Cursor::Native("arrowCursor")
    }
}

impl Cursor {
    pub unsafe fn load(&self) -> id {
        match self {
            Cursor::Native(cursor_name) => {
                let sel = Sel::register(cursor_name);
                msg_send![class!(NSCursor), performSelector: sel]
            },
            Cursor::Undocumented(cursor_name) => {
                let class = class!(NSCursor);
                let sel = Sel::register(cursor_name);
                let sel = if msg_send![class, respondsToSelector: sel] {
                    sel
                } else {
                    warn!("Cursor `{}` appears to be invalid", cursor_name);
                    sel!(arrowCursor)
                };
                msg_send![class, performSelector: sel]
            },
            Cursor::WebKit(cursor_name) => load_webkit_cursor(cursor_name),
        }
    }
}

// Note that loading `busybutclickable` with this code won't animate the frames;
// instead you'll just get them all in a column.
pub unsafe fn load_webkit_cursor(cursor_name: &str) -> id {
    static CURSOR_ROOT: &'static str = "/System/Library/Frameworks/ApplicationServices.framework/Versions/A/Frameworks/HIServices.framework/Versions/A/Resources/cursors";
    let cursor_root = NSString::alloc(nil).init_str(CURSOR_ROOT);
    let cursor_name = NSString::alloc(nil).init_str(cursor_name);
    let cursor_pdf = NSString::alloc(nil).init_str("cursor.pdf");
    let cursor_plist = NSString::alloc(nil).init_str("info.plist");
    let key_x = NSString::alloc(nil).init_str("hotx");
    let key_y = NSString::alloc(nil).init_str("hoty");

    let cursor_path: id = msg_send![cursor_root, stringByAppendingPathComponent: cursor_name];
    let pdf_path: id = msg_send![cursor_path, stringByAppendingPathComponent: cursor_pdf];
    let info_path: id = msg_send![cursor_path, stringByAppendingPathComponent: cursor_plist];

    let image = NSImage::alloc(nil).initByReferencingFile_(pdf_path);
    let info = NSDictionary::dictionaryWithContentsOfFile_(nil, info_path);
    let x = info.valueForKey_(key_x);
    let y = info.valueForKey_(key_y);
    let point = NSPoint::new(msg_send![x, doubleValue], msg_send![y, doubleValue]);
    let cursor: id = msg_send![class!(NSCursor), alloc];
    msg_send![cursor,
        initWithImage:image
        hotSpot:point
    ]
}
