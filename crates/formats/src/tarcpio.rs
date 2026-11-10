//! Member walkers for Unix archives: tar (v7, ustar, GNU long names and base-256 sizes, pax path
//! overrides) and cpio (newc, crc and portable odc). They list members and where their data
//! lies; nothing is extracted here, and no input can make them panic or loop forever.

use crate::bytes::Reader;
use serde::Serialize;

/// What a member is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum MemberKind {
    File,
    Dir,
    Symlink,
    Hardlink,
    Device,
    Fifo,
    Other,
}

/// One archive member.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Member {
    pub name: String,
    pub kind: MemberKind,
    pub size: u64,
    /// Where the member's data starts, relative to the archive start.
    pub data_offset: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub link_target: Option<String>,
    pub mode: u32,
}

/// Why GNU tar name metadata stopped a walk that still retains earlier members.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum StopReason {
    GnuMetadataTooLong,
    GnuMetadataTruncated,
}

/// The members of an archive.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct Listing {
    /// `tar`, `cpio-newc`, `cpio-crc` or `cpio-odc`.
    pub format: &'static str,
    pub members: Vec<Member>,
    /// Bytes from the archive start to the end of its last structure (end marker included).
    pub length: u64,
    /// True when the end-of-archive marker was reached.
    pub complete: bool,
    /// True when `max_members` stopped the walk.
    pub truncated: bool,
    /// GNU metadata that prevented reading the remaining member identities.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stop_reason: Option<StopReason>,
    pub warnings: Vec<String>,
}

const BLOCK: usize = 512;
const MAX_NAME: usize = 4096;

fn trim_field(f: &[u8]) -> &[u8] {
    let end = f.iter().position(|&b| b == 0).unwrap_or(f.len());
    f.get(..end).unwrap_or(&[])
}

fn text(f: &[u8]) -> String {
    String::from_utf8_lossy(trim_field(f)).into_owned()
}

/// An octal number field (spaces and NULs around it are ignored); GNU base-256 when the high bit
/// of the first byte is set.
fn octal(f: &[u8]) -> Option<u64> {
    if let Some((&first, rest)) = f.split_first()
        && first & 0x80 != 0
    {
        if first & 0x40 != 0 {
            return None; // This field holds an unsigned value.
        }
        let mut v = u64::from(first & 0x7f);
        for &b in rest {
            v = v.checked_mul(256)?.checked_add(u64::from(b))?;
        }
        return Some(v);
    }
    let padding = |&b: &u8| b == 0 || b == b' ';
    let mut digits = f.iter().copied().skip_while(padding).peekable();
    let mut v = 0u64;
    while let Some(b) = digits.next_if(|b| !padding(b)) {
        if !(b'0'..=b'7').contains(&b) {
            return None;
        }
        v = v.checked_mul(8)?.checked_add(u64::from(b - b'0'))?;
    }
    digits.all(|b| padding(&b)).then_some(v)
}

/// True when a 512-byte block is a tar header with a correct checksum.
pub fn tar_header_ok(h: &[u8]) -> bool {
    let Some(h) = h.get(..BLOCK) else { return false };
    let Some(stored) = h.get(148..156).and_then(octal) else { return false };
    let mut unsigned = 0u64;
    let mut signed = 0i64;
    for (i, &b) in h.iter().enumerate() {
        let v = if (148..156).contains(&i) { b' ' } else { b };
        unsigned += u64::from(v);
        signed += i64::from(v as i8);
    }
    // An all-zero block sums to 256 (the blank checksum field) and is the end marker, not a header.
    unsigned != 256 && (stored == unsigned || i64::try_from(stored).is_ok_and(|s| s == signed))
}

fn round_up(v: u64, to: u64) -> Option<u64> {
    let r = v % to;
    if r == 0 { Some(v) } else { v.checked_add(to - r) }
}

