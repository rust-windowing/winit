#![allow(clippy::assertions_on_constants)]

use winit_core::icon::RgbaIcon;

use super::*;

pub(crate) const PIXEL_SIZE: usize = mem::size_of::<Pixel>();

#[repr(C)]
#[derive(Debug)]
pub(crate) struct Pixel {
    pub(crate) r: u8,
    pub(crate) g: u8,
    pub(crate) b: u8,
    pub(crate) a: u8,
}

impl Pixel {
    pub fn to_packed_argb(&self) -> Cardinal {
        let mut cardinal = 0;
        assert!(CARDINAL_SIZE >= PIXEL_SIZE);
        let as_bytes = &mut cardinal as *mut _ as *mut u8;
        unsafe {
            *as_bytes.offset(0) = self.b;
            *as_bytes.offset(1) = self.g;
            *as_bytes.offset(2) = self.r;
            *as_bytes.offset(3) = self.a;
        }
        cardinal
    }
}

pub(crate) fn rgba_to_cardinals(icon: &RgbaIcon) -> Vec<Cardinal> {
    assert_eq!(icon.buffer().len() % PIXEL_SIZE, 0);
    let pixel_count = icon.buffer().len() / PIXEL_SIZE;
    assert_eq!(pixel_count, (icon.width() * icon.height()) as usize);
    let mut data = Vec::with_capacity(pixel_count);
    data.push(icon.width() as Cardinal);
    data.push(icon.height() as Cardinal);
    let pixels = icon.buffer().as_ptr() as *const Pixel;
    for pixel_index in 0..pixel_count {
        let pixel = unsafe { &*pixels.add(pixel_index) };
        data.push(pixel.to_packed_argb());
    }
    data
}
