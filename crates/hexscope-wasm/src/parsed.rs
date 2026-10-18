//! What the page reads from a parsed file: the node arrays, the facts, the
//! pictures, a ZIP's entries and a PDF's covered text, each a getter or a
//! method of [`Parsed`].

use super::*;
use crate::steps::{block_kind_code, encode, error_code};
use crate::tables::{extract_message, openable, playable};

#[wasm_bindgen]
impl Parsed {
    /// Byte offset where each node begins.
    #[wasm_bindgen(getter)]
    pub fn starts(&self) -> Vec<f64> {
        self.starts.clone()
    }

    /// Length in bytes of each node. Zero means the node has no bytes to
    /// point at, e.g. "no IDAT chunk".
    #[wasm_bindgen(getter)]
    pub fn lens(&self) -> Vec<f64> {
        self.lens.clone()
    }

    /// Parent index of each node, `-1` for the root.
    #[wasm_bindgen(getter)]
    pub fn parents(&self) -> Vec<i32> {
        self.parents.clone()
    }

    /// 0 container, 1 field, 2 warning, 3 error.
    #[wasm_bindgen(getter)]
    pub fn kinds(&self) -> Vec<u8> {
        self.kinds.clone()
    }

    /// Node labels joined by U+001F.
    #[wasm_bindgen(getter)]
    pub fn labels(&self) -> String {
        self.labels.clone()
    }

    /// Display values joined by U+001F; empty where a node has no value.
    #[wasm_bindgen(getter)]
    pub fn values(&self) -> String {
        self.values.clone()
    }

    /// `[width, height, bitDepth, colorType, interlace]`, or empty when the
    /// file has no readable IHDR.
    #[wasm_bindgen(getter)]
    pub fn ihdr(&self) -> Vec<u32> {
        self.ihdr.map(|h| h.to_vec()).unwrap_or_default()
    }

    /// `[totalEvents, literals, matches, outputBytes]` for the DEFLATE stream,
    /// or empty when nothing was decompressed.
    #[wasm_bindgen(getter)]
    pub fn trace(&self) -> Vec<f64> {
        self.trace.map(|t| t.to_vec()).unwrap_or_default()
    }

    /// Compressed size: the IDAT payload bytes, summed across chunks.
    #[wasm_bindgen(getter, js_name = idatBytes)]
    pub fn idat_bytes(&self) -> f64 {
        self.idat_bytes
    }

    /// File position of each IDAT payload as `[start, len, start, len, ...]`.
    /// Concatenated in order they are the zlib stream, whose first two bytes
    /// are the zlib header — so compressed bit `b` of the DEFLATE data lives in
    /// stream byte `2 + b / 8`.
    #[wasm_bindgen(getter)]
    pub fn segments(&self) -> Vec<f64> {
        self.segments.clone()
    }

    /// `png`, `jpeg` or `unknown`.
    #[wasm_bindgen(getter)]
    pub fn format(&self) -> String {
        self.format.to_string()
    }

    /// A photo's EXIF orientation, 1 to 8: how to turn the stored picture
    /// to show it the right way up. 1 when it says nothing.
    #[wasm_bindgen(getter)]
    pub fn orientation(&self) -> u16 {
        self.orientation
    }

    /// `[width, height]`, or empty when the file does not say.
    #[wasm_bindgen(getter)]
    pub fn dimensions(&self) -> Vec<u32> {
        self.dimensions.map(|d| d.to_vec()).unwrap_or_default()
    }

    /// What a file's metadata reveals, as `kind, text, node` triples joined
    /// by U+001F. For a photo: camera, lens, serial, owner, software, taken,
    /// thumbnail. For an Office document: title, author, editor, created,
    /// modified, revisions, editing, company, application, template.
    #[wasm_bindgen(getter)]
    pub fn facts(&self) -> String {
        let sep = SEPARATOR.to_string();
        self.facts
            .iter()
            .flat_map(|(kind, text, node)| [kind.to_string(), text.clone(), node.to_string()])
            .collect::<Vec<_>>()
            .join(&sep)
    }

