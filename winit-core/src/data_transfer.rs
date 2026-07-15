//! Cross-platform abstractions related to data transfer (i.e. clipboard and drag-and-drop).
//!
//! > **NOTE**: Interacting with the clipboard is currently not implemented in Winit, and
//! > this API is only used for drag-and-drop.
//!
//! # Quickstart
//!
//! The API in this module is used for both sending and receiving data. The flow is detailed below,
//! but to quickly get started, the relevant APIs are the following:
//!
//! ### Receiving a drag-and-drop operation
//!
//! - [`DragEntered`](crate::event::WindowEvent::DragEntered) - informs a window that a new drag
//!   operation has started.
//! - [`data_transfer`](crate::event_loop::ActiveEventLoop::data_transfer) - get metadata about the
//!   incoming transfer.
//! - [`DataTransfer`] - metadata about the incoming transfer, in particular the available types
//! - [`set_valid_dnd_actions`](crate::event_loop::ActiveEventLoop::set_valid_dnd_actions) - the
//!   application must set at least some actions as valid in order for the drag to be considered
//!   accepted.
//! - [`fetch_data_transfer`](crate::event_loop::ActiveEventLoop::fetch_data_transfer) - request the
//!   actual data, with a specific type, from the data transfer.
//! - [`DataTransferReceived`](crate::event::WindowEvent::DataTransferReceived) - the actual data,
//!   with a specific type, has been received.
//! - [`TypedData`] - provides methods to read the actual data
//!
//! ### Sending a drag-and-drop operation
//!
//! - [`DataTransferSend`] - the core trait which defines data to be sent
//! - [`DataTransferSendBuilder`] - helper to create a new outgoing data transfer from a set of
//!   types and callbacks that supply data of that type
//! - [`ActiveEventLoop::start_drag`](crate::event_loop::ActiveEventLoop::start_drag) - the
//!   application calls this to start a new drag operation
//! - [`OutgoingDragDropped`](crate::event::WindowEvent::OutgoingDragDropped)/
//!   [`OutgoingDragCanceled`](crate::event::WindowEvent::OutgoingDragCanceled) - the application
//!   receives this when the user has ended the drag operation, by dropping the data or by canceling
//!   the operation respectively
//!
//! # Detailed flow
//!
//! ## Receiving a drag-and-drop operation
//!
//! On all platforms, the process looks something like this:
//!
//! - A data transfer advertises a set of types which the data can be interpreted as. While the
//!   precise implementation depends on platform, there's a set of types which can be safely
//!   transferred between applications on all platforms (see [`TypeHint`]).
//!   - For example, if you copy or drag text from a web page, the browser may advertise the text
//!     formatted using HTML, the text formatted as RTF, and the text with all formatting removed
//!     simultaneously.
//! - An application receiving a data transfer chooses one or more types that it understands and
//!   requests the data in those formats (in practice, it will usually only request a single
//!   format).
//! - The source application converts the data stored in its memory to the requested format and
//!   asynchronously sends it to the target application
//!
//! On some platforms, the data is sometimes available synchronously, but all platforms have at
//! least some method of sending the data asynchronously and some types of data that may _only_ be
//! sent using the asynchronous interface. Because of this, the API in winit must be asynchronous.
//!
//! The flow for a user application that implements drag-and-drop would look something like this:
//!
//! - The application receives a [`DragEntered`](crate::event::WindowEvent::DragEntered) event. This
//!   event supplies a [`DataTransferId`] which can be used to request information or operations on
//!   the dragged data by using methods on [`Window`](crate::window::Window).
//! - To make sure that the operating system displays the correct cursor, and that modifier keys
//!   will change the selected drag action correctly, the application should call
//!   [`set_valid_dnd_actions`](crate::event_loop::ActiveEventLoop::set_valid_dnd_actions). See
//!   documentation on that method for details.
//! - As the drag operation continues, the window will receive
//!   [`DragPosition`](crate::event::WindowEvent::DragPosition) events.
//! - At any point during this operation, the receiving application may request either the available
//!   types or even the data being transferred. This may be useful in cases where the application
//!   wants to preload the data. For example, an image editor may want to display the image on the
//!   canvas during the drag operation.
//! - When the user tries to drop the data onto the window, that window will receive either a
//!   [`DragDropped`](crate::event::WindowEvent::DragDropped) or
//!   [`DragLeft`](crate::event::WindowEvent::DragLeft) event if the drag operation was accepted or
//!   rejected, respectively. See documentation for
//!   [`set_valid_dnd_actions`](crate::event_loop::ActiveEventLoop::set_valid_dnd_actions) for
//!   details on accepting/rejecting a drag.
//!
//! ## Sending a drag-and-drop operation
//!
//! As the source application cannot interact with the ongoing drag while it is in-flight, this flow
//! is a lot simpler.
//!
//! - The application creates a [`DataTransferSend`] with a set of types and associated data. For
//!   most cases, this can be done with [`DataTransferSendBuilder`].
//! - The application passes this [`DataTransferSend`] to
//!   [`ActiveEventLoop::start_drag`](crate::event_loop::ActiveEventLoop::start_drag)`. This is also
//!   where metadata is set, such as the icon that will be shown during the drag operation.
//! - When the drag operation completes, the application receives
//!   [`OutgoingDragDropped`](crate::event::WindowEvent::OutgoingDragDropped) with the resultant
//!   action, or [`OutgoingDragCanceled`](crate::event::WindowEvent::OutgoingDragCanceled), and
//!   handles it appropriately. For example, if the drag was successful and the operation is
//!   [`DndAction::Move`](crate::event_loop::DndAction::Move), then the application would delete the
//!   source object, since the data has now been transferred somewhere else.

