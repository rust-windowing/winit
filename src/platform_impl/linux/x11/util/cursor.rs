use std::{ffi::CString, iter, slice, sync::Arc};

use x11rb::connection::Connection;

use crate::{cursor::CursorImage, window::CursorIcon};

use super::*;

#[derive(Debug)]
struct RaiiCursor {
    xconn: Arc<XConnection>,
    cursor: ffi::Cursor,
}

impl Drop for RaiiCursor {
    fn drop(&mut self) {
        unsafe {
            (self.xconn.xlib.XFreeCursor)(self.xconn.display, self.cursor);
        }
    }
}

impl PartialEq for RaiiCursor {
    fn eq(&self, other: &Self) -> bool {
        self.cursor == other.cursor
    }
}

impl Eq for RaiiCursor {}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CustomCursorInternal {
    inner: Arc<RaiiCursor>,
}

impl CustomCursorInternal {
    pub(crate) unsafe fn new(xconn: &Arc<XConnection>, image: &CursorImage) -> Self {
        unsafe {
            let ximage =
                (xconn.xcursor.XcursorImageCreate)(image.width as i32, image.height as i32);
            if ximage.is_null() {
                panic!("failed to allocate cursor image");
            }
            (*ximage).xhot = image.hotspot_x as u32;
            (*ximage).yhot = image.hotspot_y as u32;
            (*ximage).delay = 0;

            let dst = slice::from_raw_parts_mut((*ximage).pixels, image.rgba.len() / 4);
            for (dst, chunk) in dst.iter_mut().zip(image.rgba.chunks_exact(4)) {
                *dst = (chunk[0] as u32) << 16
                    | (chunk[1] as u32) << 8
                    | (chunk[2] as u32)
                    | (chunk[3] as u32) << 24;
            }

            let cursor = (xconn.xcursor.XcursorImageLoadCursor)(xconn.display, ximage);
            (xconn.xcursor.XcursorImageDestroy)(ximage);
            Self {
                inner: Arc::new(RaiiCursor {
                    xconn: xconn.clone(),
                    cursor,
                }),
            }
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SelectedCursor {
    Custom(CustomCursorInternal),
    Named(CursorIcon),
}

impl Default for SelectedCursor {
    fn default() -> Self {
        SelectedCursor::Named(Default::default())
    }
}

impl XConnection {
    pub fn set_cursor_icon(&self, window: xproto::Window, cursor: Option<CursorIcon>) {
        let cursor = *self
            .cursor_cache
            .lock()
            .unwrap()
            .entry(cursor)
            .or_insert_with(|| self.get_cursor(cursor));

        self.update_cursor(window, cursor)
            .expect("Failed to set cursor");
    }

    pub fn set_custom_cursor(&self, window: xproto::Window, cursor: &CustomCursorInternal) {
        self.update_cursor(window, cursor.inner.cursor)
            .expect("Failed to set cursor");
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

    fn get_cursor(&self, cursor: Option<CursorIcon>) -> ffi::Cursor {
        let cursor = match cursor {
            Some(cursor) => cursor,
            None => return self.create_empty_cursor(),
        };

        let mut xcursor = 0;
        for &name in iter::once(&cursor.name()).chain(cursor.alt_names().iter()) {
            let name = CString::new(name).unwrap();
            xcursor = unsafe {
                (self.xcursor.XcursorLibraryLoadCursor)(
                    self.display,
                    name.as_ptr() as *const c_char,
                )
            };

            if xcursor != 0 {
                break;
            }
        }

        xcursor
    }

    fn update_cursor(&self, window: xproto::Window, cursor: ffi::Cursor) -> Result<(), X11Error> {
        self.xcb_connection()
            .change_window_attributes(
                window,
                &xproto::ChangeWindowAttributesAux::new().cursor(cursor as xproto::Cursor),
            )?
            .ignore_error();

        self.xcb_connection().flush()?;
        Ok(())
    }
}
