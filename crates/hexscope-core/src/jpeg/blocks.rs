//! Where each block of a JPEG's picture is in the file.
//!
//! A JPEG's pixels have no bytes of their own: the picture is cut into
//! minimum coded units — MCUs, usually 16×16 pixels — and each is coded as a
//! run of Huffman-coded bits. To know which bits are which block, the scan
//! is entropy-decoded: every Huffman code read, every coefficient's extra
//! bits skipped, no inverse DCT. That gives each MCU's first bit, and so the
//! bytes of the file that make any part of the picture (ITU T.81, F.2.2).
//!
//! Baseline and extended sequential JPEGs with Huffman coding are read.
//! Progressive and arithmetic-coded ones spread a block over several scans,
//! and are left alone.

/// Scan data bigger than this is not mapped: 64 MB of entropy-coded data
/// is far beyond any photo.
const MAX_SCAN: usize = 64 * 1024 * 1024;

/// One Huffman table, as T.81 Annex C builds it: for each code length, the
/// first code and where its symbols start.
#[derive(Clone, Default)]
struct Table {
    /// Largest code of each length, or -1 when there is none (T.81 F.2.2.3).
    maxcode: [i32; 17],
    /// Index into `symbols` of each length's first code, less that code.
    offset: [i32; 17],
    symbols: Vec<u8>,
}

impl Table {
    fn new(counts: &[u8; 16], symbols: &[u8]) -> Self {
        let mut t = Table {
            maxcode: [-1; 17],
            offset: [0; 17],
            symbols: symbols.to_vec(),
        };
        let (mut code, mut k) = (0i32, 0i32);
        for len in 1..=16 {
            let n = counts[len - 1] as i32;
            if n > 0 {
                t.offset[len] = k - code;
                code += n;
                k += n;
                t.maxcode[len] = code - 1;
            }
            code <<= 1;
        }
        t
    }
}

struct Component {
    id: u8,
    h: u8,
    v: u8,
}

/// The file's MCUs: their size in pixels, the grid, and where each starts.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BlockMap {
    /// An MCU's size in pixels.
    pub mcu_width: u32,
    pub mcu_height: u32,
    /// MCUs across and down.
    pub columns: u32,
    pub rows: u32,
    /// Blocks of 8×8 samples in each MCU, over all components.
    pub blocks_per_mcu: u32,
    /// The file bit (byte × 8 + bit, from the most significant) at which
    /// each MCU begins, then the bit after the last one read.
    pub starts: Vec<u64>,
    /// Why the map stops short of the whole picture, if it does.
    pub stopped: Option<&'static str>,
}

/// Why no map was made.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NoMap {
    /// Progressive or arithmetic-coded: blocks are spread over scans.
    Unsupported,
    /// The headers a scan needs are missing or broken.
    Unreadable,
}

impl NoMap {
    pub fn reason(self) -> &'static str {
        match self {
            NoMap::Unsupported => {
                "its blocks are spread over several scans (a progressive JPEG), which this view does not follow yet"
            }
            NoMap::Unreadable => "its headers are damaged, so its blocks cannot be found",
        }
    }
}

/// Reads bits of entropy-coded data, skipping stuffed zero bytes, and knows
/// the file position of the next bit.
struct Bits<'a> {
    data: &'a [u8],
    /// The byte holding the next bit, and how many of its bits are used.
    pos: usize,
    used: u8,
    /// A marker was met: no more data in this interval.
    at_marker: bool,
}

impl Bits<'_> {
    /// The file position of the next bit.
    fn here(&self) -> u64 {
        self.pos as u64 * 8 + self.used as u64
    }

    fn bit(&mut self) -> Option<u32> {
        if self.at_marker {
            return None;
        }
        let byte = *self.data.get(self.pos)?;
        if byte == 0xFF && self.used == 0 {
            match self.data.get(self.pos + 1) {
                Some(0x00) => {}
                _ => {
                    self.at_marker = true;
                    return None;
                }
            }
        }
        let bit = (byte >> (7 - self.used)) & 1;
        self.used += 1;
        if self.used == 8 {
            self.used = 0;
            // A stuffed zero after 0xFF is not data.
            self.pos += if byte == 0xFF { 2 } else { 1 };
        }
        Some(bit as u32)
    }

    fn skip(&mut self, n: u32) -> Option<()> {
        for _ in 0..n {
            self.bit()?;
        }
        Some(())
    }

    fn decode(&mut self, t: &Table) -> Option<u8> {
        let mut code = 0i32;
        for len in 1..=16 {
            code = (code << 1) | self.bit()? as i32;
            if code <= t.maxcode[len] {
                return t.symbols.get((code + t.offset[len]) as usize).copied();
            }
        }
        None
    }

    /// After a restart interval: to the next byte, past the RST marker.
    fn restart(&mut self) -> bool {
        if self.used > 0 {
            self.used = 0;
            self.pos += if self.data.get(self.pos) == Some(&0xFF) {
                2
            } else {
                1
            };
        }
        self.at_marker = false;
        if self.data.get(self.pos) == Some(&0xFF)
            && self
                .data
                .get(self.pos + 1)
                .is_some_and(|m| (0xD0..=0xD7).contains(m))
        {
            self.pos += 2;
            true
        } else {
            false
        }
    }
}