#![warn(missing_docs)]

use std::ops::ControlFlow;
use std::path::{Path, PathBuf};
use std::{fmt, io};

use crate::as_any::AsAny;

/// Unique identifier for a data transfer.
#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct DataTransferId(i64);

impl DataTransferId {
    /// Convert the [`DataTransferId`] into the underlying integer.
    ///
    /// This is useful if you need to pass the ID across an FFI boundary, or store it in an atomic.
    pub const fn into_raw(self) -> i64 {
        self.0
    }

    /// Construct a [`DataTransferId`] from the underlying integer.
    ///
    /// This should only be called with integers returned from [`DataTransferId::into_raw`].
    pub const fn from_raw(id: i64) -> Self {
        Self(id)
    }
}

/// The set of types supported cross-platform.
#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash)]
pub enum TypeHint {
    /// Plain UTF-8 text (see [`TypedData::try_as_string`]).
    ///
    /// **Note for platform implementations**: this hint is _only_ for UTF-8 text. If the platform
    /// returns plaintext in some format other than UTF-8 by default, a [`TypedData`]
    /// implementation marked with this type hint should convert to UTF-8.
    Plaintext,
    /// A list of URIs in the format defined by the `text/uri-list` MIME type, encoded as UTF-8 (see
    /// [`TypedData::try_as_uris`]).
    ///
    /// **Note for platform implementations**: this hint is _only_ for URIs encoded precisely in the
    /// format specified above. If the platform uses a different format, a [`TypedData`]
    /// implementation marked with this type hint should convert to that format.
    UriList,
    /// A HTML-formatted string
    Html,
    /// An RTF-formatted string
    Rtf,
    /// Audio
    Audio {
        /// An optional hint for the encoding of the supplied bytes, specified using the standard
        /// file extension for that audio format, lowercase and without the leading `.`.
        extension_hint: Option<&'static str>,
    },
    /// Image data
    Image {
        /// An optional hint for the encoding of the supplied bytes, specified using the standard
        /// file extension for that image format, lowercase and without the leading `.`.
        extension_hint: Option<&'static str>,
    },
}