    /// Compares actual copy bytes with this already-parsed source. The extra
    /// summary parse is bounded for browser memory.
    #[wasm_bindgen(js_name = verifyCopy)]
    pub fn verify_copy(&self, bytes: &[u8]) -> String {
        const MAX_VERIFICATION_BYTES: usize = 10 * 1024 * 1024;
        let Some(source) = self.verification.as_ref() else {
            return hexscope_core::verification::VerificationReport::skipped().to_json();
        };
        if bytes.len() > MAX_VERIFICATION_BYTES {
            return hexscope_core::verification::VerificationReport::skipped().to_json();
        }
        let output =
            hexscope_core::verification::snapshot(&hexscope_core::summary::summarize(bytes));
        hexscope_core::verification::compare(source, &output).to_json()
    }

    /// `[latitude, longitude, altitude, node]` in decimal degrees and metres,
    /// altitude NaN when unknown; empty when the photo carries no location.
    #[wasm_bindgen(getter)]
    pub fn location(&self) -> Vec<f64> {
        self.location.map(|l| l.to_vec()).unwrap_or_default()
    }

    /// The decompressed stream (filtered scanlines). If decoding failed, the
    /// output produced before the failure.
    #[wasm_bindgen(getter)]
    pub fn inflated(&self) -> Vec<u8> {
        self.output.clone()
    }

    /// Where a JPEG's blocks are, scan by scan: `[progressive, components,
    /// scans]`, then for each scan `[components (a bit each), ss, se, ah,
    /// al, unitWidth, unitHeight, columns, rows, blocksPerUnit, end, n]` and
    /// its `n` numbers: each unit's first file bit, then the bit after the
    /// last. Empty when they cannot be found; [`Parsed::blocks_note`] says
    /// why. Found on first asking, since a large photo takes a moment.
    #[wasm_bindgen(js_name = blockMap)]
    pub fn block_map(&mut self) -> Vec<f64> {
        if self.blocks.is_none() {
            let found = hexscope_core::jpeg::blocks::block_map(&self.source);
            let (map, note) = match found {
                Ok(m) => {
                    let mut out = vec![
                        m.progressive as u8 as f64,
                        m.components as f64,
                        m.scans.len() as f64,
                    ];
                    for s in &m.scans {
                        out.extend([
                            s.components as f64,
                            s.ss as f64,
                            s.se as f64,
                            s.ah as f64,
                            s.al as f64,
                            s.unit_width as f64,
                            s.unit_height as f64,
                            s.columns as f64,
                            s.rows as f64,
                            s.blocks_per_unit as f64,
                            s.end as f64,
                            s.starts.len() as f64,
                        ]);
                        out.extend(s.starts.iter().map(|&b| b as f64));
                    }
                    (out, m.stopped.unwrap_or("").to_string())
                }
                Err(e) => (Vec::new(), e.reason().to_string()),
            };
            self.blocks = Some(map);
            self.blocks_note = note;
        }
        self.blocks.clone().unwrap_or_default()
    }

    /// Why the last [`Parsed::block_map`] is empty or stops short; empty when it is whole.
    #[wasm_bindgen(getter, js_name = blocksNote)]
    pub fn blocks_note(&self) -> String {
        self.blocks_note.clone()
    }

    /// A PDF's attached files, by name, joined by U+001F; each opens with
    /// `extractEntry` at its index.
    #[wasm_bindgen(getter)]
    pub fn attachments(&self) -> String {
        self.attachment_names.join(&SEPARATOR.to_string())
    }

    /// A PDF's pages with text under black boxes, to draw: per page
    /// `[page, left, bottom, right, top, boxes, texts, context]`, then four
    /// numbers per box, per covered text and per text left showing on the
    /// same lines, each `[left, bottom, right, top]` in the page's points.
    /// The texts are in `blackoutTexts`, covered first, in the same order.
    #[wasm_bindgen(getter)]
    pub fn blackouts(&self) -> Vec<f64> {
        self.blackouts.clone()
    }

    /// The texts of [`Parsed::blackouts`] joined by U+001F; an empty one
    /// is in a font whose codes do not read as letters.
    #[wasm_bindgen(getter, js_name = blackoutTexts)]
    pub fn blackout_texts(&self) -> String {
        self.blackout_texts.join(&SEPARATOR.to_string())
    }

    /// The picture's preview size, `[width, height]`; empty when there is none.
    #[wasm_bindgen(getter, js_name = previewSize)]
    pub fn preview_size(&self) -> Vec<u32> {
        self.preview.as_ref().map_or(Vec::new(), |p| vec![p.0, p.1])
    }

