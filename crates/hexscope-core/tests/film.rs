use hexscope_core::film::{Mode, Settings, crop_geometry, render};

#[test]
fn neutral_positive_is_identity_at_both_depths() {
    let pixels = vec![0u8, 1, 2, 255, 128, 254, 255, 255];
    assert_eq!(
        render(&pixels, 2, 1, &Settings::default()).unwrap().pixels,
        pixels
    );
    let pixels = vec![0u16, 257, 300, 65535, 32001, 65400, 65535, 65535];
    assert_eq!(
        render(&pixels, 2, 1, &Settings::default()).unwrap().pixels,
        pixels
    );
}
#[test]
fn negative_inverts_and_mono_is_achromatic() {
    let pixels: Vec<u8> = (0..=255).flat_map(|v| [v, v, v, 255]).collect();
    let s = Settings {
        mode: Mode::MonoNegative,
        base: Some([255, 255, 255]),
        ..Settings::default()
    };
    let out = render(&pixels, 256, 1, &s).unwrap();
    assert!(out.pixels[0] > out.pixels[1020]);
    assert!(
        out.pixels
            .as_chunks::<4>()
            .0
            .iter()
            .all(|p| p[0] == p[1] && p[1] == p[2])
    );
}
#[test]
fn bounds_reject_invalid_raster_settings_and_crop() {
    assert!(render(&[0u8; 4], 1, 2, &Settings::default()).is_err());
    assert!(
        render(
            &[0u8; 4],
            1,
            1,
            &Settings {
                exposure: f64::NAN,
                ..Settings::default()
            }
        )
        .is_err()
    );
    assert!(crop_geometry(100, 100, [0.0, 0.0, 0.5, 0.0]).is_err());
    assert_eq!(
        crop_geometry(101, 50, [0.1, 0.1, 0.1, 0.1]).unwrap(),
        (10, 5, 81, 40)
    );
}
