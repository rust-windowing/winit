use std::cmp;
use x11rb::protocol::xproto::{self, ConnectionExt as _};

use super::*;
use crate::platform_impl::x11::atoms::*;

// Friendly neighborhood axis-aligned rectangle
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AaRect {
    x: i64,
    y: i64,
    width: i64,
    height: i64,
}

impl AaRect {
    pub fn new((x, y): (i32, i32), (width, height): (u32, u32)) -> Self {
        let (x, y) = (x as i64, y as i64);
        let (width, height) = (width as i64, height as i64);
        AaRect {
            x,
            y,
            width,
            height,
        }
    }

    pub fn contains_point(&self, x: i64, y: i64) -> bool {
        x >= self.x && x <= self.x + self.width && y >= self.y && y <= self.y + self.height
    }

    pub fn get_overlapping_area(&self, other: &Self) -> i64 {
        let x_overlap = cmp::max(
            0,
            cmp::min(self.x + self.width, other.x + other.width) - cmp::max(self.x, other.x),
        );
        let y_overlap = cmp::max(
            0,
            cmp::min(self.y + self.height, other.y + other.height) - cmp::max(self.y, other.y),
        );
        x_overlap * y_overlap
    }
}

#[derive(Debug, Default)]
pub struct TranslatedCoords {
    pub x_rel_root: c_int,
    pub y_rel_root: c_int,
    pub child: ffi::Window,
}

#[derive(Debug, Default)]
pub struct Geometry {
    pub root: ffi::Window,
    // If you want positions relative to the root window, use translate_coords.
    // Note that the overwhelming majority of window managers are reparenting WMs, thus the window
    // ID we get from window creation is for a nested window used as the window's client area. If
    // you call get_geometry with that window ID, then you'll get the position of that client area
    // window relative to the parent it's nested in (the frame), which isn't helpful if you want
    // to know the frame position.
    pub x_rel_parent: c_int,
    pub y_rel_parent: c_int,
    // In that same case, this will give you client area size.
    pub width: c_uint,
    pub height: c_uint,
    // xmonad and dwm were the only WMs tested that use the border return at all.
    // The majority of WMs seem to simply fill it with 0 unconditionally.
    pub border: c_uint,
    pub depth: c_uint,
}

#[derive(Debug, Clone)]
pub struct FrameExtents {
    pub left: c_ulong,
    pub right: c_ulong,
    pub top: c_ulong,
    pub bottom: c_ulong,
}

impl FrameExtents {
    pub fn new(left: c_ulong, right: c_ulong, top: c_ulong, bottom: c_ulong) -> Self {
        FrameExtents {
            left,
            right,
            top,
            bottom,
        }
    }

    pub fn from_border(border: c_ulong) -> Self {
        Self::new(border, border, border, border)
    }
}

#[derive(Debug, Clone)]
pub struct LogicalFrameExtents {
    pub left: f64,
    pub right: f64,
    pub top: f64,
    pub bottom: f64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FrameExtentsHeuristicPath {
    Supported,
    UnsupportedNested,
    UnsupportedBordered,
}

#[derive(Debug, Clone)]
pub struct FrameExtentsHeuristic {
    pub frame_extents: FrameExtents,
    pub heuristic_path: FrameExtentsHeuristicPath,
}

impl FrameExtentsHeuristic {
    pub fn inner_pos_to_outer(&self, x: i32, y: i32) -> (i32, i32) {
        use self::FrameExtentsHeuristicPath::*;
        if self.heuristic_path != UnsupportedBordered {
            (
                x - self.frame_extents.left as i32,
                y - self.frame_extents.top as i32,
            )
        } else {
            (x, y)
        }
    }

    pub fn inner_size_to_outer(&self, width: u32, height: u32) -> (u32, u32) {
        (
            width.saturating_add(
                self.frame_extents
                    .left
                    .saturating_add(self.frame_extents.right) as _,
            ),
            height.saturating_add(
                self.frame_extents
                    .top
                    .saturating_add(self.frame_extents.bottom) as _,
            ),
        )
    }
}

impl XConnection {
    fn get_frame_extents(&self, window: xproto::Window) -> Option<FrameExtents> {
        let extents_atom = self.atoms[_NET_FRAME_EXTENTS];

        if !self.hint_is_supported(extents_atom) {
            return None;
        }

        // Of the WMs tested, xmonad, i3, dwm, IceWM (1.3.x and earlier), and blackbox don't
        // support this. As this is part of EWMH (Extended Window Manager Hints), it's likely to
        // be unsupported by many smaller WMs.
        let extents: Option<Vec<c_ulong>> = self
            .get_property(window, extents_atom, xproto::AtomEnum::CARDINAL.into())
            .ok();

        extents.and_then(|extents| {
            if extents.len() >= 4 {
                Some(FrameExtents {
                    left: extents[0],
                    right: extents[1],
                    top: extents[2],
                    bottom: extents[3],
                })
            } else {
                None
            }
        })
    }

    pub fn is_top_level(&self, window: xproto::Window, root: xproto::Window) -> Option<bool> {
        let client_list_atom = self.atoms[_NET_CLIENT_LIST];

        if !self.hint_is_supported(client_list_atom) {
            return None;
        }

        let client_list: Option<Vec<xproto::Window>> = self
            .get_property(root, client_list_atom, xproto::AtomEnum::WINDOW.into())
            .ok();

        client_list.map(|client_list| client_list.contains(&window))
    }

