//! Anchor-based window placement: the types describing a placement request, and the
//! `xdg_positioner`-style algorithm ([`place_window`]) that resolves them into a concrete
//! position and size.

use dpi::{LogicalPosition, LogicalSize};

use super::WindowPositioner;

/// Anchor rect within the parent surface
/// See: https://wayland.app/protocols/xdg-shell#xdg_positioner:request:set_anchor_rect
#[derive(Debug, Clone, Copy, Default, PartialEq)]
#[non_exhaustive]
pub enum WindowAnchor {
    #[default]
    Center,
    Top,
    Bottom,
    Left,
    Right,
    TopLeft,
    BottomLeft,
    TopRight,
    BottomRight,
}

/// Defines in what direction a surface should be positioned
/// See: https://wayland.app/protocols/xdg-shell#xdg_positioner:request:set_gravity
#[derive(Debug, Clone, Copy, Default, PartialEq)]
#[non_exhaustive]
pub enum WindowGravity {
    #[default]
    Center,
    Top,
    Bottom,
    Left,
    Right,
    TopLeft,
    BottomLeft,
    TopRight,
    BottomRight,
}

bitflags::bitflags! {
    /// Specify how the window should be positioned if the originally intended position caused the
    /// surface to be constrained See: https://wayland.app/protocols/xdg-shell#xdg_positioner:request:set_constraint_adjustment
    /// For all other platforms than wayland the behaviour is simulated on the winit side
    #[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
    pub struct WindowConstraintAdjustment: u32 {
        const SLIDE_X = 1 << 0;
        const SLIDE_Y = 1 << 1;
        const FLIP_X = 1 << 2;
        const FLIP_Y = 1 << 3;
        const RESIZE_X = 1 << 4;
        const RESIZE_Y = 1 << 5;
    }
}

/// Returns, as fractions of the anchor rectangle's width/height, the point within that rectangle
/// that the popup is anchored to (0.0 = left/top edge, 0.5 = center, 1.0 = right/bottom edge).
/// Mirrors the Wayland `xdg_positioner` anchor semantics.
fn anchor_fraction(anchor: WindowAnchor) -> (f64, f64) {
    match anchor {
        WindowAnchor::Center => (0.5, 0.5),
        WindowAnchor::Top => (0.5, 0.0),
        WindowAnchor::Bottom => (0.5, 1.0),
        WindowAnchor::Left => (0.0, 0.5),
        WindowAnchor::Right => (1.0, 0.5),
        WindowAnchor::TopLeft => (0.0, 0.0),
        WindowAnchor::BottomLeft => (0.0, 1.0),
        WindowAnchor::TopRight => (1.0, 0.0),
        WindowAnchor::BottomRight => (1.0, 1.0),
    }
}

/// Returns, as fractions of the popup's own width/height, the offset from the anchor point to
/// the popup's origin (top-left corner). For example a gravity of `BottomRight` places the
/// popup's top-left corner at the anchor point, so the popup grows down and to the right.
fn gravity_fraction(gravity: WindowGravity) -> (f64, f64) {
    match gravity {
        WindowGravity::Center => (-0.5, -0.5),
        WindowGravity::Top => (-0.5, -1.0),
        WindowGravity::Bottom => (-0.5, 0.0),
        WindowGravity::Left => (-1.0, -0.5),
        WindowGravity::Right => (0.0, -0.5),
        WindowGravity::TopLeft => (-1.0, -1.0),
        WindowGravity::BottomLeft => (-1.0, 0.0),
        WindowGravity::TopRight => (0.0, -1.0),
        WindowGravity::BottomRight => (0.0, 0.0),
    }
}

