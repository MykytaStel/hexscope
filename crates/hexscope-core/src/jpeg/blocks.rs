//! Where each block of a JPEG's picture is in the file.
//!
//! A JPEG's pixels have no bytes of their own: the picture is cut into
//! blocks of 8×8 samples, grouped in minimum coded units — MCUs, usually
//! 16×16 pixels — and each is coded as a run of Huffman-coded bits. To know
//! which bits are which block, each scan is entropy-decoded: every Huffman
//! code read, every coefficient's extra bits skipped, no inverse DCT. That
//! gives each unit's first bit, and so the bytes of the file that make any
//! part of the picture (ITU T.81, F.2.2).
//!
//! A sequential JPEG usually has one scan with every component. A
//! progressive one sends the picture in several: first each block's average
//! (DC), then bands of its detail (AC), often a few bits of precision at a
//! time (T.81 G.1). A block's bits are then spread over all of them, and an
//! end-of-band run can close many blocks at once, which leaves those blocks
//! no bits of their own in that scan. Every scan is mapped. Arithmetic-coded,
//! lossless and hierarchical files are left alone.

/// Scan data bigger than this is not mapped: 64 MB of entropy-coded data
/// is far beyond any photo.
const MAX_SCAN: usize = 64 * 1024 * 1024;
/// Pictures larger than this are not mapped: past any camera, and the
/// progressive bookkeeping grows with it (8 bytes a block).
const MAX_PIXELS: u64 = 200_000_000;
/// Units over all scans: the map's own size, 8 bytes each.
const MAX_UNITS: usize = 8_000_000;
/// Scans mapped at most; a progressive photo has about ten.
const MAX_SCANS: usize = 64;

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

struct Frame {
    width: u32,
    height: u32,
    components: Vec<Component>,
    progressive: bool,
}

/// One scan: what it codes, its units' size and grid, and where each starts.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScanMap {
    /// The frame's components it codes: bit `i` for the frame's `i`-th.
    pub components: u8,
    /// Spectral selection: the first and last coefficient it codes, in
    /// zig-zag order; 0 to 63 for a sequential scan, 0 to 0 for DC only.
    pub ss: u8,
    pub se: u8,
    /// Successive approximation: the bit sent before (0 on a first pass)
    /// and the bit it sends down to (T.81 G.1.1.1.2).
    pub ah: u8,
    pub al: u8,
    /// A unit's size in pixels: an MCU when the scan has several components,
    /// one block of its component when it has one.
    pub unit_width: u32,
    pub unit_height: u32,
    /// Units across and down.
    pub columns: u32,
    pub rows: u32,
    /// Blocks of 8×8 samples in each unit.
    pub blocks_per_unit: u32,
    /// The file bit (byte × 8 + bit, from the most significant) at which
    /// each unit begins, then the bit after the last one read.
    pub starts: Vec<u64>,
    /// The byte after its data, where the next marker starts.
    pub end: usize,
}

/// Every scan of the file, in file order.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BlockMap {
    pub progressive: bool,
    /// Components in the frame.
    pub components: u8,
    pub scans: Vec<ScanMap>,
    /// Why the map stops short of the whole file, if it does.
    pub stopped: Option<&'static str>,
}

/// Why no map was made.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NoMap {
    /// Arithmetic-coded, lossless or hierarchical.
    Unsupported,
    /// Larger than [`MAX_PIXELS`].
    TooLarge,
    /// The headers a scan needs are missing or broken.
    Unreadable,
}

