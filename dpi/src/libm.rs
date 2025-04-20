// Copyright (c) 2018 Jorge Aparicio
// Copyright © 2005-2020 Rich Felker, et al.
// Copyright © 1993,2004 Sun Microsystems or
// Copyright © 2003-2011 David Schultz or
// Copyright © 2003-2009 Steven G. Kargl or
// Copyright © 2003-2009 Bruce D. Evans or
// Copyright © 2008 Stephen L. Moshier or
// Copyright © 2017-2018 Arm Limited
// SPDX-License-Identifier: MIT
// This file is licensed solely under the terms discussed in LICENSE-LIBM-MIT at the crate root.
// See the package-level README for full details.

// Taken from https://github.com/rust-lang/libm/blob/master/src/math/mod.rs#L1
macro_rules! force_eval {
    ($e:expr) => {
        unsafe { ::core::ptr::read_volatile(&$e) }
    };
}

// Taken from https://github.com/rust-lang/libm/blob/libm-v0.2.11/src/math/round.rs
pub(crate) fn round(x: f64) -> f64 {
    trunc(x + copysign(0.5 - 0.25 * f64::EPSILON, x))
}

// Adapted from: https://github.com/rust-lang/libm/blob/libm-v0.2.11/src/math/trunc.rs#L8-L12
#[allow(clippy::needless_late_init /*, reason = "The original libm code uses this style" */)]
fn trunc(x: f64) -> f64 {
    let x1p120 = f64::from_bits(0x4770000000000000); // 0x1p120f === 2 ^ 120

    let mut i: u64 = x.to_bits();
    let mut e: i64 = ((i >> 52) & 0x7ff) as i64 - 0x3ff + 12;
    let m: u64;

    if e >= 52 + 12 {
        return x;
    }
    if e < 12 {
        e = 1;
    }
    m = -1i64 as u64 >> e;
    if (i & m) == 0 {
        return x;
    }
    force_eval!(x + x1p120);
    i &= !m;
    f64::from_bits(i)
}

// Taken from https://github.com/rust-lang/libm/blob/libm-v0.2.11/src/math/copysign.rs
fn copysign(x: f64, y: f64) -> f64 {
    let mut ux = x.to_bits();
    let uy = y.to_bits();
    ux &= (!0) >> 1;
    ux |= uy & (1 << 63);
    f64::from_bits(ux)
}