/// Walk a tar archive. `None` when the first block is not a valid header.
pub fn tar(data: &[u8], max_members: usize) -> Option<Listing> {
    let r = Reader::new(data);
    if !tar_header_ok(r.slice(0, BLOCK)?) {
        return None;
    }
    let mut out = Listing { format: "tar", ..Listing::default() };
    let mut pos = 0usize;
    let mut long_name: Option<String> = None;
    let mut long_link: Option<String> = None;
    let mut pax_path: Option<String> = None;
    let mut pax_link: Option<String> = None;
    let mut pax_size: Option<u64> = None;
    let mut pax_sparse = false;
    // Every iteration advances by at least one block, so the walk is bounded by the input size.
    'headers: loop {
        let Some(h) = r.slice(pos, BLOCK) else {
            out.warnings.push(format!("archive ends inside a header at {pos:#x}"));
            out.length = data.len() as u64;
            break;
        };
        if h.iter().all(|&b| b == 0) {
            // End of archive: one or two zero blocks.
            let second = r.slice(pos + BLOCK, BLOCK).is_some_and(|b| b.iter().all(|&x| x == 0));
            out.length = (pos + if second { 2 * BLOCK } else { BLOCK }) as u64;
            out.complete = true;
            break;
        }
        if !tar_header_ok(h) {
            out.warnings.push(format!("invalid header checksum at {pos:#x}; stopping"));
            out.length = pos as u64;
            break;
        }
        let typeflag = h.get(156).copied().unwrap_or(0);
        let Some(mut size) = h.get(124..136).and_then(octal) else {
            out.warnings.push(format!("unreadable size at {pos:#x}"));
            out.length = pos as u64;
            break;
        };
        if !matches!(typeflag, b'x' | b'g' | b'L' | b'K')
            && let Some(s) = pax_size.take()
        {
            size = s;
        }
        let data_off = pos + BLOCK;
        let Some(next) = round_up(size, BLOCK as u64).and_then(|s| u64::try_from(data_off).ok()?.checked_add(s)).and_then(|n| usize::try_from(n).ok()) else {
            if matches!(typeflag, b'L' | b'K') {
                out.stop_reason = Some(StopReason::GnuMetadataTruncated);
            }
            out.warnings.push(format!("size overflows at {pos:#x}"));
            out.length = pos as u64;
            break;
        };
        let body = r.slice(data_off, usize::try_from(size).unwrap_or(usize::MAX));
        match typeflag {
            b'L' | b'K' => {
                let Some(body) = body.filter(|_| next <= data.len()) else {
                    out.stop_reason = Some(StopReason::GnuMetadataTruncated);
                    out.warnings.push(format!("GNU long-name record ends early at {pos:#x}"));
                    out.length = data.len() as u64;
                    break;
                };
                let end = body.iter().take(MAX_NAME + 1).position(|&byte| byte == 0).unwrap_or(body.len());
                if end > MAX_NAME {
                    out.stop_reason = Some(StopReason::GnuMetadataTooLong);
                    out.warnings.push(format!("GNU long name or link exceeds {MAX_NAME} bytes at {pos:#x}"));
                    out.length = pos as u64;
                    break;
                }
                let value = Some(String::from_utf8_lossy(&body[..end]).into_owned());
                if typeflag == b'L' {
                    long_name = value;
                } else {
                    long_link = value;
                }
            }
            b'x' => {
                if let Some(mut remaining) = body {
                    for _ in 0..256 {
                        if remaining.is_empty() {
                            break;
                        }
                        let Some((key, value, rest)) = pax_record(remaining) else {
                            out.warnings.push(format!("invalid pax record at {pos:#x}"));
                            out.length = pos as u64;
                            break 'headers;
                        };
                        if key.starts_with(b"GNU.sparse.") {
                            pax_sparse = true;
                        }
                        match key {
                            b"path" | b"linkpath" | b"GNU.sparse.name" => {
                                if value.len() > MAX_NAME {
                                    out.warnings.push(format!("pax name exceeds {MAX_NAME} bytes at {pos:#x}"));
                                    out.length = pos as u64;
                                    break 'headers;
                                }
                                let slot = if key == b"linkpath" { &mut pax_link } else { &mut pax_path };
                                *slot = Some(String::from_utf8_lossy(value).into_owned());
                            }
                            b"size" => pax_size = std::str::from_utf8(value).ok().and_then(|s| s.parse().ok()),
                            _ => {}
                        }
                        remaining = rest;
                    }
                    if !remaining.is_empty() {
                        out.warnings.push(format!("pax record limit reached at {pos:#x}"));
                        out.length = pos as u64;
                        break 'headers;
                    }
                }
            }
            b'g' => {}
            _ => {
                // Old GNU headers use these bytes for timestamps and sparse-file metadata.
                let ustar = h.get(257..263) == Some(b"ustar\0");
                let base = text(h.get(0..100).unwrap_or(&[]));
                let prefix = if ustar { text(h.get(345..500).unwrap_or(&[])) } else { String::new() };
                let joined = if prefix.is_empty() { base } else { format!("{prefix}/{base}") };
                let name = pax_path.take().or(long_name.take()).unwrap_or(joined);
                let link = pax_link.take().or(long_link.take()).or_else(|| Some(text(h.get(157..257).unwrap_or(&[])))).filter(|l| !l.is_empty());
                let mut kind = match typeflag {
                    b'0' | 0 | b'7' => {
                        if name.ends_with('/') {
                            MemberKind::Dir
                        } else {
                            MemberKind::File
                        }
                    }
                    b'1' => MemberKind::Hardlink,
                    b'2' => MemberKind::Symlink,
                    b'3' | b'4' => MemberKind::Device,
                    b'5' => MemberKind::Dir,
                    b'6' => MemberKind::Fifo,
                    _ => MemberKind::Other,
                };
                if std::mem::take(&mut pax_sparse) {
                    kind = MemberKind::Other;
                    out.warnings.push(format!("{name}: GNU sparse reconstruction is unsupported; stored extents are not the complete file"));
                }
                let mode = h.get(100..108).and_then(octal).and_then(|m| u32::try_from(m).ok()).unwrap_or(0);
                if body.is_none() && kind == MemberKind::File {
                    out.warnings.push(format!("{name}: data runs past the end of the input"));
                }
                if out.members.len() >= max_members {
                    out.truncated = true;
                    out.length = pos as u64;
                    break;
                }
                out.members.push(Member {
                    name,
                    kind,
                    size,
                    data_offset: data_off as u64,
                    link_target: if matches!(kind, MemberKind::Symlink | MemberKind::Hardlink) { link } else { None },
                    mode,
                });
            }
        }
        if next > data.len() {
            out.length = data.len() as u64;
            out.warnings.push("archive ends early (no end-of-archive marker)".into());
            break;
        }
        pos = next;
    }
    Some(out)
}