/// Adjusts a single axis of the popup's placement to stay within `[clip_min, clip_max]`, applying
/// `flip`/`slide`/`resize` in the order the `xdg_positioner` protocol suggests: flip, then slide,
/// then resize. `flipped_origin` is the alternate origin obtained by mirroring both the anchor
/// edge and the gravity on this axis; it is only used when `flip` is set and it actually results
/// in a better fit than the original origin.
fn constrain_axis(
    origin: f64,
    extent: f64,
    (clip_min, clip_max): (f64, f64),
    flipped_origin: f64,
    (flip, slide, resize): (bool, bool, bool),
) -> (f64, f64) {
    let fits = |o: f64| o >= clip_min && o + extent <= clip_max;

    let mut origin = origin;
    if !fits(origin) && flip && fits(flipped_origin) {
        origin = flipped_origin;
    }

    if !fits(origin) && slide {
        // `extent` may exceed the clip size, in which case `clip_max - extent` would be less
        // than `clip_min`; `.max(clip_min)` keeps the clamp range valid in that case.
        let slide_max = (clip_max - extent).max(clip_min);
        origin = origin.clamp(clip_min, slide_max);
    }

    let mut extent = extent;
    if resize && !fits(origin) {
        let clamped_origin = origin.max(clip_min);
        // Intersect with the clip rectangle on both ends, not just `clip_max`: if only the
        // leading edge overflows (`origin < clip_min`) while the trailing edge already fits,
        // this must shrink down to the original trailing edge rather than growing all the way
        // out to `clip_max`.
        extent = (clip_max.min(origin + extent) - clamped_origin).max(0.0);
        origin = clamped_origin;
    }

    (origin, extent)
}

/// Finds a placement for a window of `window_size`, anchored per `positioner` (whose
/// [`anchor_rect`](WindowPositioner::anchor_rect) and
/// [`positioner_offset`](WindowPositioner::positioner_offset) are converted to logical
/// coordinates using `scale_factor`, then interpreted in the same coordinate space as `clip`), and
/// constrained to stay within the `clip` rectangle according to
/// [`constraint_adjustment`](WindowPositioner::constraint_adjustment).
///
/// This mirrors the Wayland `xdg_positioner` placement algorithm used natively on Wayland, for
/// backends (such as Win32 and AppKit) that have no equivalent native concept and therefore need
/// to compute the popup position themselves: `anchor` selects a point on the anchor rectangle,
/// `gravity` decides which corner/edge of the popup is placed at that point, and if the resulting
/// rectangle doesn't fit inside the clip rectangle, `constraint_adjustment`'s flags decide whether
/// (and how) the popup is moved (slide), mirrored to the other side of the anchor point (flip),
/// or shrunk (resize) to fit. Axes are adjusted independently. If none of the flags are set for an
/// axis, that axis is left as computed even if it doesn't fit, matching the protocol's "none"
/// behavior.
pub fn place_window(
    positioner: &WindowPositioner,
    scale_factor: f64,
    window_size: LogicalSize<f64>,
    (clip_position, clip_size): (LogicalPosition<f64>, LogicalSize<f64>),
) -> (LogicalPosition<f64>, LogicalSize<f64>) {
    let anchor = positioner.anchor;
    let gravity = positioner.gravity;
    let constraint_adjustment = positioner.constraint_adjustment;
    let (anchor_position, anchor_size) = positioner.anchor_rect;
    let anchor_position = anchor_position.to_logical::<f64>(scale_factor);
    let anchor_size = anchor_size.to_logical::<f64>(scale_factor);
    let offset = positioner.positioner_offset.to_logical::<f64>(scale_factor);

    let (anchor_fx, anchor_fy) = anchor_fraction(anchor);
    let (gravity_fx, gravity_fy) = gravity_fraction(gravity);

    let anchor_point_x = anchor_position.x + anchor_size.width * anchor_fx;
    let anchor_point_y = anchor_position.y + anchor_size.height * anchor_fy;

    let origin_x = anchor_point_x + window_size.width * gravity_fx + offset.x;
    let origin_y = anchor_point_y + window_size.height * gravity_fy + offset.y;

    // Flipping mirrors both the anchor edge and the gravity on that axis, effectively placing the
    // popup on the opposite side of the anchor rectangle.
    let flipped_anchor_point_x = anchor_position.x + anchor_size.width * (1.0 - anchor_fx);
    let flipped_anchor_point_y = anchor_position.y + anchor_size.height * (1.0 - anchor_fy);
    let flipped_x = flipped_anchor_point_x + window_size.width * (-1.0 - gravity_fx) + offset.x;
    let flipped_y = flipped_anchor_point_y + window_size.height * (-1.0 - gravity_fy) + offset.y;

    let clip_min_x = clip_position.x;
    let clip_max_x = clip_position.x + clip_size.width;
    let clip_min_y = clip_position.y;
    let clip_max_y = clip_position.y + clip_size.height;

    let (x, width) = constrain_axis(
        origin_x,
        window_size.width,
        (clip_min_x, clip_max_x),
        flipped_x,
        (
            constraint_adjustment.contains(WindowConstraintAdjustment::FLIP_X),
            constraint_adjustment.contains(WindowConstraintAdjustment::SLIDE_X),
            constraint_adjustment.contains(WindowConstraintAdjustment::RESIZE_X),
        ),
    );
    let (y, height) = constrain_axis(
        origin_y,
        window_size.height,
        (clip_min_y, clip_max_y),
        flipped_y,
        (
            constraint_adjustment.contains(WindowConstraintAdjustment::FLIP_Y),
            constraint_adjustment.contains(WindowConstraintAdjustment::SLIDE_Y),
            constraint_adjustment.contains(WindowConstraintAdjustment::RESIZE_Y),
        ),
    );

    (LogicalPosition::new(x, y), LogicalSize::new(width, height))
}

