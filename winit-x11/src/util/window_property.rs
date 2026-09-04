use std::error::Error;
use std::fmt;
use std::sync::Arc;

use bytemuck::{NoUninit, Pod};
use x11rb::connection::Connection;
use x11rb::errors::ReplyError;

use super::*;

pub const CARDINAL_SIZE: usize = mem::size_of::<u32>();

pub type Cardinal = u32;

#[derive(Debug, Clone)]
pub enum GetPropertyError {
    X11rbError(Arc<ReplyError>),
    TypeMismatch(xproto::Atom),
    FormatMismatch(c_int),
    Unknown,
}

impl GetPropertyError {
    pub fn is_actual_property_type(&self, t: xproto::Atom) -> bool {
        if let GetPropertyError::TypeMismatch(actual_type) = *self {
            actual_type == t
        } else {
            false
        }
    }
}

impl<T: Into<ReplyError>> From<T> for GetPropertyError {
    fn from(e: T) -> Self {
        Self::X11rbError(Arc::new(e.into()))
    }
}

impl fmt::Display for GetPropertyError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            GetPropertyError::X11rbError(err) => err.fmt(f),
            GetPropertyError::TypeMismatch(err) => write!(f, "type mismatch: {err}"),
            GetPropertyError::FormatMismatch(err) => write!(f, "format mismatch: {err}"),
            GetPropertyError::Unknown => write!(f, "internal error"),
        }
    }
}

impl Error for GetPropertyError {}

// Number of 32-bit chunks to retrieve per iteration of get_property's inner loop.
// To test if `get_property` works correctly, set this to 1.
const PROPERTY_BUFFER_SIZE: u32 = 1024; // 4k of RAM ought to be enough for anyone!

impl XConnection {
    pub fn get_property<T: Pod>(
        &self,
        window: xproto::Window,
        property: xproto::Atom,
        property_type: xproto::Atom,
    ) -> Result<Vec<T>, GetPropertyError> {
        let mut iter = PropIterator::new(self.xcb_connection(), window, property, property_type);
        let mut data = vec![];

        loop {
            if !iter.next_window(&mut data)? {
                break;
            }
        }

        Ok(data)
    }

    pub fn change_property<'a, T: NoUninit>(
        &'a self,
        window: xproto::Window,
        property: xproto::Atom,
        property_type: xproto::Atom,
        mode: xproto::PropMode,
        new_value: &[T],
    ) -> Result<VoidCookie<'a>, X11Error> {
        assert!([1usize, 2, 4].contains(&mem::size_of::<T>()));
        self.xcb_connection()
            .change_property(
                mode,
                window,
                property,
                property_type,
                (mem::size_of::<T>() * 8) as u8,
                new_value.len().try_into().expect("too many items for property"),
                bytemuck::cast_slice::<T, u8>(new_value),
            )
            .map_err(Into::into)
    }

    /// Reads a property, appending to the `result` buffer and returning the actual type and read
    /// bytes. Note that the `XConnection::get_property` doesn't work as we need a dynamic type.
    pub fn get_dynamic_property(
        &self,
        property: xproto::Atom,
        xwindow: xproto::Window,
        result: &mut Vec<u8>,
    ) -> Result<(xproto::Atom, usize), GetPropertyError> {
        let mut resolved_format = None;
        let mut resolved_type = None;
        // Max data transferred in bytes (must be multiple of 4 for unknown reasons)
        let mut bytes_read = 0;
        loop {
            let reply = self
                .xcb_connection()
                .get_property(
                    true,
                    xwindow,
                    property,
                    xproto::AtomEnum::ANY,
                    bytes_read as u32 / 4, // Offset in 4 bytes to start of property
                    PROPERTY_BUFFER_SIZE / 4, // Number of 4 bytes to get
                )?
                .reply()?;

            // Format and type must not change
            if resolved_format.is_some_and(|resolved| resolved != reply.format) {
                return Err(GetPropertyError::FormatMismatch(reply.format.into()));
            }
            if resolved_type.is_some_and(|resolved| resolved != reply.type_) {
                return Err(GetPropertyError::TypeMismatch(reply.type_));
            }
            resolved_format = Some(reply.format);
            resolved_type = Some(reply.type_);

            // Add the data
            result.extend_from_slice(&reply.value);
            bytes_read += reply.value.len();

            // Check if we have reached the end
            if reply.bytes_after == 0 {
                trace!(
                    "Read {} into property {} data {}",
                    self.atom_str(reply.type_),
                    self.atom_str(property),
                    if reply.value.len() < 100 {
                        format!("{:?}", reply.value)
                    } else {
                        "[... many bytes]".to_string()
                    }
                );
                return Ok((reply.type_, bytes_read));
            }
        }
    }
}

/// An iterator over the "windows" of the property that we are fetching.
struct PropIterator<'a, C: ?Sized, T> {
    /// Handle to the connection.
    conn: &'a C,

    /// The window that we're fetching the property from.
    window: xproto::Window,

    /// The property that we're fetching.
    property: xproto::Atom,

    /// The type of the property that we're fetching.
    property_type: xproto::Atom,

    /// The offset of the next window, in 32-bit chunks.
    offset: u32,

    /// The format of the type.
    format: u8,

    /// Keep a reference to `T`.
    _phantom: std::marker::PhantomData<T>,
}

impl<'a, C: Connection + ?Sized, T: Pod> PropIterator<'a, C, T> {
    /// Create a new property iterator.
    fn new(
        conn: &'a C,
        window: xproto::Window,
        property: xproto::Atom,
        property_type: xproto::Atom,
    ) -> Self {
        let format = match mem::size_of::<T>() {
            1 => 8,
            2 => 16,
            4 => 32,
            _ => unreachable!(),
        };

        Self {
            conn,
            window,
            property,
            property_type,
            offset: 0,
            format,
            _phantom: Default::default(),
        }
    }

    /// Get the next window and append it to `data`.
    ///
    /// Returns whether there are more windows to fetch.
    fn next_window(&mut self, data: &mut Vec<T>) -> Result<bool, GetPropertyError> {
        // Send the request and wait for the reply.
        let reply = self
            .conn
            .get_property(
                false,
                self.window,
                self.property,
                self.property_type,
                self.offset,
                PROPERTY_BUFFER_SIZE,
            )?
            .reply()?;

        // Make sure that the reply is of the correct type.
        if reply.type_ != self.property_type {
            return Err(GetPropertyError::TypeMismatch(reply.type_));
        }

        // Make sure that the reply is of the correct format.
        if reply.format != self.format {
            return Err(GetPropertyError::FormatMismatch(reply.format.into()));
        }

        // Append the data to the output.
        if mem::size_of::<T>() == 1 && mem::align_of::<T>() == 1 {
            // We can just do a bytewise append.
            data.extend_from_slice(bytemuck::cast_slice(&reply.value));
        } else {
            // Rust's borrowing and types system makes this a bit tricky.
            //
            // We need to make sure that the data is properly aligned. Unfortunately the best
            // safe way to do this is to copy the data to another buffer and then append.
            //
            // TODO(notgull): It may be worth it to use `unsafe` to copy directly from
            // `reply.value` to `data`; check if this is faster. Use benchmarks!
            let old_len = data.len();
            let added_len = reply.value.len() / mem::size_of::<T>();
            data.resize(old_len + added_len, T::zeroed());
            bytemuck::cast_slice_mut::<T, u8>(&mut data[old_len..]).copy_from_slice(&reply.value);
        }

        // Check `bytes_after` to see if there are more windows to fetch.
        self.offset += PROPERTY_BUFFER_SIZE;
        Ok(reply.bytes_after != 0)
    }
}
