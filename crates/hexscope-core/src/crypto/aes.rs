//! AES (FIPS 197) in CBC mode: AES-128 for PDF's AESV2, AES-256 for AESV3,
//! and AES-128 encryption for revision 6's key derivation. Tables are
//! computed from the definition, not typed in.

use std::sync::OnceLock;

struct Tables {
    sbox: [u8; 256],
    inv: [u8; 256],
    /// Products by the MixColumns coefficients 2, 3, 9, 11, 13 and 14.
    by: [[u8; 256]; 6],
}

const COEFFICIENTS: [u8; 6] = [2, 3, 9, 11, 13, 14];

/// Multiplication in GF(2^8) modulo x^8 + x^4 + x^3 + x + 1.
fn mul(mut a: u8, mut b: u8) -> u8 {
    let mut p = 0;
    while b != 0 {
        if b & 1 != 0 {
            p ^= a;
        }
        let high = a & 0x80;
        a <<= 1;
        if high != 0 {
            a ^= 0x1b;
        }
        b >>= 1;
    }
    p
}

fn tables() -> &'static Tables {
    static T: OnceLock<Tables> = OnceLock::new();
    T.get_or_init(|| {
        let mut sbox = [0u8; 256];
        let mut inv = [0u8; 256];
        for x in 0..=255u8 {
            // The multiplicative inverse (0 for 0), then the affine map.
            let i = (1..=255u8).find(|&y| mul(x, y) == 1).unwrap_or(0);
            let s = i
                ^ i.rotate_left(1)
                ^ i.rotate_left(2)
                ^ i.rotate_left(3)
                ^ i.rotate_left(4)
                ^ 0x63;
            sbox[x as usize] = s;
            inv[s as usize] = x;
        }
        let by = COEFFICIENTS.map(|c| std::array::from_fn(|x| mul(x as u8, c)));
        Tables { sbox, inv, by }
    })
}

/// The round keys for a 16- or 32-byte key.
fn expand(key: &[u8]) -> Vec<[u8; 16]> {
    let t = tables();
    let nk = key.len() / 4;
    let rounds = nk + 6;
    let mut w: Vec<[u8; 4]> = key.as_chunks::<4>().0.to_vec();
    let mut rcon = 1u8;
    for i in nk..4 * (rounds + 1) {
        let mut temp = w[i - 1];
        if i % nk == 0 {
            temp.rotate_left(1);
            temp = temp.map(|b| t.sbox[b as usize]);
            temp[0] ^= rcon;
            rcon = mul(rcon, 2);
        } else if nk > 6 && i % nk == 4 {
            temp = temp.map(|b| t.sbox[b as usize]);
        }
        let prev = w[i - nk];
        w.push(std::array::from_fn(|j| prev[j] ^ temp[j]));
    }
    w.as_chunks::<4>()
        .0
        .iter()
        .map(|r| std::array::from_fn(|j| r[j / 4][j % 4]))
        .collect()
}

fn add(state: &mut [u8; 16], key: &[u8; 16]) {
    for (s, k) in state.iter_mut().zip(key) {
        *s ^= k;
    }
}

fn encrypt_block(round_keys: &[[u8; 16]], block: &mut [u8; 16]) {
    let t = tables();
    let rounds = round_keys.len() - 1;
    add(block, &round_keys[0]);
    for (r, key) in round_keys.iter().enumerate().skip(1) {
        // SubBytes and ShiftRows: row i moves left by i columns.
        let s = *block;
        for c in 0..4 {
            for row in 0..4 {
                block[c * 4 + row] = t.sbox[s[((c + row) % 4) * 4 + row] as usize];
            }
        }
        if r != rounds {
            for c in 0..4 {
                let col = [
                    block[c * 4],
                    block[c * 4 + 1],
                    block[c * 4 + 2],
                    block[c * 4 + 3],
                ];
                for row in 0..4 {
                    block[c * 4 + row] = t.by[0][col[row] as usize]
                        ^ t.by[1][col[(row + 1) % 4] as usize]
                        ^ col[(row + 2) % 4]
                        ^ col[(row + 3) % 4];
                }
            }
        }
        add(block, key);
    }
}