#[cfg(test)]
mod tests {
    use dpi::{Position, Size};

    use super::*;

    const GRID_COLS: usize = 60;
    const GRID_ROWS: usize = 24;
    const DRAW: bool = false;

    /// Prints an ASCII rendering of the clip region (`.`), the anchor rect (`A`), and the
    /// resulting popup rect (`P`, `X` where it overlaps the anchor) for a quick visual sanity
    /// check. Run with `cargo test -p winit-core positioner -- --nocapture` to see it.
    // `println!` (rather than `tracing`) is intentional: this is only meant to be read directly
    // via `--nocapture`, which doesn't require a tracing subscriber to be installed.
    #[allow(clippy::disallowed_macros)]
    fn draw(
        label: &str,
        clip: (LogicalPosition<f64>, LogicalSize<f64>),
        anchor: (LogicalPosition<f64>, LogicalSize<f64>),
        popup: (LogicalPosition<f64>, LogicalSize<f64>),
    ) {
        if !DRAW {
            return;
        }

        let (clip_position, clip_size) = clip;
        let (anchor_position, anchor_size) = anchor;
        let (popup_position, popup_size) = popup;
        let has_clip = clip_size.width > 0.0 && clip_size.height > 0.0;

        // Bounding box covering everything we're about to draw, plus a small margin.
        let mut min_x = anchor_position.x.min(popup_position.x);
        let mut min_y = anchor_position.y.min(popup_position.y);
        let mut max_x =
            (anchor_position.x + anchor_size.width).max(popup_position.x + popup_size.width);
        let mut max_y =
            (anchor_position.y + anchor_size.height).max(popup_position.y + popup_size.height);
        if has_clip {
            min_x = min_x.min(clip_position.x);
            min_y = min_y.min(clip_position.y);
            max_x = max_x.max(clip_position.x + clip_size.width);
            max_y = max_y.max(clip_position.y + clip_size.height);
        }
        let margin_x = ((max_x - min_x) * 0.1).max(1.0);
        let margin_y = ((max_y - min_y) * 0.1).max(1.0);
        min_x -= margin_x;
        min_y -= margin_y;
        max_x += margin_x;
        max_y += margin_y;

        // Terminal characters are roughly twice as tall as they are wide, so use half the
        // vertical scale to keep the rendered rectangles' proportions roughly correct.
        let scale =
            (GRID_COLS as f64 / (max_x - min_x)).min(2.0 * GRID_ROWS as f64 / (max_y - min_y));

        let mut grid = vec![vec![' '; GRID_COLS]; GRID_ROWS];
        let mut plot = |position: LogicalPosition<f64>, size: LogicalSize<f64>, ch: char| {
            let x0 = ((position.x - min_x) * scale).round() as isize;
            let y0 = ((position.y - min_y) * scale / 2.0).round() as isize;
            let x1 = ((position.x + size.width - min_x) * scale).round() as isize;
            let y1 = ((position.y + size.height - min_y) * scale / 2.0).round() as isize;
            for y in y0.max(0)..y1.min(GRID_ROWS as isize) {
                for x in x0.max(0)..x1.min(GRID_COLS as isize) {
                    let cell = &mut grid[y as usize][x as usize];
                    *cell = if *cell == ' ' || *cell == ch { ch } else { 'X' };
                }
            }
        };

        if has_clip {
            plot(clip_position, clip_size, '.');
        }
        plot(anchor_position, anchor_size, 'A');
        plot(popup_position, popup_size, 'P');

        println!("--- {label} (A = anchor, P = popup, X = overlap, . = clip region) ---");
        for row in grid {
            println!("{}", row.into_iter().collect::<String>());
        }
    }