/// Read one length-prefixed pax record, borrowing even unknown keys and values from the header.
fn pax_record(data: &[u8]) -> Option<(&[u8], &[u8], &[u8])> {
    let sp = data.iter().position(|&byte| byte == b' ')?;
    let digits = data.get(..sp)?;
    if !digits.iter().all(u8::is_ascii_digit) {
        return None;
    }
    let len: usize = std::str::from_utf8(digits).ok()?.parse().ok()?;
    let field = data.get(sp + 1..len)?.strip_suffix(b"\n")?;
    let eq = field.iter().position(|&byte| byte == b'=')?;
    if eq == 0 {
        return None;
    }
    Some((field.get(..eq)?, field.get(eq + 1..)?, data.get(len..)?))
}

fn hex_field(f: &[u8]) -> Option<u64> {
    let s = std::str::from_utf8(f).ok()?;
    u64::from_str_radix(s, 16).ok()
}

fn oct_field(f: &[u8]) -> Option<u64> {
    let s = std::str::from_utf8(f).ok()?;
    u64::from_str_radix(s, 8).ok()
}

fn mode_kind(mode: u32) -> MemberKind {
    match mode & 0o170_000 {
        0o100_000 => MemberKind::File,
        0o040_000 => MemberKind::Dir,
        0o120_000 => MemberKind::Symlink,
        0o020_000 | 0o060_000 => MemberKind::Device,
        0o010_000 => MemberKind::Fifo,
        _ => MemberKind::Other,
    }
}

