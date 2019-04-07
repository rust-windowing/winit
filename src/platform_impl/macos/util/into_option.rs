use cocoa::base::{id, nil};

pub trait IntoOption: Sized {
    fn into_option(self) -> Option<Self>;
}

impl IntoOption for id {
    fn into_option(self) -> Option<Self> {
        match self != nil {
            true => Some(self),
            false => None,
        }
    }
}