    /// A 20x10 anchor rect at (100, 100), popup size 40x30, no constraints applied
    /// (unclipped).
    #[test]
    fn test_place_popup_gravity() {
        let anchor_position = LogicalPosition::new(100., 100.);
        let anchor_size = LogicalSize::new(20., 10.);
        let popup_size = LogicalSize::new(40., 30.);
        let clip_position = LogicalPosition::new(0., 0.);
        let clip_size = LogicalSize::new(0., 0.);
        let offset = Position::Logical(LogicalPosition::new(0., 0.));

        let place = |anchor: WindowAnchor, gravity: WindowGravity| {
            let positioner = WindowPositioner::new(
                anchor,
                (Position::Logical(anchor_position), Size::Logical(anchor_size)),
                offset,
                gravity,
                WindowConstraintAdjustment::empty(),
            );
            place_window(&positioner, 1.0, popup_size, (clip_position, clip_size))
        };

        // BottomRight gravity anchored to the anchor's bottom-right corner: the popup's top-left
        // corner sits exactly at the anchor rect's bottom-right corner.
        let (origin, size) = place(WindowAnchor::BottomRight, WindowGravity::BottomRight);
        draw(
            "gravity: BottomRight anchor + BottomRight gravity",
            (clip_position, clip_size),
            (anchor_position, anchor_size),
            (origin, size),
        );
        assert_eq!(origin, LogicalPosition::new(120., 110.));
        assert_eq!(size, popup_size);

        // TopLeft gravity anchored to the anchor's top-left corner: the popup's bottom-right
        // corner sits exactly at the anchor rect's top-left corner, so the popup extends
        // up-left.
        let (origin, size) = place(WindowAnchor::TopLeft, WindowGravity::TopLeft);
        draw(
            "gravity: TopLeft anchor + TopLeft gravity",
            (clip_position, clip_size),
            (anchor_position, anchor_size),
            (origin, size),
        );
        assert_eq!(origin, LogicalPosition::new(100. - 40., 100. - 30.));

        // Bottom anchor + Bottom gravity: horizontally centered on the anchor, growing downward
        // from its bottom edge.
        let (origin, size) = place(WindowAnchor::Bottom, WindowGravity::Bottom);
        draw(
            "gravity: Bottom anchor + Bottom gravity",
            (clip_position, clip_size),
            (anchor_position, anchor_size),
            (origin, size),
        );
        assert_eq!(
            origin,
            LogicalPosition::new(100. + anchor_size.width / 2. - popup_size.width / 2., 110.)
        );

        // Center anchor + Center gravity centers the popup exactly on the anchor rect's center.
        let (origin, size) = place(WindowAnchor::Center, WindowGravity::Center);
        draw(
            "gravity: Center anchor + Center gravity",
            (clip_position, clip_size),
            (anchor_position, anchor_size),
            (origin, size),
        );
        let anchor_center =
            LogicalPosition::new(100. + anchor_size.width / 2., 100. + anchor_size.height / 2.);
        assert_eq!(
            origin,
            LogicalPosition::new(
                anchor_center.x - popup_size.width / 2.,
                anchor_center.y - popup_size.height / 2.
            )
        );
    }

