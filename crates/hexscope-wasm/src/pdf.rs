//! A PDF's pages for the page to draw on: the text under black boxes, the
//! pictures and the boxes drawn as shapes, and a copy with chosen text and
//! areas blacked out. Only the builds with PDF have it.

use super::*;
use crate::copies::into_copy;

pub(crate) fn page_areas(areas: &[f64]) -> Vec<(u32, [f64; 4])> {
    areas
        .as_chunks::<5>()
        .0
        .iter()
        .map(|[page, l, b, r, t]| (*page as u32, [*l, *b, *r, *t]))
        .collect()
}

/// A PDF's clean copy with chosen areas blacked out: `areas` is five
/// numbers per area — the page, counted from 1, then left, bottom, right
/// and top in its points. The JPEG pictures under them come painted, as
/// `jpegsUnder` asked: object numbers, each one's length, and their bytes
/// one after another. See `hexscope_core::clean::redact`.
#[wasm_bindgen(js_name = redactCopy)]
pub fn redact_copy(
    bytes: &[u8],
    areas: &[f64],
    nums: &[u32],
    lens: &[u32],
    painted: &[u8],
) -> CleanCopy {
    let mut given = Vec::new();
    let mut at = 0usize;
    for (&num, &len) in nums.iter().zip(lens) {
        let end = at.saturating_add(len as usize);
        if let Some(b) = painted.get(at..end) {
            given.push((num, b.to_vec()));
        }
        at = end;
    }
    into_copy(hexscope_core::clean::redact(
        bytes,
        &page_areas(areas),
        &given,
    ))
}

/// The JPEG pictures chosen areas cover part of, which the caller paints:
/// for each, its object number, width, height, where its bytes start, how
/// many there are, how many rectangles, then each rectangle's `x0 y0 x1
/// y1` in pixels from the top left. See `hexscope_core::pdf::jpegs_under`.
#[wasm_bindgen(js_name = jpegsUnder)]
pub fn jpegs_under(bytes: &[u8], areas: &[f64]) -> Vec<f64> {
    let mut out = Vec::new();
    for j in hexscope_core::pdf::jpegs_under(bytes, &page_areas(areas)) {
        out.extend([
            f64::from(j.num),
            f64::from(j.width),
            f64::from(j.height),
            j.start as f64,
            j.len as f64,
            j.rects.len() as f64,
        ]);
        out.extend(j.rects.iter().flatten().map(|&v| f64::from(v)));
    }
    out
}

/// Every page's visible text, glyph by glyph, for choosing what to black out.
#[wasm_bindgen]
pub struct PageTexts {
    pages: Vec<hexscope_core::pdf::PageText>,
}

#[wasm_bindgen]
impl PageTexts {
    /// Pages read.
    #[wasm_bindgen(getter)]
    pub fn count(&self) -> usize {
        self.pages.len()
    }

    /// A page's `[left, bottom, right, top]`.
    pub fn media(&self, i: usize) -> Vec<f64> {
        self.pages.get(i).map_or(Vec::new(), |p| p.media.to_vec())
    }

    /// Whether every supported form invocation on page `i` was inspected.
    pub fn complete(&self, i: usize) -> bool {
        self.pages.get(i).is_some_and(|page| page.complete)
    }

    /// A page's glyphs' areas, four numbers each.
    pub fn areas(&self, i: usize) -> Vec<f64> {
        self.pages
            .get(i)
            .map_or(Vec::new(), |p| p.glyphs.iter().flat_map(|g| g.0).collect())
    }

    /// A page's dark boxes and marks for redaction, four numbers each.
    pub fn boxes(&self, i: usize) -> Vec<f64> {
        self.pages
            .get(i)
            .map_or(Vec::new(), |p| p.boxes.iter().flatten().copied().collect())
    }

    /// A page's glyphs' text, joined by U+001F.
    pub fn texts(&self, i: usize) -> String {
        self.pages.get(i).map_or(String::new(), |p| {
            p.glyphs
                .iter()
                .map(|g| g.1.replace(SEPARATOR, ""))
                .collect::<Vec<_>>()
                .join(&SEPARATOR.to_string())
        })
    }
}

