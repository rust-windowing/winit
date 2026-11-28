use crate::data_transfer::{DataTransferId, DataTransferSend};
use crate::error::TransferError;

/// Platform extension allowing access to the primary selection, where available.
///
/// The primary selection is a separate clipboard on Wayland and X11, usually holding the most
/// recently selected text and pasted with a middle click.
///
/// Access via [`ActiveEventLoop::primary_selection_ext`].
///
/// # Example
///
/// ```ignore
/// if let Some(ext) = event_loop.primary_selection_ext() {
///     if let Some(data_transfer_id) = ext.primary_selection()? {
///         event_loop.fetch_data_transfer(data_transfer_id, &TypeHint::Plaintext)?;
///     }
/// }
/// ```
///
/// [`ActiveEventLoop::primary_selection_ext`]: crate::event_loop::ActiveEventLoop::primary_selection_ext
pub trait PrimarySelectionExt: std::fmt::Debug {
    /// Get the current primary selection contents as a [data transfer](crate::data_transfer), or
    /// `None` if the primary selection is empty.
    ///
    /// See [`ActiveEventLoop::clipboard`] for more details.
    ///
    /// [`ActiveEventLoop::clipboard`]: crate::event_loop::ActiveEventLoop::clipboard
    fn primary_selection(&self) -> Result<Option<DataTransferId>, TransferError>;

    /// Set the primary selection contents.
    ///
    /// See [`ActiveEventLoop::set_clipboard`] for more details.
    ///
    /// [`ActiveEventLoop::set_clipboard`]: crate::event_loop::ActiveEventLoop::set_clipboard
    fn set_primary_selection(
        &self,
        send_data: Box<dyn DataTransferSend>,
    ) -> Result<(), TransferError>;
}
