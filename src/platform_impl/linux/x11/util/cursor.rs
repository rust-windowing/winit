use std::{
    ffi::CString,
    hash::{Hash, Hasher},
    iter, slice,
    sync::Arc,
};

use x11rb::connection::Connection;

use crate::{platform_impl::PlatformCustomCursorBuilder, window::CursorIcon};

use super::super::ActiveEventLoop;
use super::*;

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

    pub(crate) fn set_custom_cursor(&self, window: xproto::Window, cursor: &CustomCursor) {
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SelectedCursor {
    Custom(CustomCursor),
    Named(CursorIcon),
}

#[derive(Debug, Clone)]
pub struct CustomCursor {
    inner: Arc<CustomCursorInner>,
}

impl Hash for CustomCursor {
    fn hash<H: Hasher>(&self, state: &mut H) {
        Arc::as_ptr(&self.inner).hash(state);
    }
}

impl PartialEq for CustomCursor {
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.inner, &other.inner)
    }
}

impl Eq for CustomCursor {}

impl CustomCursor {
    pub(crate) fn build(builder: PlatformCustomCursorBuilder, p: &ActiveEventLoop) -> CustomCursor {
        unsafe {
            let ximage = (p.xconn.xcursor.XcursorImageCreate)(
                builder.0.width as i32,
                builder.0.height as i32,
            );
            if ximage.is_null() {
                panic!("failed to allocate cursor image");
            }
            (*ximage).xhot = builder.0.hotspot_x as u32;
            (*ximage).yhot = builder.0.hotspot_y as u32;
            (*ximage).delay = 0;

            let dst = slice::from_raw_parts_mut((*ximage).pixels, builder.0.rgba.len() / 4);
            for (dst, chunk) in dst.iter_mut().zip(builder.0.rgba.chunks_exact(4)) {
                *dst = (chunk[0] as u32) << 16
                    | (chunk[1] as u32) << 8
                    | (chunk[2] as u32)
                    | (chunk[3] as u32) << 24;
            }

            let cursor = (p.xconn.xcursor.XcursorImageLoadCursor)(p.xconn.display, ximage);
            (p.xconn.xcursor.XcursorImageDestroy)(ximage);
            Self {
                inner: Arc::new(CustomCursorInner {
                    xconn: p.xconn.clone(),
                    cursor,
                }),
            }
        }
    }
}

#[derive(Debug)]
struct CustomCursorInner {
    xconn: Arc<XConnection>,
    cursor: ffi::Cursor,
}

impl Drop for CustomCursorInner {
    fn drop(&mut self) {
        unsafe {
            (self.xconn.xlib.XFreeCursor)(self.xconn.display, self.cursor);
        }
    }
}

impl Default for SelectedCursor {
    fn default() -> Self {
        SelectedCursor::Named(Default::default())
    }
}
