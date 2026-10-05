use hexscope_raster::{Output, process, read_recipe};
use std::io::Cursor;

const RECIPE: &str = r#"{"schema":"hexscope.film-recipe","version":1,"algorithm":"srgb-density-v1","settings":{"mode":"positive","crop":[0,0,0,0],"exposure":0,"contrast":1,"baseColor":null}}"#;
#[test]
fn tiff16_roundtrip_keeps_sub_byte_precision_and_strips_tags() {
    let rgb = vec![257u16, 300, 32001, 50000, 65400, 65535];
    let mut bytes = Cursor::new(Vec::new());
    tiff::encoder::TiffEncoder::new(&mut bytes)
        .unwrap()
        .write_image::<tiff::encoder::colortype::RGB16>(2, 1, &rgb)
        .unwrap();
    let output = process(
        bytes.get_ref(),
        &read_recipe(RECIPE).unwrap(),
        Output::Tiff16,
        4096,
    )
    .unwrap();
    assert_eq!(
        (output.width, output.height, output.source_depth),
        (2, 1, 16)
    );
    let mut decoder = tiff::decoder::Decoder::new(Cursor::new(&output.bytes)).unwrap();
    match decoder.read_image().unwrap() {
        tiff::decoder::DecodingResult::U16(actual) => assert_eq!(actual, rgb),
        _ => panic!("lost 16-bit depth"),
    }
    assert!(!decoder.more_images());
}
#[test]
fn rejects_bad_recipes_and_multi_image_tiffs() {
    assert!(read_recipe(&RECIPE.replace("srgb-density-v1", "unknown")).is_err());
    assert!(read_recipe(&RECIPE.replace("\"contrast\":1", "\"contrast\":99")).is_err());
    let mut bytes = Cursor::new(Vec::new());
    {
        let mut encoder = tiff::encoder::TiffEncoder::new(&mut bytes).unwrap();
        for _ in 0..2 {
            encoder
                .write_image::<tiff::encoder::colortype::RGB8>(1, 1, &[255, 0, 0])
                .unwrap();
        }
    }
    assert!(
        process(
            bytes.get_ref(),
            &read_recipe(RECIPE).unwrap(),
            Output::Tiff16,
            4096
        )
        .unwrap_err()
        .contains("multi")
    );
}

#[test]
fn reports_embedded_profile_and_rejects_missing_recipe_settings() {
    let mut bytes = Cursor::new(Vec::new());
    {
        let mut encoder = tiff::encoder::TiffEncoder::new(&mut bytes).unwrap();
        let mut image = encoder
            .new_image::<tiff::encoder::colortype::RGB16>(1, 1)
            .unwrap();
        image
            .encoder()
            .write_tag(tiff::tags::Tag::IccProfile, &[1u8, 2, 3, 4][..])
            .unwrap();
        image.write_data(&[300, 32001, 65400]).unwrap();
    }
    let result = process(
        bytes.get_ref(),
        &read_recipe(RECIPE).unwrap(),
        Output::Tiff16,
        4096,
    )
    .unwrap();
    assert!(result.profile_present);
    assert!(read_recipe(&RECIPE.replace(",\"baseColor\":null", "")).is_err());
}

#[test]
fn dimensions_and_decode_allocations_are_checked_before_pixels() {
    let source = include_bytes!("fixtures/precision16.tif");
    let mut oversized = source.to_vec();
    oversized[18..22].copy_from_slice(&40000u32.to_le_bytes());
    let result = process(
        &oversized,
        &read_recipe(RECIPE).unwrap(),
        Output::Tiff16,
        4096,
    );
    assert!(result.unwrap_err().contains("dimension"));
}

#[test]
fn rejects_associated_alpha_tiffs_instead_of_darkening_their_pixels() {
    for depth in [8, 16] {
        let mut bytes = Cursor::new(Vec::new());
        let mut encoder = tiff::encoder::TiffEncoder::new(&mut bytes).unwrap();
        if depth == 8 {
            let mut image = encoder
                .new_image::<tiff::encoder::colortype::RGBA8>(1, 1)
                .unwrap();
            image
                .encoder()
                .write_tag(tiff::tags::Tag::ExtraSamples, &[1u16][..])
                .unwrap();
            image.write_data(&[128, 0, 0, 128]).unwrap();
        } else {
            let mut image = encoder
                .new_image::<tiff::encoder::colortype::RGBA16>(1, 1)
                .unwrap();
            image
                .encoder()
                .write_tag(tiff::tags::Tag::ExtraSamples, &[1u16][..])
                .unwrap();
            image.write_data(&[32896, 0, 0, 32896]).unwrap();
        }
        let error = process(
            bytes.get_ref(),
            &read_recipe(RECIPE).unwrap(),
            Output::Tiff16,
            4096,
        )
        .unwrap_err();
        assert!(error.contains("associated alpha"), "{error}");
    }
}
