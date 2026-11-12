use craft_anything_formats::{FormatError, unity_texture};

fn packed(red: u32, green: u32, blue: u32, exponent: u32) -> [u8; 4] {
    (red | (green << 9) | (blue << 18) | (exponent << 27)).to_le_bytes()
}

#[test]
fn unity_shared_exponent_format_has_a_name_and_mip_size() {
    assert_eq!(unity_texture::format_name(22), "RGB9e5Float");
    assert_eq!(unity_texture::mip0_size(22, 3, 2), Some(24));
}

#[test]
fn shared_exponent_channels_use_existing_linear_to_srgb_conversion() {
    let pixel = packed(256, 128, 64, 15);
    assert_eq!(unity_texture::decode(22, 1, 1, &pixel).unwrap(), [188, 137, 99, 255]);
    let floats: Vec<u8> = [0.5f32, 0.25, 0.125, 1.0].into_iter().flat_map(f32::to_le_bytes).collect();
    assert_eq!(unity_texture::decode(22, 1, 1, &pixel).unwrap(), unity_texture::decode(20, 1, 1, &floats).unwrap());
    assert_eq!(unity_texture::decode(22, 1, 1, &packed(256, 128, 64, 16)).unwrap(), [255, 188, 137, 255]);
}

#[test]
fn exponent_extremes_are_finite_and_alpha_is_opaque() {
    for pixel in [packed(0, 0, 0, 0), packed(0, 0, 0, 31), packed(511, 511, 511, 0)] {
        assert_eq!(unity_texture::decode(22, 1, 1, &pixel).unwrap(), [0, 0, 0, 255]);
    }
    assert_eq!(unity_texture::decode(22, 1, 1, &packed(511, 511, 511, 31)).unwrap(), [255, 255, 255, 255]);
}

#[test]
fn unity_row_order_is_inverted_after_unpacking() {
    let mut input = packed(256, 0, 0, 16).to_vec();
    input.extend(packed(0, 256, 0, 16));
    assert_eq!(unity_texture::decode(22, 1, 2, &input).unwrap(), [0, 255, 0, 255, 255, 0, 0, 255]);
}

#[test]
fn rgb9e5_uses_existing_truncation_and_rgba_budget_checks() {
    let pixel = packed(256, 128, 64, 15);
    for end in 0..pixel.len() {
        assert!(matches!(unity_texture::decode(22, 1, 1, &pixel[..end]), Err(FormatError::Truncated(_))));
    }
    assert!(matches!(unity_texture::decode(22, 0, 1, &[]), Err(FormatError::Malformed(_))));
    assert!(matches!(unity_texture::decode(22, 8193, 8193, &[]), Err(FormatError::TooLarge(_))));
    let mut with_next_mip = pixel.to_vec();
    with_next_mip.extend([0xff; 4]);
    assert_eq!(unity_texture::decode(22, 1, 1, &with_next_mip).unwrap(), [188, 137, 99, 255]);
}

#[test]
fn serialized_texture_reaches_the_shared_exponent_decoder() {
    let pixel = packed(256, 128, 64, 15);
    let version = "2022.3.12f1";
    let object = craft_anything_testkit::games::unity_texture2d(version, "SharedLight", 1, 1, 22, Some(&pixel), None);
    let texture = unity_texture::parse(&object, version, false).unwrap();
    let (start, length) = texture.inline.unwrap();
    assert_eq!(unity_texture::mip0_size(texture.format, texture.width, texture.height), Some(length));
    assert_eq!(unity_texture::decode(texture.format, texture.width, texture.height, &object[start..start + length]).unwrap(), [188, 137, 99, 255]);
}