impl TypeHint {
    /// Check whether the two type hints "match".
    ///
    /// This is subtly different to direct equality. If one of the types is an image or audio with a
    /// `None` extension hint, then the other type just needs to match variant (i.e. image/audio),
    /// the extension does not also have to be `None`.
    pub fn matches(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::Plaintext, Self::Plaintext)
            | (Self::UriList, Self::UriList)
            | (Self::Html, Self::Html)
            | (Self::Rtf, Self::Rtf) => true,

            (
                Self::Audio { extension_hint: this_ext },
                Self::Audio { extension_hint: other_ext },
            )
            | (
                Self::Image { extension_hint: this_ext },
                Self::Image { extension_hint: other_ext },
            ) => match (this_ext, other_ext) {
                (Some(this_ext), Some(other_ext)) => this_ext == other_ext,
                (None, _) | (_, None) => true,
            },

            _ => false,
        }
    }
}

/// The type of a data transfer.
///
/// [`hint`](TransferType::hint) can be called to get the type in
/// a cross-platform format (see [`TypeHint`])
pub trait TransferType: AsAny + fmt::Debug {
    /// Get the cross-platform representation of this type.
    ///
    /// If this returns `None`, then this is a platform-dependent type that has no cross-platform
    /// equivalent.
    fn hint(&self) -> Option<TypeHint>;

    /// Check whether two dynamically-typed transfer types are equivalent.
    // Can't use a `PartialEq` bound because it causes a dependency cycle.
    fn matches(&self, other: &dyn TransferType) -> bool;
}

impl TransferType for TypeHint {
    fn hint(&self) -> Option<TypeHint> {
        Some(*self)
    }

    fn matches(&self, other: &dyn TransferType) -> bool {
        other.hint().is_some_and(|hint| self.matches(&hint))
    }
}

impl_dyn_casting!(TransferType);

// Replicates the cfg for `url::Url::parse`
#[cfg(any(unix, windows, target_os = "redox", target_os = "wasi", target_os = "hermit"))]
fn default_try_as_file_paths<T: TypedData + ?Sized>(data: &T) -> io::Result<Vec<PathBuf>> {
    data.try_as_uris().and_then(|uris| {
        uris.into_iter()
            .map(|uri_string| {
                Ok(url::Url::parse(&uri_string)
                    .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?
                    .to_file_path()
                    .map_err(|()| io::ErrorKind::InvalidData)?)
            })
            .collect()
    })
}

// Replicates the cfg for `url::Url::parse`
//
// It doesn't matter that this is unimplemented on the web, as we don't currently support
// drag-and-drop for web targets and the web platform can't directly access paths anyway.
#[cfg(not(any(unix, windows, target_os = "redox", target_os = "wasi", target_os = "hermit")))]
fn default_try_as_file_paths<T: TypedData + ?Sized>(data: &T) -> io::Result<Vec<PathBuf>> {
    Err(io::ErrorKind::Unsupported)
}

/// Data that has been fetched from a data transfer
///
/// ### Blocking
///
/// Note that this type provides a blocking interface. In cases where reading this type directly on
/// the event loop would cause a deadlock, the backend will make a best-effort attempt to return an
/// error with [`io::ErrorKind::Deadlock`]. For now, the only way to access the data is via blocking
/// on the event loop, so simply retrying the next time an event is received that references the
/// data transfer should be enough to ensure that the data is accessible.
pub trait TypedData: AsAny + fmt::Debug + Send + Sync {
    /// The type of this `TypedData`.
    fn type_(&self) -> &dyn TransferType;

    /// If this value is readable as bytes, return a reader than can be used to read those bytes.
    ///
    /// On some platforms, the reader must be driven incrementally upon each
    /// [`WindowEvent::DataTransferReceived`](crate::event::WindowEvent::DataTransferReceived)`. If
    /// you don't need to stream the data and just want the bytes in a single buffer, use
    /// [`TypedData::try_as_bytes`].
    fn try_read(&self) -> Option<Box<dyn io::BufRead>>;

