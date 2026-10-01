# hexscope-inflate

A dependency-free DEFLATE and zlib decoder that can report how the compressed
bits produced each part of the output. The step-by-step trace is useful for
inspectors and teaching tools; callers can also decode without retaining a
trace.

```toml
[dependencies]
hexscope-inflate = "0.1"
```

Decode a raw DEFLATE stream with an explicit output limit:

```rust
use hexscope_inflate::{inflate, InflateError, NoTrace};

fn decode(data: &[u8]) -> Result<Vec<u8>, InflateError> {
    let mut trace = NoTrace;
    inflate(data, 16 * 1024 * 1024, &mut trace)
}
```

For a zlib-wrapped stream, use `zlib_decompress`. To inspect each decoding
step, implement `EventSink` or iterate a `Decoder`. Both APIs enforce the
caller-provided output limit.

The crate implements the DEFLATE and zlib formats (RFC 1951 and RFC 1950).
It performs no file or network I/O; callers provide the bytes to decode.

Licensed under either MIT or Apache-2.0, at your option.
