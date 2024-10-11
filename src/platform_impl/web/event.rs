use crate::event::{DeviceId, FingerId as RootFingerId};

pub(crate) fn mkdid(pointer_id: i32) -> Option<DeviceId> {
    if let Ok(pointer_id) = u32::try_from(pointer_id) {
        Some(DeviceId::from_raw(pointer_id as i64))
    } else if pointer_id == -1 {
        None
    } else {
        tracing::error!("found unexpected negative `PointerEvent.pointerId`: {pointer_id}");
        None
    }
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct FingerId {
    pointer_id: i32,
}

impl FingerId {
    pub fn new(pointer_id: i32) -> Self {
        Self { pointer_id }
    }

    #[cfg(test)]
    pub const fn dummy() -> Self {
        Self { pointer_id: -1 }
    }
}

impl From<FingerId> for RootFingerId {
    fn from(id: FingerId) -> Self {
        Self(id)
    }
}
