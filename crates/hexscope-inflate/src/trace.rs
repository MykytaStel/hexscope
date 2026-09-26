use crate::engine::{BlockKind, Checkpoint, EventSink, InflateEvent, Step};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TraceSummary {
    pub total_events: u64,
    pub literals: u64,
    pub matches: u64,
    /// Total decompressed size. Paired with the compressed size by the caller
    /// to show a ratio.
    pub output_bytes: u64,
    pub checkpoints: Vec<Checkpoint>,
    /// Every block the stream opened, in order.
    pub blocks: Vec<BlockSpan>,
    /// Where the last successful step ended. After a failure, the damage lies
    /// here or just after.
    pub last_bit: u64,
}

/// Where one DEFLATE block lies in the compressed stream, in bits.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BlockSpan {
    pub kind: BlockKind,
    /// The block header's first bit.
    pub start_bit: u64,
    /// Just past the end-of-block code; `None` if decoding stopped inside it.
    pub end_bit: Option<u64>,
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
    literals: u64,
    matches: u64,
    checkpoints: Vec<Checkpoint>,
    blocks: Vec<BlockSpan>,
    last_bit: u64,
}

impl CheckpointSink {
    /// A checkpoint every `interval` steps; an interval of 0 is taken as 1.
    pub fn new(interval: u64) -> Self {
        Self {
            interval: interval.max(1),
            index: 0,
            out_pos: 0,
            literals: 0,
            matches: 0,
            checkpoints: Vec::new(),
            blocks: Vec::new(),
            last_bit: 0,
        }
    }

    pub fn finish(self) -> TraceSummary {
        TraceSummary {
            total_events: self.index,
            literals: self.literals,
            matches: self.matches,
            output_bytes: self.out_pos,
            checkpoints: self.checkpoints,
            blocks: self.blocks,
            last_bit: self.last_bit,
        }
    }
}

impl EventSink for CheckpointSink {
    fn emit(&mut self, step: &Step) {
        // Each checkpoint is the state just before a step, taken straight from
        // the decoder, so its bit and output positions describe one moment.
        // The start of the stream is implicit and not recorded.
        if step.index > 0 && step.index.is_multiple_of(self.interval) {
            self.checkpoints.push(step.checkpoint());
        }

        match step.event {
            InflateEvent::Literal { .. } => {
                self.literals += 1;
                self.out_pos += 1;
            }
            InflateEvent::Match { length, .. } => {
                self.matches += 1;
                self.out_pos += length as u64;
            }
            InflateEvent::BlockStart { kind, .. } => self.blocks.push(BlockSpan {
                kind,
                start_bit: step.bit_start,
                end_bit: None,
            }),
            InflateEvent::BlockEnd => {
                if let Some(block) = self.blocks.last_mut() {
                    block.end_bit = Some(step.bit_end);
                }
            }
        }
        self.last_bit = step.bit_end;
        self.index += 1;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::inflate;
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

        // One checkpoint before each of steps 512, 1024, ... that exists.
        let expected = (summary.total_events - 1) / 512;
        assert_eq!(
            summary.checkpoints.len() as u64,
            expected,
            "{} checkpoints for {} events",
            summary.checkpoints.len(),
            summary.total_events
        );
        for (i, cp) in summary.checkpoints.iter().enumerate() {
            assert_eq!(cp.event_index, (i as u64 + 1) * 512);
        }
    }

    #[test]
    fn checkpoints_record_increasing_positions() {
        let compressed = deflate(&corpus());
        let mut sink = CheckpointSink::new(256);
        inflate(&compressed, u64::MAX, &mut sink).unwrap();
        let summary = sink.finish();

        assert!(summary.checkpoints.len() > 5);
        for pair in summary.checkpoints.windows(2) {
            assert!(pair[1].event_index > pair[0].event_index);
            assert!(pair[1].out_pos > pair[0].out_pos);
            // The defect this guards against: bit_pos used to be the enclosing
            // block's start, so every checkpoint in a one-block stream carried
            // the same value and none could be resumed from.
            assert!(
                pair[1].bit_pos > pair[0].bit_pos,
                "bit_pos stalled at {} between events {} and {}",
                pair[0].bit_pos,
                pair[0].event_index,
                pair[1].event_index
            );
        }
    }

    #[test]
    fn every_recorded_checkpoint_can_be_resumed() {
        let input = corpus();
        let compressed = deflate(&input);
        let mut sink = CheckpointSink::new(300);
        let out = inflate(&compressed, u64::MAX, &mut sink).unwrap();

        for cp in sink.finish().checkpoints {
            let mut d = crate::Decoder::resume(&compressed, u64::MAX, &out, &cp)
                .unwrap_or_else(|e| panic!("checkpoint {cp:?} failed: {e:?}"));
            while let Some(step) = d.step() {
                step.unwrap();
            }
            assert_eq!(d.into_output(), out, "resume from {cp:?} diverged");
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
