use bitflags::bitflags;
use dpi::{PhysicalPosition, PhysicalSize, Position, Size};

pub enum Direction {
    None,
    Top,
    Bottom,
    Left,
    Right,
    TopLeft,
    BottomLeft,
    TopRight,
    BottomRight,
}

bitflags! {
    pub struct AnchorHints: u8 {
        const SlideX = 1;
        const SlideY = 2;
        const Slide = AnchorHints::SlideX.bits() | AnchorHints::SlideY.bits();
        const FlipX = 4;
        const FlipY = 8;
        const Flip = AnchorHints::FlipX.bits() | AnchorHints::FlipY.bits();
        const ResizeX = 16;
        const ResizeY = 32;
        const Resize = AnchorHints::ResizeX.bits() | AnchorHints::ResizeY.bits();
    }
}

pub struct PopupAttributes {
    pub anchor_size: Size,
    pub anchor: Direction,
    pub gravity: Direction,
    pub anchor_hints: AnchorHints,
    pub offset: Position,
}

impl Default for PopupAttributes {
    fn default() -> PopupAttributes {
        PopupAttributes {
            anchor_size: PhysicalSize::<u32>::default().into(),
            anchor: Direction::None,
            gravity: Direction::None,
            anchor_hints: AnchorHints::empty(),
            offset: PhysicalPosition::<i32>::default().into(),
        }
    }
}
