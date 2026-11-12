use craft_anything_formats::{FormatError, pixels, unreal_texture};

fn layout() -> pixels::Layout {
    unreal_texture::layout("PF_DXT3").expect("Unreal DXT3 pixels should be decoded")
}

fn solid_block(colour: u16, alpha: u8) -> [u8; 16] {
    let mut block = [0; 16];
    block[..8].fill(alpha | (alpha << 4));
    block[8..10].copy_from_slice(&colour.to_le_bytes());
    block
}

#[test]
fn unreal_dxt3_uses_sixteen_byte_blocks() {
    let layout = layout();
    assert_eq!(pixels::size(layout, 1, 1), Some(16));
    assert_eq!(pixels::size(layout, 4, 4), Some(16));
    assert_eq!(pixels::size(layout, 5, 3), Some(32));
    assert_eq!(pixels::size(layout, 5, 5), Some(64));
    assert_eq!(pixels::size(layout, u32::MAX, u32::MAX), None);
}

#[test]
fn explicit_alpha_keeps_all_sixteen_levels_in_row_order() {
    let mut block = solid_block(0xf800, 0);
    block[..8].copy_from_slice(&[0x10, 0x32, 0x54, 0x76, 0x98, 0xba, 0xdc, 0xfe]);
    let rgba = pixels::decode(layout(), 4, 4, &block).unwrap();
    for (index, pixel) in rgba.as_chunks::<4>().0.iter().enumerate() {
        assert_eq!(*pixel, [255, 0, 0, u8::try_from(index).unwrap() * 17]);
    }
}

#[test]
fn low_first_colour_still_uses_four_colour_interpolation() {
    let mut block = solid_block(0x001f, 15);
    block[10..12].copy_from_slice(&0xf800u16.to_le_bytes());
    block[12..].fill(0b11_10_01_00);
    let rgba = pixels::decode(layout(), 4, 4, &block).unwrap();
    for row in rgba.as_chunks::<16>().0 {
        assert_eq!(*row, [0, 0, 255, 255, 255, 0, 0, 255, 85, 0, 170, 255, 170, 0, 85, 255]);
    }
}

#[test]
fn clipped_edge_blocks_do_not_shift_rows_or_read_extra_texels() {
    let mut encoded = solid_block(0xf800, 15).to_vec();
    encoded.extend(solid_block(0x07e0, 8));
    let rgba = pixels::decode(layout(), 5, 3, &encoded).unwrap();
    assert_eq!(rgba.len(), 60);
    for row in rgba.as_chunks::<20>().0 {
        assert_eq!(&row[..16], [255, 0, 0, 255].repeat(4));
        assert_eq!(&row[16..], [0, 255, 0, 136]);
    }
}

#[test]
fn complete_blocks_and_rgba_budgets_are_checked_before_decoding() {
    let layout = layout();
    let block = solid_block(0xf800, 15);
    for end in 0..block.len() {
        assert!(matches!(pixels::decode(layout, 4, 4, &block[..end]), Err(FormatError::Truncated(_))));
    }
    assert!(matches!(pixels::decode_with_limit(layout, 4, 4, &[], 63), Err(FormatError::TooLarge(_))));
    assert_eq!(pixels::decode_with_limit(layout, 4, 4, &block, 64).unwrap().len(), 64);
    assert!(matches!(pixels::decode(layout, 0, 4, &[]), Err(FormatError::Malformed(_))));
    assert!(matches!(pixels::decode(layout, u32::MAX, u32::MAX, &[]), Err(FormatError::TooLarge(_))));
    let mut with_next_mip = block.to_vec();
    with_next_mip.extend([0; 16]);
    assert_eq!(pixels::decode(layout, 4, 4, &with_next_mip).unwrap(), pixels::decode(layout, 4, 4, &block).unwrap());
}

#[test]
fn cooked_unreal_texture_reaches_the_shared_decoder() {
    let block = solid_block(0x001f, 15);
    let (uasset, uexp) = craft_anything_testkit::games::unreal_texture_package("T_Alpha", "PF_DXT3", 4, 4, &block, None);
    let package = unreal_texture::package(&uasset).unwrap();
    let export = &package.exports[0];
    let start = usize::try_from(export.serial_offset - package.header_size).unwrap();
    let texture = unreal_texture::texture(&uexp[start..start + usize::try_from(export.serial_size).unwrap()], &package.names).unwrap();
    let mip = &texture.mips[0];
    let unreal_texture::MipData::Inline(offset, length) = mip.data else { panic!("expected inline pixels") };
    let decoded =
        pixels::decode(unreal_texture::layout(&texture.pixel_format).unwrap(), mip.width, mip.height, &uexp[start + offset..start + offset + length]).unwrap();
    assert_eq!(decoded, [0, 0, 255, 255].repeat(16));
}