    fn get_parent_window(&self, window: xproto::Window) -> Result<xproto::Window, PlatformError> {
        Ok(self.connection.query_tree(window)?.reply()?.parent)
    }

    fn climb_hierarchy(
        &self,
        window: xproto::Window,
        root: xproto::Window,
    ) -> Result<xproto::Window, PlatformError> {
        let mut outer_window = window;
        loop {
            let candidate = self.get_parent_window(outer_window)?;
            if candidate == root {
                break;
            }
            outer_window = candidate;
        }
        Ok(outer_window)
    }

    pub fn get_frame_extents_heuristic(
        &self,
        window: xproto::Window,
        root: xproto::Window,
    ) -> FrameExtentsHeuristic {
        use self::FrameExtentsHeuristicPath::*;

        // Position relative to root window.
        // With rare exceptions, this is the position of a nested window. Cases where the window
        // isn't nested are outlined in the comments throghout this function, but in addition to
        // that, fullscreen windows often aren't nested.
        let (inner_y_rel_root, child) = {
            let coords = self
                .connection
                .translate_coordinates(window, root, 0, 0)
                .unwrap()
                .reply()
                .expect("Failed to translate window coordinates");
            (coords.dst_y, coords.child)
        };

        let (width, height, border) = {
            let inner_geometry = self
                .connection
                .get_geometry(window)
                .unwrap()
                .reply()
                .expect("Failed to get inner window geometry");
            (
                inner_geometry.width,
                inner_geometry.height,
                inner_geometry.border_width,
            )
        };

        // The first condition is only false for un-nested windows, but isn't always false for
        // un-nested windows. Mutter/Muffin/Budgie and Marco present a mysterious discrepancy:
        // when y is on the range [0, 2] and if the window has been unfocused since being
        // undecorated (or was undecorated upon construction), the first condition is true,
        // requiring us to rely on the second condition.
        let nested = !(window == child || self.is_top_level(child, root) == Some(true));

        // Hopefully the WM supports EWMH, allowing us to get exact info on the window frames.
        if let Some(mut frame_extents) = self.get_frame_extents(window) {
            // Mutter/Muffin/Budgie and Marco preserve their decorated frame extents when
            // decorations are disabled, but since the window becomes un-nested, it's easy to
            // catch.
            if !nested {
                frame_extents = FrameExtents::new(0, 0, 0, 0);
            }

            // The difference between the nested window's position and the outermost window's
            // position is equivalent to the frame size. In most scenarios, this is equivalent to
            // manually climbing the hierarchy as is done in the case below. Here's a list of
            // known discrepancies:
            // * Mutter/Muffin/Budgie gives decorated windows a margin of 9px (only 7px on top) in
            //   addition to a 1px semi-transparent border. The margin can be easily observed by
            //   using a screenshot tool to get a screenshot of a selected window, and is
            //   presumably used for drawing drop shadows. Getting window geometry information
            //   via hierarchy-climbing results in this margin being included in both the
            //   position and outer size, so a window positioned at (0, 0) would be reported as
            //   having a position (-10, -8).
            // * Compiz has a drop shadow margin just like Mutter/Muffin/Budgie, though it's 10px
            //   on all sides, and there's no additional border.
            // * Enlightenment otherwise gets a y position equivalent to inner_y_rel_root.
            //   Without decorations, there's no difference. This is presumably related to
            //   Enlightenment's fairly unique concept of window position; it interprets
            //   positions given to XMoveWindow as a client area position rather than a position
            //   of the overall window.

            FrameExtentsHeuristic {
                frame_extents,
                heuristic_path: Supported,
            }
        } else if nested {
            // If the position value we have is for a nested window used as the client area, we'll
            // just climb up the hierarchy and get the geometry of the outermost window we're
            // nested in.
            let outer_window = self
                .climb_hierarchy(window, root)
                .expect("Failed to climb window hierarchy");
            let (outer_y, outer_width, outer_height) = {
                let outer_geometry = self
                    .connection
                    .get_geometry(outer_window)
                    .unwrap()
                    .reply()
                    .expect("Failed to get outer window geometry");
                (
                    outer_geometry.y,
                    outer_geometry.width,
                    outer_geometry.height,
                )
            };

            // Since we have the geometry of the outermost window and the geometry of the client
            // area, we can figure out what's in between.
            let diff_x = outer_width.saturating_sub(width);
            let diff_y = outer_height.saturating_sub(height);
            let offset_y = inner_y_rel_root.saturating_sub(outer_y) as _;

            let left = diff_x / 2;
            let right = left;
            let top = offset_y;
            let bottom = diff_y.saturating_sub(offset_y);

            let frame_extents = FrameExtents::new(
                left as c_ulong,
                right as c_ulong,
                top as c_ulong,
                bottom as c_ulong,
            );
            FrameExtentsHeuristic {
                frame_extents,
                heuristic_path: UnsupportedNested,
            }
        } else {
            // This is the case for xmonad and dwm, AKA the only WMs tested that supplied a
            // border value. This is convenient, since we can use it to get an accurate frame.
            let frame_extents = FrameExtents::from_border(border as c_ulong);
            FrameExtentsHeuristic {
                frame_extents,
                heuristic_path: UnsupportedBordered,
            }
        }
    }
}
