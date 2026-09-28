//! Large files, read within a time budget: a long movie, a PDF of tens of
//! thousands of objects, an archive of tens of thousands of entries, an
//! email full of links. Each is built in memory, parsed and cleaned once.
//! The budgets are generous — a slow CI machine passes them easily — so a
//! failure means work that grows faster than the file: a quadratic loop, a
//! scan of every byte where a jump would do.
//!
//! Timing means nothing in a debug build, so these run only in release:
//! `cargo test --release -p hexscope-core --test big`.

use hexscope_core::{clean, parse};
use std::time::{Duration, Instant};

/// Parses `data`, and cleans it when it can be, within `budget`.
fn within(name: &str, data: &[u8], budget: Duration) {
    let start = Instant::now();
    let doc = parse(data);
    let parsed = start.elapsed();
    let _ = clean::clean(data);
    let total = start.elapsed();
    drop(doc);
    eprintln!(
        "{name}: {} MB parsed in {parsed:?}, parsed and cleaned in {total:?}",
        data.len() / 1_000_000
    );
    assert!(total <= budget, "{name}: took {total:?}, budget {budget:?}");
}

fn boxed(kind: &[u8; 4], body: &[u8]) -> Vec<u8> {
    let mut b = ((body.len() + 8) as u32).to_be_bytes().to_vec();
    b.extend_from_slice(kind);
    b.extend_from_slice(body);
    b
}

/// A movie with 128 MB of media data: only its boxes should be read.
#[test]
#[cfg_attr(debug_assertions, ignore = "timing needs a release build")]
fn a_long_movie_is_read_by_its_boxes() {
    let mut mp4 = boxed(b"ftyp", b"isom\0\0\x02\0isomiso2mp41");
    let mut mvhd = vec![0u8; 100];
    mvhd[15] = 0xE8; // timescale 1000
    let udta = boxed(
        b"udta",
        &boxed(b"\xa9xyz", b"\0\x12\x15\xc7+48.8584+002.2945/"),
    );
    let mut moov_body = boxed(b"mvhd", &mvhd);
    moov_body.extend(udta);
    mp4.extend(boxed(b"moov", &moov_body));
    let media = vec![0u8; 128 << 20];
    mp4.extend(((media.len() + 8) as u32).to_be_bytes());
    mp4.extend_from_slice(b"mdat");
    mp4.extend(media);
    within("movie", &mp4, Duration::from_millis(600));
}

/// A PDF of 30,000 objects with no cross-reference table: found by scanning.
#[test]
#[cfg_attr(debug_assertions, ignore = "timing needs a release build")]
fn a_pdf_of_many_objects() {
    let mut pdf = b"%PDF-1.7\n".to_vec();
    pdf.extend_from_slice(b"1 0 obj << /Type /Catalog /Pages 2 0 R >> endobj\n");
    let pages = 10_000;
    let kids: Vec<String> = (0..pages).map(|i| format!("{} 0 R", 3 + i * 2)).collect();
    pdf.extend(
        format!(
            "2 0 obj << /Type /Pages /Count {pages} /Kids [{}] >> endobj\n",
            kids.join(" ")
        )
        .bytes(),
    );
    for i in 0..pages {
        let (page, content) = (3 + i * 2, 4 + i * 2);
        let text = format!("BT /F1 12 Tf 72 700 Td (Page {i} of a long report) Tj ET");
        pdf.extend(
            format!(
                "{page} 0 obj << /Type /Page /Parent 2 0 R /MediaBox [0 0 612 792] /Contents {content} 0 R >> endobj\n\
                 {content} 0 obj << /Length {} >> stream\n{text}\nendstream endobj\n",
                text.len()
            )
            .bytes(),
        );
    }
    pdf.extend_from_slice(b"trailer << /Root 1 0 R >>\n%%EOF\n");
    within("pdf", &pdf, Duration::from_secs(2));
}

