#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct DeviceId(pub i32);

impl DeviceId {
    pub const fn dummy() -> Self {
        Self(0)
    }
}
