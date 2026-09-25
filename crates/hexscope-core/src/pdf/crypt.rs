//! PDF's standard security handler (ISO 32000-1 §7.6.3, and revision 6 from
//! ISO 32000-2), for documents that open without a password.
//!
//! Most encrypted PDFs have an empty user password: anyone can open them,
//! and the encryption only asks viewers to restrict printing or copying. A
//! viewer decrypts them without asking, and so does this — to read what the
//! document says about who made it. A document that needs a password stays
//! unread.

use super::lexer::Obj;
use crate::crypto::aes::{cbc_decrypt, cbc_encrypt, unpad};
use crate::crypto::md5::md5;
use crate::crypto::rc4::rc4;
use crate::crypto::sha2::{sha256, sha384, sha512};

/// Algorithm 2's padding string.
const PAD: [u8; 32] = [
    0x28, 0xBF, 0x4E, 0x5E, 0x4E, 0x75, 0x8A, 0x41, 0x64, 0x00, 0x4E, 0x56, 0xFF, 0xFA, 0x01, 0x08,
    0x2E, 0x2E, 0x00, 0xB6, 0xD0, 0x68, 0x3E, 0x80, 0x2F, 0x0C, 0xA9, 0xFE, 0x64, 0x53, 0x69, 0x7A,
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Method {
    None,
    Rc4,
    Aes128,
    Aes256,
}

/// What a document's encryption is, in a few words, and whether it opened.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Lock {
    /// Opens without a password; `scheme` names the cipher.
    Open { scheme: &'static str },
    /// Needs a password to open.
    Password { scheme: &'static str },
    /// A handler or version this does not read.
    Unknown,
}

/// The key to an encrypted document that opens without a password.
#[derive(Debug, Clone)]
pub struct Decryptor {
    key: Vec<u8>,
    strings: Method,
    streams: Method,
    /// Revision 4's option to leave the XMP metadata unencrypted.
    pub encrypt_metadata: bool,
}

fn bytes(obj: Option<&Obj>) -> Option<&[u8]> {
    match obj? {
        Obj::Str(s) => Some(s),
        _ => None,
    }
}

/// Reads the encryption dictionary and tries the empty password.
pub(crate) fn open(encrypt: &Obj, id0: &[u8]) -> (Lock, Option<Decryptor>) {
    if encrypt.get("Filter").and_then(Obj::name) != Some("Standard") {
        return (Lock::Unknown, None);
    }
    let v = encrypt.get("V").and_then(Obj::int).unwrap_or(0);
    let r = encrypt.get("R").and_then(Obj::int).unwrap_or(0);
    let (Some(o), Some(u)) = (bytes(encrypt.get("O")), bytes(encrypt.get("U"))) else {
        return (Lock::Unknown, None);
    };
    let encrypt_metadata = !matches!(encrypt.get("EncryptMetadata"), Some(Obj::Bool(false)));

    // Which cipher strings and streams use: fixed before V4, named by crypt
    // filters from V4 on.
    let filter = |name: &str| -> Option<Method> {
        let f = encrypt.get(name).and_then(Obj::name).unwrap_or("Identity");
        if f == "Identity" {
            return Some(Method::None);
        }
        match encrypt.get("CF")?.get(f)?.get("CFM").and_then(Obj::name) {
            Some("V2") => Some(Method::Rc4),
            Some("AESV2") => Some(Method::Aes128),
            Some("AESV3") => Some(Method::Aes256),
            Some("None") => Some(Method::None),
            _ => None,
        }
    };
    let (strings, streams) = match v {
        1 | 2 => (Method::Rc4, Method::Rc4),
        4 | 5 => match (filter("StrF"), filter("StmF")) {
            (Some(s), Some(t)) => (s, t),
            _ => return (Lock::Unknown, None),
        },
        _ => return (Lock::Unknown, None),
    };
    let scheme = match strongest(strings, streams) {
        Method::Aes256 => "AES-256",
        Method::Aes128 => "AES-128",
        Method::Rc4 if v == 1 || r == 2 => "RC4, 40-bit",
        Method::Rc4 => "RC4",
        Method::None => "no cipher",
    };

    let key = match r {
        2..=4 => {
            let p = encrypt.get("P").and_then(Obj::int).unwrap_or(0) as u32;
            let n = if r == 2 {
                5
            } else {
                let bits = encrypt.get("Length").and_then(Obj::int).unwrap_or(40);
                usize::try_from(bits / 8).unwrap_or(5).clamp(5, 16)
            };
            let key = key_r2_r4(o, p, id0, r, n, encrypt_metadata);
            user_matches(&key, u, id0, r).then_some(key)
        }
        5 | 6 => key_r5_r6(u, bytes(encrypt.get("UE")), r),
        _ => return (Lock::Unknown, None),
    };
    match key {
        Some(key) => (
            Lock::Open { scheme },
            Some(Decryptor {
                key,
                strings,
                streams,
                encrypt_metadata,
            }),
        ),
        None => (Lock::Password { scheme }, None),
    }
}

/// The stronger of two ciphers, to name the document's encryption by.
fn strongest(a: Method, b: Method) -> Method {
    let rank = |m: Method| match m {
        Method::None => 0,
        Method::Rc4 => 1,
        Method::Aes128 => 2,
        Method::Aes256 => 3,
    };
    if rank(b) > rank(a) { b } else { a }
}

/// Algorithm 2, for the empty password.
fn key_r2_r4(o: &[u8], p: u32, id0: &[u8], r: i64, n: usize, encrypt_metadata: bool) -> Vec<u8> {
    let mut input = PAD.to_vec();
    input.extend_from_slice(o);
    input.extend_from_slice(&p.to_le_bytes());
    input.extend_from_slice(id0);
    if r >= 4 && !encrypt_metadata {
        input.extend_from_slice(&[0xFF; 4]);
    }
    let mut h = md5(&input);
    if r >= 3 {
        for _ in 0..50 {
            h = md5(&h[..n]);
        }
    }
    h[..n].to_vec()
}

/// Algorithms 4 and 5: whether `key` is the one the empty password gives.
fn user_matches(key: &[u8], u: &[u8], id0: &[u8], r: i64) -> bool {
    if r == 2 {
        return rc4(key, &PAD) == u.get(..32).unwrap_or_default();
    }
    let mut input = PAD.to_vec();
    input.extend_from_slice(id0);
    let mut x = rc4(key, &md5(&input));
    for i in 1..=19u8 {
        let k: Vec<u8> = key.iter().map(|b| b ^ i).collect();
        x = rc4(&k, &x);
    }
    u.get(..16) == Some(&x[..])
}

/// Revisions 5 and 6: check the empty password against U, then unwrap the
/// file key from UE.
fn key_r5_r6(u: &[u8], ue: Option<&[u8]>, r: i64) -> Option<Vec<u8>> {
    let (hash, validation, key_salt) = (u.get(..32)?, u.get(32..40)?, u.get(40..48)?);
    let derive = |salt: &[u8]| -> [u8; 32] {
        if r == 5 {
            sha256(salt)
        } else {
            hash_2b(b"", salt, b"")
        }
    };
    if derive(validation) != hash {
        return None;
    }
    let intermediate = derive(key_salt);
    let ue = ue?.get(..32)?;
    cbc_decrypt(&intermediate, &[0; 16], ue)
}

/// ISO 32000-2 Algorithm 2.B: at least 64 rounds of AES and SHA-2, each
/// round choosing its hash from the one before.
fn hash_2b(password: &[u8], salt: &[u8], udata: &[u8]) -> [u8; 32] {
    let mut k = sha256(&[password, salt, udata].concat()).to_vec();
    let mut e: Vec<u8> = Vec::new();
    let mut round = 0usize;
    while round < 64 || e.last().is_some_and(|&b| b as usize > round - 32) {
        let k1 = [password, &k, udata].concat().repeat(64);
        let (Ok(key), Ok(iv)) = (
            <[u8; 16]>::try_from(&k[..16]),
            <[u8; 16]>::try_from(&k[16..32]),
        ) else {
            break;
        };
        e = cbc_encrypt(&key, &iv, &k1).unwrap_or_default();
        k = match e.iter().take(16).map(|&b| b as u32).sum::<u32>() % 3 {
            0 => sha256(&e).to_vec(),
            1 => sha384(&e).to_vec(),
            _ => sha512(&e).to_vec(),
        };
        round += 1;
        // A round count beyond any real document's: stop regardless.
        if round > 4096 {
            break;
        }
    }
    let mut out = [0u8; 32];
    out.copy_from_slice(&k[..32]);
    out
}

impl Decryptor {
    /// Algorithm 1: the key for one object.
    fn object_key(&self, num: u32, gen_: u16, aes: bool) -> Vec<u8> {
        let mut input = self.key.clone();
        input.extend_from_slice(&num.to_le_bytes()[..3]);
        input.extend_from_slice(&gen_.to_le_bytes());
        if aes {
            input.extend_from_slice(b"sAlT");
        }
        let n = (self.key.len() + 5).min(16);
        md5(&input)[..n].to_vec()
    }

    fn apply(&self, method: Method, num: u32, gen_: u16, data: &[u8]) -> Option<Vec<u8>> {
        match method {
            Method::None => Some(data.to_vec()),
            Method::Rc4 => Some(rc4(&self.object_key(num, gen_, false), data)),
            Method::Aes128 | Method::Aes256 => {
                let key = if method == Method::Aes256 {
                    self.key.clone()
                } else {
                    self.object_key(num, gen_, true)
                };
                // The first block is the IV.
                let iv: [u8; 16] = data.get(..16)?.try_into().ok()?;
                unpad(cbc_decrypt(&key, &iv, &data[16..])?)
            }
        }
    }

    /// A string of object `num`.
    pub fn string(&self, num: u32, gen_: u16, data: &[u8]) -> Option<Vec<u8>> {
        self.apply(self.strings, num, gen_, data)
    }

    /// A stream's data, before its filters are undone.
    pub fn stream(&self, num: u32, gen_: u16, data: &[u8]) -> Option<Vec<u8>> {
        self.apply(self.streams, num, gen_, data)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn revision_6_hash_stops_by_its_rule() {
        // Deterministic, and the same for the same input.
        let a = hash_2b(b"", b"12345678", b"");
        assert_eq!(a, hash_2b(b"", b"12345678", b""));
        assert_ne!(a, hash_2b(b"", b"12345679", b""));
    }

    #[test]
    fn a_dictionary_this_does_not_know_is_unknown() {
        let d = super::super::lexer::Lexer::new(b"<< /Filter /Other /V 2 /R 3 >>", 0)
            .value()
            .unwrap()
            .obj;
        assert_eq!(open(&d, b"").0, Lock::Unknown);
    }
}
