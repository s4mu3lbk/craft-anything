use craft_anything_formats::{
    FormatError,
    pixels::{self, Layout},
};

#[test]
fn default_rgba_budget_is_checked_before_reading_source_pixels() {
    // The existing texture export limit is an area limit, not a limit on either axis.
    for (width, height) in [(8192, 8192), (65536, 1024)] {
        assert!(matches!(pixels::decode(Layout::R8, width, height, &[]), Err(FormatError::Truncated(_))));
    }
    assert!(matches!(pixels::decode(Layout::R8, 8192, 8193, &[]), Err(FormatError::TooLarge(_))));
}

#[test]
fn explicit_rgba_budget_rejects_expansion_before_source_validation() {
    for layout in [Layout::R8, Layout::Alpha8, Layout::Dxt1, Layout::Bc4] {
        for data in [&[][..], &[0; 16][..]] {
            for limit in [0, 15] {
                assert!(matches!(pixels::decode_with_limit(layout, 2, 2, data, limit), Err(FormatError::TooLarge(_))), "layout {layout:?}, limit {limit}");
            }
        }
        assert!(matches!(pixels::decode_with_limit(layout, 2, 2, &[], 16), Err(FormatError::Truncated(_))), "layout {layout:?}");
    }
}

#[test]
fn rgba_budget_boundary_preserves_plain_pixels_and_cropped_blocks() {
    for limit in [8, 9] {
        assert_eq!(pixels::decode_with_limit(Layout::R8, 2, 1, &[7, 8], limit).unwrap(), [7, 7, 7, 255, 8, 8, 8, 255]);
    }
    assert!(matches!(pixels::decode_with_limit(Layout::Dxt1, 5, 5, &[0; 32], 99), Err(FormatError::TooLarge(_))));
    assert_eq!(pixels::decode_with_limit(Layout::Dxt1, 5, 5, &[0; 32], 100).unwrap(), [0, 0, 0, 255].repeat(25));
    // A single texel still needs a full 16-byte DXT5 block, but only four RGBA output bytes.
    assert_eq!(pixels::decode_with_limit(Layout::Dxt5, 1, 1, &[0; 16], 4).unwrap(), [0; 4]);
}

#[test]
fn caller_budget_can_exceed_default_but_not_allocation_limits() {
    assert!(matches!(pixels::decode_with_limit(Layout::R8, 8192, 8193, &[], u64::MAX), Err(FormatError::Truncated(_))));
    for layout in [Layout::R8, Layout::Alpha8, Layout::Dxt1, Layout::Bc4] {
        for (width, height) in [(u32::MAX, u32::MAX), (1 << 31, 1 << 30)] {
            assert!(matches!(pixels::decode_with_limit(layout, width, height, &[], u64::MAX), Err(FormatError::TooLarge(_))), "layout {layout:?}");
        }
    }
}

#[test]
fn empty_pictures_remain_malformed_with_a_zero_budget() {
    for (width, height) in [(0, 2), (2, 0)] {
        assert!(matches!(pixels::decode_with_limit(Layout::R8, width, height, &[], 0), Err(FormatError::Malformed(_))));
    }
}

#[test]
fn packed_16_bit_pixels_keep_channel_order_at_the_budget_boundary() {
    for (layout, data, expected) in [
        (Layout::Rgb565, [0x00, 0xf8, 0xe0, 0x07], [255, 0, 0, 255, 0, 255, 0, 255]),
        (Layout::Rgba4444, [0x0f, 0xf0, 0xff, 0x00], [255, 0, 0, 255, 0, 0, 255, 255]),
        (Layout::Argb4444, [0x00, 0xff, 0xf0, 0x80], [255, 0, 0, 255, 0, 255, 0, 136]),
    ] {
        assert_eq!(pixels::decode_with_limit(layout, 2, 1, &data, 8).unwrap(), expected);
        assert_eq!(pixels::decode(layout, 2, 1, &data).unwrap(), expected);
    }
}

#[test]
fn expanded_rgba_dimensions_are_checked_before_reading_or_allocating() {
    for layout in [Layout::R8, Layout::Alpha8, Layout::Dxt1, Layout::Bc4] {
        assert!(matches!(pixels::decode(layout, u32::MAX, u32::MAX, &[]), Err(FormatError::TooLarge(_))), "layout {layout:?}");
        // The product fits u64 but exceeds the largest byte vector on a 64-bit target.
        assert!(matches!(pixels::decode(layout, 1 << 31, 1 << 30, &[]), Err(FormatError::TooLarge(_))), "layout {layout:?}");
    }
}

#[cfg(target_pointer_width = "32")]
#[test]
fn rgba_expansion_cannot_wrap_a_32_bit_output_length() {
    assert!(matches!(pixels::decode(Layout::Dxt1, 65536, 16384, &[]), Err(FormatError::TooLarge(_))));
}

#[test]
fn ordinary_pixel_sizes_and_truncated_inputs_keep_their_results() {
    assert_eq!(pixels::decode(Layout::R8, 2, 1, &[7, 8]).unwrap(), [7, 7, 7, 255, 8, 8, 8, 255]);
    assert_eq!(pixels::decode(Layout::Dxt1, 5, 5, &[0; 32]).unwrap().len(), 100);
    assert!(matches!(pixels::decode(Layout::R8, 2, 1, &[7]), Err(FormatError::Truncated(_))));
    assert!(matches!(pixels::decode(Layout::Dxt1, 0, 1, &[]), Err(FormatError::Malformed(_))));
}
