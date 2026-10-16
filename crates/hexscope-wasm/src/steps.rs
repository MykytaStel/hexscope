//! The DEFLATE player's steps, decoded on demand from the nearest checkpoint
//! and written as numbers for the page.

use super::*;

impl Parsed {
    /// A decoder whose next step is `index`, or `None` when decoding stops
    /// before reaching it.
    pub(crate) fn decoder_before(&self, index: f64) -> Option<Decoder<'_>> {
        let body = self.body()?;
        if !(index.is_finite() && index >= 0.0) {
            return None;
        }
        let index = index as u64;
        let mut d = self.decoder_at(body, index);
        while d.next_index() < index {
            d.step()?.ok()?;
        }
        Some(d)
    }

    /// The DEFLATE data: the zlib stream minus its 2-byte header and 4-byte
    /// Adler-32 trailer — exactly what the core decompressed.
    pub(crate) fn body(&self) -> Option<&[u8]> {
        let end = self.stream.len().checked_sub(self.trailer_len)?;
        self.stream.get(self.header_len..end)
    }

    /// A decoder positioned at or shortly before step `from`, resumed from the
    /// nearest checkpoint so seeking anywhere in a large file replays at most
    /// one checkpoint interval. Checkpoints recorded before a failure are
    /// still valid, since the partial output backs them.
    pub(crate) fn decoder_at<'a>(&'a self, body: &'a [u8], from: u64) -> Decoder<'a> {
        let nearest = self.checkpoints.partition_point(|c| c.event_index <= from);
        if let Some(cp) = nearest.checked_sub(1).and_then(|i| self.checkpoints.get(i))
            && let Ok(d) = Decoder::resume(body, MAX_PIXEL_BYTES, &self.output, cp)
        {
            return d;
        }
        Decoder::new(body, MAX_PIXEL_BYTES)
    }
}

pub(crate) fn block_kind_code(kind: BlockKind) -> f64 {
    match kind {
        BlockKind::Stored => 0.0,
        BlockKind::Fixed => 1.0,
        BlockKind::Dynamic => 2.0,
    }
}

/// The error's number on the JS side, where `codes.ts` words it.
pub(crate) fn error_code(err: InflateError) -> f64 {
    match err {
        InflateError::Bits => 0.0,
        InflateError::BadHuffmanCode => 1.0,
        InflateError::BadCodeLengths => 2.0,
        InflateError::BadDistance => 3.0,
        InflateError::BadSymbol => 4.0,
        InflateError::BadZlibHeader => 5.0,
        InflateError::ChecksumMismatch => 6.0,
        InflateError::OutputTooLarge => 7.0,
        InflateError::BadCheckpoint => 8.0,
    }
}

pub(crate) fn encode(step: &Step, out: &mut Vec<f64>) {
    let (kind, a, b) = match step.event {
        InflateEvent::BlockStart { kind, .. } => (0.0, block_kind_code(kind), 0.0),
        InflateEvent::Literal { byte } => (1.0, byte as f64, 0.0),
        InflateEvent::Match { distance, length } => (2.0, distance as f64, length as f64),
        InflateEvent::BlockEnd => (3.0, 0.0, 0.0),
    };
    out.extend_from_slice(&[
        kind,
        a,
        b,
        step.bit_start as f64,
        step.bit_end as f64,
        step.out_start as f64,
    ]);
}
