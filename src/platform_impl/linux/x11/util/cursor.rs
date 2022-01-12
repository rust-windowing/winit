use crate::window::CursorIcon;

use super::*;

impl XConnection {
    pub fn set_cursor_icon(&self, window: ffi::Window, cursor: Option<CursorIcon>) {
        let cursor = *self
            .cursor_cache
            .lock()
            .unwrap()
            .entry(cursor)
            .or_insert_with(|| self.get_cursor(cursor));

        self.update_cursor(window, cursor);
    }

    fn create_empty_cursor(&self) -> ffi::Cursor {
        let data = 0;
        let pixmap = unsafe {
            let screen = (self.xlib.XDefaultScreen)(self.display);
            let window = (self.xlib.XRootWindow)(self.display, screen);
            (self.xlib.XCreateBitmapFromData)(self.display, window, &data, 1, 1)
        };

        if pixmap == 0 {
            panic!("failed to allocate pixmap for cursor");
        }

        unsafe {
            // We don't care about this color, since it only fills bytes
            // in the pixmap which are not 0 in the mask.
            let mut dummy_color = MaybeUninit::uninit();
            let cursor = (self.xlib.XCreatePixmapCursor)(
                self.display,
                pixmap,
                pixmap,
                dummy_color.as_mut_ptr(),
                dummy_color.as_mut_ptr(),
                0,
                0,
            );
            (self.xlib.XFreePixmap)(self.display, pixmap);

            cursor
        }
    }

    fn load_cursor(&self, name: &[u8]) -> ffi::Cursor {
        unsafe {
            (self.xcursor.XcursorLibraryLoadCursor)(self.display, name.as_ptr() as *const c_char)
        }
    }

    fn load_first_existing_cursor(&self, names: &[&[u8]]) -> ffi::Cursor {
        for name in names.iter() {
            let xcursor = self.load_cursor(name);
            if xcursor != 0 {
                return xcursor;
            }
        }
        0
    }

    fn get_cursor(&self, cursor: Option<CursorIcon>) -> ffi::Cursor {
        let cursor = match cursor {
            Some(cursor) => cursor,
            None => return self.create_empty_cursor(),
        };

        let load = |name: &[u8]| self.load_cursor(name);

        let loadn = |names: &[&[u8]]| self.load_first_existing_cursor(names);

        // Try multiple names in some cases where the name
        // differs on the desktop environments or themes.
        //
        // Try the better looking (or more suiting) names first.
        match cursor {
            CursorIcon::Alias => load(b"link\0"),
            CursorIcon::Arrow => load(b"arrow\0"),
            CursorIcon::Cell => load(b"plus\0"),
            CursorIcon::Copy => load(b"copy\0"),
            CursorIcon::Crosshair => load(b"crosshair\0"),
            CursorIcon::Default => load(b"left_ptr\0"),
            CursorIcon::Hand => loadn(&[b"hand2\0", b"hand1\0"]),
            CursorIcon::Help => load(b"question_arrow\0"),
            CursorIcon::Move => load(b"move\0"),
            CursorIcon::Grab => loadn(&[b"openhand\0", b"grab\0"]),
            CursorIcon::Grabbing => loadn(&[b"closedhand\0", b"grabbing\0"]),
            CursorIcon::Progress => load(b"left_ptr_watch\0"),
            CursorIcon::AllScroll => load(b"all-scroll\0"),
            CursorIcon::ContextMenu => load(b"context-menu\0"),

            CursorIcon::NoDrop => loadn(&[b"no-drop\0", b"circle\0"]),
            CursorIcon::NotAllowed => load(b"crossed_circle\0"),

            // Resize cursors
            CursorIcon::EResize => load(b"right_side\0"),
            CursorIcon::NResize => load(b"top_side\0"),
            CursorIcon::NeResize => load(b"top_right_corner\0"),
            CursorIcon::NwResize => load(b"top_left_corner\0"),
            CursorIcon::SResize => load(b"bottom_side\0"),
            CursorIcon::SeResize => load(b"bottom_right_corner\0"),
            CursorIcon::SwResize => load(b"bottom_left_corner\0"),
            CursorIcon::WResize => load(b"left_side\0"),
            CursorIcon::EwResize => load(b"h_double_arrow\0"),
            CursorIcon::NsResize => load(b"v_double_arrow\0"),
            CursorIcon::NwseResize => loadn(&[b"bd_double_arrow\0", b"size_fdiag\0"]),
            CursorIcon::NeswResize => loadn(&[b"fd_double_arrow\0", b"size_bdiag\0"]),
            CursorIcon::ColResize => loadn(&[b"split_h\0", b"h_double_arrow\0"]),
            CursorIcon::RowResize => loadn(&[b"split_v\0", b"v_double_arrow\0"]),

            CursorIcon::Text => loadn(&[b"text\0", b"xterm\0"]),
            CursorIcon::VerticalText => load(b"vertical-text\0"),

            CursorIcon::Wait => load(b"watch\0"),

            CursorIcon::ZoomIn => load(b"zoom-in\0"),
            CursorIcon::ZoomOut => load(b"zoom-out\0"),
        }
    }

    fn update_cursor(&self, window: ffi::Window, cursor: ffi::Cursor) {
        unsafe {
            (self.xlib.XDefineCursor)(self.display, window, cursor);

            self.flush_requests().expect("Failed to set the cursor");
        }
    }
}
