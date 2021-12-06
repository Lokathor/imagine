# imagine

Project Goals:

* To provide image format decoders for various image formats.
* Decoders should be *possible* to use without the library doing any allocation
  (the user provides any necessary buffers).
* Decoders should be *simple* to use when allocation is allowed. Providing
  functions to "just get me the RGBA8 pixels", and things like that.

Project Status:

* `png` can be decodes properly. The API isn't good, but it does work.
* `bmp` is being added in another branch.
* Other image formats are planned as time permits.
