use std::path::PathBuf;

use sctk::data_device_manager::data_offer::DragOffer;

pub struct DndOfferState {
    pub offer: DragOffer,
    pub file_path: PathBuf,
}