/// Walk a cpio archive (newc `070701`, crc `070702`, odc `070707`). `None` when the first header
/// is not one of these.
pub fn cpio(data: &[u8], max_members: usize) -> Option<Listing> {
    cpio_with_checksum(data, max_members, &mut |body| CpioChecksum::Value(body.iter().fold(0u32, |sum, &byte| sum.wrapping_add(u32::from(byte)))))
}

/// A checksum sum, or the reason a caller stopped verification before completing it.
#[derive(Clone, Copy)]
pub(crate) enum CpioChecksum {
    Value(u32),
    Skipped,
    Cancelled,
}

/// Let a scanner share its checksum work allowance and cancellation across overlapping probes.
pub(crate) fn cpio_with_checksum(data: &[u8], max_members: usize, checksum: &mut dyn FnMut(&[u8]) -> CpioChecksum) -> Option<Listing> {
    let r = Reader::new(data);
    let format = match r.slice(0, 6)? {
        b"070701" => "cpio-newc",
        b"070702" => "cpio-crc",
        b"070707" => "cpio-odc",
        _ => return None,
    };
    let odc = format == "cpio-odc";
    let mut hard_links = std::collections::HashMap::<(u64, u64, u64), Vec<usize>>::new();
    let mut out = Listing { format, ..Listing::default() };
    let mut pos = 0usize;
    let align = |v: usize| if odc { Some(v) } else { v.checked_add(3).map(|x| x & !3) };
    loop {
        let hdr_len = if odc { 76 } else { 110 };
        let Some(h) = r.slice(pos, hdr_len) else {
            out.warnings.push(format!("archive ends inside a header at {pos:#x}"));
            out.length = data.len() as u64;
            break;
        };
        if h.get(..6) != r.slice(0, 6) {
            out.warnings.push(format!("bad header magic at {pos:#x}; stopping"));
            out.length = pos as u64;
            break;
        }
        let fields = if odc {
            (h.get(18..24).and_then(oct_field), h.get(59..65).and_then(oct_field), h.get(65..76).and_then(oct_field))
        } else {
            (h.get(14..22).and_then(hex_field), h.get(94..102).and_then(hex_field), h.get(54..62).and_then(hex_field))
        };
        let (Some(mode), Some(namesize), Some(filesize)) = fields else {
            out.warnings.push(format!("unreadable header fields at {pos:#x}"));
            out.length = pos as u64;
            break;
        };
        let namesize = usize::try_from(namesize).unwrap_or(usize::MAX);
        if namesize == 0 || namesize > MAX_NAME {
            out.warnings.push(format!("implausible name size at {pos:#x}"));
            out.length = pos as u64;
            break;
        }
        let Some(name_bytes) = r.slice(pos + hdr_len, namesize) else {
            out.warnings.push(format!("name runs past the end at {pos:#x}"));
            out.length = data.len() as u64;
            break;
        };
        // newc permits extra trailing NULs in namesize for alignment.
        let Some(end) = name_bytes.iter().position(|&b| b == 0) else {
            out.warnings.push(format!("unterminated member name at {pos:#x}"));
            out.length = pos as u64;
            break;
        };
        if end == 0 || name_bytes.get(end..).is_some_and(|pad| pad.iter().any(|&b| b != 0)) {
            out.warnings.push(format!("invalid member name at {pos:#x}"));
            out.length = pos as u64;
            break;
        }
        let name = text(name_bytes);
        let Some(data_off) = (pos + hdr_len).checked_add(namesize).and_then(align) else { break };
        let size = usize::try_from(filesize).unwrap_or(usize::MAX);
        let Some(next) = data_off.checked_add(size).and_then(align) else {
            out.warnings.push(format!("size overflows at {pos:#x}"));
            out.length = pos as u64;
            break;
        };
        if name == "TRAILER!!!" {
            out.length = next.min(data.len()) as u64;
            if size == 0 && next <= data.len() {
                out.complete = true;
            } else {
                out.warnings.push(format!("invalid or truncated trailer at {pos:#x}"));
            }
            break;
        }
        let mode = u32::try_from(mode).unwrap_or(0);
        let kind = mode_kind(mode);
        let link_target = if kind == MemberKind::Symlink { r.slice(data_off, size.min(MAX_NAME)).map(text) } else { None };
        if out.members.len() >= max_members {
            out.truncated = true;
            out.length = pos as u64;
            break;
        }
        if format == "cpio-crc" {
            match h.get(102..110).filter(|field| field.iter().all(u8::is_ascii_hexdigit)).and_then(hex_field).and_then(|n| u32::try_from(n).ok()) {
                Some(expected) => {
                    if let Some(body) = r.slice(data_off, size) {
                        match checksum(body) {
                            CpioChecksum::Value(actual) => {
                                if actual != expected {
                                    out.warnings.push(format!("{name}: data checksum mismatch (stored {expected:08x}, computed {actual:08x})"));
                                }
                            }
                            CpioChecksum::Skipped => out.warnings.push(format!("{name}: data checksum not verified (scan byte budget exhausted)")),
                            CpioChecksum::Cancelled => return None,
                        }
                    }
                }
                None => out.warnings.push(format!("{name}: unreadable data checksum")),
            }
        }
        // newc stores hard-linked files by device and inode. GNU cpio may put the
        // payload on the last name, so identify aliases after scanning the group.
        if !odc
            && kind == MemberKind::File
            && h.get(38..46).and_then(hex_field).is_some_and(|n| n > 1)
            && let (Some(ino), Some(major), Some(minor)) =
                (h.get(6..14).and_then(hex_field), h.get(62..70).and_then(hex_field), h.get(70..78).and_then(hex_field))
        {
            hard_links.entry((major, minor, ino)).or_default().push(out.members.len());
        }
        out.members.push(Member { name, kind, size: filesize, data_offset: data_off as u64, link_target, mode });
        if next > data.len() {
            out.warnings.push("archive ends early (no trailer)".into());
            out.length = data.len() as u64;
            break;
        }
        pos = next;
    }
    for group in hard_links.values().filter(|group| group.len() > 1) {
        let source = group.iter().rev().copied().find(|&i| out.members.get(i).is_some_and(|m| m.size > 0)).or_else(|| group.first().copied());
        let Some(source) = source else { continue };
        let Some(name) = out.members.get(source).map(|m| m.name.clone()) else { continue };
        for &index in group {
            if index != source
                && let Some(alias) = out.members.get_mut(index)
                && alias.size == 0
            {
                alias.kind = MemberKind::Hardlink;
                alias.link_target = Some(name.clone());
            }
        }
    }
    Some(out)
}

