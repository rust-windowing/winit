use std::ops::Mul;

use dpi::{LogicalSize, PhysicalSize};

/// A wp-fractional-scale scale.
///
/// This type implements the `physical_size = round_half_up(logical_size * scale)`
/// operation with infinite precision as required by the protocol.
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub struct Scale {
    scale: u64,
}

const BASE: u32 = 120;
const BASE_U64: u64 = BASE as u64;
const BASE_F64: f64 = BASE as f64;

impl Scale {
    pub fn from_wp_fractional_scale(v: u32) -> Self {
        assert!(v > 0);
        Self { scale: v as u64 }
    }

    pub fn from_integer_scale(v: u32) -> Self {
        Self::from_wp_fractional_scale(v.saturating_mul(BASE))
    }

    pub fn to_f64(self) -> f64 {
        self.scale as f64 / BASE_F64
    }

    pub fn round_up(self) -> u32 {
        ((self.scale + BASE_U64 - 1) / BASE_U64) as u32
    }

    fn surface_to_buffer<const N: usize>(self, sizes: [u32; N]) -> [u32; N] {
        sizes.map(|surface| {
            // buffer = floor((surface * scale + 60) / 120)
            let buffer = (surface as u64 * self.scale + BASE_U64 / 2) / BASE_U64;
            buffer.min(u32::MAX as u64) as u32
        })
    }
}

impl Mul<Scale> for LogicalSize<u32> {
    type Output = PhysicalSize<u32>;

    fn mul(self, scale: Scale) -> Self::Output {
        let [width, height] = scale.surface_to_buffer([self.width, self.height]);
        PhysicalSize { width, height }
    }
}