impl NoMap {
    pub fn reason(self) -> &'static str {
        match self {
            NoMap::Unsupported => {
                "it is coded arithmetically or losslessly, which this view does not follow"
            }
            NoMap::TooLarge => "the picture is too large to map here",
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

    /// `n` bits as a number, most significant first.
    fn read(&mut self, n: u32) -> Option<u32> {
        let mut v = 0;
        for _ in 0..n {
            v = (v << 1) | self.bit()?;
        }
        Some(v)
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

/// What a scan codes, which decides how each block is read (T.81 G.1.2).
#[derive(Clone, Copy, PartialEq, Eq)]
enum Pass {
    /// Every coefficient at once.
    Sequential,
    /// The DC coefficient's first bits.
    DcFirst,
    /// One more bit of each DC coefficient.
    DcRefine,
    /// A band of AC coefficients' first bits.
    AcFirst,
    /// One more bit of a band of AC coefficients.
    AcRefine,
}

/// Maps every scan of a Huffman-coded JPEG, sequential or progressive.
pub fn block_map(data: &[u8]) -> Result<BlockMap, NoMap> {
    let mut dc: [Option<Table>; 4] = Default::default();
    let mut ac: [Option<Table>; 4] = Default::default();
    let mut frame: Option<Frame> = None;
    let mut interval = 0usize;
    // Per component, per block: which AC coefficients are already nonzero,
    // as bits by zig-zag index — what a refining scan needs to know.
    let mut nonzero: Vec<Vec<u64>> = Vec::new();
    let mut scans: Vec<ScanMap> = Vec::new();
    let mut units = 0usize;
    let mut stopped = None;
    let mut pos = 2;
    if data.get(..2) != Some(&[0xFF, 0xD8]) {
        return Err(NoMap::Unreadable);
    }
    // Damage before the first scan leaves nothing to show; after it, the
    // scans already read still stand.
    macro_rules! broken {
        ($why:expr) => {{
            if scans.is_empty() {
                return Err(NoMap::Unreadable);
            }
            stopped = Some($why);
            break;
        }};
    }
    loop {
        // Fill bytes may come before a marker.
        while data.get(pos) == Some(&0xFF) && data.get(pos + 1) == Some(&0xFF) {
            pos += 1;
        }
        if pos >= data.len() && !scans.is_empty() {
            break;
        }
        if data.get(pos) != Some(&0xFF) {
            broken!("the file breaks between two scans");
        }
        let Some(&marker) = data.get(pos + 1) else {
            broken!("the file breaks between two scans")
        };
        if marker == 0xD9 {
            break;
        }
        let Some(len) = be16(data, pos + 2).filter(|&l| l >= 2) else {
            broken!("the file breaks between two scans")
        };
        let Some(body) = data.get(pos + 4..pos + 2 + len) else {
            broken!("the file breaks between two scans")
        };
        match marker {
            0xC4 => {
                let mut b = body;
                while b.len() >= 17 {
                    let (class, id) = (b[0] >> 4, (b[0] & 3) as usize);
                    let counts: [u8; 16] = b[1..17].try_into().map_err(|_| NoMap::Unreadable)?;
                    let n: usize = counts.iter().map(|&c| c as usize).sum();
                    let Some(symbols) = b.get(17..17 + n) else {
                        broken!("a Huffman table is cut short")
                    };
                    let table = Some(Table::new(&counts, symbols));
                    if class == 0 {
                        dc[id] = table;
                    } else {
                        ac[id] = table;
                    }
                    b = &b[17 + n..];
                }
            }
            0xC0..=0xC2 => {
                let f = read_frame(body, marker == 0xC2).ok_or(NoMap::Unreadable)?;
                if f.width as u64 * f.height as u64 > MAX_PIXELS {
                    return Err(NoMap::TooLarge);
                }
                frame = Some(f);
            }
            0xC3 | 0xC5..=0xC7 | 0xC9..=0xCB | 0xCD..=0xCF => {
                return Err(NoMap::Unsupported);
            }
            0xDD => interval = be16(body, 0).ok_or(NoMap::Unreadable)?,
            0xDA => {
                let Some(f) = frame.as_ref() else {
                    return Err(NoMap::Unreadable);
                };
                if scans.len() == MAX_SCANS {
                    stopped = Some("it has more scans than are mapped here");
                    break;
                }
                let tables = Tables { dc: &dc, ac: &ac };
                let start = pos + 2 + len;
                let (map, short) =
                    match scan(data, start, body, f, interval, tables, &mut nonzero, units) {
                        Ok(done) => done,
                        Err(e) if scans.is_empty() => return Err(e),
                        Err(_) => broken!("a scan's header is damaged"),
                    };
                units += map.starts.len();
                pos = map.end;
                scans.push(map);
                if short.is_some() {
                    stopped = short;
                    break;
                }
                continue;
            }
            _ => {}
        }
        pos += 2 + len;
    }
    let f = frame.ok_or(NoMap::Unreadable)?;
    Ok(BlockMap {
        progressive: f.progressive,
        components: f.components.len() as u8,
        scans,
        stopped,
    })
}

fn read_frame(body: &[u8], progressive: bool) -> Option<Frame> {
    let height = be16(body, 1)? as u32;
    let width = be16(body, 3)? as u32;
    let n = *body.get(5)? as usize;
    let components = (0..n)
        .map(|i| {
            let c = body.get(6 + i * 3..9 + i * 3)?;
            Some(Component {
                id: c[0],
                h: (c[1] >> 4).max(1),
                v: (c[1] & 15).max(1),
            })
        })
        .collect::<Option<Vec<_>>>()?;
    if components.is_empty() || components.len() > 4 || width == 0 || height == 0 {
        return None;
    }
    Some(Frame {
        width,
        height,
        components,
        progressive,
    })
}

#[derive(Clone, Copy)]
struct Tables<'a> {
    dc: &'a [Option<Table>; 4],
    ac: &'a [Option<Table>; 4],
}

/// One component's part in a scan.
struct Part<'a> {
    /// Its place in the frame.
    index: usize,
    /// Blocks it has in each unit, across and down.
    h: u32,
    v: u32,
    dc: Option<&'a Table>,
    ac: Option<&'a Table>,
}

/// Maps one scan, whose data begins at `start`. Returns the map, and why it
/// stops short when it does.
#[allow(clippy::too_many_arguments)]
fn scan(
    data: &[u8],
    start: usize,
    sos: &[u8],
    frame: &Frame,
    interval: usize,
    tables: Tables,
    nonzero: &mut Vec<Vec<u64>>,
    units_so_far: usize,
) -> Result<(ScanMap, Option<&'static str>), NoMap> {
    let components = &frame.components;
    let n = *sos.first().ok_or(NoMap::Unreadable)? as usize;
    if n == 0 || n > components.len() {
        return Err(NoMap::Unreadable);
    }
    let tail = sos.get(1 + n * 2..4 + n * 2).ok_or(NoMap::Unreadable)?;
    let (ss, se, ah, al) = (tail[0], tail[1], tail[2] >> 4, tail[2] & 15);
    let pass = if !frame.progressive {
        Pass::Sequential
    } else {
        // A DC scan codes DC alone; an AC scan, one component (T.81 G.1.1.1.1).
        if se > 63 || ss > se || (ss == 0 && se != 0) || (ss > 0 && n != 1) {
            return Err(NoMap::Unreadable);
        }
        match (ss == 0, ah == 0) {
            (true, true) => Pass::DcFirst,
            (true, false) => Pass::DcRefine,
            (false, true) => Pass::AcFirst,
            (false, false) => Pass::AcRefine,
        }
    };
    let (ss, se) = if pass == Pass::Sequential {
        (0, 63)
    } else {
        (ss, se)
    };
    let hmax = components.iter().map(|c| c.h).max().unwrap_or(1) as u32;
    let vmax = components.iter().map(|c| c.v).max().unwrap_or(1) as u32;
    let mut parts = Vec::with_capacity(n);
    let mut mask = 0u8;
    for i in 0..n {
        let sel = sos.get(1 + i * 2..3 + i * 2).ok_or(NoMap::Unreadable)?;
        let index = components
            .iter()
            .position(|c| c.id == sel[0])
            .ok_or(NoMap::Unreadable)?;
        let c = &components[index];
        mask |= 1 << index;
        let dc = tables.dc[(sel[1] >> 4) as usize & 3].as_ref();
        let ac = tables.ac[(sel[1] & 15) as usize & 3].as_ref();
        let needs_dc = matches!(pass, Pass::Sequential | Pass::DcFirst);
        let needs_ac = matches!(pass, Pass::Sequential | Pass::AcFirst | Pass::AcRefine);
        if (needs_dc && dc.is_none()) || (needs_ac && ac.is_none()) {
            return Err(NoMap::Unreadable);
        }
        parts.push(Part {
            index,
            h: c.h as u32,
            v: c.v as u32,
            dc,
            ac,
        });
    }
    // One component alone is coded block by block; several, interleaved in
    // MCUs of every component's blocks (T.81 A.2).
    let (unit_w, unit_h, blocks) = if n == 1 {
        let p = &mut parts[0];
        let size = (8 * hmax / p.h, 8 * vmax / p.v);
        p.h = 1;
        p.v = 1;
        (size.0, size.1, 1)
    } else {
        (8 * hmax, 8 * vmax, parts.iter().map(|p| p.h * p.v).sum())
    };
    let columns = frame.width.div_ceil(unit_w);
    let rows = frame.height.div_ceil(unit_h);
    let total = columns as usize * rows as usize;
    if units_so_far + total + 1 > MAX_UNITS {
        return Err(NoMap::TooLarge);
    }
    // An AC scan has one component, and its units are that component's
    // blocks, so a unit's index is its block's.
    let refines = matches!(pass, Pass::AcFirst | Pass::AcRefine);
    if refines {
        if nonzero.len() < components.len() {
            nonzero.resize(components.len(), Vec::new());
        }
        let flags = &mut nonzero[parts[0].index];
        if flags.len() < total {
            flags.resize(total, 0);
        }
    }

    let end = (start + MAX_SCAN).min(data.len());
    let mut bits = Bits {
        data: &data[..end],
        pos: start,
        used: 0,
        at_marker: false,
    };
    let mut starts = Vec::with_capacity(total + 1);
    let mut stopped = None;
    let mut eobrun = 0u32;
    'units: for i in 0..total {
        if interval > 0 && i > 0 && i % interval == 0 {
            if !bits.restart() {
                stopped = Some("a restart marker is missing");
                break;
            }
            eobrun = 0;
        }
        starts.push(bits.here());
        for p in &parts {
            for _ in 0..p.h * p.v {
                let ok = match pass {
                    Pass::Sequential => sequential(&mut bits, p),
                    Pass::DcFirst => p.dc.and_then(|d| {
                        let s = bits.decode(d)?;
                        bits.skip(s as u32)
                    }),
                    Pass::DcRefine => bits.skip(1),
                    Pass::AcFirst | Pass::AcRefine => {
                        let band = Band { ss, se };
                        let flags = nonzero.get_mut(p.index).and_then(|f| f.get_mut(i));
                        flags.and_then(|flags| {
                            if pass == Pass::AcFirst {
                                ac_first(&mut bits, p.ac, band, &mut eobrun, flags)
                            } else {
                                ac_refine(&mut bits, p.ac, band, &mut eobrun, flags)
                            }
                        })
                    }
                };
                if ok.is_none() {
                    starts.pop();
                    stopped = Some("the scan data ends, or breaks, before the last block");
                    break 'units;
                }
            }
        }
    }
    starts.push(bits.here());
    // Past the padding, to the marker that ends the scan.
    let mut next = (bits.here() / 8) as usize;
    while let (Some(&a), Some(&b)) = (data.get(next), data.get(next + 1)) {
        if a == 0xFF && b != 0x00 && b != 0xFF && !(0xD0..=0xD7).contains(&b) {
            break;
        }
        next += 1;
    }
    let map = ScanMap {
        components: mask,
        ss,
        se,
        ah,
        al,
        unit_width: unit_w,
        unit_height: unit_h,
        columns,
        rows,
        blocks_per_unit: blocks,
        starts,
        end: next.min(data.len()),
    };
    Ok((map, stopped))
}

