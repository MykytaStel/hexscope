# hexscope-core

Parse binary files into a tree whose nodes point to the exact bytes they
describe. The library accepts an in-memory byte slice and does not perform
file or network I/O.

```toml
[dependencies]
hexscope-core = "0.1"
```

```rust
use hexscope_core::{Format, parse};

fn inspect(bytes: &[u8]) {
    let document = parse(bytes);
    assert_eq!(document.format(), Format::Png);

    for node in document.tree().nodes() {
        println!("{}: {}..{}", node.label, node.range.start, node.range.end());
    }
}
```

An unrecognised or damaged input still produces a parse tree; damage is
represented in the tree with byte ranges so callers can show where it is.
The default `documents` feature adds PDF, ZIP-based documents, WebAssembly,
email and CFB parsing. Disable default features for the smaller
images-and-video parser.

Supported top-level formats include PNG, JPEG, HEIF, WebP, GIF, video,
PDF, ZIP, WebAssembly, EML and CFB. Coverage varies within each format;
see the [known issues](https://github.com/MykytaStel/hexscope/blob/main/docs/known-issues.md)
for current limits.

Licensed under either MIT or Apache-2.0, at your option.