    /// Place the Anchor so that the popup will go outside of the right side of the clip rectangle
    /// Because of FLIP_X the popup will be flipped on the left side of the anchor rectangle
    #[test]
    fn test_place_popup_flip() {
        // Anchor rect hugging the right edge of a 300x300 clip region; with BottomRight gravity
        // the popup would overflow past the right edge, so flipping should place it to the left
        // instead.
        let clip_position = LogicalPosition::new(0., 0.);
        let clip_size = LogicalSize::new(300., 300.);
        let anchor_position = LogicalPosition::new(280., 100.);
        let anchor_size = LogicalSize::new(10., 10.);
        let popup_size = LogicalSize::new(50., 50.);

        let flip_only = WindowConstraintAdjustment::FLIP_X | WindowConstraintAdjustment::FLIP_Y;
        let offset = Position::Logical(LogicalPosition::new(0., 0.));

        // Without flipping the popup would start at x=290 and end at x=340, past the clip's
        // right edge (300); flipping mirrors both anchor edge and gravity, so it should end up
        // entirely to the left of the anchor rect instead, fully inside the clip region.
        let positioner = WindowPositioner::new(
            WindowAnchor::TopRight,
            (Position::Logical(anchor_position), Size::Logical(anchor_size)),
            offset,
            WindowGravity::BottomRight,
            flip_only,
        );
        let (origin, size) = place_window(&positioner, 1.0, popup_size, (clip_position, clip_size));
        draw("flip", (clip_position, clip_size), (anchor_position, anchor_size), (origin, size));
        assert!(origin.x >= 0. && origin.x + size.width <= 300.);
        assert_eq!(size, popup_size);
        // Flipped horizontally: popup's right edge lands on the anchor rect's left edge
        // (x=280).
        assert_eq!(origin.x, 280. - popup_size.width);
        // Not flipped vertically: still grows down from the anchor's top edge.
        assert_eq!(origin.y, 100.);
    }

    /// Anchor near the bottom-right corner of the clip region; sliding (without flipping)
    /// should shift the popup back into view while keeping its size and general placement
    /// direction.
    #[test]
    fn test_place_popup_slide() {
        let clip_position = LogicalPosition::new(0., 0.);
        let clip_size = LogicalSize::new(300., 300.);
        let anchor_position = LogicalPosition::new(280., 280.);
        let popup_size = LogicalSize::new(50., 50.);

        let slide_only = WindowConstraintAdjustment::SLIDE_X | WindowConstraintAdjustment::SLIDE_Y;
        let offset = Position::Logical(LogicalPosition::new(0., 0.));

        let positioner = WindowPositioner::new(
            WindowAnchor::BottomRight,
            (Position::Logical(anchor_position), Size::Logical(LogicalSize::new(0., 0.))),
            offset,
            WindowGravity::BottomRight,
            slide_only,
        );
        let (origin, size) = place_window(&positioner, 1.0, popup_size, (clip_position, clip_size));
        draw(
            "slide",
            (clip_position, clip_size),
            (anchor_position, LogicalSize::new(0., 0.)),
            (origin, size),
        );
        assert_eq!(size, popup_size);
        assert_eq!(origin, LogicalPosition::new(250., 250.));
    }

    /// Popup larger than the clip region on both axes; with only `resize` enabled it should
    /// be shrunk (and clamped) to exactly fill the clip region rather than sliding or
    /// flipping.
    #[test]
    fn test_place_popup_resize() {
        let clip_position = LogicalPosition::new(0., 0.);
        let clip_size = LogicalSize::new(300., 300.);
        let anchor_position = LogicalPosition::new(0., 0.);
        let popup_size = LogicalSize::new(500., 500.);

        let resize_only =
            WindowConstraintAdjustment::RESIZE_X | WindowConstraintAdjustment::RESIZE_Y;
        let offset = Position::Logical(LogicalPosition::new(0., 0.));

        let positioner = WindowPositioner::new(
            WindowAnchor::TopLeft,
            (Position::Logical(anchor_position), Size::Logical(LogicalSize::new(0., 0.))),
            offset,
            WindowGravity::BottomRight,
            resize_only,
        );
        let (origin, size) = place_window(&positioner, 1.0, popup_size, (clip_position, clip_size));
        draw(
            "resize",
            (clip_position, clip_size),
            (anchor_position, LogicalSize::new(0., 0.)),
            (origin, size),
        );
        assert_eq!(origin, clip_position);
        assert_eq!(size, clip_size);
    }

