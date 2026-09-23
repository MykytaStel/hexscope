/// Lookup table for the reflected polynomial 0xEDB88320, built at compile
/// time. One table step per byte instead of eight shift-and-xor rounds.
const TABLE: [u32; 256] = {
    let mut table = [0u32; 256];
    let mut i = 0;
    while i < 256 {
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

/// Continues a CRC over more bytes, so a checksum can span several slices
/// without copying them together. Start from `0xFFFF_FFFF`, finish with `!`.
pub fn crc32_update(mut crc: u32, data: &[u8]) -> u32 {
    for &byte in data {
        crc = TABLE[((crc ^ byte as u32) & 0xFF) as usize] ^ (crc >> 8);
    }
    crc
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
