use crate::window::CursorIcon;

use super::*;
use std::slice;
use xcb_dl_util::cursor::{XcbCursorImage, XcbLoadCursorConfig};

impl XConnection {
    pub fn set_cursor_icon(&self, window: ffi::xcb_window_t, cursor: Option<CursorIcon>) {
        let cursor = *self
            .cursor_cache
            .lock()
            .entry(cursor)
            .or_insert_with(|| self.get_cursor(cursor));

        self.update_cursor(window, cursor);
    }

    fn create_empty_cursor(&self) -> ffi::xcb_cursor_t {
        let image = XcbCursorImage {
            width: 1,
            height: 1,
            xhot: 0,
            yhot: 0,
            delay: 0,
            pixels: vec![0],
            ..Default::default()
        };
        unsafe {
            let cursor =
                self.cursors
                    .create_cursor(&self.xcb, &self.render, slice::from_ref(&image));
            match cursor {
                Ok(c) => c,
                Err(e) => {
                    log::error!("Could not create empty cursor: {}", e);
                    0
                }
            }
        }
    }

    fn load_cursor(&self, name: &str) -> Option<ffi::xcb_cursor_t> {
        unsafe {
            let config = XcbLoadCursorConfig {
                name,
                ..Default::default()
            };
            match self.cursors.load_cursor(&self.xcb, &self.render, &config) {
                Ok(c) => Some(c),
                Err(e) => {
                    log::debug!("Could not load cursor {}: {}", name, e);
                    None
                }
            }
        }
    }

    fn load_first_existing_cursor(&self, names: &[&str]) -> Option<ffi::xcb_cursor_t> {
        for name in names.iter() {
            if let Some(xcursor) = self.load_cursor(name) {
                return Some(xcursor);
            }
        }
        None
    }

    fn get_cursor(&self, cursor: Option<CursorIcon>) -> ffi::xcb_cursor_t {
        let cursor = match cursor {
            Some(cursor) => cursor,
            None => return self.create_empty_cursor(),
        };

        let load = |name: &str| self.load_cursor(name);

        let loadn = |names: &[&str]| self.load_first_existing_cursor(names);

        // Try multiple names in some cases where the name
        // differs on the desktop environments or themes.
        //
        // Try the better looking (or more suiting) names first.
        let cursor = match cursor {
            CursorIcon::Alias => load("link"),
            CursorIcon::Arrow => load("arrow"),
            CursorIcon::Cell => load("plus"),
            CursorIcon::Copy => load("copy"),
            CursorIcon::Crosshair => load("crosshair"),
            CursorIcon::Default => load("left_ptr"),
            CursorIcon::Hand => loadn(&["hand2", "hand1"]),
            CursorIcon::Help => load("question_arrow"),
            CursorIcon::Move => load("move"),
            CursorIcon::Grab => loadn(&["openhand", "grab"]),
            CursorIcon::Grabbing => loadn(&["closedhand", "grabbing"]),
            CursorIcon::Progress => load("left_ptr_watch"),
            CursorIcon::AllScroll => load("all-scroll"),
            CursorIcon::ContextMenu => load("context-menu"),

            CursorIcon::NoDrop => loadn(&["no-drop", "circle"]),
            CursorIcon::NotAllowed => load("crossed_circle"),

            // Resize cursors
            CursorIcon::EResize => load("right_side"),
            CursorIcon::NResize => load("top_side"),
            CursorIcon::NeResize => load("top_right_corner"),
            CursorIcon::NwResize => load("top_left_corner"),
            CursorIcon::SResize => load("bottom_side"),
            CursorIcon::SeResize => load("bottom_right_corner"),
            CursorIcon::SwResize => load("bottom_left_corner"),
            CursorIcon::WResize => load("left_side"),
            CursorIcon::EwResize => load("h_double_arrow"),
            CursorIcon::NsResize => load("v_double_arrow"),
            CursorIcon::NwseResize => loadn(&["bd_double_arrow", "size_bdiag"]),
            CursorIcon::NeswResize => loadn(&["fd_double_arrow", "size_fdiag"]),
            CursorIcon::ColResize => loadn(&["split_h", "h_double_arrow"]),
            CursorIcon::RowResize => loadn(&["split_v", "v_double_arrow"]),

            CursorIcon::Text => loadn(&["text", "xterm"]),
            CursorIcon::VerticalText => load("vertical-text"),

            CursorIcon::Wait => load("watch"),

            CursorIcon::ZoomIn => load("zoom-in"),
            CursorIcon::ZoomOut => load("zoom-out"),
        };

        cursor.unwrap_or(0)
    }

    fn update_cursor(&self, window: ffi::xcb_window_t, cursor: ffi::xcb_cursor_t) {
        unsafe {
            let cookie = self.xcb.xcb_change_window_attributes(
                self.c,
                window,
                ffi::XCB_CW_CURSOR,
                &cursor as *const _ as _,
            );
            if let Err(e) = self.check_cookie(cookie) {
                log::error!("Failed to set the cursor: {}", e);
            }
        }
    }
}
