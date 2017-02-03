
LibFLIC [![Version][version-img]][version-url] [![Status][travis-ci-img]][travis-ci-url]
=======


About
-----

LibFLIC provides routines for encoding and decoding Autodesk Animator
FLI and Autodesk Animator Pro FLC files.

The code is based on the documentation and source code of
[Animator and Animator Pro][animator-pro]
that has been released by Jim Kent.

LibFLIC is written entirely in Rust.  C bindings to the underlying
codecs are provided.


Examples
--------

A few example programs are provided in the `examples/` directory:

* _quickfli_ - a simple FLIC player.
* _recompress_ - loads and saves FLIC files.
* _browse_ - display postage stamps (thumbnails).

To clone this repository, run:

```sh
git clone https://github.com/wangds/libflic.git
```

Then build the library and run the example programs using Cargo.

```sh
cargo build --example quickfli
```

To play a FLIC file, run:

```sh
cargo run --example quickfli <example.flc>
```


Basic Usage
-----------

Add LibFLIC as a dependency to your project's Cargo.toml:

```toml
[dependencies]
flic = "0.1"
```

Import the library in your project, e.g.:

```rust
extern crate flic;

use flic::{FlicFile,RasterMut};
```

The `FlicFile` type refers to FLIC files streamed from disk.  When
opening a FLIC file, it will first read the FLIC metadata such as the
animation's dimensions and speed.  `FlicFile` will keep the file open.

```rust
let flic = FlicFile::open(Path::new("example.flc"))?;
```

Allocate the pixel data and palette data buffers to which we will
decode the animation.

```rust
let flic_w = flic.width() as usize;
let flic_h = flic.height() as usize;
let mut buf = vec![0; flic_w * flic_h];
let mut pal = vec![0; 3 * 256];
```

It is convenient to group these two buffers, along with their
dimensions and strides, together to form a single `Raster` or
`RasterMut` type.

LibFLIC will ask for a `Raster` type for operations that require
read-only access to the buffers (e.g. encoding), and a `RasterMut`
type when it requires read-write access (e.g. decoding).

For example, to decode a frame, we first create a `RasterMut` by
borrowing `buf` and `pal` mutably as shown below.  Rasters are cheap
to create, so don't worry about creating and dropping them frequently.

```rust
let mut raster = RasterMut::new(flic_w, flic_h, &mut buf, &mut pal);
flic.read_next_frame(&mut raster);
```

Since FLIC files store the differences between consecutive frames,
when reading the next frame in the animation, it is up to the library
user to ensure that the buffer and palettes contain the previous
frame's data.


Documentation
-------------

* [Documentation][documentation].


Author
------

David Wang


[animator-pro]: https://github.com/AnimatorPro/Animator-Pro
[documentation]: https://docs.rs/flic/
[travis-ci-img]: https://travis-ci.org/wangds/libflic.svg?branch=master
[travis-ci-url]: https://travis-ci.org/wangds/libflic
[version-img]: https://img.shields.io/crates/v/flic.svg
[version-url]: https://crates.io/crates/flic
