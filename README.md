[![License:Zlib](https://img.shields.io/badge/License-Zlib-brightgreen.svg)](https://opensource.org/licenses/Zlib)
[![crates.io](https://img.shields.io/crates/v/imagine.svg)](https://crates.io/crates/imagine)
[![docs.rs](https://docs.rs/imagine/badge.svg)](https://docs.rs/imagine/)

# imagine

## Goals

* To provide image format **decoders** for various image formats:
  * Decoders should be *possible* to use without `imagine` doing any allocation
    (the user provides any necessary buffers).
  * Decoders should be *simple* to use when `imagine` is allowed to allocate for
    you. Functions to "just get me the RGBA8 pixels", and things like that.
* To provide image format **encoders** for various image formats:
  * Depending on format, a low-compression or no-compression encoder will likely
    be available even without `imagine` being allowed to allocate.
  * Depending on format, a good compressor will be available when `imagine` is
    allowed to allocate.

## Status

* `png` and `bmp` can both be decoded properly without `imagine` allocating. The
  "simple" API for doing so is not yet developed. This means that currently you
  need quite a bit of knowledge about the details of each format to actually
  decode an image. See the `demo` example if you want to try and understand how
  things work (though the `demo` doesn't yet handle all cases either...).
* Changes are expected to break things in upcoming versions! We're `0.0.z` for a
  reason.

There's many places for improvement, file PRs if you like!

## License

All the code and docs are licensed as given in the `Cargo.toml` file.

**However,** this repository also contains a number of test images (within the
`tests/` directory), many of which are used under various other permissive
licenses. In this case, each folder of images will have a file giving the
license information. Since this library's test data is not included in your own
program when you use the crate as a dependency this should not generally be a
concern.