/// A sequential block: its DC difference, then its AC run-lengths.
fn sequential(bits: &mut Bits, p: &Part) -> Option<()> {
    let s = bits.decode(p.dc?)?;
    bits.skip(s as u32)?;
    let a = p.ac?;
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
}

/// The coefficients a progressive AC scan codes, in zig-zag order.
#[derive(Clone, Copy)]
struct Band {
    ss: u8,
    se: u8,
}

/// The first pass over a band of AC coefficients (T.81 G.1.2.2): as
/// sequential, but a run of whole blocks can end at once — EOBn.
fn ac_first(
    bits: &mut Bits,
    ac: Option<&Table>,
    band: Band,
    eobrun: &mut u32,
    nz: &mut u64,
) -> Option<()> {
    if *eobrun > 0 {
        *eobrun -= 1;
        return Some(());
    }
    let a = ac?;
    let mut k = band.ss as u32;
    while k <= band.se as u32 {
        let rs = bits.decode(a)?;
        let (r, s) = ((rs >> 4) as u32, (rs & 15) as u32);
        if s == 0 {
            if r < 15 {
                // This block, and 2^r - 1 + so many more, end here.
                *eobrun = (1 << r) - 1;
                if r > 0 {
                    *eobrun += bits.read(r)?;
                }
                break;
            }
            k += 16;
        } else {
            k += r;
            bits.skip(s)?;
            if k < 64 {
                *nz |= 1 << k;
            }
            k += 1;
        }
    }
    Some(())
}