    /// If this value is readable as bytes, return those bytes.
    ///
    /// If this returns [`WouldBlock`](std::io::ErrorKind::WouldBlock), then it should be called
    /// again upon next receiving
    /// [`WindowEvent::DataTransferReceived`](crate::event::WindowEvent::DataTransferReceived)
    fn try_as_bytes(&self) -> io::Result<Vec<u8>> {
        let mut reader = self
            .try_read()
            .ok_or_else(|| io::Error::other("This `TypedData` is not readable as bytes"))?;

        let mut out = Vec::new();

        reader.read_to_end(&mut out)?;

        Ok(out)
    }

    /// Read this value as a list of URIs.
    ///
    /// If this value is not readable as URIs, return an error.
    ///
    /// The returned `String`s should be interpreted as URIs conforming to [RFC 3986](https://www.rfc-editor.org/info/rfc3986/).
    ///
    /// If this returns [`WouldBlock`](std::io::ErrorKind::WouldBlock), then it should be called
    /// again upon next receiving
    /// [`WindowEvent::DataTransferReceived`](crate::event::WindowEvent::DataTransferReceived)
    fn try_as_uris(&self) -> io::Result<Vec<String>>;

    /// Read this value as a list of paths.
    ///
    /// This is provided as a convenience method to avoid the need for the user to manually parse
    /// the result of [`try_as_uris`](TypedData::try_as_uris). `try_as_uris` should be preferred
    /// when the extra complexity is acceptable, as it is more generic.
    ///
    /// If this value is not readable as URIs, return an error.
    ///
    /// If this returns [`WouldBlock`](std::io::ErrorKind::WouldBlock), then it should be called
    /// again upon next receiving
    /// [`WindowEvent::DataTransferReceived`](crate::event::WindowEvent::DataTransferReceived)
    fn try_as_file_paths(&self) -> io::Result<Vec<PathBuf>> {
        default_try_as_file_paths(self)
    }

    /// Read this value as a plain text string.
    ///
    /// If this value is not readable as a string, return an error.
    ///
    /// If this returns [`WouldBlock`](std::io::ErrorKind::WouldBlock), then it should be called
    /// again upon next receiving
    /// [`WindowEvent::DataTransferReceived`](crate::event::WindowEvent::DataTransferReceived)
    fn try_as_string(&self) -> io::Result<String>;
}

// Required for `WindowEvent` to implement `PartialEq` - we just implement this on a best-effort
// basis.
impl PartialEq for dyn TypedData {
    fn eq(&self, other: &Self) -> bool {
        std::ptr::addr_eq(self, other)
    }
}

impl_dyn_casting!(TypedData);

/// Metadata about a data transfer. This does not allow actually receiving data, as that is an
/// asynchronous operation. To fetch the data from the source application, see
/// [`ActiveEventLoop::fetch_data_transfer`](crate::event_loop::ActiveEventLoop::fetch_data_transfer).
pub trait DataTransfer: AsAny + fmt::Debug {
    /// Iterate over each type advertized by this `DataTransfer`. This is just a minor optimization,
    /// in most cases you should probably use [`has_type`](DataTransfer::has_type) or
    /// [`available_types`](DataTransfer::available_types).
    fn for_each_available_type<'this>(
        &'this self,
        func: &'_ mut dyn FnMut(&'this dyn TransferType) -> ControlFlow<()>,
    );

    /// Display the list of all available types.
    ///
    /// This is useful if more-complex type matching is required, but for most cases
    /// [`has_type`](DataTransfer::has_type) should be used.
    fn available_types(&self) -> Vec<&'_ dyn TransferType> {
        let mut out = Vec::new();

        self.for_each_available_type(&mut |ty| {
            out.push(ty);
            ControlFlow::Continue(())
        });

        out
    }

    /// Check if the supplied type is provided by this [`DataTransfer`].
    ///
    /// Supplying a [`TypeHint`] as the type is supported on all platforms, but if some
    /// platform-specific type is required then that platform's implementation of `TransferType` can
    /// be used.
    fn has_type(&self, type_: &dyn TransferType) -> bool {
        let mut found = false;
        self.for_each_available_type(&mut |haystack| {
            if haystack.matches(type_) {
                found = true;
                ControlFlow::Break(())
            } else {
                ControlFlow::Continue(())
            }
        });

        found
    }
}

