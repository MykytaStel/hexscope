//! Optional codecs around the shared, codec-free film renderer.
#![forbid(unsafe_code)]
use hexscope_core::film::{self, Mode, Settings};
#[cfg(feature = "native")]
use image::ImageDecoder;
use image::{DynamicImage, GenericImageView};
use serde::Deserialize;
use std::io::Cursor;
const MAX_BYTES: usize = 50 * 1024 * 1024;
const MAX_ALLOC: u64 = 256 * 1024 * 1024;
#[derive(Deserialize)]
struct Recipe {
    schema: String,
    version: u8,
    algorithm: String,
    settings: RecipeSettings,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct RecipeSettings {
    mode: String,
    crop: [f64; 4],
    exposure: f64,
    contrast: f64,
    #[serde(deserialize_with = "required_base")]
    base_color: Option<[f64; 3]>,
}
fn required_base<'de, D: serde::Deserializer<'de>>(
    deserializer: D,
) -> Result<Option<[f64; 3]>, D::Error> {
    Option::<[f64; 3]>::deserialize(deserializer)
}
pub fn read_recipe(text: &str) -> Result<Settings, String> {
    if text.len() > 16_384 {
        return Err("recipe exceeds 16 KiB".into());
    }
    let r: Recipe = serde_json::from_str(text).map_err(|_| "invalid film recipe")?;
    if r.schema != "hexscope.film-recipe" || r.version != 1 || r.algorithm != "srgb-density-v1" {
        return Err("unsupported film recipe version or algorithm".into());
    }
    let mode = match r.settings.mode.as_str() {
        "positive" => Mode::Positive,
        "color-negative" => Mode::ColorNegative,
        "mono-negative" => Mode::MonoNegative,
        _ => return Err("invalid film mode".into()),
    };
    let base = match r.settings.base_color {
        None => None,
        Some(values) => {
            if values
                .iter()
                .any(|v| !v.is_finite() || !(1.0..=255.0).contains(v))
            {
                return Err("invalid base color".into());
            }
            Some(values.map(|v| v.round() as u8))
        }
    };
    let settings = Settings {
        mode,
        crop: r.settings.crop,
        exposure: r.settings.exposure,
        contrast: r.settings.contrast,
        base,
    };
    settings.validate().map_err(String::from)?;
    Ok(settings)
}
#[derive(Clone, Copy, Debug)]
pub enum Output {
    Jpeg,
    Tiff16,
}
#[derive(Debug)]
pub struct Processed {
    pub bytes: Vec<u8>,
    pub width: u32,
    pub height: u32,
    pub source_width: u32,
    pub source_height: u32,
    pub source_depth: u8,
    pub profile_present: bool,
    pub limited: bool,
    pub base: [u8; 3],
}
fn dimensions(w: u32, h: u32) -> Result<(), String> {
    if w == 0 || h == 0 || w > 30_000 || h > 30_000 || u64::from(w) * u64::from(h) > 120_000_000 {
        return Err("source exceeds dimension or 120 megapixel limit".into());
    }
    Ok(())
}
fn allocation(w: u32, h: u32, raw: u64) -> Result<(), String> {
    dimensions(w, h)?;
    if raw
        .checked_add(u64::from(w) * u64::from(h) * 8)
        .is_none_or(|v| v > MAX_ALLOC)
    {
        return Err("source exceeds 256 MiB decode/conversion allocation budget".into());
    }
    Ok(())
}
pub fn is_tiff(bytes: &[u8]) -> bool {
    matches!(
        bytes.get(..4),
        Some(b"II\x2a\0" | b"MM\0\x2a" | b"II\x2b\0" | b"MM\0\x2b")
    )
}
fn decode_tiff(bytes: &[u8]) -> Result<(DynamicImage, bool), String> {
    use tiff::{
        ColorType,
        decoder::{Decoder, DecodingResult, Limits},
        tags::Tag,
    };
    let mut limits = Limits::default();
    limits.decoding_buffer_size = MAX_ALLOC as usize / 2;
    limits.intermediate_buffer_size = 16 * 1024 * 1024;
    limits.ifd_value_size = 1024 * 1024;
    let mut decoder = Decoder::new(Cursor::new(bytes))
        .map_err(|e| e.to_string())?
        .with_limits(limits);
    if decoder.more_images() {
        return Err("multi-image TIFF is not supported; split its pages first".into());
    }
    let (w, h) = decoder.dimensions().map_err(|e| e.to_string())?;
    let color = decoder.colortype().map_err(|e| e.to_string())?;
    let (channels, depth) = match color {
        ColorType::Gray(d) => (1, d),
        ColorType::GrayA(d) => (2, d),
        ColorType::RGB(d) => (3, d),
        ColorType::RGBA(d) => (4, d),
        _ => {
            return Err(
                "TIFF requires grayscale or RGB samples; CMYK/palette/float are not supported"
                    .into(),
            );
        }
    };
    if !matches!(depth, 8 | 16) {
        return Err("TIFF requires 8-bit or 16-bit integer samples".into());
    }
    if channels == 2 || channels == 4 {
        let extra = decoder
            .get_tag_u16_vec(Tag::ExtraSamples)
            .map_err(|e| e.to_string())?;
        if extra.as_slice() != [2] {
            return Err("TIFF associated alpha or unspecified extra samples are not supported; convert to straight alpha or RGB first".into());
        }
    }
    allocation(
        w,
        h,
        u64::from(w) * u64::from(h) * channels * u64::from(depth) / 8,
    )?;
    let profile = decoder
        .find_tag(Tag::IccProfile)
        .map_err(|e| e.to_string())?
        .is_some();
    let orientation = decoder
        .find_tag_unsigned::<u16>(Tag::Orientation)
        .map_err(|e| e.to_string())?
        .unwrap_or(1);
    if !(1..=8).contains(&orientation) {
        return Err("invalid TIFF orientation".into());
    }
    let orientation = image::metadata::Orientation::from_exif(orientation as u8)
        .ok_or("invalid TIFF orientation")?;
    let data = decoder.read_image().map_err(|e| e.to_string())?;
    let invalid = || "TIFF pixel length does not match dimensions".to_string();
    let mut image = match (data, channels) {
        (DecodingResult::U8(v), 1) => {
            DynamicImage::ImageLuma8(image::GrayImage::from_raw(w, h, v).ok_or_else(invalid)?)
        }
        (DecodingResult::U8(v), 2) => {
            DynamicImage::ImageLumaA8(image::GrayAlphaImage::from_raw(w, h, v).ok_or_else(invalid)?)
        }
        (DecodingResult::U8(v), 3) => {
            DynamicImage::ImageRgb8(image::RgbImage::from_raw(w, h, v).ok_or_else(invalid)?)
        }
        (DecodingResult::U8(v), 4) => {
            DynamicImage::ImageRgba8(image::RgbaImage::from_raw(w, h, v).ok_or_else(invalid)?)
        }
        (DecodingResult::U16(v), 1) => {
            DynamicImage::ImageLuma16(image::ImageBuffer::from_raw(w, h, v).ok_or_else(invalid)?)
        }
        (DecodingResult::U16(v), 2) => {
            DynamicImage::ImageLumaA16(image::ImageBuffer::from_raw(w, h, v).ok_or_else(invalid)?)
        }
        (DecodingResult::U16(v), 3) => {
            DynamicImage::ImageRgb16(image::ImageBuffer::from_raw(w, h, v).ok_or_else(invalid)?)
        }
        (DecodingResult::U16(v), 4) => {
            DynamicImage::ImageRgba16(image::ImageBuffer::from_raw(w, h, v).ok_or_else(invalid)?)
        }
        _ => return Err("unsupported TIFF sample type".into()),
    };
    image.apply_orientation(orientation);
    Ok((image, profile))
}
fn decode(bytes: &[u8]) -> Result<(DynamicImage, bool), String> {
    if bytes.len() > MAX_BYTES {
        return Err("source exceeds 50 MiB file limit".into());
    }
    if is_tiff(bytes) {
        return decode_tiff(bytes);
    }
    #[cfg(feature = "native")]
    {
        let mut reader = image::ImageReader::new(Cursor::new(bytes))
            .with_guessed_format()
            .map_err(|e| e.to_string())?;
        if !matches!(
            reader.format(),
            Some(image::ImageFormat::Jpeg | image::ImageFormat::Png | image::ImageFormat::WebP)
        ) {
            return Err("film accepts JPEG, PNG, WebP and TIFF only".into());
        }
        let mut limits = image::Limits::default();
        limits.max_image_width = Some(30_000);
        limits.max_image_height = Some(30_000);
        limits.max_alloc = Some(MAX_ALLOC / 2);
        reader.limits(limits);
        let mut decoder = reader.into_decoder().map_err(|e| e.to_string())?;
        let (w, h) = decoder.dimensions();
        allocation(w, h, decoder.total_bytes())?;
        let profile = decoder.icc_profile().map_err(|e| e.to_string())?.is_some();
        let orientation = decoder.orientation().map_err(|e| e.to_string())?;
        let mut image = DynamicImage::from_decoder(decoder).map_err(|e| e.to_string())?;
        image.apply_orientation(orientation);
        Ok((image, profile))
    }
    #[cfg(not(feature = "native"))]
    {
        Err("this optional codec accepts TIFF only".into())
    }
}
pub fn process(
    bytes: &[u8],
    settings: &Settings,
    format: Output,
    max_side: u32,
) -> Result<Processed, String> {
    settings.validate().map_err(String::from)?;
    if max_side == 0 || max_side > 30_000 {
        return Err("invalid output side limit".into());
    }
    let (image, profile) = decode(bytes)?;
    let (sw, sh) = image.dimensions();
    let depth = if image.color().bits_per_pixel() / u16::from(image.color().channel_count()) == 16 {
        16
    } else {
        8
    };
    let scale = (f64::from(max_side) / f64::from(sw.max(sh)))
        .min((film::MAX_PIXELS as f64 / (f64::from(sw) * f64::from(sh))).sqrt())
        .min(1.0);
    let (w, h) = (
        (f64::from(sw) * scale).floor().max(1.0) as u32,
        (f64::from(sh) * scale).floor().max(1.0) as u32,
    );
    let mut image = image;
    if (w, h) != (sw, sh) {
        image = image.resize_exact(w, h, image::imageops::FilterType::Triangle);
    }
    let (x, y, cw, ch) = film::crop_geometry(w, h, settings.crop).map_err(String::from)?;
    let cropped = image.crop_imm(x, y, cw, ch);
    drop(image);
    let rgba = cropped.into_rgba16();
    let mut samples = rgba.into_raw();
    for p in samples.as_chunks_mut::<4>().0.iter_mut() {
        let a = u64::from(p[3]);
        for c in &mut p[..3] {
            *c = ((u64::from(*c) * a + 65535 * (65535 - a) + 32767) / 65535) as u16;
        }
        p[3] = 65535;
    }
    let rendered = film::render(&samples, cw, ch, settings).map_err(String::from)?;
    drop(samples);
    let rgb = rendered
        .pixels
        .as_chunks::<4>()
        .0
        .iter()
        .flat_map(|p| p[..3].iter().copied())
        .collect::<Vec<_>>();
    let base = rendered.base;
    drop(rendered);
    let mut output = Cursor::new(Vec::new());
    match format {
        Output::Tiff16 => tiff::encoder::TiffEncoder::new(&mut output)
            .map_err(|e| e.to_string())?
            .write_image::<tiff::encoder::colortype::RGB16>(cw, ch, &rgb)
            .map_err(|e| e.to_string())?,
        #[cfg(feature = "native")]
        Output::Jpeg => {
            let pixels = rgb
                .iter()
                .map(|v| ((u32::from(*v) + 128) / 257) as u8)
                .collect::<Vec<_>>();
            image::codecs::jpeg::JpegEncoder::new_with_quality(&mut output, 94)
                .encode(&pixels, cw, ch, image::ExtendedColorType::Rgb8)
                .map_err(|e| e.to_string())?;
        }
        #[cfg(not(feature = "native"))]
        Output::Jpeg => return Err("JPEG encoding uses the browser canvas".into()),
    }
    Ok(Processed {
        bytes: output.into_inner(),
        width: cw,
        height: ch,
        source_width: sw,
        source_height: sh,
        source_depth: depth,
        profile_present: profile,
        limited: (w, h) != (sw, sh),
        base,
    })
}
#[cfg(feature = "web")]
mod web;