/// Reads every page's visible text: see `hexscope_core::pdf::page_texts`.
#[wasm_bindgen(js_name = pageTexts)]
pub fn page_texts(bytes: &[u8]) -> PageTexts {
    PageTexts {
        pages: hexscope_core::pdf::page_texts(bytes),
    }
}

/// The pictures a PDF page draws, to show a scanned page while choosing
/// what to black out on it.
#[wasm_bindgen]
pub struct PagePictures {
    pictures: Vec<hexscope_core::pdf::PagePicture>,
    /// The page each is on, counted from 1.
    pages: Vec<u32>,
}

#[wasm_bindgen]
impl PagePictures {
    #[wasm_bindgen(getter)]
    pub fn count(&self) -> usize {
        self.pictures.len()
    }

    /// The page picture `i` is on, counted from 1.
    pub fn page(&self, i: usize) -> u32 {
        self.pages.get(i).copied().unwrap_or(0)
    }

    /// `[a b c d e f]`, from the picture's unit square to the page.
    pub fn matrix(&self, i: usize) -> Vec<f64> {
        self.pictures
            .get(i)
            .map_or(Vec::new(), |p| p.matrix.to_vec())
    }

    /// Where a JPEG's bytes start and how many there are; empty when the
    /// picture comes as pixels.
    pub fn jpeg(&self, i: usize) -> Vec<f64> {
        self.pictures
            .get(i)
            .and_then(|p| p.jpeg)
            .map_or(Vec::new(), |(s, n)| vec![s as f64, n as f64])
    }

    /// Width and height of a picture that comes as pixels.
    pub fn size(&self, i: usize) -> Vec<u32> {
        self.pictures
            .get(i)
            .and_then(|p| p.rgba.as_ref())
            .map_or(Vec::new(), |(w, h, _)| vec![*w, *h])
    }

    /// Its RGBA pixels, rows from the top.
    pub fn rgba(&self, i: usize) -> Vec<u8> {
        self.pictures
            .get(i)
            .and_then(|p| p.rgba.as_ref())
            .map_or(Vec::new(), |(_, _, px)| px.clone())
    }
}

/// See `hexscope_core::pdf::page_pictures`.
#[wasm_bindgen(js_name = pagePictures)]
pub fn page_pictures(bytes: &[u8], page: u32) -> PagePictures {
    let pictures = hexscope_core::pdf::page_pictures(bytes, page);
    PagePictures {
        pages: vec![page; pictures.len()],
        pictures,
    }
}

/// The dark boxes pages fill, where there are enough to be a QR code drawn
/// in boxes: see `hexscope_core::pdf::shapes`.
#[wasm_bindgen]
pub struct PdfShapes {
    pages: Vec<(u32, [f64; 4], Vec<[f64; 4]>)>,
}

#[wasm_bindgen]
impl PdfShapes {
    #[wasm_bindgen(getter)]
    pub fn count(&self) -> usize {
        self.pages.len()
    }

    /// The page, counted from 1.
    pub fn page(&self, i: usize) -> u32 {
        self.pages.get(i).map_or(0, |p| p.0)
    }

    /// Its media box: left, bottom, right, top.
    pub fn media(&self, i: usize) -> Vec<f64> {
        self.pages.get(i).map_or(Vec::new(), |p| p.1.to_vec())
    }

    /// Its dark boxes, four numbers each: left, bottom, right, top.
    pub fn boxes(&self, i: usize) -> Vec<f64> {
        self.pages
            .get(i)
            .map_or(Vec::new(), |p| p.2.iter().flatten().copied().collect())
    }
}

#[wasm_bindgen(js_name = pdfShapes)]
pub fn pdf_shapes(bytes: &[u8], pages: u32) -> PdfShapes {
    PdfShapes {
        pages: hexscope_core::pdf::shapes(bytes, pages),
    }
}

/// Every picture the first `pages` pages draw, each once: see
/// `hexscope_core::pdf::pictures`.
#[wasm_bindgen(js_name = pdfPictures)]
pub fn pdf_pictures(bytes: &[u8], pages: u32) -> PagePictures {
    let (pages, pictures) = hexscope_core::pdf::pictures(bytes, pages)
        .into_iter()
        .unzip();
    PagePictures { pictures, pages }
}
