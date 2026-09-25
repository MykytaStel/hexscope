use super::*;
use crate::model::NodeKind;

fn leb(mut n: u64) -> Vec<u8> {
    let mut out = Vec::new();
    loop {
        let b = (n & 0x7F) as u8;
        n >>= 7;
        if n == 0 {
            out.push(b);
            return out;
        }
        out.push(b | 0x80);
    }
}

fn name(s: &str) -> Vec<u8> {
    let mut out = leb(s.len() as u64);
    out.extend_from_slice(s.as_bytes());
    out
}

fn section(id: u8, contents: &[u8]) -> Vec<u8> {
    let mut out = vec![id];
    out.extend(leb(contents.len() as u64));
    out.extend_from_slice(contents);
    out
}

fn custom(title: &str, payload: &[u8]) -> Vec<u8> {
    let mut contents = name(title);
    contents.extend_from_slice(payload);
    section(0, &contents)
}

fn module(sections: &[Vec<u8>]) -> Vec<u8> {
    let mut out = b"\0asm\x01\0\0\0".to_vec();
    for s in sections {
        out.extend_from_slice(s);
    }
    out
}

/// A module like a small Rust build: it writes through WASI, exports
/// `main`, keeps its function names, its producers, a source map link and a
/// path from its builder's computer in its data.
fn sample() -> Vec<u8> {
    // Types: (i32, i32, i32, i32) -> i32 for fd_write, () -> () for main.
    let types = [
        vec![2],
        vec![0x60, 4, 0x7F, 0x7F, 0x7F, 0x7F, 1, 0x7F],
        vec![0x60, 0, 0],
    ]
    .concat();
    let imports = [
        vec![1],
        name("wasi_snapshot_preview1"),
        name("fd_write"),
        vec![0, 0],
    ]
    .concat();
    let functions = vec![1, 1];
    let memory = vec![1, 1, 1, 16];
    let exports = [
        vec![2],
        name("main"),
        vec![0, 1],
        name("memory"),
        vec![2, 0],
    ]
    .concat();
    // main: no locals, calls nothing, ends.
    let body = vec![0, 0x0B];
    let code = [vec![1], leb(body.len() as u64), body].concat();
    let text = b"panicked at /Users/alice/projects/hello/src/main.rs:3:5";
    let data = [
        vec![1, 0, 0x41, 8, 0x0B],
        leb(text.len() as u64),
        text.to_vec(),
    ]
    .concat();
    let function_names = [vec![2], leb(0), name("fd_write"), leb(1), name("main")].concat();
    let names = [
        vec![0],
        leb(name("hello").len() as u64),
        name("hello"),
        vec![1],
        leb(function_names.len() as u64),
        function_names,
    ]
    .concat();
    let producers = [
        vec![2],
        name("language"),
        vec![1],
        name("Rust"),
        name(""),
        name("processed-by"),
        vec![1],
        name("rustc"),
        name("1.90.0 (1159e78c4 2025-09-14)"),
    ]
    .concat();
    module(&[
        section(1, &types),
        section(2, &imports),
        section(3, &functions),
        section(5, &memory),
        section(7, &exports),
        section(10, &code),
        section(11, &data),
        custom("name", &names),
        custom("producers", &producers),
        custom(
            "sourceMappingURL",
            &name("http://localhost:8000/hello.wasm.map"),
        ),
    ])
}

fn labels(doc: &WasmDocument) -> Vec<String> {
    doc.tree.nodes().iter().map(|n| n.label.clone()).collect()
}

fn problems(doc: &WasmDocument) -> Vec<String> {
    doc.tree
        .nodes()
        .iter()
        .filter(|n| matches!(n.kind, NodeKind::Error | NodeKind::Warning))
        .map(|n| n.label.clone())
        .collect()
}

#[test]
fn a_module_is_its_sections() {
    let doc = parse_wasm(&sample());
    assert!(problems(&doc).is_empty(), "{:?}", problems(&doc));
    let l = labels(&doc);
    for want in [
        "magic",
        "version",
        "section · type",
        "type 0",
        "section · import",
        "import wasi_snapshot_preview1.fd_write",
        "section · export",
        "export “main”",
        "section · code",
        "function 1 · main",
        "data segment 0",
        "custom section · name",
        "custom section · producers",
        "custom section · sourceMappingURL",
    ] {
        assert!(l.iter().any(|x| x == want), "{want} in {l:?}");
    }
    assert_eq!(doc.functions, 2, "one imported, one defined");
    let root = doc.tree.get(0);
    assert_eq!(
        root.value,
        Some(Value::Text("WebAssembly · 2 functions · “hello”".into()))
    );
}

