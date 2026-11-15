/// Lookup table for the reflected polynomial 0xEDB88320, built at compile
/// time. The WebAssembly build keeps this single compact table.
const TABLE: [u32; 256] = {
    let mut table = [0u32; 256];
    let mut i = 0;
    while i < table.len() {
        let mut c = i as u32;
        let mut k = 0;
        while k < 8 {
            c = if c & 1 != 0 {
                0xEDB8_8320 ^ (c >> 1)
            } else {
                c >> 1
            };
            k += 1;
        }
        table[i] = c;
        i += 1;
    }
    table
};

/// Native slicing-by-8 tables. Keeping them out of Wasm preserves its tight
/// download budgets; there, the single-byte table below stays in use.
#[cfg(not(target_arch = "wasm32"))]
const TABLES: [[u32; 256]; 8] = {
    let mut tables = [[0u32; 256]; 8];
    tables[0] = TABLE;
    let mut i = 0;
    while i < TABLE.len() {
        let mut c = TABLE[i];
        let mut slice = 1;
        while slice < tables.len() {
            c = TABLE[(c & 0xFF) as usize] ^ (c >> 8);
            tables[slice][i] = c;
            slice += 1;
        }
        i += 1;
    }
    tables
};

/// Continues a CRC over more bytes, so a checksum can span several slices
/// without copying them together. Start from `0xFFFF_FFFF`, finish with `!`.
pub fn crc32_update(mut crc: u32, data: &[u8]) -> u32 {
    #[cfg(not(target_arch = "wasm32"))]
    {
        let (blocks, tail) = data.as_chunks::<8>();
        for block in blocks {
            let low = crc ^ u32::from_le_bytes([block[0], block[1], block[2], block[3]]);
            let high = u32::from_le_bytes([block[4], block[5], block[6], block[7]]);
            crc = TABLES[7][(low & 0xFF) as usize]
                ^ TABLES[6][((low >> 8) & 0xFF) as usize]
                ^ TABLES[5][((low >> 16) & 0xFF) as usize]
                ^ TABLES[4][(low >> 24) as usize]
                ^ TABLES[3][(high & 0xFF) as usize]
                ^ TABLES[2][((high >> 8) & 0xFF) as usize]
                ^ TABLES[1][((high >> 16) & 0xFF) as usize]
                ^ TABLES[0][(high >> 24) as usize];
        }
        for &byte in tail {
            crc = TABLE[((crc ^ byte as u32) & 0xFF) as usize] ^ (crc >> 8);
        }
        crc
    }

    #[cfg(target_arch = "wasm32")]
    {
        for &byte in data {
            crc = TABLE[((crc ^ byte as u32) & 0xFF) as usize] ^ (crc >> 8);
        }
        crc
    }
}

/// Standard CRC-32 (IEEE 802.3, reflected, polynomial 0xEDB88320) — the one
/// PNG uses for every chunk.
pub fn crc32(data: &[u8]) -> u32 {
    !crc32_update(0xFFFF_FFFF, data)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn matches_known_vectors() {
        assert_eq!(crc32(b""), 0x0000_0000);
        assert_eq!(crc32(b"123456789"), 0xCBF4_3926);
        // "IEND" with no data — the CRC every PNG ends with.
        assert_eq!(crc32(b"IEND"), 0xAE42_6082);
    }

    #[test]
    fn streaming_matches_one_shot() {
        let data = b"IHDR and then a payload that spans two slices";
        let (head, tail) = data.split_at(4);
        let streamed = !crc32_update(crc32_update(0xFFFF_FFFF, head), tail);
        assert_eq!(streamed, crc32(data));
    }
}