/// The data of a member, when it lies inside `data`.
pub fn member_data<'a>(data: &'a [u8], m: &Member) -> Option<&'a [u8]> {
    Reader::new(data).slice(usize::try_from(m.data_offset).ok()?, usize::try_from(m.size).ok()?)
}

#[cfg(test)]
mod tests {
    use super::*;
    use craft_anything_testkit::firmware as fx;

    #[test]
    fn oversized_gnu_names_do_not_alias_their_truncated_prefix() {
        for kind in *b"LK" {
            let mut long = vec![b'n'; MAX_NAME + 1];
            long.push(0);
            let data = fx::tar_entries(&[
                ("before", b'0', b"kept", ""),
                ("././@LongLink", kind, &long, ""),
                ("short", if kind == b'K' { b'2' } else { b'0' }, b"", "fallback"),
            ]);
            let listing = tar(&data, 10).unwrap();
            assert!(!listing.complete);
            assert_eq!(listing.stop_reason, Some(StopReason::GnuMetadataTooLong));
            assert_eq!(listing.members.len(), 1);
            assert_eq!(listing.members[0].name, "before");
            assert_eq!(member_data(&data, &listing.members[0]).unwrap(), b"kept");
            assert!(listing.warnings.iter().any(|warning| warning.contains("GNU long name or link exceeds")));
        }
    }

