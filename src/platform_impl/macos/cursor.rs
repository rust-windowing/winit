use objc2::rc::Id;

use super::appkit::NSCursor;
use super::EventLoopWindowTarget;
use crate::cursor::OnlyCursorImageBuilder;

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct CustomCursor(pub(crate) Id<NSCursor>);

impl CustomCursor {
    pub(crate) fn build<T>(
        cursor: OnlyCursorImageBuilder,
        _: &EventLoopWindowTarget<T>,
    ) -> CustomCursor {
        Self(NSCursor::from_image(&cursor.0))
    }
}
