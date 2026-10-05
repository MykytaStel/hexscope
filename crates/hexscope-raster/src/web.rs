use crate::{Output, decode_tiff, process, read_recipe};
use hexscope_core::film;
#[derive(serde::Deserialize)]
struct MosaicSignal {
    index: usize,
    serial: Option<String>,
    location: Option<[f64; 2]>,
    taken: Option<String>,
}
#[wasm_bindgen(js_name=photoMosaic)]
pub fn photo_mosaic(input: &str) -> Result<String, JsError> {
    if input.len() > 1024 * 1024 {
        return Err(JsError::new("mosaic signals exceed 1 MiB"));
    }
    let signals: Vec<MosaicSignal> =
        serde_json::from_str(input).map_err(|_| JsError::new("invalid mosaic signals"))?;
    let signals = signals
        .into_iter()
        .map(|s| hexscope_core::mosaic::Signal {
            index: s.index,
            serial: s.serial,
            location: s.location,
            taken: s.taken,
        })
        .collect::<Vec<_>>();
    hexscope_core::mosaic::analyze(&signals)
        .map(|r| r.to_json())
        .map_err(JsError::new)
}
use wasm_bindgen::prelude::*;
#[wasm_bindgen]
pub struct RasterResult {
    bytes: Vec<u8>,
    pixels: Vec<u8>,
    info: String,
}
#[wasm_bindgen]
impl RasterResult {
    #[wasm_bindgen(getter)]
    pub fn bytes(&self) -> Vec<u8> {
        self.bytes.clone()
    }
    #[wasm_bindgen(getter)]
    pub fn pixels(&self) -> Vec<u8> {
        self.pixels.clone()
    }
    #[wasm_bindgen(getter)]
    pub fn info(&self) -> String {
        self.info.clone()
    }
}
#[wasm_bindgen(js_name=renderRgba8)]
pub fn render_rgba8(
    pixels: &[u8],
    width: u32,
    height: u32,
    recipe: &str,
) -> Result<RasterResult, JsError> {
    let settings = read_recipe(recipe).map_err(|e| JsError::new(&e))?;
    let rendered = film::render(pixels, width, height, &settings).map_err(JsError::new)?;
    Ok(RasterResult {bytes:vec![],pixels:rendered.pixels,info:serde_json::json!({"baseColor":rendered.base,"clipping":{"shadows":rendered.shadows,"highlights":rendered.highlights}}).to_string()})
}
#[wasm_bindgen(js_name=processTiff)]
pub fn process_tiff(bytes: &[u8], recipe: &str, max_side: u32) -> Result<RasterResult, JsError> {
    let settings = read_recipe(recipe).map_err(|e| JsError::new(&e))?;
    let out = process(bytes, &settings, Output::Tiff16, max_side).map_err(|e| JsError::new(&e))?;
    let (image, _) = decode_tiff(&out.bytes).map_err(|e| JsError::new(&e))?;
    Ok(RasterResult {pixels:image.into_rgba8().into_raw(),info:serde_json::json!({"width":out.width,"height":out.height,"sourceWidth":out.source_width,"sourceHeight":out.source_height,"sourceDepth":out.source_depth,"profilePresent":out.profile_present,"limited":out.limited,"baseColor":out.base}).to_string(),bytes:out.bytes})
}
#[wasm_bindgen(js_name=encodeTiff8)]
pub fn encode_tiff8(pixels: &[u8], width: u32, height: u32) -> Result<Vec<u8>, JsError> {
    // Validate bounds before the codec's allocation.
    let rendered =
        film::render(pixels, width, height, &film::Settings::default()).map_err(JsError::new)?;
    let rgb = rendered
        .pixels
        .as_chunks::<4>()
        .0
        .iter()
        .flat_map(|p| p[..3].iter().map(|v| u16::from(*v) * 257))
        .collect::<Vec<_>>();
    let mut bytes = std::io::Cursor::new(Vec::new());
    tiff::encoder::TiffEncoder::new(&mut bytes)
        .map_err(|e| JsError::new(&e.to_string()))?
        .write_image::<tiff::encoder::colortype::RGB16>(width, height, &rgb)
        .map_err(|e| JsError::new(&e.to_string()))?;
    Ok(bytes.into_inner())
}