    #[test]
    fn gnu_long_names_keep_exact_limits_and_short_terminated_values() {
        for kind in *b"LK" {
            for terminated in [false, true] {
                let mut long = vec![b'n'; MAX_NAME];
                if terminated {
                    long.push(0);
                }
                let data = fx::tar_entries(&[("././@LongLink", kind, &long, ""), ("short", if kind == b'K' { b'2' } else { b'0' }, b"", "fallback")]);
                let listing = tar(&data, 10).unwrap();
                assert!(listing.complete, "{:?}", listing.warnings);
                let member = &listing.members[0];
                let name = if kind == b'L' { member.name.as_str() } else { member.link_target.as_deref().unwrap() };
                assert_eq!(name, "n".repeat(MAX_NAME));
            }
        }
        let mut padded = b"short\0".to_vec();
        padded.resize(MAX_NAME + 100, 0);
        let data = fx::tar_entries(&[("././@LongLink", b'L', &padded, ""), ("fallback", b'0', b"payload", "")]);
        assert_eq!(tar(&data, 10).unwrap().members[0].name, "short");
    }

    fn sparse_pax_record(key: &str, value: &str) -> Vec<u8> {
        let field = format!(" {key}={value}\n");
        let mut length = field.len() + 1;
        loop {
            let actual = field.len() + length.to_string().len();
            if actual == length {
                return format!("{length}{field}").into_bytes();
            }
            length = actual;
        }
    }

    #[test]
    fn gnu_sparse_pax_members_are_not_regular_file_bytes() {
        for fields in [
            vec![("GNU.sparse.size", "100"), ("GNU.sparse.numblocks", "1"), ("GNU.sparse.offset", "50"), ("GNU.sparse.numbytes", "3")],
            vec![("GNU.sparse.size", "100"), ("GNU.sparse.map", "50,3"), ("GNU.sparse.name", "original.bin")],
            vec![("GNU.sparse.major", "1"), ("GNU.sparse.minor", "0"), ("GNU.sparse.realsize", "100"), ("GNU.sparse.name", "original.bin")],
        ] {
            let metadata: Vec<u8> = fields.iter().flat_map(|(key, value)| sparse_pax_record(key, value)).collect();
            let version_one = fields.iter().any(|(key, _)| *key == "GNU.sparse.major");
            let mut sparse_body = if version_one { b"1\n50\n3\n".to_vec() } else { Vec::new() };
            if version_one {
                sparse_body.resize(512, 0);
            }
            sparse_body.extend_from_slice(b"abc");
            let data = fx::tar_entries(&[("metadata", b'x', &metadata, ""), ("stored-name", b'0', &sparse_body, ""), ("normal", b'0', b"ordinary", "")]);
            let listing = tar(&data, 10).unwrap();
            assert!(listing.complete, "{:?}", listing.warnings);
            assert_eq!(listing.members.len(), 2);
            assert_eq!(listing.members[0].kind, MemberKind::Other);
            let named = fields.iter().any(|(key, _)| *key == "GNU.sparse.name");
            assert_eq!(listing.members[0].name, if named { "original.bin" } else { "stored-name" });
            assert_eq!(listing.members[1].kind, MemberKind::File);
            assert_eq!(member_data(&data, &listing.members[1]).unwrap(), b"ordinary");
            assert_eq!(listing.warnings.len(), 1);
            assert!(listing.warnings[0].contains("sparse reconstruction is unsupported"));
        }
    }

    #[test]
    fn sparse_metadata_survives_other_extension_headers() {
        let sparse = sparse_pax_record("GNU.sparse.map", "0,3");
        let path = sparse_pax_record("path", "renamed");
        let data = fx::tar_entries(&[
            ("sparse", b'x', &sparse, ""),
            ("name", b'L', b"long-name\0", ""),
            ("path", b'x', &path, ""),
            ("header", b'0', b"abc", ""),
            ("next", b'0', b"ok", ""),
        ]);
        let listing = tar(&data, 10).unwrap();
        assert_eq!(listing.members[0].name, "renamed");
        assert_eq!(listing.members[0].kind, MemberKind::Other);
        assert_eq!(listing.members[1].kind, MemberKind::File);
    }

