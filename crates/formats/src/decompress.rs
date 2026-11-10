//! Bounded, pure-Rust decompressors for the stream formats a firmware image commonly carries:
//! xz, standalone LZMA (`.lzma`), bzip2 and Zstandard. Each decoder caps its output at `max_out`
//! bytes (returning `TooLarge` rather than allocating without limit) and never panics on a corrupt
//! or truncated stream. gzip/zlib/zip live in `firmware`; these complete the set.
//!
//! All three backing crates (`lzma-rs`, `bzip2-rs`, `ruzstd`) are pure Rust and build for
//! `wasm32-unknown-unknown`.

use std::io::{self, Read, Write};

/// Why a decode stopped.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum DecompressError {
    #[error("the compressed stream is damaged")]
    Corrupt,
    #[error("the compressed stream ends early")]
    Truncated,
    #[error("the output would exceed the size limit")]
    TooLarge,
}

/// A `Write` sink that collects at most `cap` bytes, then refuses the rest. The `exceeded` flag lets
/// the caller tell a size-limit stop from a genuine decode error.
struct CapWrite {
    buf: Vec<u8>,
    cap: usize,
    exceeded: bool,
}

impl CapWrite {
    fn new(cap: usize) -> Self {
        CapWrite { buf: Vec::new(), cap, exceeded: false }
    }
}

