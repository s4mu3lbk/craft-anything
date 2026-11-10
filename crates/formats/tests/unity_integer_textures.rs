use craft_anything_formats::pixels::{self, Layout};
use craft_anything_formats::{FormatError, unity_texture};

#[test]
fn r16_uses_little_endian_unsigned_samples_and_the_full_range() {
    for (sample, grey) in [(0u16, 0), (128, 0), (129, 1), (0x1234, 18), (32768, 128), (u16::MAX, 255)] {
        assert_eq!(unity_texture::mip0_size(9, 1, 1), Some(2));
        assert_eq!(unity_texture::decode(9, 1, 1, &sample.to_le_bytes()).unwrap(), [grey, grey, grey, 255]);
    }
}

#[test]
fn rg16_keeps_two_eight_bit_channels_without_inventing_blue() {
    assert_eq!(unity_texture::mip0_size(62, 2, 1), Some(4));
    assert_eq!(unity_texture::decode(62, 2, 1, &[10, 200, 255, 0]).unwrap(), [10, 200, 0, 255, 255, 0, 0, 255]);
}

#[test]
fn both_integer_formats_flip_unity_rows_and_read_only_the_first_mip() {
    assert_eq!(unity_texture::decode(9, 1, 2, &[0, 0, 255, 255, 42, 42]).unwrap(), [255, 255, 255, 255, 0, 0, 0, 255]);
    assert_eq!(unity_texture::decode(62, 1, 2, &[1, 2, 3, 4, 42, 42]).unwrap(), [3, 4, 0, 255, 1, 2, 0, 255]);
    assert_eq!(pixels::decode(Layout::Rg8, 1, 2, &[1, 2, 3, 4]).unwrap(), [1, 2, 0, 255, 3, 4, 0, 255]);
}

#[test]
fn short_mips_empty_dimensions_and_output_budget_excess_are_refused() {
    for (format, layout) in [(9, Layout::R16), (62, Layout::Rg8)] {
        assert!(matches!(unity_texture::decode(format, 1, 1, &[0]), Err(FormatError::Truncated(_))));
        assert!(matches!(unity_texture::decode(format, 0, 1, &[]), Err(FormatError::Malformed(_))));
        assert!(matches!(pixels::decode_with_limit(layout, 1, 1, &[0, 0], 3), Err(FormatError::TooLarge(_))));
        assert!(matches!(unity_texture::decode(format, u32::MAX, u32::MAX, &[]), Err(FormatError::TooLarge(_))));
    }
}
