use craft_anything_formats::{FormatError, pyc};

fn long(negative: bool, digits: &[u16]) -> Vec<u8> {
    let mut bytes = vec![b'l'];
    let count = digits.len() as i32 * if negative { -1 } else { 1 };
    bytes.extend_from_slice(&count.to_le_bytes());
    for digit in digits {
        bytes.extend_from_slice(&digit.to_le_bytes());
    }
    bytes
}

#[test]
fn every_long_digit_must_fit_fifteen_bits() {
    for negative in [false, true] {
        for digits in [vec![0x8000], vec![0xffff], vec![0x8000, 1], vec![1, 0xffff], vec![1, 1, 1, 1, 1, 1, 1, 0x8000]] {
            assert!(matches!(pyc::read_value(&long(negative, &digits), None), Err(FormatError::Malformed(_))), "negative {negative}, digits {digits:?}");
        }
    }
}

#[test]
fn nonzero_long_counts_require_a_nonzero_most_significant_digit() {
    for negative in [false, true] {
        for digits in [vec![0], vec![1, 0], vec![0, 0], vec![1, 1, 1, 1, 1, 1, 1, 0]] {
            assert!(matches!(pyc::read_value(&long(negative, &digits), None), Err(FormatError::Malformed(_))), "negative {negative}, digits {digits:?}");
        }
    }
}

#[test]
fn canonical_zero_small_and_large_integers_still_read() {
    assert_eq!(pyc::read_value(&long(false, &[]), None).unwrap().value, pyc::Value::Int(0));
    for negative in [false, true] {
        for (digits, value) in [(vec![0x7fff], 32767), (vec![0, 1], 32768), (vec![1, 1], 32769)] {
            let expected = if negative { -value } else { value };
            assert_eq!(pyc::read_value(&long(negative, &digits), None).unwrap().value, pyc::Value::Int(expected));
        }
        // Legal integers beyond i64 remain readable even though their numeric value isn't kept.
        assert_eq!(pyc::read_value(&long(negative, &[0, 0, 0, 0, 0, 0, 0, 1]), None).unwrap().value, pyc::Value::Other);
    }
}