fn be16(data: &[u8], at: usize) -> Option<usize> {
    Some(u16::from_be_bytes([*data.get(at)?, *data.get(at + 1)?]) as usize)
}

/// Maps the first scan of a sequential Huffman-coded JPEG.
pub fn block_map(data: &[u8]) -> Result<BlockMap, NoMap> {
    let mut dc: [Option<Table>; 4] = Default::default();
    let mut ac: [Option<Table>; 4] = Default::default();
    let mut components: Vec<Component> = Vec::new();
    let (mut width, mut height) = (0u32, 0u32);
    let mut interval = 0usize;
    let mut pos = 2;
    if data.get(..2) != Some(&[0xFF, 0xD8]) {
        return Err(NoMap::Unreadable);
    }
    loop {
        // Fill bytes may come before a marker.
        while data.get(pos) == Some(&0xFF) && data.get(pos + 1) == Some(&0xFF) {
            pos += 1;
        }
        if data.get(pos) != Some(&0xFF) {
            return Err(NoMap::Unreadable);
        }
        let marker = *data.get(pos + 1).ok_or(NoMap::Unreadable)?;
        let len = be16(data, pos + 2).ok_or(NoMap::Unreadable)?;
        let body = data.get(pos + 4..pos + 2 + len).ok_or(NoMap::Unreadable)?;
        if len < 2 {
            return Err(NoMap::Unreadable);
        }
        match marker {
            0xC4 => {
                let mut b = body;
                while b.len() >= 17 {
                    let (class, id) = (b[0] >> 4, (b[0] & 3) as usize);
                    let counts: [u8; 16] = b[1..17].try_into().map_err(|_| NoMap::Unreadable)?;
                    let n: usize = counts.iter().map(|&c| c as usize).sum();
                    let symbols = b.get(17..17 + n).ok_or(NoMap::Unreadable)?;
                    let table = Some(Table::new(&counts, symbols));
                    if class == 0 {
                        dc[id] = table;
                    } else {
                        ac[id] = table;
                    }
                    b = &b[17 + n..];
                }
            }
            0xC0 | 0xC1 => {
                height = be16(body, 1).ok_or(NoMap::Unreadable)? as u32;
                width = be16(body, 3).ok_or(NoMap::Unreadable)? as u32;
                let n = *body.get(5).ok_or(NoMap::Unreadable)? as usize;
                components = (0..n)
                    .map(|i| {
                        let c = body.get(6 + i * 3..9 + i * 3)?;
                        Some(Component {
                            id: c[0],
                            h: (c[1] >> 4).max(1),
                            v: (c[1] & 15).max(1),
                        })
                    })
                    .collect::<Option<_>>()
                    .ok_or(NoMap::Unreadable)?;
            }
            0xC2 | 0xC3 | 0xC5..=0xC7 | 0xC9..=0xCB | 0xCD..=0xCF => {
                return Err(NoMap::Unsupported);
            }
            0xDD => interval = be16(body, 0).ok_or(NoMap::Unreadable)?,
            0xDA => {
                return scan(
                    data,
                    pos + 2 + len,
                    body,
                    &components,
                    width,
                    height,
                    interval,
                    &dc,
                    &ac,
                );
            }
            0xD9 => return Err(NoMap::Unreadable),
            _ => {}
        }
        pos += 2 + len;
    }
}

