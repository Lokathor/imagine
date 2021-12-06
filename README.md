[![License:Zlib](https://img.shields.io/badge/License-Zlib-brightgreen.svg)](https://opensource.org/licenses/Zlib)
[![crates.io](https://img.shields.io/crates/v/imagine.svg)](https://crates.io/crates/imagine)
[![docs.rs](https://docs.rs/imagine/badge.svg)](https://docs.rs/imagine/)

# imagine

Project Goals:

* To provide image format **decoders** for various image formats:
  * Decoders should be *possible* to use without the library doing any allocation
    (the user provides any necessary buffers).
  * Decoders should be *simple* to use when the library is allowed to allocate
    for you. Functions to "just get me the RGBA8 pixels", and things like that.
* To provide image format **encoders** for various image formats:
  * Depending on format, a low-compression or no-compression encoder will likely
    be available even without the library being allowed to allocate.
  * Depending on format, a good compressor will be available when the library is
    allowed to allocate.

Project Status:

* `png` can be decoded properly without allocating. The "simple" API for doing
  so is not yet developed. See the `demo` example for how to do it yourself.
* Changes are expected to break things in upcoming versions!
