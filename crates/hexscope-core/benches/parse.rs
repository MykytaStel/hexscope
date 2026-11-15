use criterion::{Criterion, criterion_group, criterion_main};
use flate2::Compression;
use flate2::write::{DeflateEncoder, ZlibEncoder};
use hexscope_core::crc32::crc32;
use hexscope_core::pdf::parse_pdf;
use hexscope_core::png::parse_png;
use hexscope_core::zip::{extract, parse_zip};
use hexscope_inflate::{CheckpointSink, NoTrace, inflate, zlib_decompress};
use std::hint::black_box;
use std::io::Write;
use std::time::Duration;

fn chunk(kind: &[u8; 4], data: &[u8]) -> Vec<u8> {
    let mut out = (data.len() as u32).to_be_bytes().to_vec();
    out.extend_from_slice(kind);
    out.extend_from_slice(data);
    let mut crc_input = kind.to_vec();
    crc_input.extend_from_slice(data);
    out.extend_from_slice(&crc32(&crc_input).to_be_bytes());
    out
}

fn png_with_idat(width: u32, height: u32, idat: &[u8]) -> Vec<u8> {
    let mut ihdr = Vec::new();
    ihdr.extend_from_slice(&width.to_be_bytes());
    ihdr.extend_from_slice(&height.to_be_bytes());
    ihdr.extend_from_slice(&[8, 2, 0, 0, 0]);

    let mut png = vec![0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A];
    png.extend_from_slice(&chunk(b"IHDR", &ihdr));
    png.extend_from_slice(&chunk(b"IDAT", idat));
    png.extend_from_slice(&chunk(b"IEND", &[]));
    png
}

/// Builds a ~10 MB PNG in memory: noisy pixel data so DEFLATE has real work.
fn big_png() -> Vec<u8> {
    let (width, height) = (1900u32, 1900u32);
    let mut raw = Vec::with_capacity(((width * 3 + 1) * height) as usize);
    let mut state: u64 = 0x2545_F491_4F6C_DD1D;
    for y in 0..height {
        raw.push(0u8); // filter type None
        for x in 0..width {
            // A cheap LCG rather than `x ^ y`: the xor pattern compresses to a
            // few hundred KB, so the benchmark would measure a file far smaller
            // than the budget is about.
            state = state
                .wrapping_mul(6364136223846793005)
                .wrapping_add((x as u64) ^ ((y as u64) << 32));
            let v = (state >> 33) as u32;
            raw.extend_from_slice(&[v as u8, (v >> 8) as u8, (v >> 16) as u8]);
        }
    }

    let mut enc = ZlibEncoder::new(Vec::new(), Compression::fast());
    enc.write_all(&raw).unwrap();
    let idat = enc.finish().unwrap();
    let decoded = zlib_decompress(&idat, 512 * 1024 * 1024, &mut NoTrace)
        .expect("benchmark IDAT must decode");
    assert!(
        decoded == raw,
        "benchmark inflater must reproduce the generated scanlines"
    );

    let png = png_with_idat(width, height, &idat);

    // The budget in the spec is stated for a 10 MB file. If the generator ever
    // drifts back to producing something highly compressible, fail loudly
    // rather than quietly benchmarking the wrong thing.
    assert!(
        png.len() >= 10 * 1024 * 1024,
        "benchmark input is only {} bytes; the budget is about 10 MB",
        png.len()
    );
    png
}

/// Builds the same 10 MiB of decoded pixels with maximum LZ77 repetition.
fn repetitive_png() -> Vec<u8> {
    let (width, height) = (1900u32, 1900u32);
    // Every scanline starts with filter type 0; every pixel is black RGB.
    let raw = vec![0u8; ((width * 3 + 1) * height) as usize];
    let mut enc = ZlibEncoder::new(Vec::new(), Compression::best());
    enc.write_all(&raw).unwrap();
    let idat = enc.finish().unwrap();
    assert!(
        zlib_decompress(&idat, 512 * 1024 * 1024, &mut NoTrace).unwrap() == raw,
        "repetitive PNG scanlines must round-trip"
    );
    png_with_idat(width, height, &idat)
}

fn pdf_with_flate_xmp() -> Vec<u8> {
    let mut xmp = b"<x:xmpmeta><rdf:Description xmp:CreatorTool=\"Hexscope bench\"/>".to_vec();
    xmp.resize(4 * 1024 * 1024, b' ');
    xmp.extend_from_slice(b"</x:xmpmeta>");
    let mut encoder = ZlibEncoder::new(Vec::new(), Compression::best());
    encoder.write_all(&xmp).unwrap();
    let compressed = encoder.finish().unwrap();

    let mut pdf = format!(
        "%PDF-1.7\n1 0 obj\n<< /Type /Catalog /Metadata 2 0 R >>\nendobj\n2 0 obj\n<< /Type /Metadata /Subtype /XML /Filter /FlateDecode /Length {} >>\nstream\n",
        compressed.len()
    )
    .into_bytes();
    pdf.extend_from_slice(&compressed);
    pdf.extend_from_slice(b"\nendstream\nendobj\ntrailer\n<< /Root 1 0 R >>\n%%EOF\n");
    let parsed = parse_pdf(&pdf);
    assert!(parsed.metadata_complete, "benchmark XMP must be read fully");
    assert!(
        parsed.facts.iter().any(|fact| fact.kind == "application"),
        "benchmark XMP must produce its declared CreatorTool fact"
    );
    pdf
}

