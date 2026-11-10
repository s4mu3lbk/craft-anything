//! romfs (`-rom1fs-`): a tiny read-only filesystem. Big-endian throughout. Each file header is
//! 16-byte aligned: next-header pointer (with the type in its low 4 bits), spec info, size,
//! checksum, then a NUL-terminated 16-aligned name, then the file data. We walk the directory tree
//! from the root, following sibling (`next`) and child (`spec` for directories) pointers, bounded
//! against cycles and runaway sizes.

use super::{FsEntry, FsImage, Limits, is_dot, join};
use crate::bytes::Reader;

fn align16(n: usize) -> Option<usize> {
    n.checked_add(15).map(|v| v & !15)
}

/// Name field length (NUL-terminated, rounded up to 16), and the trimmed name.
fn name_at(data: &[u8], off: usize, max_name: usize) -> Option<(String, usize)> {
    let rest = data.get(off..)?;
    let end = rest.iter().take(max_name.clamp(1, 1024)).position(|&b| b == 0)?;
    let name = String::from_utf8_lossy(rest.get(..end)?).into_owned();
    Some((name, align16(end + 1)?))
}

pub fn read(data: &[u8], limits: &Limits) -> FsImage {
    let mut img = FsImage::new("romfs");
    let r = Reader::new(data);
    if r.slice(0, 8) != Some(b"-rom1fs-") {
        img.warn("not a romfs image");
        img.complete = false;
        return img;
    }
    let Some(size) = r.u32_be(8).filter(|&size| size >= 16) else {
        img.warn("romfs: invalid image size");
        img.complete = false;
        return img;
    };
    if size as usize > data.len() {
        img.warn("romfs: image ends before its declared size");
        img.complete = false;
    }
    let data = data.get(..size as usize).unwrap_or(data);
    let r = Reader::new(data);
    // Root directory's first entry sits just after the volume name.
    let Some((_, vol_len)) = name_at(data, 16, 128) else {
        img.warn("volume name unreadable");
        img.complete = false;
        return img;
    };
    let Some(root) = 16usize.checked_add(vol_len) else {
        img.complete = false;
        return img;
    };
    let mut visited: Vec<usize> = Vec::new();
    // A dot-named root is the container itself, not a child to skip with `.` and `..`.
    let root_container = r.u32_be(root).is_some_and(|kind| kind & 7 == 1)
        && root.checked_add(16).and_then(|at| name_at(data, at, limits.max_name)).is_some_and(|(name, _)| name == "." || name.is_empty());
    if root_container {
        visited.push(root);
        let first = root.checked_add(4).and_then(|at| r.u32_be(at)).unwrap_or(0) as usize;
        walk(data, first, "", limits, &mut img, &mut visited, 0);
    } else {
        walk(data, root, "", limits, &mut img, &mut visited, 0);
    }
    img
}

#[allow(clippy::too_many_arguments)]
fn walk(data: &[u8], first: usize, parent: &str, limits: &Limits, img: &mut FsImage, visited: &mut Vec<usize>, depth: usize) {
    if depth > limits.max_depth {
        img.warn("directory nesting limit reached");
        img.complete = false;
        return;
    }
    let r = Reader::new(data);
    let mut off = first;
    let mut siblings = 0usize;
    while off != 0 {
        if !off.is_multiple_of(16) || r.slice(off, 16).is_none() {
            img.warn("romfs: header pointer outside the image or unaligned");
            img.complete = false;
            break;
        }
        if img.entries.len() >= limits.max_entries {
            img.complete = false;
            return;
        }
        if visited.contains(&off) {
            img.warn("romfs header cycle detected");
            img.complete = false;
            return;
        }
        visited.push(off);
        siblings += 1;
        if siblings > limits.max_entries {
            img.complete = false;
            return;
        }
        let Some(next_raw) = r.u32_be(off) else { break };
        let ftype = next_raw & 0x7;
        let next = (next_raw & 0xffff_fff0) as usize;
        let spec = r.u32_be(off + 4).unwrap_or(0) as usize;
        let size = r.u32_be(off + 8).unwrap_or(0) as usize;
        let Some((name, name_len)) = name_at(data, off + 16, limits.max_name) else {
            img.warn("romfs: unreadable entry name");
            img.complete = false;
            break;
        };
        if !is_dot(&name) && !name.is_empty() {
            let path = join(parent, &name);
            match ftype {
                1 => {
                    // Directory: recurse into its first child at `spec`.
                    img.entries.push(FsEntry::dir(path.clone()));
                    if spec != 0 {
                        walk(data, spec, &path, limits, img, visited, depth + 1);
                    }
                }
                2 | 3 => {
                    let payload = (size as u64 <= limits.max_file_bytes && img.bytes.saturating_add(size as u64) <= limits.max_total_bytes)
                        .then_some(off + 16)
                        .and_then(|start| start.checked_add(name_len))
                        .and_then(|start| r.slice(start, size));
                    if payload.is_some() {
                        img.bytes = img.bytes.saturating_add(size as u64);
                    } else {
                        img.warn(format!("{path}: entry data not recovered (size/bounds)"));
                        img.complete = false;
                    }
                    if ftype == 2 {
                        img.entries.push(FsEntry::file(path, size as u64, payload.map(<[u8]>::to_vec)));
                    } else {
                        let symlink_target = payload.map(|bytes| String::from_utf8_lossy(bytes).into_owned());
                        img.entries.push(FsEntry { path, kind: super::FsKind::Symlink, size: size as u64, symlink_target, data: None });
                    }
                }
                _ => img.entries.push(FsEntry { path, kind: super::FsKind::Other, size: size as u64, symlink_target: None, data: None }),
            }
        }
        off = next;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_a_small_image() {
        let img = read(&craft_anything_testkit::firmware::romfs_image(), &Limits::default());
        assert!(img.complete, "{:?}", img.warnings);
        let f = img.entries.iter().find(|e| e.path == "etc/hostname").expect("etc/hostname");
        assert_eq!(f.data.as_deref(), Some(b"craft-router\n".as_slice()));
        assert!(img.entries.iter().any(|e| e.path == "etc" && e.kind == super::super::FsKind::Dir));
        assert_eq!(img.entries.iter().find(|e| e.path == "readme.txt").unwrap().data.as_deref(), Some(b"hello romfs".as_slice()));
    }

    #[test]
    fn hostile_never_panics() {
        let base = craft_anything_testkit::firmware::romfs_image();
        for m in craft_anything_testkit::mutations(&base) {
            let _ = read(&m, &Limits::default());
        }
        for len in 0..base.len().min(400) {
            let _ = read(&base[..len], &Limits::default());
        }
        for seed in 0..32 {
            let _ = read(&craft_anything_testkit::noise(seed, 300), &Limits::default());
        }
    }
}
