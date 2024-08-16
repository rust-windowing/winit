use crate::event::FingerId as RootFingerId;

#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct DeviceId(pub(crate) u32);

impl DeviceId {
    pub fn new(pointer_id: i32) -> Option<Self> {
        if let Ok(pointer_id) = u32::try_from(pointer_id) {
            Some(Self(pointer_id))
        } else if pointer_id == -1 {
            None
        } else {
            tracing::error!("found unexpected negative `PointerEvent.pointerId`: {pointer_id}");
            None
        }
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

    #[cfg(test)]
    pub const fn dummy() -> Self {
        Self { pointer_id: -1, primary: false }
    }

    pub fn is_primary(self) -> bool {
        self.primary
    }
}

impl From<FingerId> for RootFingerId {
    fn from(id: FingerId) -> Self {
        Self(id)
    }
}
