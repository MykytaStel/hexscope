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
    let step = size.div_ceil(MAX_SAMPLES).max(1);
    let mut base_hist = vec![vec![0u32; 256]; 3];
    let mut count = 0;
    for i in (0..size).step_by(step) {
        for c in 0..3 {
            base_hist[c][(source[i * 4 + c].index() * 255 + T::MAX / 2) / T::MAX] += 1;
        }
        count += 1;
    }
    let base = settings.base.unwrap_or_else(|| {
        std::array::from_fn(|c| percentile(&base_hist[c], count, 0.95).max(1) as u8)
    });
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
    for i in (0..size).step_by(step) {
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
