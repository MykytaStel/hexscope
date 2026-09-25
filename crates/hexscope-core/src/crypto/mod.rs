//! The ciphers and hashes PDF encryption needs, written out so the parser
//! keeps no dependencies. They decrypt what a PDF already lets anyone read:
//! nothing here is for protecting secrets.

pub mod aes;
pub mod md5;
pub mod rc4;
pub mod sha2;

#[cfg(test)]
pub(crate) fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

#[cfg(test)]
pub(crate) fn unhex(s: &str) -> Vec<u8> {
    (0..s.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&s[i..i + 2], 16).unwrap())
        .collect()
}
