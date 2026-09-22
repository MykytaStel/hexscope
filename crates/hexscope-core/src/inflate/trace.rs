use crate::inflate::engine::{EventSink, InflateEvent};

/// A resume point. Replaying from here needs the output produced so far, which
/// the caller already holds, plus the bit position in the compressed stream.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Checkpoint {
    pub event_index: u64,
    pub bit_pos: u64,
    pub out_pos: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TraceSummary {
    pub total_events: u64,
    pub literals: u64,
    pub matches: u64,
    /// Total decompressed size. Paired with the compressed size by the caller
    /// to show a ratio.
    pub output_bytes: u64,
    pub checkpoints: Vec<Checkpoint>,
}

impl TraceSummary {
    /// Events that produce output, i.e. excluding block markers.
    pub fn coding_events(&self) -> u64 {
        self.literals + self.matches
    }
}

/// Counts events and records a checkpoint every `interval` events, so the UI
/// can jump into the middle of a long stream without keeping every step.
pub struct CheckpointSink {
    interval: u64,
    index: u64,
    out_pos: u64,
    last_bit_pos: u64,
    literals: u64,
    matches: u64,
    checkpoints: Vec<Checkpoint>,
}

impl CheckpointSink {
    pub fn new(interval: u64) -> Self {
        assert!(interval > 0, "checkpoint interval must be positive");
        Self {
            interval,
            index: 0,
            out_pos: 0,
            last_bit_pos: 0,
            literals: 0,
            matches: 0,
            checkpoints: Vec::new(),
        }
    }

    pub fn finish(self) -> TraceSummary {
        TraceSummary {
            total_events: self.index,
            literals: self.literals,
            matches: self.matches,
            output_bytes: self.out_pos,
            checkpoints: self.checkpoints,
        }
    }
}

impl EventSink for CheckpointSink {
    fn emit(&mut self, event: InflateEvent) {
        match event {
            InflateEvent::BlockStart { bit_pos, .. } => self.last_bit_pos = bit_pos,
            InflateEvent::Literal { .. } => {
                self.literals += 1;
                self.out_pos += 1;
            }
            InflateEvent::Match { length, .. } => {
                self.matches += 1;
                self.out_pos += length as u64;
            }
            InflateEvent::BlockEnd => {}
        }

        self.index += 1;
        if self.index.is_multiple_of(self.interval) {
            self.checkpoints.push(Checkpoint {
                event_index: self.index,
                bit_pos: self.last_bit_pos,
                out_pos: self.out_pos,
            });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::inflate::engine::inflate;
    use flate2::Compression;
    use flate2::write::DeflateEncoder;
    use std::io::Write;

    fn deflate(input: &[u8]) -> Vec<u8> {
        let mut enc = DeflateEncoder::new(Vec::new(), Compression::best());
        enc.write_all(input).unwrap();
        enc.finish().unwrap()
    }

    fn corpus() -> Vec<u8> {
        let mut input = Vec::new();
        for i in 0..4000u32 {
            input.extend_from_slice(format!("row {} value {}\n", i, i % 31).as_bytes());
        }
        input
    }

    #[test]
    fn records_checkpoints_at_the_requested_interval() {
        let input = corpus();
        let compressed = deflate(&input);
        let mut sink = CheckpointSink::new(512);
        let out = inflate(&compressed, u64::MAX, &mut sink).unwrap();
        let summary = sink.finish();

        assert_eq!(out, input);
        assert!(summary.total_events > 2000, "got {}", summary.total_events);
        // The event stream must account for every decompressed byte.
        assert_eq!(summary.output_bytes, input.len() as u64);
        assert!(
            summary.matches > 0,
            "a repetitive corpus must produce matches"
        );

        // One checkpoint per interval, give or take the final partial one.
        let expected = summary.total_events / 512;
        assert!(
            summary.checkpoints.len() as u64 >= expected
                && summary.checkpoints.len() as u64 <= expected + 1,
            "{} checkpoints for {} events",
            summary.checkpoints.len(),
            summary.total_events
        );
    }

    #[test]
    fn checkpoints_record_increasing_positions() {
        let compressed = deflate(&corpus());
        let mut sink = CheckpointSink::new(256);
        inflate(&compressed, u64::MAX, &mut sink).unwrap();
        let summary = sink.finish();

        for pair in summary.checkpoints.windows(2) {
            assert!(pair[1].event_index > pair[0].event_index);
            assert!(pair[1].out_pos >= pair[0].out_pos);
        }
    }

    #[test]
    fn an_empty_stream_produces_no_checkpoints() {
        let compressed = deflate(b"");
        let mut sink = CheckpointSink::new(64);
        inflate(&compressed, u64::MAX, &mut sink).unwrap();
        assert!(sink.finish().checkpoints.is_empty());
    }
}