/// A PDF as modern writers save it: its pages packed, a hundred to a
/// compressed object stream. Each is found through an index, its stream
/// unpacked once for the index and once when asked for.
#[test]
#[cfg_attr(debug_assertions, ignore = "timing needs a release build")]
fn a_pdf_of_packed_objects() {
    use flate2::{Compression, write::ZlibEncoder};
    use std::io::Write;
    let pages = 10_000u32;
    let per = 100u32;
    let mut pdf = b"%PDF-1.7\n".to_vec();
    pdf.extend_from_slice(b"1 0 obj << /Type /Catalog /Pages 2 0 R >> endobj\n");
    let kids: Vec<String> = (0..pages).map(|i| format!("{} 0 R", 10 + i)).collect();
    pdf.extend(
        format!(
            "2 0 obj << /Type /Pages /Count {pages} /Kids [{}] >> endobj\n",
            kids.join(" ")
        )
        .bytes(),
    );
    let content = b"BT /F1 12 Tf 72 700 Td (A packed page) Tj ET";
    pdf.extend(format!("3 0 obj << /Length {} >> stream\n", content.len()).bytes());
    pdf.extend_from_slice(content);
    pdf.extend_from_slice(b"\nendstream endobj\n");
    for s in 0..pages / per {
        let (mut head, mut body) = (String::new(), String::new());
        for k in 0..per {
            let num = 10 + s * per + k;
            head.push_str(&format!("{num} {} ", body.len()));
            body.push_str(
                "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 612 792] /Contents 3 0 R >> ",
            );
        }
        let raw = format!("{head}{body}");
        let mut z = ZlibEncoder::new(Vec::new(), Compression::fast());
        z.write_all(raw.as_bytes()).unwrap();
        let packed = z.finish().unwrap();
        pdf.extend(
            format!(
                "{} 0 obj << /Type /ObjStm /N {per} /First {} /Filter /FlateDecode /Length {} >> stream\n",
                100_000 + s,
                head.len(),
                packed.len()
            )
            .bytes(),
        );
        pdf.extend(packed);
        pdf.extend_from_slice(b"\nendstream endobj\n");
    }
    pdf.extend_from_slice(b"trailer << /Root 1 0 R >>\n%%EOF\n");
    within("packed pdf", &pdf, Duration::from_secs(2));
}

/// A stored ZIP of 20,000 small entries.
#[test]
#[cfg_attr(debug_assertions, ignore = "timing needs a release build")]
fn an_archive_of_many_entries() {
    let mut zip = Vec::new();
    let mut central = Vec::new();
    let n = 20_000u32;
    for i in 0..n {
        let name = format!("folder/file-{i}.txt");
        let data = format!("entry {i}\n");
        let crc = hexscope_core::crc32::crc32(data.as_bytes());
        let offset = zip.len() as u32;
        let mut local = vec![0x50, 0x4B, 3, 4, 20, 0, 0, 0, 0, 0, 0, 0, 0, 0];
        local.extend(crc.to_le_bytes());
        local.extend((data.len() as u32).to_le_bytes());
        local.extend((data.len() as u32).to_le_bytes());
        local.extend((name.len() as u16).to_le_bytes());
        local.extend(0u16.to_le_bytes());
        local.extend(name.as_bytes());
        local.extend(data.as_bytes());
        zip.extend(local);
        let mut c = vec![0x50, 0x4B, 1, 2, 20, 0, 20, 0, 0, 0, 0, 0, 0, 0, 0, 0];
        c.extend(crc.to_le_bytes());
        c.extend((data.len() as u32).to_le_bytes());
        c.extend((data.len() as u32).to_le_bytes());
        c.extend((name.len() as u16).to_le_bytes());
        c.extend([0u8; 12]);
        c.extend(offset.to_le_bytes());
        c.extend(name.as_bytes());
        central.extend(c);
    }
    let cd_offset = zip.len() as u32;
    let cd_len = central.len() as u32;
    zip.extend(central);
    zip.extend([0x50, 0x4B, 5, 6, 0, 0, 0, 0]);
    zip.extend((n as u16).to_le_bytes());
    zip.extend((n as u16).to_le_bytes());
    zip.extend(cd_len.to_le_bytes());
    zip.extend(cd_offset.to_le_bytes());
    zip.extend(0u16.to_le_bytes());
    within("zip", &zip, Duration::from_secs(3));
}