impl Write for CapWrite {
    fn write(&mut self, data: &[u8]) -> io::Result<usize> {
        if self.buf.len().saturating_add(data.len()) > self.cap {
            self.exceeded = true;
            return Err(io::Error::other("output size limit reached"));
        }
        self.buf.extend_from_slice(data);
        Ok(data.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

fn cap_of(max_out: u64) -> usize {
    usize::try_from(max_out).unwrap_or(usize::MAX)
}

/// Read a `Read` source into a `Vec`, capped at `cap` bytes.
fn read_capped(mut src: impl Read, cap: usize) -> Result<Vec<u8>, DecompressError> {
    let mut out = Vec::new();
    let mut chunk = [0u8; 64 * 1024];
    loop {
        match src.read(&mut chunk) {
            Ok(0) => return Ok(out),
            Ok(n) => {
                let slice = chunk.get(..n).unwrap_or(&[]);
                if out.len().saturating_add(slice.len()) > cap {
                    return Err(DecompressError::TooLarge);
                }
                out.extend_from_slice(slice);
            }
            Err(e) if e.kind() == io::ErrorKind::Interrupted => continue,
            Err(_) => return Err(DecompressError::Corrupt),
        }
    }
}

/// Decompress an xz stream, producing at most `max_out` bytes.
pub fn xz(input: &[u8], max_out: u64) -> Result<Vec<u8>, DecompressError> {
    let mut out = CapWrite::new(cap_of(max_out));
    let mut cursor = io::Cursor::new(input);
    match lzma_rs::xz_decompress(&mut cursor, &mut out) {
        Ok(()) => Ok(out.buf),
        Err(_) if out.exceeded => Err(DecompressError::TooLarge),
        Err(lzma_rs::error::Error::HeaderTooShort(_)) => Err(DecompressError::Truncated),
        Err(_) => Err(DecompressError::Corrupt),
    }
}

/// Decompress a standalone LZMA (`.lzma`, `0x5d` props) stream, producing at most `max_out` bytes.
pub fn lzma(input: &[u8], max_out: u64) -> Result<Vec<u8>, DecompressError> {
    let cap = cap_of(max_out).min(isize::MAX as usize);
    let (header, stream) = input.split_at_checked(13).ok_or(DecompressError::Truncated)?;
    let mut header = <[u8; 13]>::try_from(header).map_err(|_| DecompressError::Truncated)?;
    let declared = u64::from_le_bytes(header[5..13].try_into().map_err(|_| DecompressError::Truncated)?);
    if declared != u64::MAX && declared > cap as u64 {
        return Err(DecompressError::TooLarge);
    }
    // The decoder retains a window before flushing to CapWrite. Bound that window too,
    // so a tiny output budget cannot first grow an arbitrarily large dictionary.
    let dictionary = u32::from_le_bytes(header[1..5].try_into().map_err(|_| DecompressError::Truncated)?);
    let window_limit = u32::try_from(cap).unwrap_or(u32::MAX).max(4096);
    let dictionary = dictionary.max(4096).min(window_limit);
    header[1..5].copy_from_slice(&dictionary.to_le_bytes());
    let options = lzma_rs::decompress::Options { memlimit: Some(dictionary as usize), ..Default::default() };
    let mut out = CapWrite::new(cap);
    let mut cursor = io::Cursor::new(header).chain(io::Cursor::new(stream));
    match lzma_rs::lzma_decompress_with_options(&mut cursor, &mut out, &options) {
        Ok(()) => Ok(out.buf),
        Err(_) if out.exceeded => Err(DecompressError::TooLarge),
        Err(lzma_rs::error::Error::HeaderTooShort(_)) => Err(DecompressError::Truncated),
        Err(_) => Err(DecompressError::Corrupt),
    }
}

/// Decode a complete LZMA piece with properties but no size header, bounding its dictionary and output.
pub(crate) fn lzma_raw_complete(input: &[u8], out_len: u32) -> Result<Vec<u8>, DecompressError> {
    let cap = usize::try_from(out_len).map_err(|_| DecompressError::TooLarge)?;
    if cap > isize::MAX as usize {
        return Err(DecompressError::TooLarge);
    }
    let (properties, stream) = input.split_at_checked(5).ok_or(DecompressError::Truncated)?;
    let [property, d0, d1, d2, d3] = <[u8; 5]>::try_from(properties).map_err(|_| DecompressError::Truncated)?;
    // No match can reach farther back than the entire output. Keep the decoder's minimum window,
    // but avoid oversized dictionary arithmetic on 32-bit targets before its memory limit is checked.
    let dictionary = u32::from_le_bytes([d0, d1, d2, d3]).max(4096).min(out_len.max(4096));
    let [d0, d1, d2, d3] = dictionary.to_le_bytes();
    let properties = [property, d0, d1, d2, d3];
    let decode = |size: Option<u64>| {
        let options =
            lzma_rs::decompress::Options { unpacked_size: lzma_rs::decompress::UnpackedSize::UseProvided(size), memlimit: Some(cap), ..Default::default() };
        let mut out = CapWrite::new(cap);
        let mut cursor = io::Cursor::new(properties).chain(io::Cursor::new(stream));
        match lzma_rs::lzma_decompress_with_options(&mut cursor, &mut out, &options) {
            Ok(()) if cursor.get_ref().1.position() == stream.len() as u64 => Ok(out.buf),
            Ok(()) => Err(DecompressError::Corrupt),
            Err(_) if out.exceeded => Err(DecompressError::TooLarge),
            Err(lzma_rs::error::Error::HeaderTooShort(_)) => Err(DecompressError::Truncated),
            Err(_) => Err(DecompressError::Corrupt),
        }
    };
    let output = decode(None)?;
    if output.len() == cap {
        return Ok(output);
    }
    // A zero range code at EOF can precede the last zero literals in a stream without an end marker.
    // Retry a short result with its known size only after completion succeeded; never recover a failed probe.
    drop(output);
    decode(Some(u64::from(out_len)))
}

/// Decode the property bytes and raw stream of a ZIP method-14 member.
pub(crate) fn lzma_zip(input: &[u8], expected_size: u64, has_end_marker: bool, max_out: u64) -> Result<Vec<u8>, DecompressError> {
    let cap = cap_of(max_out).min(isize::MAX as usize);
    if expected_size > cap as u64 {
        return Err(DecompressError::TooLarge);
    }
    let (properties, stream) = input.split_at_checked(5).ok_or(DecompressError::Truncated)?;
    let [property, d0, d1, d2, d3] = <[u8; 5]>::try_from(properties).map_err(|_| DecompressError::Truncated)?;
    let window_limit = u32::try_from(cap).unwrap_or(u32::MAX).max(4096);
    let dictionary = u32::from_le_bytes([d0, d1, d2, d3]).max(4096).min(window_limit);
    let [d0, d1, d2, d3] = dictionary.to_le_bytes();
    let properties = [property, d0, d1, d2, d3];
    let size = if has_end_marker { None } else { Some(expected_size) };
    let options = lzma_rs::decompress::Options {
        unpacked_size: lzma_rs::decompress::UnpackedSize::UseProvided(size),
        memlimit: Some(dictionary as usize),
        ..Default::default()
    };
    let mut out = CapWrite::new(cap);
    let mut cursor = io::Cursor::new(properties).chain(io::Cursor::new(stream));
    match lzma_rs::lzma_decompress_with_options(&mut cursor, &mut out, &options) {
        Ok(()) if cursor.get_ref().1.position() == stream.len() as u64 => Ok(out.buf),
        Ok(()) => Err(DecompressError::Corrupt),
        Err(_) if out.exceeded => Err(DecompressError::TooLarge),
        Err(lzma_rs::error::Error::HeaderTooShort(_)) => Err(DecompressError::Truncated),
        Err(_) => Err(DecompressError::Corrupt),
    }
}

/// Decompress a bzip2 stream, producing at most `max_out` bytes.
pub fn bzip2(input: &[u8], max_out: u64) -> Result<Vec<u8>, DecompressError> {
    read_capped(bzip2_rs::DecoderReader::new(input), cap_of(max_out))
}

/// Decompress a Zstandard stream, producing at most `max_out` bytes.
pub fn zstd(mut input: &[u8], max_out: u64) -> Result<Vec<u8>, DecompressError> {
    if input.is_empty() {
        return Err(DecompressError::Corrupt);
    }
    let mut out = CapWrite::new(cap_of(max_out));
    while !input.is_empty() {
        if matches!(input.get(..4), Some([0x50..=0x5f, 0x2a, 0x4d, 0x18])) {
            let header = input.get(4..8).and_then(|s| s.try_into().ok()).ok_or(DecompressError::Truncated)?;
            let len = u32::from_le_bytes(header) as usize;
            input = input.get(8..).and_then(|body| body.get(len..)).ok_or(DecompressError::Truncated)?;
            continue;
        }
        let mut decoder = ruzstd::decoding::StreamingDecoder::new(&mut input).map_err(|_| DecompressError::Corrupt)?;
        if io::copy(&mut decoder, &mut out).is_err() {
            return Err(if out.exceeded { DecompressError::TooLarge } else { DecompressError::Corrupt });
        }
    }
    Ok(out.buf)
}

/// Decompress one raw LZ4 block (no frame) that inflates to exactly `out_len` bytes, as Unity
/// bundles store them.
pub fn lz4_block(input: &[u8], out_len: usize) -> Result<Vec<u8>, DecompressError> {
    let mut out = Vec::with_capacity(out_len.min(64 << 20));
    let mut i = 0usize;
    let byte = |i: &mut usize| -> Result<u8, DecompressError> {
        let b = *input.get(*i).ok_or(DecompressError::Truncated)?;
        *i += 1;
        Ok(b)
    };
    // A 4-bit length of 15 continues in the following bytes, 255 at a time.
    let length = |i: &mut usize, base: u8| -> Result<usize, DecompressError> {
        let mut n = usize::from(base);
        if base == 15 {
            loop {
                let b = byte(i)?;
                n = n.checked_add(usize::from(b)).ok_or(DecompressError::Corrupt)?;
                if b != 255 {
                    break;
                }
            }
        }
        Ok(n)
    };
    while i < input.len() {
        let token = byte(&mut i)?;
        let literals = length(&mut i, token >> 4)?;
        let end = i.checked_add(literals).ok_or(DecompressError::Corrupt)?;
        let run = input.get(i..end).ok_or(DecompressError::Truncated)?;
        if out.len() + literals > out_len {
            return Err(DecompressError::TooLarge);
        }
        out.extend_from_slice(run);
        i = end;
        // The last sequence has literals only.
        if i >= input.len() {
            break;
        }
        let offset = usize::from(u16::from_le_bytes([byte(&mut i)?, byte(&mut i)?]));
        if offset == 0 || offset > out.len() {
            return Err(DecompressError::Corrupt);
        }
        let len = length(&mut i, token & 15)?.checked_add(4).ok_or(DecompressError::Corrupt)?;
        if out.len() + len > out_len {
            return Err(DecompressError::TooLarge);
        }
        // Each copied run extends the same repeating pattern, so it can
        // supply the next run even when the match overlaps its destination.
        let start = out.len() - offset;
        let mut remaining = len;
        while remaining > 0 {
            let count = remaining.min(out.len() - start);
            out.extend_from_within(start..start + count);
            remaining -= count;
        }
    }
    if out.len() != out_len {
        return Err(DecompressError::Truncated);
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use craft_anything_testkit::firmware as fx;

    #[test]
    fn lz4_blocks_with_literals_and_overlapping_matches() {
        // "abcabcabcabc!": 3 literals, a 9-byte match 3 back (overlapping), then "!".
        let block = [0x35, b'a', b'b', b'c', 3, 0, 0x10, b'!'];
        assert_eq!(lz4_block(&block, 13).unwrap(), b"abcabcabcabc!");
        // A long literal run uses the extra length bytes.
        let mut long = vec![0xf0, 5]; // 15 + 5 = 20 literals
        long.extend_from_slice(&[b'x'; 20]);
        assert_eq!(lz4_block(&long, 20).unwrap(), [b'x'; 20]);
        assert_eq!(lz4_block(&block, 12), Err(DecompressError::TooLarge));
        assert_eq!(lz4_block(&block, 14), Err(DecompressError::Truncated));
        assert_eq!(lz4_block(&[0x10, b'a', 9, 0], 10), Err(DecompressError::Corrupt), "a match before the start");
        for n in 0..64u8 {
            let junk: Vec<u8> = (0..n).map(|k| k.wrapping_mul(37).wrapping_add(n)).collect();
            let _ = lz4_block(&junk, 100);
        }
    }

    #[test]
    fn long_lz4_matches_repeat_the_entire_offset_pattern() {
        for pattern in [&b"x"[..], &b"abc"[..], &b"01234567"[..]] {
            let mut block = vec![((pattern.len() as u8) << 4) | 15];
            block.extend_from_slice(pattern);
            block.extend_from_slice(&(pattern.len() as u16).to_le_bytes());
            block.extend_from_slice(&[255, 255, 7, 0x50]);
            block.extend_from_slice(b"final");
            let mut expected = pattern.to_vec();
            expected.extend(pattern.iter().copied().cycle().take(19 + 255 + 255 + 7));
            expected.extend_from_slice(b"final");
            assert_eq!(lz4_block(&block, expected.len()).unwrap(), expected);
        }
    }

    #[test]
    fn xz_round_trip() {
        let c = fx::XZ_FIXTURE;
        assert_eq!(xz(c, 1 << 20).unwrap(), fx::XZ_PLAIN);
        // A tiny cap reports TooLarge, never a partial lie.
        assert_eq!(xz(c, 4), Err(DecompressError::TooLarge));
        // Truncation never panics.
        for len in 0..c.len() {
            let _ = xz(&c[..len], 1 << 20);
        }
    }

    #[test]
    fn lzma_round_trip() {
        let c = fx::LZMA_FIXTURE;
        assert_eq!(lzma(c, 1 << 20).unwrap(), fx::LZMA_PLAIN);
        assert_eq!(lzma(c, 4), Err(DecompressError::TooLarge));
        for len in 0..c.len() {
            let _ = lzma(&c[..len], 1 << 20);
        }
    }

    #[test]
    fn standalone_lzma_checks_declared_sizes_before_decoding() {
        let mut header = [0u8; 13];
        header[0] = 0x5d;
        header[1..5].copy_from_slice(&u32::MAX.to_le_bytes());
        header[5..13].copy_from_slice(&1_000_000_000u64.to_le_bytes());
        assert_eq!(lzma(&header, 64), Err(DecompressError::TooLarge));
        assert_eq!(lzma(&header[..12], 64), Err(DecompressError::Truncated));
        let mut valid = fx::LZMA_FIXTURE.to_vec();
        valid[1..5].copy_from_slice(&u32::MAX.to_le_bytes());
        assert_eq!(lzma(&valid, fx::LZMA_PLAIN.len() as u64).unwrap(), fx::LZMA_PLAIN);
        assert_eq!(lzma(&valid, 0), Err(DecompressError::TooLarge));
    }

    #[test]
    fn standalone_lzma_stops_before_a_large_window_reaches_a_damaged_tail() {
        let plain = vec![b'a'; 16 * 1024];
        let mut compressed = Vec::new();
        lzma_rs::lzma_compress(&mut io::Cursor::new(&plain), &mut compressed).unwrap();
        compressed.truncate(compressed.len() - 5);
        assert_eq!(lzma(&compressed, 64), Err(DecompressError::TooLarge));
        assert_eq!(lzma(&compressed, plain.len() as u64), Err(DecompressError::Corrupt));
    }

    #[test]
    fn bzip2_round_trip() {
        let c = fx::BZIP2_FIXTURE;
        assert_eq!(bzip2(c, 1 << 20).unwrap(), fx::BZIP2_PLAIN);
        assert_eq!(bzip2(c, 2), Err(DecompressError::TooLarge));
        for len in 0..c.len() {
            let _ = bzip2(&c[..len], 1 << 20);
        }
    }

    #[test]
    fn zstd_round_trip() {
        let c = fx::ZSTD_FIXTURE;
        assert_eq!(zstd(c, 1 << 20).unwrap(), fx::ZSTD_PLAIN);
        assert_eq!(zstd(c, 2), Err(DecompressError::TooLarge));
        for len in 0..c.len() {
            let _ = zstd(&c[..len], 1 << 20);
        }
    }

    #[test]
    fn zstandard_members_can_contain_multiple_frames() {
        let frame = fx::ZSTD_FIXTURE;
        let plain = fx::ZSTD_PLAIN;
        let skip = [0x50, 0x2a, 0x4d, 0x18, 3, 0, 0, 0, 1, 2, 3];
        let input = [frame, &skip, frame].concat();
        let expected = [plain, plain].concat();
        assert_eq!(zstd(&input, expected.len() as u64).unwrap(), expected);
        assert_eq!(zstd(&input, plain.len() as u64), Err(DecompressError::TooLarge));
        assert!(zstd(&[frame, &skip[..9]].concat(), u64::MAX).is_err());
        assert!(zstd(&[frame, &frame[..5]].concat(), u64::MAX).is_err());
    }

    #[test]
    fn garbage_never_panics() {
        for seed in 0..64u64 {
            let n = craft_anything_testkit::noise(seed, 400);
            let _ = xz(&n, 1 << 20);
            let _ = lzma(&n, 1 << 20);
            let _ = bzip2(&n, 1 << 20);
            let _ = zstd(&n, 1 << 20);
        }
    }
}
