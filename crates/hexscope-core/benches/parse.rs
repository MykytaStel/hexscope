use criterion::{Criterion, criterion_group, criterion_main};
use hexscope_core::png::parse_png;
use std::hint::black_box;
use std::time::Duration;

/// Builds a ~10 MB PNG in memory: noisy pixel data so DEFLATE has real work.
fn big_png() -> Vec<u8> {
    use flate2::Compression;
    use flate2::write::ZlibEncoder;
    use std::io::Write;

    let (width, height) = (1600u32, 1600u32);
    let mut raw = Vec::with_capacity(((width * 3 + 1) * height) as usize);
    for y in 0..height {
        raw.push(0u8); // filter type None
        for x in 0..width {
            let v = (x ^ y) as u8;
            raw.extend_from_slice(&[v, v.wrapping_mul(7), v.wrapping_add(31)]);
        }
    }

    let mut enc = ZlibEncoder::new(Vec::new(), Compression::fast());
    enc.write_all(&raw).unwrap();
    let idat = enc.finish().unwrap();

    fn chunk(kind: &[u8; 4], data: &[u8]) -> Vec<u8> {
        let mut out = (data.len() as u32).to_be_bytes().to_vec();
        out.extend_from_slice(kind);
        out.extend_from_slice(data);
        let mut crc_input = kind.to_vec();
        crc_input.extend_from_slice(data);
        out.extend_from_slice(&hexscope_core::crc32::crc32(&crc_input).to_be_bytes());
        out
    }

    let mut ihdr = Vec::new();
    ihdr.extend_from_slice(&width.to_be_bytes());
    ihdr.extend_from_slice(&height.to_be_bytes());
    ihdr.extend_from_slice(&[8, 2, 0, 0, 0]);

    let mut png = vec![0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A];
    png.extend_from_slice(&chunk(b"IHDR", &ihdr));
    png.extend_from_slice(&chunk(b"IDAT", &idat));
    png.extend_from_slice(&chunk(b"IEND", &[]));
    png
}

fn bench_parse(c: &mut Criterion) {
    let png = big_png();
    let mb = png.len() as f64 / 1_048_576.0;
    println!("benchmark input: {mb:.1} MB");

    let mut group = c.benchmark_group("parse_png");
    // The spec's budget: 10 MB in under 300 ms.
    group.measurement_time(Duration::from_secs(20));
    group.bench_function("10mb", |b| b.iter(|| parse_png(black_box(&png))));
    group.finish();
}

criterion_group!(benches, bench_parse);
criterion_main!(benches);