    /// The preview's pixels, four bytes each.
    #[wasm_bindgen(getter, js_name = previewPixels)]
    pub fn preview_pixels(&self) -> Vec<u8> {
        self.preview.as_ref().map_or(Vec::new(), |p| p.2.clone())
    }

    /// Each scanline's filter type, 0 to 4, in the order they are stored:
    /// top to bottom, or for an interlaced PNG pass by pass. Rows past a
    /// decoding failure are absent.
    #[wasm_bindgen(getter, js_name = rowFilters)]
    pub fn row_filters(&self) -> Vec<u8> {
        let Some([w, h, depth, color, interlace @ (0 | 1)]) = self.ihdr else {
            return Vec::new();
        };
        let ihdr = hexscope_core::png::fields::Ihdr {
            width: w,
            height: h,
            bit_depth: depth as u8,
            color_type: color as u8,
            interlace: interlace as u8,
        };
        // Each pass's rows: where the first starts, its length, how many.
        let rows: Vec<(usize, usize, usize)> = if interlace == 1 {
            let Some(passes) = hexscope_core::png::adam7::passes(&ihdr) else {
                return Vec::new();
            };
            passes
                .iter()
                .filter(|p| p.width > 0 && p.height > 0)
                .map(|p| (p.start, p.stride + 1, p.height as usize))
                .collect()
        } else {
            let Some(row) = ihdr.stride().and_then(|s| s.checked_add(1)) else {
                return Vec::new();
            };
            vec![(0, row, h as usize)]
        };
        rows.iter()
            .flat_map(|&(start, row, n)| {
                (0..n).map_while(move |r| self.output.get(start + r * row).copied())
            })
            .collect()
    }

    /// The step that wrote output byte `pos` (`by` 0) or read DEFLATE bit
    /// `pos` (`by` 1), as `[index, kind, a, b, bitStart, bitEnd, outStart]`
    /// in the layout of [`Parsed::steps`]; empty when no step does. Resumes
    /// from the nearest checkpoint, so it costs one interval at most.
    pub fn locate(&self, by: u8, pos: f64) -> Vec<f64> {
        let Some(body) = self.body() else {
            return Vec::new();
        };
        if !(pos.is_finite() && pos >= 0.0) {
            return Vec::new();
        }
        let pos = pos as u64;
        let before = |c: &Checkpoint| {
            if by == 0 {
                c.out_pos <= pos
            } else {
                c.bit_pos <= pos
            }
        };
        let nearest = self.checkpoints.partition_point(before);
        let mut d = nearest
            .checked_sub(1)
            .and_then(|i| self.checkpoints.get(i))
            .and_then(|cp| Decoder::resume(body, MAX_PIXEL_BYTES, &self.output, cp).ok())
            .unwrap_or_else(|| Decoder::new(body, MAX_PIXEL_BYTES));
        while let Some(Ok(step)) = d.step() {
            let produced = match step.event {
                InflateEvent::Literal { .. } => 1,
                InflateEvent::Match { length, .. } => length as u64,
                _ => 0,
            };
            let (start, end, done) = if by == 0 {
                (
                    step.out_start,
                    step.out_start + produced,
                    step.out_start > pos,
                )
            } else {
                (step.bit_start, step.bit_end, step.bit_start > pos)
            };
            if done {
                break;
            }
            if start <= pos && pos < end {
                let mut out = vec![step.index as f64];
                encode(&step, &mut out);
                return out;
            }
        }
        Vec::new()
    }

    /// Up to `count` DEFLATE steps starting at step `from`, flattened with
    /// [`STEP_STRIDE`] numbers per step:
    /// `[kind, a, b, bitStart, bitEnd, outStart]`.
    ///
    /// | kind | meaning        | a                              | b      |
    /// |------|----------------|--------------------------------|--------|
    /// | 0    | block start    | 0 stored, 1 fixed, 2 dynamic   | —      |
    /// | 1    | literal        | the byte                       | —      |
    /// | 2    | back-reference | distance                       | length |
    /// | 3    | block end      | —                              | —      |
    /// | 4    | decoding failed here; always the last step          |
    ///
    /// Bits count from the start of the DEFLATE data, after the zlib header.
    pub fn steps(&self, from: f64, count: u32) -> Vec<f64> {
        let Some(body) = self.body() else {
            return Vec::new();
        };
        let from = if from.is_finite() && from > 0.0 {
            from as u64
        } else {
            0
        };
        let wanted = count as usize * STEP_STRIDE;
        let mut decoder = self.decoder_at(body, from);

        // `count` comes from the caller: reserve for a normal batch, not for
        // whatever was asked, or a huge count aborts the worker before a
        // single step is decoded. The vector still grows to fit.
        let mut out = Vec::with_capacity(wanted.min(4096 * STEP_STRIDE));
        let mut last_end = 0u64;
        while out.len() < wanted {
            match decoder.step() {
                None => break,
                Some(Ok(step)) => {
                    last_end = step.bit_end;
                    if step.index >= from {
                        encode(&step, &mut out);
                    }
                }
                Some(Err(_)) => {
                    let at = decoder.output().len() as f64;
                    out.extend_from_slice(&[
                        4.0,
                        0.0,
                        0.0,
                        last_end as f64,
                        decoder.bit_pos() as f64,
                        at,
                    ]);
                    break;
                }
            }
        }
        out
    }

