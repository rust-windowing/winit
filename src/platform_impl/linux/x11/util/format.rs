use std::{fmt::Debug, mem, os::raw::*};

// This isn't actually the number of the bits in the format.
// X11 does a match on this value to determine which type to call sizeof on.
// Thus, we use 32 for c_long, since 32 maps to c_long which maps to 64.
// ...if that sounds confusing, then you know why this enum is here.
#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum Format {
    Char = 8,
    Short = 16,
    Long = 32,
}

impl Format {
    pub fn from_format(format: usize) -> Option<Self> {
        match format {
            8 => Some(Format::Char),
            16 => Some(Format::Short),
            32 => Some(Format::Long),
            _ => None,
        }
    }

    pub fn get_actual_size(&self) -> usize {
        match self {
            Format::Char => mem::size_of::<c_char>(),
            Format::Short => mem::size_of::<c_short>(),
            Format::Long => mem::size_of::<c_long>(),
        }
    }
}

pub trait Formattable: Debug + Clone + Copy + PartialEq + PartialOrd {
    const FORMAT: Format;
}

// You might be surprised by the absence of c_int, but not as surprised as X11 would be by the presence of it.
impl Formattable for c_schar {
    const FORMAT: Format = Format::Char;
}
impl Formattable for c_uchar {
    const FORMAT: Format = Format::Char;
}
impl Formattable for c_short {
    const FORMAT: Format = Format::Short;
}
impl Formattable for c_ushort {
    const FORMAT: Format = Format::Short;
}
impl Formattable for c_long {
    const FORMAT: Format = Format::Long;
}
impl Formattable for c_ulong {
    const FORMAT: Format = Format::Long;
}
