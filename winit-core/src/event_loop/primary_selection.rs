use crate::data_transfer::{DataTransferId, DataTransferSend};
use crate::error::TransferError;

/// Platform extension allowing access to the primary selection, where available.
///
/// Access via [`ActiveEventLoop::primary_selection_ext`].
///
/// # Example
///
/// ```ignore
/// if let Some(ext) = event_loop.primary_selection_ext() {
///     if let Some(data_transfer_id) = ext.primary_selection()? {
///         ext.fetch_data_transfer(data_transfer_id, &TypeHint::Plaintext)?;
///     }
/// }
/// ```
pub trait PrimarySelectionExt: std::fmt::Debug {
    /// Get the current primary selection contents as a [data transfer](crate::data_transfer), or
    /// `None` if the primary selection is empty.
    ///
    /// See [`ActiveEventLoop::clipboard`] for more details.
    fn primary_selection(&self) -> Result<Option<DataTransferId>, TransferError>;

    /// Set the primary selection contents.
    ///
    /// See [`ActiveEventLoop::set_clipboard`] for more details.
    fn set_primary_selection(
        &self,
        send_data: Box<dyn DataTransferSend>,
    ) -> Result<(), TransferError>;
}