/// A refining pass over a band (T.81 G.1.2.3): one correction bit for each
/// coefficient already nonzero, and the new ones, found by counting zeros.
fn ac_refine(
    bits: &mut Bits,
    ac: Option<&Table>,
    band: Band,
    eobrun: &mut u32,
    nz: &mut u64,
) -> Option<()> {
    let (ss, se) = (band.ss as u32, band.se as u32);
    let mut k = ss;
    if *eobrun == 0 {
        let a = ac?;
        while k <= se {
            let rs = bits.decode(a)?;
            let (mut r, s) = ((rs >> 4) as i32, (rs & 15) as u32);
            if s != 0 {
                // A new coefficient: its sign.
                bits.skip(1)?;
            } else if r != 15 {
                *eobrun = 1 << r;
                if r > 0 {
                    *eobrun += bits.read(r as u32)?;
                }
                break;
            }
            // Past r zeros — correcting the nonzero ones on the way — to
            // where the new coefficient goes.
            while k <= se {
                if *nz & (1 << k) != 0 {
                    bits.skip(1)?;
                } else {
                    if r == 0 {
                        break;
                    }
                    r -= 1;
                }
                k += 1;
            }
            if s != 0 && k < 64 {
                *nz |= 1 << k;
            }
            k += 1;
        }
    }
    if *eobrun > 0 {
        // Past the end of band: only corrections, one per nonzero coefficient left.
        let rest = if k > se {
            0
        } else {
            let upto = if se == 63 {
                u64::MAX
            } else {
                (1u64 << (se + 1)) - 1
            };
            *nz & upto & !((1u64 << k) - 1)
        };
        bits.skip(rest.count_ones())?;
        *eobrun -= 1;
    }
    Some(())
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

    /// Every scan's last unit ends at the marker after it, give or take
    /// padding bits: a decoder that lost its place would not.
    fn ends_at_markers(data: &[u8], m: &BlockMap) {
        for (n, s) in m.scans.iter().enumerate() {
            assert_eq!(
                (s.columns * s.rows) as usize + 1,
                s.starts.len(),
                "scan {n} is whole"
            );
            assert!(
                s.starts.windows(2).all(|w| w[0] <= w[1]),
                "scan {n} is in order"
            );
            let end = *s.starts.last().unwrap();
            let marker = s.end as u64 * 8;
            // Padding that makes a byte of 0xFF is followed by a stuffed zero.
            let last = (end / 8) as usize;
            let slack = if data[last] == 0xFF && data[last + 1] == 0 {
                16
            } else {
                8
            };
            assert!(
                end <= marker && marker - end < slack,
                "scan {n} ends at bit {end}, its marker at {marker}"
            );
            assert_eq!(data[s.end], 0xFF);
        }
    }

    #[test]
    fn every_block_of_the_sample_photo_is_found() {
        let data = fixture("photo.jpg");
        let m = block_map(&data).unwrap();
        assert_eq!(m.stopped, None);
        assert!(!m.progressive);
        assert_eq!(m.scans.len(), 1);
        let s = &m.scans[0];
        assert_eq!(
            (
                s.unit_width,
                s.unit_height,
                s.columns,
                s.rows,
                s.blocks_per_unit
            ),
            (16, 16, 40, 30, 6)
        );
        assert_eq!(s.components, 0b111);
        assert!(s.starts.windows(2).all(|w| w[0] < w[1]), "blocks take bits");
        ends_at_markers(&data, &m);
        assert_eq!(data[s.end + 1], 0xD9, "the one scan ends at EOI");
    }

    #[test]
    fn every_scan_of_a_progressive_photo_is_found() {
        let data = std::fs::read(format!(
            "{}/tests/fixtures/progressive.jpg",
            env!("CARGO_MANIFEST_DIR")
        ))
        .unwrap();
        let m = block_map(&data).unwrap();
        assert_eq!(m.stopped, None);
        assert!(m.progressive);
        assert!(m.scans.len() >= 6, "{} scans", m.scans.len());
        ends_at_markers(&data, &m);
        // First the averages of every component, then bands of detail,
        // refined bit by bit.
        let first = &m.scans[0];
        assert_eq!(
            (first.ss, first.se, first.ah, first.components),
            (0, 0, 0, 0b111)
        );
        assert!(
            m.scans.iter().any(|s| s.ss > 0 && s.ah > 0),
            "a refining AC scan"
        );
        assert!(
            m.scans
                .iter()
                .any(|s| s.ss > 0 && s.components == 0b1 && s.unit_width == 8)
        );
    }

    #[test]
    fn broken_files_say_why() {
        assert_eq!(block_map(b"not a jpeg"), Err(NoMap::Unreadable));
        let data = fixture("photo.jpg");
        for cut in [3, 100, 600] {
            let _ = block_map(&data[..cut]);
        }
        let cut = block_map(&data[..data.len() / 2]).unwrap();
        assert!(cut.stopped.is_some());
        let progressive = std::fs::read(format!(
            "{}/tests/fixtures/progressive.jpg",
            env!("CARGO_MANIFEST_DIR")
        ))
        .unwrap();
        // Cut in its middle: the scans before the cut still stand.
        let half = block_map(&progressive[..progressive.len() / 2]).unwrap();
        assert!(half.stopped.is_some());
        assert!(half.scans.len() >= 2);
    }
}