    /// What the file is made of: [`SLICE_STRIDE`] numbers per slice,
    /// `[start, len, role, node]`, covering every byte in order. Roles: 0
    /// content, 1 metadata, 2 thumbnail, 3 structure, 4 hidden, 5 damaged;
    /// node -1 where no node covers the bytes.
    #[wasm_bindgen(getter)]
    pub fn composition(&self) -> Vec<f64> {
        self.composition.clone()
    }

    /// Per node, an index into the explanation tables below, or -1.
    #[wasm_bindgen(getter, js_name = docIds)]
    pub fn doc_ids(&self) -> Vec<i32> {
        self.docs.ids.clone()
    }

    /// Each explanation's sentence and citation, as a key into what the
    /// page fetches (hexscope-core's `docs::every`).
    #[wasm_bindgen(getter, js_name = docKeys)]
    pub fn doc_keys(&self) -> Vec<u32> {
        self.docs.keys.clone()
    }

    /// Per explanation: 0 not a problem, 1 damage, 2 something hidden,
    /// 3 an oddity.
    #[wasm_bindgen(getter, js_name = docConcerns)]
    pub fn doc_concerns(&self) -> Vec<u8> {
        self.docs.concerns.clone()
    }

    /// Per ZIP entry, [`ENTRY_STRIDE`] numbers: `[node, method, flags,
    /// compressed, uncompressed, playable, openable]`. Empty for other formats.
    #[wasm_bindgen(getter)]
    pub fn entries(&self) -> Vec<f64> {
        let Some(zip) = &self.zip else {
            return Vec::new();
        };
        zip.entries
            .iter()
            .flat_map(|e| {
                [
                    e.node as f64,
                    e.method as f64,
                    e.flags as f64,
                    e.compressed as f64,
                    e.uncompressed as f64,
                    if playable(e) { 1.0 } else { 0.0 },
                    if openable(e) { 1.0 } else { 0.0 },
                ]
            })
            .collect()
    }

    /// Bytes of wrapper before the DEFLATE data in the stream the segments
    /// describe: compressed bit `b` lives in stream byte `streamHeader + b / 8`.
    #[wasm_bindgen(getter, js_name = streamHeader)]
    pub fn stream_header(&self) -> f64 {
        self.header_len as f64
    }

    /// ZIP entry `index`, decompressed and checked, to be parsed as a file
    /// of its own. Empty on failure, with the reason in `extractError`.
    #[wasm_bindgen(js_name = extractEntry)]
    pub fn extract_entry(&mut self, index: u32) -> Vec<u8> {
        self.extract_error.clear();
        // Each family's way in only where the build reads that family.
        let attached = !self.attachment_names.is_empty();
        let result = match &self.zip {
            None if cfg!(feature = "mail") && attached && self.format == "eml" => {
                hexscope_core::eml::attachment_bytes(&self.pdf_source, index as usize)
                    .map_err(str::to_string)
            }
            None if cfg!(feature = "office") && attached && self.format == "msg" => {
                hexscope_core::cfb::attachment_bytes(&self.pdf_source, index as usize)
                    .map_err(str::to_string)
            }
            None if cfg!(feature = "pdf") && attached && self.format == "pdf" => {
                hexscope_core::pdf::attachment_bytes(&self.pdf_source, index as usize)
                    .map_err(str::to_string)
            }
            None => Err("this file is not an archive".to_string()),
            Some(_) if !cfg!(feature = "office") => {
                Err("this module opens no archives".to_string())
            }
            Some(zip) => match zip.entries.get(index as usize) {
                None => Err(format!("there is no entry {index}")),
                Some(e) if !openable(e) => Err("this entry cannot be opened".to_string()),
                Some(e) => extract(&zip.source, e, MAX_PIXEL_BYTES).map_err(extract_message),
            },
        };
        result.unwrap_or_else(|reason| {
            self.extract_error = reason;
            Vec::new()
        })
    }

