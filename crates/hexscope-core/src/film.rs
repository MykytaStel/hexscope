//! Codec-free sRGB density rendition. No claim of calibrated film recovery.
pub const MAX_PIXELS: usize = 12_000_000;
const MAX_SAMPLES: usize = 131_072;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mode {
    Positive,
    ColorNegative,
    MonoNegative,
}
#[derive(Clone, Debug)]
pub struct Settings {
    pub mode: Mode,
    pub crop: [f64; 4],
    pub exposure: f64,
    pub contrast: f64,
    pub base: Option<[u8; 3]>,
}
impl Default for Settings {
    fn default() -> Self {
        Self {
            mode: Mode::Positive,
            crop: [0.0; 4],
            exposure: 0.0,
            contrast: 1.0,
            base: None,
        }
    }
}
impl Settings {
    pub fn validate(&self) -> Result<(), &'static str> {
        if !self.exposure.is_finite()
            || !(-2.0..=2.0).contains(&self.exposure)
            || !self.contrast.is_finite()
            || !(0.5..=2.0).contains(&self.contrast)
            || self
                .crop
                .iter()
                .any(|v| !v.is_finite() || !(0.0..=0.45).contains(v))
            || self.base.is_some_and(|b| b.contains(&0))
        {
            return Err("invalid film settings");
        }
        Ok(())
    }
}
pub fn crop_geometry(
    width: u32,
    height: u32,
    crop: [f64; 4],
) -> Result<(u32, u32, u32, u32), &'static str> {
    if width == 0
        || height == 0
        || width > 30_000
        || height > 30_000
        || crop
            .iter()
            .any(|v| !v.is_finite() || !(0.0..=0.45).contains(v))
    {
        return Err("invalid film crop");
    }
    let x = (f64::from(width) * crop[0])
        .round()
        .min(f64::from(width - 1)) as u32;
    let y = (f64::from(height) * crop[1])
        .round()
        .min(f64::from(height - 1)) as u32;
    Ok((
        x,
        y,
        ((f64::from(width) * (1.0 - crop[2])).round() as u32)
            .saturating_sub(x)
            .max(1),
        ((f64::from(height) * (1.0 - crop[3])).round() as u32)
            .saturating_sub(y)
            .max(1),
    ))
}
pub trait Sample: Copy + Default {
    const MAX: usize;
    fn index(self) -> usize;
    fn from_index(v: usize) -> Self;
}
impl Sample for u8 {
    const MAX: usize = 255;
    fn index(self) -> usize {
        self as usize
    }
    fn from_index(v: usize) -> Self {
        v as u8
    }
}
impl Sample for u16 {
    const MAX: usize = 65535;
    fn index(self) -> usize {
        self as usize
    }
    fn from_index(v: usize) -> Self {
        v as u16
    }
}
#[derive(Debug)]
pub struct Rendered<T> {
    pub pixels: Vec<T>,
    pub base: [u8; 3],
    pub shadows: f64,
    pub highlights: f64,
}
fn percentile(hist: &[u32], count: usize, fraction: f64) -> usize {
    let target = (count as f64 * fraction).ceil().max(1.0) as usize;
    let mut sum = 0usize;
    for (i, n) in hist.iter().enumerate() {
        sum += *n as usize;
        if sum >= target {
            return i;
        }
    }
    hist.len() - 1
}
fn sample_indices(size: usize) -> Vec<usize> {
    if size <= MAX_SAMPLES {
        return (0..size).collect();
    }
    let count = size.min(MAX_SAMPLES);
    (0..count)
        .map(|stratum| {
            // Divide first: multiplying `stratum * size` overflows usize on
            // wasm32 for ordinary rasters above the sample cap.
            let base_width = size / count;
            let remainder = size % count;
            // Spread the remainder evenly. A u64 product is safe here: both
            // factors are below MAX_SAMPLES, even on wasm32.
            let start =
                stratum * base_width + (stratum as u64 * remainder as u64 / count as u64) as usize;
            let next = stratum + 1;
            let end = next * base_width + (next as u64 * remainder as u64 / count as u64) as usize;
            let width = end - start;
            let mut hash = (stratum as u64)
                .wrapping_add((size as u64).wrapping_mul(0x9e3779b97f4a7c15))
                .wrapping_add(0x9e3779b97f4a7c15);
            hash = (hash ^ (hash >> 30)).wrapping_mul(0xbf58476d1ce4e5b9);
            hash = (hash ^ (hash >> 27)).wrapping_mul(0x94d049bb133111eb);
            hash ^= hash >> 31;
            start + (hash % width as u64) as usize
        })
        .collect()
}
fn linear(v: f64) -> f64 {
    if v <= 0.04045 {
        v / 12.92
    } else {
        ((v + 0.055) / 1.055).powf(2.4)
    }
}
fn srgb(v: f64) -> f64 {
    let v = v.clamp(0.0, 1.0);
    if v <= 0.0031308 {
        v * 12.92
    } else {
        1.055 * v.powf(1.0 / 2.4) - 0.055
    }
}
pub fn render<T: Sample>(
    source: &[T],
    width: u32,
    height: u32,
    settings: &Settings,
) -> Result<Rendered<T>, &'static str> {
    settings.validate()?;
    let size = (width as usize)
        .checked_mul(height as usize)
        .ok_or("invalid film raster")?;
    if width == 0
        || height == 0
        || width > 30_000
        || height > 30_000
        || size > MAX_PIXELS
        || source.len() != size * 4
    {
        return Err("invalid film raster");
    }
    let samples = sample_indices(size);
    let count = samples.len();
    let base = if let Some(base) = settings.base {
        base
    } else {
        let mut base_hist = vec![vec![0u32; 256]; 3];
        for &i in &samples {
            for c in 0..3 {
                base_hist[c][(source[i * 4 + c].index() * 255 + T::MAX / 2) / T::MAX] += 1;
            }
        }
        std::array::from_fn(|c| percentile(&base_hist[c], count, 0.95).max(1) as u8)
    };
    let lin = (0..=T::MAX)
        .map(|v| linear(v as f64 / T::MAX as f64))
        .collect::<Vec<_>>();
    let density = base.map(|b| {
        lin.iter()
            .map(|v| -(v / linear(f64::from(b) / 255.0)).clamp(0.0001, 1.0).ln())
            .collect::<Vec<_>>()
    });
    let mut tones = vec![0u32; 2048];
    let mut shadows = 0;
    let mut highlights = 0;
    for &i in &samples {
        let rgb = &source[i * 4..i * 4 + 3];
        shadows += usize::from(rgb.iter().any(|v| v.index() <= T::MAX / 255));
        highlights += usize::from(rgb.iter().any(|v| v.index() >= T::MAX - T::MAX / 255));
        let d =
            (density[0][rgb[0].index()] + density[1][rgb[1].index()] + density[2][rgb[2].index()])
                / 3.0;
        tones[((d / 10.0 * 2047.0).floor() as usize).min(2047)] += 1;
    }
    let low = percentile(&tones, count, 0.01) as f64 / 2047.0 * 10.0;
    let high = percentile(&tones, count, 0.99) as f64 / 2047.0 * 10.0;
    let lut = density.map(|channel| {
        channel
            .iter()
            .enumerate()
            .map(|(v, d)| {
                let tone = if settings.mode == Mode::Positive {
                    lin[v]
                } else if high - low < 0.005 {
                    0.18
                } else {
                    ((d - low) / (high - low)).max(0.0)
                };
                (srgb(
                    0.18 * (tone.max(0.0) / 0.18).powf(settings.contrast)
                        * 2f64.powf(settings.exposure),
                ) * T::MAX as f64)
                    .round() as usize
            })
            .collect::<Vec<_>>()
    });
    let mut pixels = vec![T::default(); source.len()];
    for (input, output) in source
        .as_chunks::<4>()
        .0
        .iter()
        .zip(pixels.as_chunks_mut::<4>().0.iter_mut())
    {
        let rgb = std::array::from_fn::<_, 3, _>(|c| lut[c][input[c].index()]);
        for c in 0..3 {
            output[c] = T::from_index(if settings.mode == Mode::MonoNegative {
                (0.2126 * rgb[0] as f64 + 0.7152 * rgb[1] as f64 + 0.0722 * rgb[2] as f64).round()
                    as usize
            } else {
                rgb[c]
            });
        }
        output[3] = T::from_index(T::MAX);
    }
    Ok(Rendered {
        pixels,
        base,
        shadows: (shadows as f64 / count as f64 * 1000.0).round() / 10.0,
        highlights: (highlights as f64 / count as f64 * 1000.0).round() / 10.0,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rgba8(pixels: &[[u8; 3]]) -> Vec<u8> {
        pixels
            .iter()
            .flat_map(|rgb| [rgb[0], rgb[1], rgb[2], 0])
            .collect()
    }

    #[test]
    fn stratified_samples_are_deterministic_unique_in_bounds_and_cover_the_raster() {
        let size = MAX_SAMPLES * 17 + 113;
        let first = sample_indices(size);
        let second = sample_indices(size);

        assert_eq!(first, second);
        assert_eq!(first.len(), MAX_SAMPLES);
        assert!(first.iter().all(|&index| index < size));
        assert!(first.windows(2).all(|pair| pair[0] < pair[1]));
        for (stratum, &index) in first.iter().enumerate() {
            let start = stratum * size / MAX_SAMPLES;
            let end = (stratum + 1) * size / MAX_SAMPLES;
            assert!(
                (start..end).contains(&index),
                "sample {index} missed stratum {stratum}"
            );
        }
    }

    #[test]
    fn visits_every_pixel_below_the_cap_and_handles_cap_edges() {
        for size in [1, MAX_SAMPLES - 1, MAX_SAMPLES, MAX_SAMPLES + 1] {
            let samples = sample_indices(size);
            assert_eq!(samples.len(), size.min(MAX_SAMPLES));
            assert!(samples.iter().all(|&index| index < size));
            assert!(samples.windows(2).all(|pair| pair[0] < pair[1]));
            if size <= MAX_SAMPLES {
                assert_eq!(samples, (0..size).collect::<Vec<_>>());
            }
        }
    }

    #[test]
    fn periodic_stripes_do_not_lock_to_a_fixed_stride() {
        let size = 2_000_003;
        let samples = sample_indices(size);
        let full_mean = (0..size)
            .map(|index| if index % 10 < 5 { 1.0 } else { 0.0 })
            .sum::<f64>()
            / size as f64;
        let sampled_mean = samples
            .iter()
            .map(|&index| if index % 10 < 5 { 1.0 } else { 0.0 })
            .sum::<f64>()
            / samples.len() as f64;
        let old_step = size.div_ceil(MAX_SAMPLES).max(1);
        let fixed_step_mean = (0..size)
            .step_by(old_step)
            .map(|index| if index % 10 < 5 { 1.0 } else { 0.0 })
            .sum::<f64>()
            / size.div_ceil(old_step) as f64;
        assert!((fixed_step_mean - full_mean).abs() > 0.05);
        assert!(
            (sampled_mean - full_mean).abs() < 0.01,
            "{sampled_mean} vs {full_mean}"
        );
    }

    #[test]
    fn flat_fields_and_gradients_are_representative_in_wide_and_tall_rasters() {
        for (width, height) in [(2_000_000usize, 1usize), (1usize, 2_000_000usize)] {
            let size = width * height;
            let samples = sample_indices(size);
            let flat = samples.iter().map(|_| 37.0).sum::<f64>() / samples.len() as f64;
            assert_eq!(flat, 37.0);

            let sampled = samples
                .iter()
                .map(|&index| index as f64 / (size - 1) as f64)
                .sum::<f64>()
                / samples.len() as f64;
            let full = (size - 1) as f64 / 2.0 / (size - 1) as f64;
            assert!(
                (sampled - full).abs() < 0.005,
                "{width}x{height}: {sampled} vs {full}"
            );

            let bins = 16usize;
            let mut full_hist = vec![0usize; bins];
            for index in 0..size {
                full_hist[index * bins / size] += 1;
            }
            let mut sampled_hist = vec![0usize; bins];
            for &index in &samples {
                sampled_hist[index * bins / size] += 1;
            }
            for (bin, (sample_count, full_count)) in
                sampled_hist.iter().zip(full_hist.iter()).enumerate()
            {
                let sample_share = *sample_count as f64 / samples.len() as f64;
                let full_share = *full_count as f64 / size as f64;
                assert!(
                    (sample_share - full_share).abs() < 0.005,
                    "{width}x{height}, bin {bin}: {sample_share} vs {full_share}"
                );
            }
        }
    }

    #[test]
    fn positive_at_neutral_controls_preserves_8_bit_pixels_and_alpha_is_opaque() {
        let source = rgba8(&[[10, 80, 200], [220, 160, 40]]);
        let rendered = render(&source, 2, 1, &Settings::default()).unwrap();
        assert_eq!(&rendered.pixels[..3], &[10, 80, 200]);
        assert_eq!(&rendered.pixels[4..7], &[220, 160, 40]);
        assert_eq!(rendered.pixels[3], 255);
        assert_eq!(rendered.pixels[7], 255);
    }

    #[test]
    fn mono_negative_inverts_density_and_remains_achromatic() {
        let pixels = (0..64u8)
            .flat_map(|index| [30 + index * 3, 30 + index * 3, 30 + index * 3, 255])
            .collect::<Vec<_>>();
        let rendered = render(
            &pixels,
            64,
            1,
            &Settings {
                mode: Mode::MonoNegative,
                ..Settings::default()
            },
        )
        .unwrap();
        assert!(rendered.pixels[4 * 10] > rendered.pixels[4 * 50]);
        let (pixels, remainder) = rendered.pixels.as_chunks::<4>();
        assert!(remainder.is_empty());
        for pixel in pixels {
            assert_eq!(pixel[0], pixel[1]);
            assert_eq!(pixel[1], pixel[2]);
        }
    }

    #[test]
    fn calibrated_color_base_removes_equal_channel_transmission() {
        let base = [230, 180, 120];
        let source = (0..100u16)
            .flat_map(|i| {
                let transmission = 0.1 + f64::from(i) * 0.008;
                base.map(|channel| {
                    (linear(f64::from(channel) / 255.0) * transmission).clamp(0.0, 1.0)
                })
                .map(|value| (srgb(value) * 255.0).round() as u8)
                .into_iter()
                .chain([255])
            })
            .collect::<Vec<_>>();
        let settings = Settings {
            mode: Mode::ColorNegative,
            base: Some(base),
            ..Settings::default()
        };
        let rendered = render(&source, 100, 1, &settings).unwrap();
        let middle = &rendered.pixels[200..203];
        let spread = middle.iter().max().unwrap() - middle.iter().min().unwrap();
        assert!(spread <= 4, "channel spread: {spread}, output: {middle:?}");
        assert_eq!(rendered.base, base);

        let uncalibrated = render(
            &source,
            100,
            1,
            &Settings {
                mode: Mode::ColorNegative,
                base: Some([255; 3]),
                ..Settings::default()
            },
        )
        .unwrap();
        assert_ne!(&uncalibrated.pixels[200..203], middle);
    }

    #[test]
    fn clipping_bounds_are_reported_and_16_bit_positive_is_preserved() {
        let source = [
            0u16, 5000, 6000, 0, 20_000, 22_000, 30_000, 0, 65_535, 255, 255, 0, 65_535, 25_000,
            25_000, 0,
        ];
        let rendered = render(&source, 4, 1, &Settings::default()).unwrap();
        assert_eq!(rendered.shadows, 50.0);
        assert_eq!(rendered.highlights, 50.0);
        assert_eq!(rendered.pixels[0..3], source[0..3]);
        assert_eq!(rendered.pixels[4..7], source[4..7]);
        assert_eq!(rendered.pixels[3], u16::MAX);
    }

    #[test]
    fn rejects_invalid_dimensions_settings_and_raster_lengths() {
        assert!(render::<u8>(&[], 0, 1, &Settings::default()).is_err());
        assert!(render::<u8>(&[], 1, 1, &Settings::default()).is_err());
        assert!(render::<u8>(&[], 30_001, 1, &Settings::default()).is_err());
        assert!(render::<u8>(&[], 3_500, 3_500, &Settings::default()).is_err());
        let pixels = [1u8, 2, 3, 4];
        let settings = Settings {
            contrast: f64::INFINITY,
            ..Settings::default()
        };
        assert!(render(&pixels, 1, 1, &settings).is_err());
    }
}
