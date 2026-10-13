# hexscope-raster

The codecs around [hexscope](https://github.com/MykytaStel/hexscope)'s film
lab: it reads a film scan as JPEG, PNG, WebP or TIFF, develops it with the
codec-free renderer in `hexscope-core` — a negative turned positive, its
orange mask taken out — and writes a JPEG or a 16-bit TIFF.

Used by the `hexscope film` command of `hexscope-cli`, and by the site in
WebAssembly. Files stay in memory: no file or network I/O.

Licensed under MIT or Apache-2.0, at your option.