    /// Popup that only overflows the clip region on its leading edge (its trailing edge
    /// already fits comfortably). With only `resize` enabled, it should shrink down to the
    /// intersection with the clip region -- i.e. stay anchored to its original trailing edge --
    /// rather than growing all the way out to the far side of the clip region.
    #[test]
    fn test_place_popup_resize_leading_edge_only() {
        let clip_position = LogicalPosition::new(0., 0.);
        let clip_size = LogicalSize::new(300., 300.);
        // TopLeft anchor + BottomRight gravity places the popup's origin exactly at the anchor
        // position, so this popup starts at x=-20 (off the left edge) and ends at x=30 (well
        // within the clip region).
        let anchor_position = LogicalPosition::new(-20., 0.);
        let popup_size = LogicalSize::new(50., 50.);

        let resize_only = WindowConstraintAdjustment::RESIZE_X;
        let offset = Position::Logical(LogicalPosition::new(0., 0.));

        let positioner = WindowPositioner::new(
            WindowAnchor::TopLeft,
            (Position::Logical(anchor_position), Size::Logical(LogicalSize::new(0., 0.))),
            offset,
            WindowGravity::BottomRight,
            resize_only,
        );
        let (origin, size) = place_window(&positioner, 1.0, popup_size, (clip_position, clip_size));
        draw(
            "resize (leading edge only)",
            (clip_position, clip_size),
            (anchor_position, LogicalSize::new(0., 0.)),
            (origin, size),
        );
        assert_eq!(origin, LogicalPosition::new(0., 0.));
        // Trailing edge (originally at x=30) must stay put -- the popup should shrink to width
        // 30, not grow out to the clip region's full width of 300.
        assert_eq!(size, LogicalSize::new(30., 50.));
    }

    /// With no constraint-adjustment flags set, an overflowing popup is left exactly where
    /// the anchor/gravity math puts it, matching the `xdg_positioner` "none" behavior.
    #[test]
    fn test_place_popup_no_adjustment() {
        let clip_position = LogicalPosition::new(0., 0.);
        let clip_size = LogicalSize::new(100., 100.);
        let anchor_position = LogicalPosition::new(90., 90.);
        let popup_size = LogicalSize::new(50., 50.);
        let offset = Position::Logical(LogicalPosition::new(0., 0.));

        let positioner = WindowPositioner::new(
            WindowAnchor::TopLeft,
            (Position::Logical(anchor_position), Size::Logical(LogicalSize::new(0., 0.))),
            offset,
            WindowGravity::BottomRight,
            WindowConstraintAdjustment::empty(),
        );
        let (origin, size) = place_window(&positioner, 1.0, popup_size, (clip_position, clip_size));
        draw(
            "no adjustment",
            (clip_position, clip_size),
            (anchor_position, LogicalSize::new(0., 0.)),
            (origin, size),
        );
        assert_eq!(origin, anchor_position);
        assert_eq!(size, popup_size);
    }

    /// Sanity check: when the popup already fits, none of the adjustment flags should move
    /// or resize it, regardless of which are enabled.
    #[test]
    fn test_place_popup_all_adjustment_no_op_when_fits() {
        let clip_position = LogicalPosition::new(0., 0.);
        let clip_size = LogicalSize::new(300., 300.);
        let anchor_position = LogicalPosition::new(100., 100.);
        let popup_size = LogicalSize::new(50., 50.);
        let offset = Position::Logical(LogicalPosition::new(0., 0.));

        let positioner = WindowPositioner::new(
            WindowAnchor::TopLeft,
            (Position::Logical(anchor_position), Size::Logical(LogicalSize::new(0., 0.))),
            offset,
            WindowGravity::BottomRight,
            WindowConstraintAdjustment::all(),
        );
        let (origin, size) = place_window(&positioner, 1.0, popup_size, (clip_position, clip_size));
        draw(
            "all adjustment, already fits",
            (clip_position, clip_size),
            (anchor_position, LogicalSize::new(0., 0.)),
            (origin, size),
        );
        assert_eq!(origin, anchor_position);
        assert_eq!(size, popup_size);
    }
}