    #[test]
    fn gnu_metadata_stop_reasons_keep_partial_listings_and_marker_compatibility() {
        for kind in *b"LK" {
            let mut data = fx::tar_entries(&[("before", b'0', b"kept", ""), ("././@LongLink", kind, b"long\0", ""), ("fallback", b'0', b"later", "")]);
            data.truncate(3 * BLOCK + 2);
            let listing = tar(&data, 10).unwrap();
            assert_eq!(listing.stop_reason, Some(StopReason::GnuMetadataTruncated));
            assert!(!listing.complete);
            assert!(!listing.truncated);
            assert_eq!(listing.members.len(), 1);
            assert_eq!(member_data(&data, &listing.members[0]).unwrap(), b"kept");
        }
        let mut data = fx::tar_entries(&[("file", b'0', b"kept", "")]);
        data.truncate(2 * BLOCK);
        let listing = tar(&data, 10).unwrap();
        assert!(!listing.complete);
        assert_eq!(listing.stop_reason, None);
        assert_eq!(member_data(&data, &listing.members[0]).unwrap(), b"kept");
        let cpio_listing = cpio(&fx::cpio_newc(&[("file", b"kept", 0o100644)]), 10).unwrap();
        assert_eq!(cpio_listing.stop_reason, None);
    }

    #[test]
    fn gnu_metadata_requires_its_complete_padded_record() {
        let body = b"next\0";
        for kind in *b"LK" {
            let data = fx::tar_entries(&[
                ("before", b'0', b"kept", ""),
                ("././@LongLink", kind, body, ""),
                ("fallback", if kind == b'L' { b'0' } else { b'2' }, b"", "fallback"),
            ]);
            for padding in [0, 1, BLOCK - body.len() - 1] {
                let short = &data[..3 * BLOCK + body.len() + padding];
                let listing = tar(short, 10).unwrap();
                assert_eq!(listing.stop_reason, Some(StopReason::GnuMetadataTruncated));
                assert!(!listing.complete);
                assert_eq!(listing.members.len(), 1);
                assert_eq!(member_data(short, &listing.members[0]).unwrap(), b"kept");
            }
            let listing = tar(&data, 10).unwrap();
            assert!(listing.complete);
            assert_eq!(listing.stop_reason, None);
            assert_eq!(listing.members.len(), 2);
        }
    }

    #[test]
    fn walks_tar() {
        let t = fx::tar(&[("etc/passwd", b"root:x:0:0\n"), ("bin/", b""), ("long/".repeat(30).as_str(), b"deep")]);
        let l = tar(&t, 100).unwrap();
        assert!(l.complete, "{:?}", l.warnings);
        assert_eq!(l.members.len(), 3);
        assert_eq!(l.members[0].name, "etc/passwd");
        assert_eq!(member_data(&t, &l.members[0]).unwrap(), b"root:x:0:0\n");
        assert_eq!(l.members[1].kind, MemberKind::Dir);
        assert!(l.members[2].name.starts_with("long/long/"));
        assert_eq!(l.length, t.len() as u64);
        let l2 = tar(&t, 1).unwrap();
        assert!(l2.truncated);
        assert_eq!(l2.members.len(), 1);
    }

    #[test]
    fn cpio_names_and_trailers_must_be_complete() {
        let bytes = fx::cpio_newc(&[("abc", b"x", 0o100644)]);
        let trailer = bytes.windows(6).rposition(|b| b == b"070701").unwrap();
        for (offset, byte) in [(113, b'x'), (111, 0)] {
            let mut damaged = bytes.clone();
            damaged[offset] = byte;
            let listed = cpio(&damaged, 10).unwrap();
            assert!(!listed.complete);
            assert!(listed.members.is_empty());
        }
        let mut padded_name = bytes.clone();
        padded_name[112] = 0;
        assert_eq!(cpio(&padded_name, 10).unwrap().members[0].name, "ab");
        let mut with_payload = bytes.clone();
        with_payload[trailer + 61] = b'1';
        assert!(!cpio(&with_payload, 10).unwrap().complete);
        assert!(!cpio(&bytes[..bytes.len() - 1], 10).unwrap().complete);
    }