#[test]
fn it_says_who_built_it_and_how() {
    let doc = parse_wasm(&sample());
    let fact = |kind: &str| {
        doc.facts
            .iter()
            .find(|f| f.kind == kind)
            .map(|f| f.text.clone())
    };
    assert_eq!(fact("language").as_deref(), Some("Rust"));
    assert_eq!(
        fact("toolchain").as_deref(),
        Some("rustc 1.90.0 (1159e78c4 2025-09-14)")
    );
    assert_eq!(
        fact("names").as_deref(),
        Some("module “hello”, 2 function names")
    );
    assert_eq!(
        fact("sourcemap").as_deref(),
        Some("http://localhost:8000/hello.wasm.map")
    );
    let paths = fact("paths").unwrap();
    assert!(
        paths.starts_with("alice, in paths like /Users/alice/projects/hello/src/main.rs"),
        "{paths}"
    );
    // Each fact points at a node that holds it.
    for f in &doc.facts {
        assert!(doc.tree.try_get(f.node).is_some(), "{}", f.kind);
    }
    let whats: Vec<_> = doc.strip.iter().map(|s| s.what).collect();
    assert_eq!(
        whats,
        [
            "function names",
            "the names of the tools that built it",
            "a link to its source map"
        ]
    );
}

#[test]
fn the_clean_copy_keeps_the_program() {
    let data = sample();
    let c = crate::clean::clean(&data).unwrap();
    let doc = parse_wasm(&c.bytes);
    assert!(problems(&doc).is_empty(), "{:?}", problems(&doc));
    assert!(!labels(&doc).iter().any(|l| l.starts_with("custom section")));
    assert!(
        doc.facts.iter().all(|f| f.kind == "paths"),
        "the data's paths stay"
    );
    assert_eq!(doc.functions, 2);
    // Everything before the custom sections is the same, byte for byte.
    let first = data.len() - c.removed.iter().map(|r| r.bytes as usize).sum::<usize>();
    assert_eq!(c.bytes, data[..first]);
    assert!(matches!(
        crate::clean::clean(&c.bytes),
        Err(crate::clean::CleanError::NothingToRemove)
    ));
}

#[test]
fn damage_is_named() {
    let good = sample();
    // Cut short anywhere: a tree, and no panic.
    for cut in 0..good.len() {
        let doc = parse_wasm(&good[..cut]);
        assert!(!doc.tree.is_empty());
    }
    let cut = parse_wasm(&good[..good.len() - 3]);
    assert!(
        problems(&cut)
            .iter()
            .any(|p| p.contains("runs past the end")),
        "{:?}",
        problems(&cut)
    );

    let out_of_order = module(&[section(7, &[0]), section(1, &[0])]);
    assert!(
        problems(&parse_wasm(&out_of_order))
            .iter()
            .any(|p| p.contains("out of order"))
    );
    let unknown = module(&[section(42, &[1, 2, 3])]);
    assert!(
        problems(&parse_wasm(&unknown))
            .iter()
            .any(|p| p.starts_with("unknown section id 42"))
    );
    let left_over = module(&[section(3, &[1, 0, 9, 9])]);
    assert!(
        problems(&parse_wasm(&left_over))
            .iter()
            .any(|p| p.contains("after the section's last entry"))
    );
    let broken_entry = module(&[section(2, &[1, 3, b'a'])]);
    assert!(
        problems(&parse_wasm(&broken_entry))
            .iter()
            .any(|p| p.contains("middle of an entry"))
    );
    assert!(matches!(
        crate::clean::clean(&out_of_order),
        Err(crate::clean::CleanError::Damaged)
    ));
}

#[test]
fn a_component_is_listed_not_decoded() {
    let mut data = b"\0asm\x0d\0\x01\0".to_vec();
    data.extend(section(1, &[0, 1, 2]));
    let doc = parse_wasm(&data);
    assert!(doc.component);
    assert!(labels(&doc).iter().any(|l| l == "component section 1"));
    assert!(problems(&doc).is_empty());
}

#[test]
fn dwarf_and_home_directories_in_it_are_found() {
    let dwarf = b"\x01\x02/home/bob/src/lib.rs\0/Users/Shared/x/";
    let data = module(&[custom(".debug_str", dwarf), custom(".debug_info", &[0; 40])]);
    let doc = parse_wasm(&data);
    let fact = |kind: &str| {
        doc.facts
            .iter()
            .find(|f| f.kind == kind)
            .map(|f| f.text.clone())
    };
    assert_eq!(
        fact("debug").as_deref(),
        Some("DWARF, 79 bytes in 2 sections")
    );
    let paths = fact("paths").unwrap();
    assert!(
        paths.starts_with("bob, in paths like /home/bob/src/lib.rs"),
        "{paths}"
    );
    assert!(!paths.contains("Shared"));
}