#[allow(clippy::too_many_arguments)]
fn scan(
    data: &[u8],
    start: usize,
    sos: &[u8],
    components: &[Component],
    width: u32,
    height: u32,
    interval: usize,
    dc: &[Option<Table>; 4],
    ac: &[Option<Table>; 4],
) -> Result<BlockMap, NoMap> {
    if components.is_empty() || width == 0 || height == 0 {
        return Err(NoMap::Unreadable);
    }
    let n = *sos.first().ok_or(NoMap::Unreadable)? as usize;
    let hmax = components.iter().map(|c| c.h).max().unwrap_or(1) as u32;
    let vmax = components.iter().map(|c| c.v).max().unwrap_or(1) as u32;
    // For each component in the scan: its sampling and its tables.
    let mut parts = Vec::new();
    for i in 0..n {
        let sel = sos.get(1 + i * 2..3 + i * 2).ok_or(NoMap::Unreadable)?;
        let c = components
            .iter()
            .find(|c| c.id == sel[0])
            .ok_or(NoMap::Unreadable)?;
        let d = dc[(sel[1] >> 4) as usize & 3]
            .as_ref()
            .ok_or(NoMap::Unreadable)?;
        let a = ac[(sel[1] & 15) as usize & 3]
            .as_ref()
            .ok_or(NoMap::Unreadable)?;
        parts.push((c.h as u32, c.v as u32, d, a));
    }
    // One component alone is coded block by block; several, interleaved in
    // MCUs of every component's blocks (T.81 A.2).
    let (mcu_w, mcu_h, blocks) = if n == 1 {
        let (h, v, ..) = parts[0];
        parts[0].0 = 1;
        parts[0].1 = 1;
        (8 * hmax / h, 8 * vmax / v, 1)
    } else {
        let blocks: u32 = parts.iter().map(|p| p.0 * p.1).sum();
        (8 * hmax, 8 * vmax, blocks)
    };
    let columns = width.div_ceil(mcu_w);
    let rows = height.div_ceil(mcu_h);
    let total = columns as usize * rows as usize;

    let end = (start + MAX_SCAN).min(data.len());
    let mut bits = Bits {
        data: &data[..end],
        pos: start,
        used: 0,
        at_marker: false,
    };
    let mut starts = Vec::with_capacity(total + 1);
    let mut stopped = None;
    'mcus: for i in 0..total {
        if interval > 0 && i > 0 && i % interval == 0 && !bits.restart() {
            stopped = Some("a restart marker is missing");
            break;
        }
        starts.push(bits.here());
        for &(h, v, d, a) in &parts {
            for _ in 0..h * v {
                let block = (|| {
                    let s = bits.decode(d)?;
                    bits.skip(s as u32)?;
                    let mut k = 1;
                    while k < 64 {
                        let rs = bits.decode(a)?;
                        let (r, s) = (rs >> 4, rs & 15);
                        if s == 0 {
                            if r != 15 {
                                break;
                            }
                            k += 16;
                        } else {
                            k += r as usize;
                            bits.skip(s as u32)?;
                            k += 1;
                        }
                    }
                    Some(())
                })();
                if block.is_none() {
                    starts.pop();
                    stopped = Some("the scan data ends, or breaks, before the last block");
                    break 'mcus;
                }
            }
        }
    }
    starts.push(bits.here());
    Ok(BlockMap {
        mcu_width: mcu_w,
        mcu_height: mcu_h,
        columns,
        rows,
        blocks_per_mcu: blocks,
        starts,
        stopped,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_huffman_table_decodes_its_codes() {
        // Lengths: two 2-bit codes (00, 01), one 3-bit code (100).
        let mut counts = [0u8; 16];
        counts[1] = 2;
        counts[2] = 1;
        let t = Table::new(&counts, &[7, 8, 9]);
        let data = [0b0001_1000u8];
        let mut b = Bits {
            data: &data,
            pos: 0,
            used: 0,
            at_marker: false,
        };
        assert_eq!(b.decode(&t), Some(7));
        assert_eq!(b.decode(&t), Some(8));
        assert_eq!(b.decode(&t), Some(9));
        assert_eq!(b.here(), 7);
    }

    #[test]
    fn stuffed_zeros_are_not_data_and_markers_stop() {
        let data = [0xFF, 0x00, 0xAA, 0xFF, 0xD9];
        let mut b = Bits {
            data: &data,
            pos: 0,
            used: 0,
            at_marker: false,
        };
        for _ in 0..8 {
            assert_eq!(b.bit(), Some(1));
        }
        assert_eq!(
            b.here(),
            16,
            "the next bit is in the byte after the stuffed zero"
        );
        b.skip(8).unwrap();
        assert_eq!(b.bit(), None);
    }

    fn fixture(name: &str) -> Vec<u8> {
        std::fs::read(format!(
            "{}/../../apps/web/public/samples/{name}",
            env!("CARGO_MANIFEST_DIR")
        ))
        .unwrap()
    }

    #[test]
    fn every_block_of_the_sample_photo_is_found() {
        let data = fixture("photo.jpg");
        let m = block_map(&data).unwrap();
        assert_eq!(m.stopped, None);
        assert_eq!(m.columns * m.mcu_width >= 640, true);
        assert_eq!((m.columns * m.rows) as usize + 1, m.starts.len());
        assert!(
            m.starts.windows(2).all(|w| w[0] < w[1]),
            "blocks take bits, in order"
        );
        // The last block ends at the EOI marker, give or take padding bits.
        let eoi = data.windows(2).rposition(|w| w == [0xFF, 0xD9]).unwrap() as u64;
        let end = *m.starts.last().unwrap();
        assert!(
            end <= eoi * 8 && eoi * 8 - end < 8,
            "ends at {end}, EOI at {}",
            eoi * 8
        );
    }

    #[test]
    fn broken_and_progressive_files_say_why() {
        assert_eq!(block_map(b"not a jpeg"), Err(NoMap::Unreadable));
        let data = fixture("photo.jpg");
        for cut in [3, 100, 600] {
            let _ = block_map(&data[..cut]);
        }
        let cut = block_map(&data[..data.len() / 2]).unwrap();
        assert!(cut.stopped.is_some());
    }
}
