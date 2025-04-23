# DPI

Full docs can be found on docs.rs.

## License

Most of DPI is licensed under the Apache License, Version 2.0 ([LICENSE](LICENSE)).
All files except for `src/libm.rs` (and `LICENSE-LIBM-MIT`) are available solely under that license.

For its `no_std` support, DPI uses code from the [libm](https://crates.io/crates/libm) crate.
This is in the `libm.rs` file, and is licensed solely under the MIT Licence ([LICENSE-LIBM-MIT](LICENSE-LIBM-MIT)).
That file contains details of all potentially applicable copyright notices.
This is feature gated to only be included if you disable the `std` feature, otherwise it will not be compiled into your final binary
(and so these license terms will not apply).

Overall, this means that the license for this crate depends on what features you have enabled.
If you enable the `std` feature, then DPI uses only code available under the Apache-2.0 license, and so can be used under the terms of that license.
However, if you disable the `std` feature, then both these licenses must be followed to use the crate as a whole.
