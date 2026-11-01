const POLYNOMIAL: u32 = 0x04C1_1DB7;

const TABLE: [u32; 256] = {
    let mut table = [0; 256];
    let mut byte = 0;
    while byte < table.len() {
        let mut crc = (byte as u32) << 24;
        let mut bit = 0;
        while bit < 8 {
            crc = if crc & 0x8000_0000 != 0 {
                (crc << 1) ^ POLYNOMIAL
            } else {
                crc << 1
            };
            bit += 1;
        }
        table[byte] = crc;
        byte += 1;
    }
    table
};

fn update(mut crc: u32, bytes: &[u8]) -> u32 {
    for &byte in bytes {
        let index = ((crc >> 24) as u8 ^ byte) as usize;
        crc = (crc << 8) ^ TABLE[index];
    }
    crc
}

/// Computes the Ogg page CRC with its stored checksum field treated as zero.
pub(crate) fn checksum(page: &[u8]) -> Option<u32> {
    if page.len() < 27 {
        return None;
    }
    Some(update(update(update(0, &page[..22]), &[0; 4]), &page[26..]))
}

#[cfg(test)]
pub(crate) fn write_checksum(page: &mut [u8]) {
    let crc = checksum(page).expect("an Ogg page has a 27-byte header");
    page[22..26].copy_from_slice(&crc.to_le_bytes());
}

#[cfg(test)]
mod tests {
    use super::{checksum, write_checksum};

    #[test]
    fn matches_an_ogg_crc_vector_and_ignores_the_stored_field() {
        let mut page = b"OggS\0\x02".to_vec();
        page.extend_from_slice(&[0; 8]); // granule position
        page.extend_from_slice(&7u32.to_le_bytes());
        page.extend_from_slice(&0u32.to_le_bytes()); // sequence
        page.extend_from_slice(&[0; 4]); // checksum
        page.extend_from_slice(&[1, 1, b'x']); // one segment and one byte

        assert_eq!(checksum(&page), Some(0x0F33_CF96));
        write_checksum(&mut page);
        assert_eq!(&page[22..26], &[0x96, 0xCF, 0x33, 0x0F]);
        assert_eq!(checksum(&page), Some(0x0F33_CF96));
    }
}
