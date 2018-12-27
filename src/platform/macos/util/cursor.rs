use cocoa::{
    appkit::NSImage, base::{id, nil, YES},
    foundation::{NSDictionary, NSPoint, NSString},
};
use objc::runtime::Sel;

use super::IntoOption;
use MouseCursor;

pub enum Cursor {
    Native(&'static str),
    Undocumented(&'static str),
    WebKit(&'static str),
}

impl From<MouseCursor> for Cursor {
    fn from(cursor: MouseCursor) -> Self {
        match cursor {
            MouseCursor::Arrow | MouseCursor::Default => Cursor::Native("arrowCursor"),
            MouseCursor::Hand => Cursor::Native("pointingHandCursor"),
            MouseCursor::Grabbing | MouseCursor::Grab => Cursor::Native("closedHandCursor"),
            MouseCursor::Text => Cursor::Native("IBeamCursor"),
            MouseCursor::VerticalText => Cursor::Native("IBeamCursorForVerticalLayout"),
            MouseCursor::Copy => Cursor::Native("dragCopyCursor"),
            MouseCursor::Alias => Cursor::Native("dragLinkCursor"),
            MouseCursor::NotAllowed | MouseCursor::NoDrop => Cursor::Native("operationNotAllowedCursor"),
            MouseCursor::ContextMenu => Cursor::Native("contextualMenuCursor"),
            MouseCursor::Crosshair => Cursor::Native("crosshairCursor"),
            MouseCursor::EResize => Cursor::Native("resizeRightCursor"),
            MouseCursor::NResize => Cursor::Native("resizeUpCursor"),
            MouseCursor::WResize => Cursor::Native("resizeLeftCursor"),
            MouseCursor::SResize => Cursor::Native("resizeDownCursor"),
            MouseCursor::EwResize | MouseCursor::ColResize => Cursor::Native("resizeLeftRightCursor"),
            MouseCursor::NsResize | MouseCursor::RowResize => Cursor::Native("resizeUpDownCursor"),

            // Undocumented cursors: https://stackoverflow.com/a/46635398/5435443
            MouseCursor::Help => Cursor::Undocumented("_helpCursor"),
            MouseCursor::ZoomIn => Cursor::Undocumented("_zoomInCursor"),
            MouseCursor::ZoomOut => Cursor::Undocumented("_zoomOutCursor"),
            MouseCursor::NeResize => Cursor::Undocumented("_windowResizeNorthEastCursor"),
            MouseCursor::NwResize => Cursor::Undocumented("_windowResizeNorthWestCursor"),
            MouseCursor::SeResize => Cursor::Undocumented("_windowResizeSouthEastCursor"),
            MouseCursor::SwResize => Cursor::Undocumented("_windowResizeSouthWestCursor"),
            MouseCursor::NeswResize => Cursor::Undocumented("_windowResizeNorthEastSouthWestCursor"),
            MouseCursor::NwseResize => Cursor::Undocumented("_windowResizeNorthWestSouthEastCursor"),

            // While these are available, the former just loads a white arrow,
            // and the latter loads an ugly deflated beachball!
            // MouseCursor::Move => Cursor::Undocumented("_moveCursor"),
            // MouseCursor::Wait => Cursor::Undocumented("_waitCursor"),

            // An even more undocumented cursor...
            // https://bugs.eclipse.org/bugs/show_bug.cgi?id=522349
            // This is the wrong semantics for `Wait`, but it's the same as
            // what's used in Safari and Chrome.
            MouseCursor::Wait | MouseCursor::Progress => Cursor::Undocumented("busyButClickableCursor"),

            // For the rest, we can just snatch the cursors from WebKit...
            // They fit the style of the native cursors, and will seem
            // completely standard to macOS users.
            // https://stackoverflow.com/a/21786835/5435443
            MouseCursor::Move | MouseCursor::AllScroll => Cursor::WebKit("move"),
            MouseCursor::Cell => Cursor::WebKit("cell"),
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
                msg_send![class!(NSCursor), performSelector:sel]
            },
            Cursor::Undocumented(cursor_name) => {
                let class = class!(NSCursor);
                let sel = Sel::register(cursor_name);
                let sel = if msg_send![class, respondsToSelector:sel] {
                    sel
                } else {
                    warn!("Cursor `{}` appears to be invalid", cursor_name);
                    sel!(arrowCursor)
                };
                msg_send![class, performSelector:sel]
            },
            Cursor::WebKit(cursor_name) => load_webkit_cursor(cursor_name)
                .unwrap_or_else(|message| {
                    warn!("{}", message);
                    Self::default().load()
                }),
        }
    }
}

// Note that loading `busybutclickable` with this code won't animate the frames;
// instead you'll just get them all in a column.
unsafe fn load_webkit_cursor(cursor_name_str: &str) -> Result<id, String> {
    static CURSOR_ROOT: &'static str = "/System/Library/Frameworks/ApplicationServices.framework/Versions/A/Frameworks/HIServices.framework/Versions/A/Resources/cursors";
    let cursor_root = NSString::alloc(nil).init_str(CURSOR_ROOT);
    let cursor_name = NSString::alloc(nil).init_str(cursor_name_str);
    let cursor_pdf = NSString::alloc(nil).init_str("cursor.pdf");
    let cursor_plist = NSString::alloc(nil).init_str("info.plist");
    let key_x = NSString::alloc(nil).init_str("hotx");
    let key_y = NSString::alloc(nil).init_str("hoty");

    let cursor_path: id = msg_send![cursor_root,
        stringByAppendingPathComponent:cursor_name
    ];
    let pdf_path: id = msg_send![cursor_path,
        stringByAppendingPathComponent:cursor_pdf
    ];
    let info_path: id = msg_send![cursor_path,
        stringByAppendingPathComponent:cursor_plist
    ];

    let image = NSImage::alloc(nil)
        .initByReferencingFile_(pdf_path)
        // This will probably never be `None`, since images are loaded lazily...
        .into_option()
        // because of that, we need to check for validity.
        .filter(|image| image.isValid() == YES)
        .ok_or_else(||
            format!("Failed to read image for `{}` cursor", cursor_name_str)
        )?;
    let info = NSDictionary::dictionaryWithContentsOfFile_(nil, info_path)
        .into_option()
        .ok_or_else(||
            format!("Failed to read info for `{}` cursor", cursor_name_str)
        )?;
    let x = info.valueForKey_(key_x);
    let y = info.valueForKey_(key_y);
    let point = NSPoint::new(
        msg_send![x, doubleValue],
        msg_send![y, doubleValue],
    );
    let cursor: id = msg_send![class!(NSCursor), alloc];
    let cursor: id = msg_send![cursor, initWithImage:image hotSpot:point];
    cursor
        .into_option()
        .ok_or_else(||
            format!("Failed to initialize `{}` cursor", cursor_name_str)
        )
}