fn zip_with_deflated_member(raw: &[u8]) -> Vec<u8> {
    const NAME: &[u8] = b"payload.bin";
    let mut encoder = DeflateEncoder::new(Vec::new(), Compression::best());
    encoder.write_all(raw).unwrap();
    let compressed = encoder.finish().unwrap();
    let crc = crc32(raw);
    assert_eq!(
        crc,
        crc32_reference(raw),
        "CRC must match bitwise reference"
    );
    let compressed_len = compressed.len() as u32;
    let raw_len = raw.len() as u32;

    let mut zip = Vec::new();
    let local_offset = zip.len() as u32;
    zip.extend_from_slice(b"PK\x03\x04");
    zip.extend_from_slice(&20u16.to_le_bytes());
    zip.extend_from_slice(&0u16.to_le_bytes());
    zip.extend_from_slice(&8u16.to_le_bytes());
    zip.extend_from_slice(&[0; 4]);
    zip.extend_from_slice(&crc.to_le_bytes());
    zip.extend_from_slice(&compressed_len.to_le_bytes());
    zip.extend_from_slice(&raw_len.to_le_bytes());
    zip.extend_from_slice(&(NAME.len() as u16).to_le_bytes());
    zip.extend_from_slice(&0u16.to_le_bytes());
    zip.extend_from_slice(NAME);
    zip.extend_from_slice(&compressed);

    let central_offset = zip.len() as u32;
    zip.extend_from_slice(b"PK\x01\x02");
    zip.extend_from_slice(&20u16.to_le_bytes());
    zip.extend_from_slice(&20u16.to_le_bytes());
    zip.extend_from_slice(&0u16.to_le_bytes());
    zip.extend_from_slice(&8u16.to_le_bytes());
    zip.extend_from_slice(&[0; 4]);
    zip.extend_from_slice(&crc.to_le_bytes());
    zip.extend_from_slice(&compressed_len.to_le_bytes());
    zip.extend_from_slice(&raw_len.to_le_bytes());
    zip.extend_from_slice(&(NAME.len() as u16).to_le_bytes());
    zip.extend_from_slice(&[0; 8]);
    zip.extend_from_slice(&0u32.to_le_bytes());
    zip.extend_from_slice(&local_offset.to_le_bytes());
    zip.extend_from_slice(NAME);

    let central_size = zip.len() as u32 - central_offset;
    zip.extend_from_slice(b"PK\x05\x06");
    zip.extend_from_slice(&[0; 4]);
    zip.extend_from_slice(&1u16.to_le_bytes());
    zip.extend_from_slice(&1u16.to_le_bytes());
    zip.extend_from_slice(&central_size.to_le_bytes());
    zip.extend_from_slice(&central_offset.to_le_bytes());
    zip.extend_from_slice(&0u16.to_le_bytes());

    let parsed = parse_zip(&zip);
    assert_eq!(
        parsed.entries.len(),
        1,
        "benchmark ZIP must parse one entry"
    );
    assert_eq!(
        extract(&zip, &parsed.entries[0], raw.len() as u64).unwrap(),
        raw,
        "benchmark ZIP entry must round-trip and pass CRC"
    );
    zip
}

fn crc32_reference(data: &[u8]) -> u32 {
    let mut crc = 0xFFFF_FFFFu32;
    for &byte in data {
        crc ^= byte as u32;
        for _ in 0..8 {
            crc = (crc >> 1) ^ if crc & 1 != 0 { 0xEDB8_8320 } else { 0 };
        }
    }
    !crc
}

fn zip_with_many_empty_members(count: u16) -> Vec<u8> {
    let mut zip = Vec::with_capacity(count as usize * 100);
    let mut central = Vec::with_capacity(count as usize * 64);
    for index in 0..count {
        let name = format!("file-{index:05}.bin");
        let offset = zip.len() as u32;
        zip.extend_from_slice(b"PK\x03\x04");
        zip.extend_from_slice(&20u16.to_le_bytes());
        zip.extend_from_slice(&0u16.to_le_bytes());
        zip.extend_from_slice(&0u16.to_le_bytes());
        zip.extend_from_slice(&[0; 16]);
        zip.extend_from_slice(&(name.len() as u16).to_le_bytes());
        zip.extend_from_slice(&0u16.to_le_bytes());
        zip.extend_from_slice(name.as_bytes());

        central.extend_from_slice(b"PK\x01\x02");
        central.extend_from_slice(&20u16.to_le_bytes());
        central.extend_from_slice(&20u16.to_le_bytes());
        central.extend_from_slice(&0u16.to_le_bytes());
        central.extend_from_slice(&0u16.to_le_bytes());
        central.extend_from_slice(&[0; 16]);
        central.extend_from_slice(&(name.len() as u16).to_le_bytes());
        central.extend_from_slice(&[0; 8]);
        central.extend_from_slice(&0u32.to_le_bytes());
        central.extend_from_slice(&offset.to_le_bytes());
        central.extend_from_slice(name.as_bytes());
    }
    let central_offset = zip.len() as u32;
    zip.extend_from_slice(&central);
    zip.extend_from_slice(b"PK\x05\x06");
    zip.extend_from_slice(&[0; 4]);
    zip.extend_from_slice(&count.to_le_bytes());
    zip.extend_from_slice(&count.to_le_bytes());
    zip.extend_from_slice(&(central.len() as u32).to_le_bytes());
    zip.extend_from_slice(&central_offset.to_le_bytes());
    zip.extend_from_slice(&0u16.to_le_bytes());
    assert_eq!(parse_zip(&zip).entries.len(), count as usize);
    zip
}

