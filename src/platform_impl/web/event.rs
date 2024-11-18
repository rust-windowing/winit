use crate::event::DeviceId;

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