impl_dyn_casting!(DataTransfer);

/// Kinds of data that can be sent via a `DataTransfer`.
///
/// Some kinds of data cannot be represented by just a binary blob in a cross-platform way.
/// File URIs on Windows and macOS are represented as arrays of strings, and strings have
/// different encoding on different platforms. To allow this to be represented, we allow
/// supplying strings and URIs separately from binary blobs.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum SendData {
    /// List of URIs.
    ///
    /// These should conform to [RFC 3986](https://www.rfc-editor.org/info/rfc3986/).
    /// If you just want to send file paths, see [`SendData::from_file_paths`].
    ///
    /// Note that `SendData` implements `From<String>` and `From<Vec<u8>>`, but _not_
    /// `From<Vec<String>>`, as it is not necessarily obvious to a reader that `Vec<String>`
    /// will be interpreted as a URI list. However, it _does_ implement [`From<Url>`](url::Url),
    /// if you are using the [`url`](https://docs.rs/url/2) crate.
    Uris(Vec<String>),
    /// String
    ///
    /// This can also be constructed with the [`From<String>`](std::string::String) implementation.
    String(String),
    /// Binary blob
    ///
    /// This can also be constructed with the [`From<Vec<u8>>`](std::vec::Vec) implementation.
    Bytes(Vec<u8>),
}

impl SendData {
    /// Create [`SendData::Uris`] from an iterator of [`Path`]s.
    ///
    /// All paths must be absolute, and on Windows must include either a drive prefix (e.g. `C:\`)
    /// or a UNC prefix (`\\`). See documentation for [`url::Url::from_file_path`].
    pub fn from_file_paths<I>(paths: I) -> Option<Self>
    where
        I: IntoIterator,
        I::Item: AsRef<Path>,
    {
        // Replicates the cfg for `url::Url::from_file_path`
        #[cfg(any(unix, windows, target_os = "redox", target_os = "wasi", target_os = "hermit"))]
        fn from_file_paths_impl<I>(paths: I) -> Option<SendData>
        where
            I: IntoIterator,
            I::Item: AsRef<Path>,
        {
            paths
                .into_iter()
                .map(url::Url::from_file_path)
                .map(|result| result.map(String::from))
                .collect::<Result<Vec<_>, ()>>()
                .map(SendData::Uris)
                .ok()
        }

        // Replicates the cfg for `url::Url::from_file_path`
        //
        // It doesn't matter that this is unimplemented on the web, as we don't currently support
        // drag-and-drop for web targets and the web platform can't directly access paths
        // anyway.
        #[cfg(not(any(
            unix,
            windows,
            target_os = "redox",
            target_os = "wasi",
            target_os = "hermit"
        )))]
        fn from_file_paths_impl<I>(paths: I) -> Option<SendData> {
            None
        }

        from_file_paths_impl(paths)
    }
}

// We monomorphize these `From` implementations instead of making them generic, in order to
// prevent accidentally casting to the wrong type.
impl From<String> for SendData {
    fn from(value: String) -> Self {
        Self::String(value)
    }
}

impl From<Vec<u8>> for SendData {
    fn from(value: Vec<u8>) -> Self {
        Self::Bytes(value)
    }
}

impl From<Vec<url::Url>> for SendData {
    fn from(value: Vec<url::Url>) -> Self {
        Self::Uris(value.into_iter().map(Into::into).collect())
    }
}

/// Trait for sending data via a data transfer.
///
/// See [`ActiveEventLoop::start_drag`](crate::event_loop::ActiveEventLoop::start_drag) for where
/// this is used. To build an implementation of this trait dynamically in a cross-platform way, use
/// [`DataTransferSendBuilder`].
pub trait DataTransferSend: DataTransfer + Send {
    /// Get the data for the specified type, or `None` if this value does not supply the given data
    /// type.
    fn data_for_type(&self, type_: &dyn TransferType) -> Option<SendData>;
}

impl_dyn_casting!(DataTransferSend);