    #[wasm_bindgen(getter, js_name = extractError)]
    pub fn extract_error(&self) -> String {
        self.extract_error.clone()
    }

    /// Makes ZIP entry `index` the stream the player steps through: its data
    /// is decoded once, with checkpoints, as a PNG's IDAT stream is at parse
    /// time. Returns false, changing nothing, if the entry cannot be played.
    #[wasm_bindgen(js_name = selectEntry)]
    pub fn select_entry(&mut self, index: u32) -> bool {
        if !cfg!(feature = "office") {
            return false;
        }
        let Some(zip) = &self.zip else {
            return false;
        };
        let Some(e) = zip.entries.get(index as usize).filter(|e| playable(e)) else {
            return false;
        };
        let (start, end) = (e.data.start as usize, e.data.end() as usize);
        let Some(data) = zip.source.get(start..end) else {
            return false;
        };
        let data = data.to_vec();
        let segment = [e.data.start as f64, e.data.len as f64];

        let mut sink = CheckpointSink::new(CHECKPOINT_INTERVAL);
        let result = inflate(&data, MAX_PIXEL_BYTES, &mut sink);
        let summary = sink.finish();
        self.output = match result {
            Ok(out) => out,
            // Replay to the failure so the player has real bytes to draw.
            Err(_) => {
                let mut d = Decoder::new(&data, MAX_PIXEL_BYTES);
                while let Some(Ok(_)) = d.step() {}
                d.into_output()
            }
        };
        self.trace = Some([
            summary.total_events as f64,
            summary.literals as f64,
            summary.matches as f64,
            summary.output_bytes as f64,
        ]);
        self.checkpoints = summary.checkpoints;
        self.segments = segment.to_vec();
        self.idat_bytes = segment[1];
        self.stream = data;
        self.header_len = 0;
        self.trailer_len = 0;
        true
    }

    /// What step `index` read, part by part. Layout: `[block_start, error,
    /// n, then n × (kind, bit_start, bit_end, value, code, code_len)]`,
    /// `error` -1 when the step decoded. Empty when there is no such step.
    pub fn explain(&self, index: f64) -> Vec<f64> {
        let Some(e) = self
            .decoder_before(index)
            .and_then(|mut d| d.explain_next())
        else {
            return Vec::new();
        };
        let mut out = Vec::with_capacity(3 + e.parts.len() * PART_STRIDE);
        out.push(e.block_start as f64);
        out.push(e.error.map_or(-1.0, error_code));
        out.push(e.parts.len() as f64);
        for p in &e.parts {
            out.extend_from_slice(&[
                p.kind as u8 as f64,
                p.bit_start as f64,
                p.bit_end as f64,
                p.value as f64,
                p.code as f64,
                p.code_len as f64,
            ]);
        }
        out
    }

    /// The code tables of the block holding step `index`. Layout: `[kind,
    /// hlit, hdist, hclen, then for literal/length and for distance: groups,
    /// then per group len, first_code, count, symbols…]`. Empty for a stored
    /// block or when there is no such step.
    pub fn tables(&self, index: f64) -> Vec<f64> {
        let Some(mut d) = self.decoder_before(index) else {
            return Vec::new();
        };
        // A block's codes are in place before its steps; a header step builds
        // them, so for it they appear only after.
        let tables = d.tables().or_else(|| {
            d.step();
            d.tables()
        });
        let Some(t) = tables else {
            return Vec::new();
        };
        let h = t.header;
        let mut out = vec![
            block_kind_code(t.kind),
            h.map_or(0.0, |h| h.hlit as f64),
            h.map_or(0.0, |h| h.hdist as f64),
            h.map_or(0.0, |h| h.hclen as f64),
        ];
        for groups in [&t.lit_len, &t.distance] {
            out.push(groups.len() as f64);
            for g in groups {
                out.extend_from_slice(&[g.len as f64, g.first_code as f64, g.symbols.len() as f64]);
                out.extend(g.symbols.iter().map(|&s| s as f64));
            }
        }
        out
    }
}
