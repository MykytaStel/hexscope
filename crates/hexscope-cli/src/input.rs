//! Bounded whole-file reads, including files that grow after metadata was read.
use std::{
    fs::File,
    io::{self, Read},
    path::Path,
};

pub const MAX_INPUT_BYTES: u64 = 512 * 1024 * 1024;

pub fn read(path: impl AsRef<Path>) -> io::Result<Vec<u8>> {
    let file = File::open(path)?;
    let too_large = || {
        io::Error::new(
            io::ErrorKind::InvalidData,
            "file exceeds the 512 MiB whole-file reading limit",
        )
    };
    if file.metadata()?.len() > MAX_INPUT_BYTES {
        return Err(too_large());
    }
    let mut bytes = Vec::new();
    file.take(MAX_INPUT_BYTES + 1).read_to_end(&mut bytes)?;
    if bytes.len() as u64 > MAX_INPUT_BYTES {
        return Err(too_large());
    }
    Ok(bytes)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn refuses_an_oversized_sparse_file_before_reading_it() {
        let dir = std::env::temp_dir().join(format!("hexscope-budget-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("large.bin");
        File::create(&path)
            .unwrap()
            .set_len(MAX_INPUT_BYTES + 1)
            .unwrap();
        assert_eq!(read(&path).unwrap_err().kind(), io::ErrorKind::InvalidData);
        std::fs::remove_dir_all(dir).unwrap();
    }
}