fn decrypt_block(round_keys: &[[u8; 16]], block: &mut [u8; 16]) {
    let t = tables();
    let rounds = round_keys.len() - 1;
    add(block, &round_keys[rounds]);
    for r in (0..rounds).rev() {
        // InvShiftRows and InvSubBytes: row i moves right by i columns.
        let s = *block;
        for c in 0..4 {
            for row in 0..4 {
                block[((c + row) % 4) * 4 + row] = t.inv[s[c * 4 + row] as usize];
            }
        }
        add(block, &round_keys[r]);
        if r != 0 {
            for c in 0..4 {
                let col = [
                    block[c * 4],
                    block[c * 4 + 1],
                    block[c * 4 + 2],
                    block[c * 4 + 3],
                ];
                for row in 0..4 {
                    block[c * 4 + row] = t.by[5][col[row] as usize]
                        ^ t.by[3][col[(row + 1) % 4] as usize]
                        ^ t.by[4][col[(row + 2) % 4] as usize]
                        ^ t.by[2][col[(row + 3) % 4] as usize];
                }
            }
        }
    }
}

/// CBC decryption. `None` when the key or data has the wrong size.
pub fn cbc_decrypt(key: &[u8], iv: &[u8; 16], data: &[u8]) -> Option<Vec<u8>> {
    if !matches!(key.len(), 16 | 32) || !data.len().is_multiple_of(16) {
        return None;
    }
    let keys = expand(key);
    let mut prev = *iv;
    let mut out = Vec::with_capacity(data.len());
    for chunk in data.as_chunks::<16>().0 {
        let mut block = *chunk;
        decrypt_block(&keys, &mut block);
        for (b, p) in block.iter_mut().zip(prev) {
            *b ^= p;
        }
        out.extend_from_slice(&block);
        prev = *chunk;
    }
    Some(out)
}

/// CBC encryption without padding. `None` when the key or data has the
/// wrong size.
pub fn cbc_encrypt(key: &[u8], iv: &[u8; 16], data: &[u8]) -> Option<Vec<u8>> {
    if !matches!(key.len(), 16 | 32) || !data.len().is_multiple_of(16) {
        return None;
    }
    let keys = expand(key);
    let mut prev = *iv;
    let mut out = Vec::with_capacity(data.len());
    for chunk in data.as_chunks::<16>().0 {
        let mut block: [u8; 16] = std::array::from_fn(|i| chunk[i] ^ prev[i]);
        encrypt_block(&keys, &mut block);
        out.extend_from_slice(&block);
        prev = block;
    }
    Some(out)
}

/// Removes PKCS #5 padding, when it is valid.
pub fn unpad(mut data: Vec<u8>) -> Option<Vec<u8>> {
    let n = *data.last()? as usize;
    if n == 0
        || n > 16
        || n > data.len()
        || !data[data.len() - n..].iter().all(|&b| b as usize == n)
    {
        return None;
    }
    data.truncate(data.len() - n);
    Some(data)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::crypto::{hex, unhex};

    #[test]
    fn fips_197_appendix_c() {
        let plain = unhex("00112233445566778899aabbccddeeff");
        let zero = [0u8; 16];
        for (key, want) in [
            (
                "000102030405060708090a0b0c0d0e0f",
                "69c4e0d86a7b0430d8cdb78070b4c55a",
            ),
            (
                "000102030405060708090a0b0c0d0e0f101112131415161718191a1b1c1d1e1f",
                "8ea2b7ca516745bfeafc49904b496089",
            ),
        ] {
            let key = unhex(key);
            // With a zero IV, one CBC block is one plain AES block.
            let ct = cbc_encrypt(&key, &zero, &plain).unwrap();
            assert_eq!(hex(&ct), want);
            assert_eq!(cbc_decrypt(&key, &zero, &ct).unwrap(), plain);
        }
    }

    #[test]
    fn sp_800_38a_cbc_aes128() {
        let key = unhex("2b7e151628aed2a6abf7158809cf4f3c");
        let iv: [u8; 16] = unhex("000102030405060708090a0b0c0d0e0f")
            .try_into()
            .unwrap();
        let plain = unhex("6bc1bee22e409f96e93d7e117393172aae2d8a571e03ac9c9eb76fac45af8e51");
        let ct = cbc_encrypt(&key, &iv, &plain).unwrap();
        assert_eq!(
            hex(&ct),
            "7649abac8119b246cee98e9b12e9197d5086cb9b507219ee95db113a917678b2"
        );
        assert_eq!(cbc_decrypt(&key, &iv, &ct).unwrap(), plain);
    }

    #[test]
    fn padding_is_checked() {
        assert_eq!(unpad(b"abc\x02\x02".to_vec()), Some(b"abc".to_vec()));
        assert_eq!(unpad(b"abc\x01\x02".to_vec()), None);
        assert_eq!(unpad(b"abc\x00".to_vec()), None);
        assert_eq!(unpad(Vec::new()), None);
        assert_eq!(cbc_decrypt(&[0; 15], &[0; 16], &[0; 16]), None);
        assert_eq!(cbc_decrypt(&[0; 16], &[0; 16], &[0; 15]), None);
    }
}
