//! Copies of a file: without what it reveals, and with what survived of a
//! damaged one put back in order.

use super::*;

/// A copy of a file without what it reveals, or why there is none.
#[wasm_bindgen]
pub struct CleanCopy {
    bytes: Vec<u8>,
    removed: Vec<(String, u64)>,
    error: String,
    orientation: u16,
}

#[wasm_bindgen]
impl CleanCopy {
    /// The clean copy; empty when none was made.
    #[wasm_bindgen(getter)]
    pub fn bytes(&self) -> Vec<u8> {
        self.bytes.clone()
    }

    /// What was removed, as `what, bytes` pairs joined by U+001F.
    #[wasm_bindgen(getter)]
    pub fn removed(&self) -> String {
        let sep = SEPARATOR.to_string();
        self.removed
            .iter()
            .flat_map(|(what, n)| [sanitise(what), n.to_string()])
            .collect::<Vec<_>>()
            .join(&sep)
    }

    /// Why no copy was made; empty when one was.
    #[wasm_bindgen(getter)]
    pub fn error(&self) -> String {
        self.error.clone()
    }

    /// The orientation kept in a minimal EXIF block, or 0 when none was needed.
    #[wasm_bindgen(getter, js_name = orientationKept)]
    pub fn orientation_kept(&self) -> u16 {
        self.orientation
    }
}

/// Makes a copy of `bytes` without what it reveals about the people behind
/// it: see `hexscope_core::clean`. `notes` also empties Excel's and
/// PowerPoint's comments and speaker notes.
#[wasm_bindgen(js_name = cleanCopy)]
pub fn clean_copy(bytes: &[u8], notes: bool) -> CleanCopy {
    into_copy(clean_with(
        bytes,
        CleanOptions {
            comments_and_notes: notes,
        },
    ))
}

/// A copy, or why there is none, as the page reads it.
pub(crate) fn into_copy(
    result: Result<hexscope_core::clean::Cleaned, hexscope_core::clean::CleanError>,
) -> CleanCopy {
    match result {
        Ok(c) => CleanCopy {
            bytes: c.bytes,
            removed: c.removed.into_iter().map(|r| (r.what, r.bytes)).collect(),
            error: String::new(),
            orientation: c.orientation_kept.unwrap_or(0),
        },
        Err(e) => CleanCopy {
            bytes: Vec::new(),
            removed: Vec::new(),
            error: e.reason().to_string(),
            orientation: 0,
        },
    }
}

/// The clean copy of a movie given as [`parse_movie`] takes it: the bytes
/// given, changed; each left-out body goes back after its header.
#[wasm_bindgen(js_name = cleanMovie)]
pub fn clean_movie(bytes: &[u8], gaps_at: &[f64]) -> CleanCopy {
    into_copy(clean_video_gapped(bytes, &gaps(gaps_at)))
}

/// A damaged file's copy with what survived put back in order.
#[wasm_bindgen]
pub struct RepairCopy {
    bytes: Vec<u8>,
    fixed: Vec<String>,
    error: String,
}

#[wasm_bindgen]
impl RepairCopy {
    /// The repaired copy; empty when none was made.
    #[wasm_bindgen(getter)]
    pub fn bytes(&self) -> Vec<u8> {
        self.bytes.clone()
    }

    /// What was done, in words, joined by U+001F.
    #[wasm_bindgen(getter)]
    pub fn fixed(&self) -> String {
        self.fixed
            .iter()
            .map(|f| sanitise(f))
            .collect::<Vec<_>>()
            .join(&SEPARATOR.to_string())
    }

    /// Why no copy was made; empty when one was.
    #[wasm_bindgen(getter)]
    pub fn error(&self) -> String {
        self.error.clone()
    }
}

/// Repairs what can be repaired: see `hexscope_core::repair`.
#[wasm_bindgen(js_name = repairCopy)]
pub fn repair_copy(bytes: &[u8]) -> RepairCopy {
    match hexscope_core::repair::repair(bytes) {
        Ok(r) => RepairCopy {
            bytes: r.bytes,
            fixed: r.fixed,
            error: String::new(),
        },
        Err(e) => RepairCopy {
            bytes: Vec::new(),
            fixed: Vec::new(),
            error: e.reason().to_string(),
        },
    }
}
