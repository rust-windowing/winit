#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Id(pub i32);

impl Id {
    pub unsafe fn dummy() -> Self {
        Id(0)
    }
}
