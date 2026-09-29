//! A movie too large to read whole, as the page reads one: every box but
//! the picture and sound, and a clean copy that takes the media from the
//! file as it is, so a long video is never held in memory.

use hexscope_core::video::Gap;
use std::fs::File;
use std::io::{self, Read, Seek, SeekFrom, Write};
use std::path::Path;

/// Movies past this size are read in parts.
const LARGE: u64 = 64 << 20;
/// Media shorter than this is read with the rest.
const MIN_GAP: u64 = 1 << 20;
const MAX_BOXES: usize = 100_000;

pub struct Movie {
    /// The file without its media.
    pub given: Vec<u8>,
    pub gaps: Vec<Gap>,
    /// Per gap: where its body goes back in `given`, where it starts in the file, its length.
    joins: Vec<(usize, u64, u64)>,
}

fn read_at(f: &mut File, at: u64, buf: &mut [u8]) -> io::Result<usize> {
    f.seek(SeekFrom::Start(at))?;
    let mut n = 0;
    while n < buf.len() {
        match f.read(&mut buf[n..])? {
            0 => break,
            k => n += k,
        }
    }
    Ok(n)
}

fn append(f: &mut File, from: u64, to: u64, out: &mut Vec<u8>) -> io::Result<()> {
    f.seek(SeekFrom::Start(from))?;
    f.take(to - from).read_to_end(out)?;
    Ok(())
}

/// The movie at `path` in parts, or `None` when it is not a large movie
/// with media worth leaving out.
pub fn read(path: &Path) -> io::Result<Option<Movie>> {
    let mut f = File::open(path)?;
    let len = f.metadata()?.len();
    if len < LARGE {
        return Ok(None);
    }
    let mut head = vec![0; 64 * 1024];
    let n = read_at(&mut f, 0, &mut head)?;
    head.truncate(n);
    if hexscope_core::heif::is_heif(&head) || !hexscope_core::video::starts_a_video(&head, len) {
        return Ok(None);
    }

    let mut media = Vec::new();
    let mut at = 0;
    for _ in 0..MAX_BOXES {
        if at + 8 > len {
            break;
        }
        let mut h = [0; 16];
        let n = read_at(&mut f, at, &mut h)?;
        let size32 = u64::from(u32::from_be_bytes([h[0], h[1], h[2], h[3]]));
        let header = if size32 == 1 { 16 } else { 8 };
        let size = match size32 {
            1 if n == 16 => {
                u64::from_be_bytes([h[8], h[9], h[10], h[11], h[12], h[13], h[14], h[15]])
            }
            1 => 0,
            0 => len - at,
            s => s,
        };
        if size < header || size > len - at {
            break;
        }
        if &h[4..8] == b"mdat" && size - header >= MIN_GAP {
            media.push((at, header, size));
        }
        at += size;
    }
    if media.is_empty() {
        return Ok(None);
    }

    let mut given = Vec::new();
    let mut gaps = Vec::new();
    let mut joins = Vec::new();
    let mut from = 0;
    for (at, header, size) in media {
        append(&mut f, from, at + header, &mut given)?;
        gaps.push(Gap {
            at: given.len() as u64 - header,
            len: size - header,
        });
        joins.push((given.len(), at + header, size - header));
        from = at + size;
    }
    append(&mut f, from, len, &mut given)?;
    Ok(Some(Movie { given, gaps, joins }))
}

impl Movie {
    /// Writes the clean copy: `copy`, made from `given`, with each body
    /// copied from `source` after its header.
    pub fn write_copy(&self, copy: &[u8], source: &Path, out: &mut impl Write) -> io::Result<()> {
        let short = || io::Error::from(io::ErrorKind::UnexpectedEof);
        let mut f = File::open(source)?;
        let mut from = 0;
        for &(at, start, len) in &self.joins {
            out.write_all(copy.get(from..at).ok_or_else(short)?)?;
            f.seek(SeekFrom::Start(start))?;
            if io::copy(&mut (&mut f).take(len), out)? != len {
                return Err(short());
            }
            from = at;
        }
        out.write_all(copy.get(from..).ok_or_else(short)?)
    }
}