    #[test]
    fn cpio_hard_link_aliases_do_not_appear_as_empty_files() {
        for content_first in [true, false] {
            let data = if content_first { [b"abc".as_slice(), b""] } else { [b"".as_slice(), b"abc"] };
            let mut archive = fx::cpio_newc(&[("one", data[0], 0o100644), ("two", data[1], 0o100644), ("separate", b"", 0o100644)]);
            let starts: Vec<_> = archive.windows(6).enumerate().filter(|(_, bytes)| *bytes == b"070701").map(|(i, _)| i).collect();
            for &start in &starts[..3] {
                archive[start + 6..start + 14].copy_from_slice(b"00000015");
                archive[start + 38..start + 46].copy_from_slice(b"00000002");
            }
            archive[starts[2] + 70..starts[2] + 78].copy_from_slice(b"00000001");
            let (file, alias) = if content_first { (0, 1) } else { (1, 0) };
            for crc in [false, true] {
                if crc {
                    for &start in &starts {
                        archive[start..start + 6].copy_from_slice(b"070702");
                    }
                    archive[starts[file] + 102..starts[file] + 110].copy_from_slice(b"00000126");
                }
                let listing = cpio(&archive, 10).unwrap();
                assert_eq!(listing.members[file].kind, MemberKind::File);
                assert_eq!(member_data(&archive, &listing.members[file]), Some(b"abc".as_slice()));
                assert_eq!(listing.members[alias].kind, MemberKind::Hardlink);
                assert_eq!(listing.members[alias].link_target.as_deref(), Some(listing.members[file].name.as_str()));
                assert_eq!(listing.members[2].kind, MemberKind::File);
                assert!(listing.warnings.is_empty());
            }
        }
    }

    #[test]
    fn walks_cpio() {
        let c = fx::cpio_newc(&[("init", b"#!/bin/sh\n", 0o100755), ("dev", b"", 0o040755), ("sh", b"busybox", 0o120777)]);
        let l = cpio(&c, 100).unwrap();
        assert!(l.complete);
        assert_eq!(l.members.len(), 3);
        assert_eq!(member_data(&c, &l.members[0]).unwrap(), b"#!/bin/sh\n");
        assert_eq!(l.members[2].kind, MemberKind::Symlink);
        assert_eq!(l.members[2].link_target.as_deref(), Some("busybox"));
        assert_eq!(l.length, c.len() as u64);
    }

    #[test]
    fn octal_fields() {
        assert_eq!(octal(b"0000644 \0"), Some(0o644));
        assert_eq!(octal(b"  \0"), Some(0));
        assert_eq!(octal(b"9"), None);
        assert_eq!(octal(&[0x80, 0, 0, 1, 0]), Some(256));
        assert_eq!(octal(&[0xff; 8]), None);
        assert_eq!(octal(&[0xff; 12]), None);
    }

    #[test]
    fn hostile_archives_do_not_panic() {
        let t = fx::tar(&[("a", b"hello"), ("b/c", b"world")]);
        let c = fx::cpio_newc(&[("a", b"hello", 0o100644)]);
        for s in [&t, &c] {
            for m in craft_anything_testkit::mutations(s) {
                let _ = tar(&m, 10);
                let _ = cpio(&m, 10);
            }
            for n in 0..s.len().min(800) {
                let _ = tar(&s[..n], 10);
                let _ = cpio(&s[..n], 10);
            }
        }
        for seed in 0..50 {
            let n = craft_anything_testkit::noise(seed, 2048);
            let _ = tar(&n, 10);
            let _ = cpio(&n, 10);
        }
        assert!(pax_record(b"12 path=abc\n").is_some());
        assert!(pax_record(b"9999999999999999999999 x").is_none());
    }
}