fn bench_parse(c: &mut Criterion) {
    let png = big_png();
    let repetitive = repetitive_png();
    let mb = png.len() as f64 / 1_048_576.0;
    println!("benchmark input: {mb:.1} MB");
    // The generated PNG has one IHDR chunk followed by one IDAT chunk.
    let idat_len = u32::from_be_bytes(png[33..37].try_into().unwrap()) as usize;
    let idat = &png[41..41 + idat_len];
    let deflate = &idat[2..idat.len() - 4];

    let mut group = c.benchmark_group("parse_png");
    // The spec's budget: 10 MB in under 300 ms.
    group.measurement_time(Duration::from_secs(20));
    group.bench_function("10mb", |b| b.iter(|| parse_png(black_box(&png))));
    group.bench_function("repetitive_10mb", |b| {
        b.iter(|| parse_png(black_box(&repetitive)))
    });
    group.finish();

    // Split out decompression, trace collection, and Adler-32 so the full
    // parser's time points at a component before we change its hot loop.
    let mut parts = c.benchmark_group("png_components");
    parts.measurement_time(Duration::from_secs(20));
    let repetitive_zlib = {
        let repeated = vec![0xA5; 10 * 1024 * 1024];
        let mut enc = ZlibEncoder::new(Vec::new(), Compression::best());
        enc.write_all(&repeated).unwrap();
        let zlib = enc.finish().unwrap();
        assert!(
            zlib_decompress(&zlib, 512 * 1024 * 1024, &mut NoTrace).unwrap() == repeated,
            "match-heavy benchmark stream must reproduce its input"
        );
        zlib
    };
    let repetitive_deflate = &repetitive_zlib[2..repetitive_zlib.len() - 4];
    parts.bench_function("inflate_no_trace", |b| {
        b.iter(|| inflate(black_box(deflate), 512 * 1024 * 1024, &mut NoTrace))
    });
    parts.bench_function("inflate_repetitive", |b| {
        b.iter(|| {
            inflate(
                black_box(repetitive_deflate),
                512 * 1024 * 1024,
                &mut NoTrace,
            )
        })
    });
    parts.bench_function("inflate_checkpoints", |b| {
        b.iter(|| {
            let mut sink = CheckpointSink::new(512);
            let out = inflate(black_box(deflate), 512 * 1024 * 1024, &mut sink);
            black_box((out, sink.finish()))
        })
    });
    parts.bench_function("zlib_no_trace", |b| {
        b.iter(|| zlib_decompress(black_box(idat), 512 * 1024 * 1024, &mut NoTrace))
    });
    parts.bench_function("zlib_checkpoints", |b| {
        b.iter(|| {
            let mut sink = CheckpointSink::new(512);
            let out = zlib_decompress(black_box(idat), 512 * 1024 * 1024, &mut sink);
            black_box((out, sink.finish()))
        })
    });
    parts.finish();

    let pdf = pdf_with_flate_xmp();
    let zip_payload = vec![0xA5; 10 * 1024 * 1024];
    let zip = zip_with_deflated_member(&zip_payload);
    let many_entries = zip_with_many_empty_members(50_000);
    assert_eq!(crc32(b"123456789"), 0xCBF4_3926);
    let mut shared = c.benchmark_group("shared_components");
    shared.measurement_time(Duration::from_secs(3));
    shared.bench_function("crc32_10mb", |b| b.iter(|| crc32(black_box(&zip_payload))));
    shared.finish();

    let mut formats = c.benchmark_group("format_parsing");
    formats.measurement_time(Duration::from_secs(3));
    formats.bench_function("pdf_flate_xmp_4mb", |b| {
        b.iter(|| parse_pdf(black_box(&pdf)))
    });
    formats.bench_function("zip_50k_empty_entries", |b| {
        b.iter(|| parse_zip(black_box(&many_entries)))
    });
    formats.bench_function("zip_extract_deflated_10mb", |b| {
        let doc = parse_zip(&zip);
        b.iter(|| {
            extract(
                black_box(&zip),
                black_box(&doc.entries[0]),
                16 * 1024 * 1024,
            )
        })
    });
    formats.finish();
}

criterion_group!(benches, bench_parse);
criterion_main!(benches);