type SendDataCallback<T> = Box<dyn Fn(&T, &dyn TransferType) -> Option<SendData> + Send>;

/// Dynamic builder for an implementation of [`DataTransferSend`].
///
/// On all platforms, inter-application data transfer (i.e. clipboard and drag-and-drop) works like
/// so:
///
/// - The source advertises a set of types that it can transfer.
/// - The destination picks one or more of those types to receive.
/// - The source sends the data for that type.
///
/// This type abstracts that in a way that allows data to be sent cross-platform. `T` is an optional
/// state value, which allows the user to have a single source of truth for their data, converting
/// it lazily to the requested type.
pub struct DataTransferSendBuilder<T> {
    state: T,
    types: Vec<(Box<dyn TransferType + Send>, SendDataCallback<T>)>,
}

impl<T> fmt::Debug for DataTransferSendBuilder<T>
where
    T: fmt::Debug,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("NewDataTransferBuilder").field("state", &self.state).finish_non_exhaustive()
    }
}

impl<T> DataTransfer for DataTransferSendBuilder<T>
where
    T: fmt::Debug + Send + 'static,
{
    fn for_each_available_type<'this>(
        &'this self,
        func: &'_ mut dyn FnMut(&'this dyn TransferType) -> ControlFlow<()>,
    ) {
        let _ = self.types.iter().try_for_each(|(ty, _)| func(&**ty));
    }
}

impl<T> DataTransferSend for DataTransferSendBuilder<T>
where
    T: fmt::Debug + Send + 'static,
{
    fn data_for_type(&self, type_: &dyn TransferType) -> Option<SendData> {
        self.data_for_type(type_)
    }
}

impl<T> DataTransferSendBuilder<T> {
    /// Create a new [`DataTransferSendBuilder`], with a state value which acts as
    /// the single source of truth for the underlying data.
    pub fn new(state: T) -> Self {
        Self { state, types: vec![] }
    }
}

impl<T> DataTransferSendBuilder<T> {
    fn data_for_type(&self, type_: &dyn TransferType) -> Option<SendData> {
        let (_, func) = self.types.iter().find(|(ty, _)| ty.matches(type_))?;

        func(&self.state, type_)
    }

    /// Add a callback which converts the builder's state to the given type. In
    /// most cases, `type_` will be [`TypeHint`].
    pub fn add_type<Ty, F, O>(&mut self, type_: Ty, func: F) -> &mut Self
    where
        Ty: TransferType + Send,
        F: Fn(&T, &dyn TransferType) -> Option<O> + Send + 'static,
        O: Into<SendData>,
    {
        self.types
            .push((Box::new(type_), Box::new(move |state, ty| func(state, ty).map(Into::into))));
        self
    }

    /// Return a new builder, adding a callback which converts the builder's state
    /// to the given type.
    ///
    /// For cross-platform use, `type_` will be [`TypeHint`]. The closure additionally receives
    /// a [`TransferType`], which is not necessarily the same as `type_` for the following reasons:
    ///
    /// - The OS may have multiple types which are equivalent to the supplied type
    /// - `TypeHint::Audio` and `TypeHint::Image` with `extension_hint: None` will advertise all
    ///   supported audio and image formats, in which case the closure may receive a type with an
    ///   extension chosen by the receiving application.
    pub fn with_type<Ty, F, O>(mut self, type_: Ty, func: F) -> Self
    where
        Ty: TransferType + Send,
        F: Fn(&T, &dyn TransferType) -> Option<O> + Send + 'static,
        O: Into<SendData>,
    {
        self.add_type(type_, func);
        self
    }
}

impl<T> DataTransferSendBuilder<T>
where
    T: fmt::Debug + Send + 'static,
{
    /// Consume the builder, returning an implementation of [`DataTransferSend`].
    ///
    /// Note that this is only provided for explicitness and ergonomics. [`DataTransferSendBuilder`]
    /// implements [`DataTransferSend`] and this method is equivalent to [`Box::new`].
    pub fn build(self) -> Box<dyn DataTransferSend> {
        Box::new(self)
    }
}
