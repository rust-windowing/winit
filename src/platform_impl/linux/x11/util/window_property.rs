use super::*;

use std::sync::Arc;
use x11rb::errors::{ConnectionError, ReplyError};
use x11rb::protocol::xproto::{self, ConnectionExt as _};

pub type Cardinal = c_long;
pub const CARDINAL_SIZE: usize = mem::size_of::<c_long>();

#[derive(Debug, Clone)]
pub(crate) enum GetPropertyError {
    XError(Arc<PlatformError>),
    TypeMismatch(xproto::Atom),
    FormatMismatch(u8),
}

impl From<PlatformError> for GetPropertyError {
    fn from(value: PlatformError) -> Self {
        GetPropertyError::XError(Arc::new(value))
    }
}

impl From<ConnectionError> for GetPropertyError {
    fn from(value: ConnectionError) -> Self {
        PlatformError::from(value).into()
    }
}

impl From<ReplyError> for GetPropertyError {
    fn from(value: ReplyError) -> Self {
        PlatformError::from(value).into()
    }
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

// Number of 32-bit chunks to retrieve per iteration of get_property's inner loop.
// To test if `get_property` works correctly, set this to 1.
const PROPERTY_BUFFER_SIZE: u32 = 1024; // 4k of RAM ought to be enough for anyone!

impl XConnection {
    pub fn get_property<T: bytemuck::Pod>(
        &self,
        window: xproto::Window,
        property: xproto::Atom,
        property_type: xproto::Atom,
    ) -> Result<Vec<T>, GetPropertyError> {
        let mut data = Vec::new();
        let mut offset = 0;

        loop {
            // Fetch the next chunk of data.
            let property_reply = self
                .connection
                .get_property(
                    false,
                    window,
                    property,
                    property_type,
                    offset,
                    PROPERTY_BUFFER_SIZE,
                )?
                .reply()?;

            // Ensure that the property type matches.
            if property_reply.type_ != property_type {
                return Err(GetPropertyError::TypeMismatch(property_reply.type_));
            }

            // Ensure that the format is right.
            if property_reply.format as usize != mem::size_of::<T>() * 8 {
                return Err(GetPropertyError::FormatMismatch(property_reply.format));
            }

            // Append the data to the output.
            let bytes_after = property_reply.bytes_after;
            append_byte_vector(&mut data, property_reply.value);

            // If there is no more data, we're done.
            if bytes_after == 0 {
                return Ok(data);
            }

            // Add to the offset and go again.
            offset += PROPERTY_BUFFER_SIZE;
        }
    }

    pub fn change_property<'a, T: bytemuck::NoUninit>(
        &'a self,
        window: xproto::Window,
        property: xproto::Atom,
        property_type: xproto::Atom,
        mode: xproto::PropMode,
        new_value: &[T],
    ) -> Result<XcbVoidCookie<'a>, PlatformError> {
        // Preform the property change.
        let cookie = self.connection.change_property(
            mode,
            window,
            property,
            property_type,
            (mem::size_of::<T>() * 8) as u8,
            new_value.len() as _,
            bytemuck::cast_slice::<T, u8>(new_value),
        )?;

        Ok(cookie)
    }
}

/// Append a byte vector to a vector of real elements.
fn append_byte_vector<T: bytemuck::Pod>(real: &mut Vec<T>, bytes: Vec<u8>) {
    // If the type is equivalent to a byte, this cast will succeed no matter what.
    if mem::size_of::<T>() == 1 && mem::align_of::<T>() == 1 {
        let mut bytes_casted = bytemuck::allocation::cast_vec::<u8, T>(bytes);
        real.append(&mut bytes_casted);
        return;
    }

    // Add enough buffer space to hold the new data.
    debug_assert!(
        bytes.len() % mem::size_of::<T>() == 0,
        "Byte vector is not a multiple of the element size"
    );
    let additional_space = bytes.len() / mem::size_of::<T>();
    let new_len = real.len() + additional_space;
    let former_len = real.len();
    real.resize(new_len, T::zeroed());

    // Get a handle to the new space in the vector.
    let new_space = &mut real[former_len..];

    // Copy the data into the new space.
    let new_bytes = bytemuck::cast_slice_mut::<T, u8>(new_space);
    new_bytes.copy_from_slice(&bytes);
}
