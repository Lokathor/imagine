# imagine

Project Goals:

* To provide image format decoders for various image formats.
* Decoders should be *possible* to use without allocation (into user provided buffers).
* Decoders should be *simple* to use when allocation is allowed. A function to
  "just get me the RGBA8 pixels", and so on.

Project Status:

* `png` should decode properly right now, though the interface is not yet good.
* `bmp` is being added in another branch.
* Other image formats are planned as time permits.