/// An email whose web page holds 5,000 links, and a large attachment.
#[test]
#[cfg_attr(debug_assertions, ignore = "timing needs a release build")]
fn an_email_full_of_links() {
    let mut html = String::new();
    for i in 0..5_000 {
        html.push_str(&format!("<p><a href=\"https://site{i}.example.com/a?b={i}&amp;c=d\">www.example-bank.com/{i}</a></p>\n"));
    }
    let attachment = "QUJD".repeat(2_000_000);
    let eml = format!(
        "From: A <a@example.com>\r\nTo: b@example.net\r\nSubject: links\r\nMIME-Version: 1.0\r\n\
         Content-Type: multipart/mixed; boundary=\"b\"\r\n\r\n--b\r\nContent-Type: text/html\r\n\r\n{html}\r\n\
         --b\r\nContent-Type: application/octet-stream; name=\"x.bin\"\r\nContent-Transfer-Encoding: base64\r\n\r\n{attachment}\r\n--b--\r\n"
    );
    within("email", eml.as_bytes(), Duration::from_secs(2));
}

/// A workbook with a sheet of 200,000 cells, a thousand hidden rows.
#[test]
#[cfg_attr(debug_assertions, ignore = "timing needs a release build")]
fn a_large_workbook() {
    let mut sheet = String::from("<worksheet><sheetData>");
    for r in 1..=20_000 {
        let hidden = if r % 20 == 0 { " hidden=\"1\"" } else { "" };
        sheet.push_str(&format!("<row r=\"{r}\"{hidden}>"));
        for c in 0..10 {
            sheet.push_str(&format!(
                "<c r=\"{}{r}\"><v>{}</v></c>",
                (b'A' + c) as char,
                r * 10 + c as u32
            ));
        }
        sheet.push_str("</row>");
    }
    sheet.push_str("</sheetData></worksheet>");
    let parts = [
        ("[Content_Types].xml", "<Types/>".to_string()),
        (
            "xl/workbook.xml",
            "<workbook><sheets><sheet name=\"Data\" sheetId=\"1\"/></sheets></workbook>"
                .to_string(),
        ),
        ("xl/worksheets/sheet1.xml", sheet),
        (
            "docProps/core.xml",
            "<cp:coreProperties><dc:creator>Someone</dc:creator></cp:coreProperties>".to_string(),
        ),
    ];
    let mut zip = Vec::new();
    let mut central = Vec::new();
    for (name, data) in &parts {
        let crc = hexscope_core::crc32::crc32(data.as_bytes());
        let offset = zip.len() as u32;
        zip.extend([0x50, 0x4B, 3, 4, 20, 0, 0, 0, 0, 0, 0, 0, 0, 0]);
        zip.extend(crc.to_le_bytes());
        zip.extend((data.len() as u32).to_le_bytes());
        zip.extend((data.len() as u32).to_le_bytes());
        zip.extend((name.len() as u16).to_le_bytes());
        zip.extend(0u16.to_le_bytes());
        zip.extend(name.as_bytes());
        zip.extend(data.as_bytes());
        central.extend([0x50, 0x4B, 1, 2, 20, 0, 20, 0, 0, 0, 0, 0, 0, 0, 0, 0]);
        central.extend(crc.to_le_bytes());
        central.extend((data.len() as u32).to_le_bytes());
        central.extend((data.len() as u32).to_le_bytes());
        central.extend((name.len() as u16).to_le_bytes());
        central.extend([0u8; 12]);
        central.extend(offset.to_le_bytes());
        central.extend(name.as_bytes());
    }
    let (cd_offset, cd_len) = (zip.len() as u32, central.len() as u32);
    zip.extend(central);
    zip.extend([0x50, 0x4B, 5, 6, 0, 0, 0, 0]);
    zip.extend((parts.len() as u16).to_le_bytes());
    zip.extend((parts.len() as u16).to_le_bytes());
    zip.extend(cd_len.to_le_bytes());
    zip.extend(cd_offset.to_le_bytes());
    zip.extend(0u16.to_le_bytes());
    let doc = parse(&zip);
    if let hexscope_core::Document::Zip(z) = &doc {
        assert!(
            z.facts
                .iter()
                .any(|f| f.kind == "hiddencells" && f.text.starts_with("1000 hidden rows")),
            "{:?}",
            z.facts
        );
    }
    within("xlsx", &zip, Duration::from_secs(2));
}
