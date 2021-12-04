use crate::backend::Backend;

mod x11;

pub fn backends() -> Vec<Box<dyn Backend>> {
    vec![x11::backend()]
}
