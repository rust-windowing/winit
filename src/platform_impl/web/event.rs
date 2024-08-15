#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct DeviceId(i32);

impl DeviceId {
    pub fn new(pointer_id: i32) -> Self {
        Self(pointer_id)
    }

    pub const fn dummy() -> Self {
        Self(-1)
    }
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct FingerId {
    pointer_id: i32,
    primary: bool,
}

impl FingerId {
    pub fn new(pointer_id: i32, primary: bool) -> Self {
        Self { pointer_id, primary }
    }

    pub const fn dummy() -> Self {
        Self { pointer_id: -1, primary: false }
    }

    pub fn is_primary(self) -> bool {
        self.primary
    }
}
