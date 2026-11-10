//! Source maps (revision 3), including indexed maps with sections.
//!
//! Lookups follow the "original positions for a generated position" rule: on the generated line,
//! take the greatest segment column not after the requested column and return every segment that
//! starts there. Lines are 0-based here (callers add one for display); columns are 0-based UTF-16
//! code units as the format defines.

use serde_json::Value;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Limits {
    /// Biggest map JSON accepted.
    pub max_bytes: usize,
    /// Most decoded segments across the whole map.
    pub max_segments: usize,
    /// Deepest section nesting (the format allows one level; we tolerate a few).
    pub max_depth: usize,
}

impl Default for Limits {
    fn default() -> Self {
        Limits { max_bytes: 64 * 1024 * 1024, max_segments: 4_000_000, max_depth: 8 }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum SourceMapError {
    #[error("source map is {0} bytes; the limit is {1}")]
    TooLarge(usize, usize),
    #[error("source map is not valid JSON: {0}")]
    BadJson(String),
    #[error("not a version 3 source map")]
    BadVersion,
    #[error("source map mappings are malformed at segment {0}")]
    BadMappings(usize),
    #[error("source map index has invalid sections")]
    BadSections,
    #[error("source map has more than {0} segments")]
    TooManySegments(usize),
    #[error("source map sections nest deeper than {0}")]
    TooDeep(usize),
}

/// One decoded mapping segment.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Segment {
    pub generated_column: u32,
    /// (source index, original line, original column, name index)
    pub original: Option<(u32, u32, u32, Option<u32>)>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SourceMap {
    pub file: Option<String>,
    pub source_root: Option<String>,
    pub sources: Vec<Option<String>>,
    pub sources_content: Vec<Option<String>>,
    pub names: Vec<String>,
    pub ignore_list: Vec<u32>,
    /// Segments per generated line, sorted by column.
    pub lines: Vec<Vec<Segment>>,
    pub segments: usize,
    /// Indexed maps: (line offset, column offset, map).
    pub sections: Vec<(u32, u32, SourceMap)>,
}

/// A position in an original source.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct OriginalPosition {
    pub source_index: u32,
    pub source: Option<String>,
    /// 0-based.
    pub line: u32,
    /// 0-based.
    pub column: u32,
    pub name: Option<String>,
    /// 0-based generated column of the matched segment.
    pub generated_column: u32,
}

impl SourceMap {
    /// The source path with `sourceRoot` applied.
    pub fn source_path(&self, index: u32) -> Option<String> {
        let s = self.sources.get(index as usize)?.as_ref()?;
        Some(match self.source_root.as_deref().filter(|r| !r.is_empty()) {
            Some(root) if !s.contains("://") && !s.starts_with('/') => {
                let join = if root.ends_with('/') { "" } else { "/" };
                format!("{root}{join}{s}")
            }
            _ => s.clone(),
        })
    }

    pub fn source_content(&self, index: u32) -> Option<&str> {
        self.sources_content.get(index as usize)?.as_deref()
    }

    /// Every original position for a generated (line, column), both 0-based.
    pub fn original_positions(&self, line: u32, column: u32) -> Vec<OriginalPosition> {
        if !self.sections.is_empty() {
            // Last section that starts at or before the position.
            let end = self.sections.partition_point(|(l, c, _)| (*l, *c) <= (line, column));
            let Some((l, c, map)) = end.checked_sub(1).and_then(|i| self.sections.get(i)) else { return Vec::new() };
            let rel_line = line.saturating_sub(*l);
            let rel_col = if rel_line == 0 { column.saturating_sub(*c) } else { column };
            return map
                .original_positions(rel_line, rel_col)
                .into_iter()
                .filter_map(|mut position| {
                    if rel_line == 0 {
                        position.generated_column = position.generated_column.checked_add(*c)?;
                    }
                    Some(position)
                })
                .collect();
        }
        let Some(segs) = self.lines.get(line as usize) else { return Vec::new() };
        // Greatest generated column <= column.
        let idx = segs.partition_point(|s| s.generated_column <= column);
        let Some(best) = idx.checked_sub(1).and_then(|i| segs.get(i)) else { return Vec::new() };
        let col = best.generated_column;
        let first = segs.partition_point(|s| s.generated_column < col);
        segs.get(first..idx)
            .unwrap_or_default()
            .iter()
            .filter_map(|s| {
                let (src, l, c, n) = s.original?;
                Some(OriginalPosition {
                    source_index: src,
                    source: self.source_path(src),
                    line: l,
                    column: c,
                    name: n.and_then(|n| self.names.get(n as usize).cloned()),
                    generated_column: col,
                })
            })
            .collect()
    }

    /// All sources, flattened across sections.
    pub fn all_sources(&self) -> Vec<String> {
        let mut out: Vec<String> = (0..self.sources.len() as u32).filter_map(|i| self.source_path(i)).collect();
        for (_, _, m) in &self.sections {
            out.extend(m.all_sources());
        }
        out
    }
}

/// Parse a source map.
pub fn parse(json: &[u8], limits: Limits) -> Result<SourceMap, SourceMapError> {
    if json.len() > limits.max_bytes {
        return Err(SourceMapError::TooLarge(json.len(), limits.max_bytes));
    }
    // Some servers prefix maps with `)]}'` to defeat XSSI; the format allows skipping that line.
    let body = if json.starts_with(b")]}") { json.iter().position(|b| *b == b'\n').and_then(|p| json.get(p + 1..)).unwrap_or(&[]) } else { json };
    let v: Value = serde_json::from_slice(body).map_err(|e| SourceMapError::BadJson(e.to_string()))?;
    let mut budget = limits.max_segments;
    from_value(&v, 0, limits, &mut budget)
}

fn from_value(v: &Value, depth: usize, limits: Limits, budget: &mut usize) -> Result<SourceMap, SourceMapError> {
    if depth > limits.max_depth {
        return Err(SourceMapError::TooDeep(limits.max_depth));
    }
    if v.get("version").and_then(Value::as_u64) != Some(3) {
        return Err(SourceMapError::BadVersion);
    }
    let strs = |k: &str| -> Vec<Option<String>> {
        v.get(k).and_then(Value::as_array).map(|a| a.iter().map(|s| s.as_str().map(String::from)).collect()).unwrap_or_default()
    };
    let mut map = SourceMap {
        file: v.get("file").and_then(Value::as_str).map(String::from),
        source_root: v.get("sourceRoot").and_then(Value::as_str).map(String::from),
        sources: strs("sources"),
        sources_content: strs("sourcesContent"),
        names: strs("names").into_iter().map(Option::unwrap_or_default).collect(),
        ignore_list: v
            .get("ignoreList")
            .or_else(|| v.get("x_google_ignoreList"))
            .and_then(Value::as_array)
            .map(|a| a.iter().filter_map(|n| n.as_u64().and_then(|n| u32::try_from(n).ok())).collect())
            .unwrap_or_default(),
        ..Default::default()
    };
    if let Some(sections) = v.get("sections") {
        let sections = sections.as_array().ok_or(SourceMapError::BadSections)?;
        for section in sections {
            let offset = |key: &str| section.get("offset")?.get(key)?.as_u64().and_then(|n| u32::try_from(n).ok());
            let line = offset("line").ok_or(SourceMapError::BadSections)?;
            let col = offset("column").ok_or(SourceMapError::BadSections)?;
            if map.sections.last().is_some_and(|(l, c, _)| (*l, *c) > (line, col)) {
                return Err(SourceMapError::BadSections);
            }
            let sub = match (section.get("map"), section.get("url")) {
                (Some(m), None) => from_value(m, depth + 1, limits, budget)?,
                // External maps are never fetched here. Retain their start so a lookup
                // cannot leak through to the previous inline map.
                (None, Some(Value::String(_))) => SourceMap::default(),
                _ => return Err(SourceMapError::BadSections),
            };
            map.segments = map.segments.saturating_add(sub.segments);
            map.sections.push((line, col, sub));
        }
        return Ok(map);
    }
    let mappings = v.get("mappings").and_then(Value::as_str).unwrap_or_default();
    decode_mappings(mappings, &mut map, budget)?;
    Ok(map)
}

fn b64(c: u8) -> Option<i64> {
    Some(match c {
        b'A'..=b'Z' => i64::from(c - b'A'),
        b'a'..=b'z' => i64::from(c - b'a') + 26,
        b'0'..=b'9' => i64::from(c - b'0') + 52,
        b'+' => 62,
        b'/' => 63,
        _ => return None,
    })
}

/// Decode one VLQ field from `bytes` at `*pos`.
fn vlq(bytes: &[u8], pos: &mut usize) -> Option<i64> {
    let mut result = 0u32;
    let mut shift = 0u32;
    loop {
        let d = b64(*bytes.get(*pos)?)? as u32;
        *pos += 1;
        let bits = d & 31;
        if bits != 0 {
            if bits > u32::MAX.checked_shr(shift)? {
                return None;
            }
            result |= bits << shift;
        }
        if d & 32 == 0 {
            break;
        }
        // Redundant zero groups do not change the value.
        shift = shift.saturating_add(5);
    }
    let magnitude = i64::from(result >> 1);
    Some(match (result & 1, magnitude) {
        (1, 0) => i64::from(i32::MIN),
        (1, n) => -n,
        _ => magnitude,
    })
}

fn decode_mappings(m: &str, map: &mut SourceMap, budget: &mut usize) -> Result<(), SourceMapError> {
    let bytes = m.as_bytes();
    let (mut src, mut ol, mut oc, mut name) = (0i64, 0i64, 0i64, 0i64);
    let mut pos = 0usize;
    let mut line: Vec<Segment> = Vec::new();
    let mut col = 0i64;
    let mut count = 0usize;
    let to_u32 = |v: i64, count: usize| u32::try_from(v).map_err(|_| SourceMapError::BadMappings(count));
    while pos <= bytes.len() {
        match bytes.get(pos) {
            None | Some(b';') => {
                line.sort_by_key(|s| s.generated_column);
                map.lines.push(std::mem::take(&mut line));
                col = 0;
                pos += 1;
                continue;
            }
            Some(b',') => {
                pos += 1;
                continue;
            }
            _ => {}
        }
        if *budget == 0 {
            return Err(SourceMapError::TooManySegments(count));
        }
        *budget -= 1;
        count += 1;
        col += vlq(bytes, &mut pos).ok_or(SourceMapError::BadMappings(count))?;
        let mut seg = Segment { generated_column: to_u32(col, count)?, original: None };
        if !matches!(bytes.get(pos), None | Some(b',') | Some(b';')) {
            src += vlq(bytes, &mut pos).ok_or(SourceMapError::BadMappings(count))?;
            ol += vlq(bytes, &mut pos).ok_or(SourceMapError::BadMappings(count))?;
            oc += vlq(bytes, &mut pos).ok_or(SourceMapError::BadMappings(count))?;
            let mut n = None;
            if !matches!(bytes.get(pos), None | Some(b',') | Some(b';')) {
                name += vlq(bytes, &mut pos).ok_or(SourceMapError::BadMappings(count))?;
                n = Some(to_u32(name, count)?);
            }
            seg.original = Some((to_u32(src, count)?, to_u32(ol, count)?, to_u32(oc, count)?, n));
        }
        if !matches!(bytes.get(pos), None | Some(b',') | Some(b';')) {
            return Err(SourceMapError::BadMappings(count));
        }
        line.push(seg);
    }
    map.segments = count;
    Ok(())
}

/// Find the `sourceMappingURL` comment near the end of a script.
pub fn find_source_mapping_url(js: &str) -> Option<String> {
    for line in js.lines().rev().take(64) {
        let t = line.trim();
        for prefix in ["//# sourceMappingURL=", "//@ sourceMappingURL=", "/*# sourceMappingURL="] {
            if let Some(rest) = t.strip_prefix(prefix) {
                let url = rest.trim_end_matches("*/").trim();
                if !url.is_empty() {
                    return Some(url.to_string());
                }
            }
        }
    }
    None
}

/// Decode a `data:` URL, including percent escapes and optional base64 content.
pub fn decode_data_url(url: &str) -> Option<Vec<u8>> {
    let (scheme, rest) = url.split_once(':')?;
    if !scheme.eq_ignore_ascii_case("data") {
        return None;
    }
    let rest = rest.split_once('#').map_or(rest, |(body, _)| body);
    let (meta, payload) = rest.split_once(',')?;
    let decoded: std::borrow::Cow<'_, [u8]> = if payload.contains('%') {
        let hex = |b: u8| match b {
            b'0'..=b'9' => Some(b - b'0'),
            b'a'..=b'f' => Some(b - b'a' + 10),
            b'A'..=b'F' => Some(b - b'A' + 10),
            _ => None,
        };
        let mut decoded = Vec::with_capacity(payload.len());
        let mut bytes = payload.bytes();
        while let Some(b) = bytes.next() {
            decoded.push(if b == b'%' { (hex(bytes.next()?)? << 4) | hex(bytes.next()?)? } else { b });
        }
        std::borrow::Cow::Owned(decoded)
    } else {
        std::borrow::Cow::Borrowed(payload.as_bytes())
    };
    if meta.rsplit_once(';').is_some_and(|(_, flag)| flag.trim_matches(|c: char| c.is_ascii_whitespace()).eq_ignore_ascii_case("base64")) {
        base64_decode(std::str::from_utf8(&decoded).ok()?)
    } else {
        Some(decoded.into_owned())
    }
}

/// Standard or URL-safe base64, padding optional. `None` on any invalid character.
pub fn base64_decode(text: &str) -> Option<Vec<u8>> {
    crate::bytes::decode_base64(text, true)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lookup_keeps_coincident_mappings_and_stops_at_an_unmapped_segment() {
        let map = parse(br#"{"version":3,"sources":["a","b"],"mappings":"EAAA,ACAA,E"}"#, Limits::default()).unwrap();
        assert!(map.original_positions(0, 1).is_empty());
        let matches = map.original_positions(0, 3);
        assert_eq!(matches.iter().map(|p| p.source_index).collect::<Vec<_>>(), [0, 1]);
        assert!(map.original_positions(0, 4).is_empty());
    }

    #[test]
    fn relative_sources_keep_the_url_root_delimiter() {
        for (root, expected) in [
            ("webpack:///", "webpack:///src/main.js"),
            ("file:///", "file:///src/main.js"),
            ("https://host/scripts/", "https://host/scripts/src/main.js"),
            ("src", "src/src/main.js"),
        ] {
            let json = serde_json::json!({"version":3,"sourceRoot":root,"sources":["src/main.js"],"mappings":"AAAA"});
            let map = parse(json.to_string().as_bytes(), Limits::default()).unwrap();
            assert_eq!(map.all_sources(), [expected]);
            assert_eq!(map.original_positions(0, 0)[0].source.as_deref(), Some(expected));
        }
    }

    #[test]
    fn decodes_and_looks_up() {
        // Generated "a;b" from two lines of src.js.
        let m = br#"{"version":3,"sources":["src.js"],"names":["foo"],"mappings":"AAAAA,EAAE;AACA","sourcesContent":["x\ny"]}"#;
        let map = parse(m, Limits::default()).unwrap();
        assert_eq!(map.lines.len(), 2);
        let p = map.original_positions(0, 1);
        assert_eq!(p[0].source.as_deref(), Some("src.js"));
        assert_eq!(p[0].name.as_deref(), Some("foo"));
        assert_eq!((p[0].line, p[0].column), (0, 0));
        let q = map.original_positions(0, 2);
        assert_eq!(q[0].column, 2);
        let r = map.original_positions(1, 0);
        assert_eq!(r[0].line, 1);
        assert!(map.original_positions(5, 0).is_empty());
        assert_eq!(map.source_content(0), Some("x\ny"));
    }

    #[test]
    fn data_url_fragments_are_not_part_of_the_source_map() {
        assert_eq!(decode_data_url("DATA:application/json;BASE64,e30=#section"), Some(b"{}".to_vec()));
        assert_eq!(decode_data_url("data:text/plain,one%23two#ignored%"), Some(b"one#two".to_vec()));
        assert_eq!(decode_data_url("data:,body?query#fragment"), Some(b"body?query".to_vec()));
        assert!(decode_data_url("http:,payload").is_none());
    }

    #[test]
    fn indexed_maps() {
        let m = br#"{"version":3,"sections":[{"offset":{"line":0,"column":0},"map":{"version":3,"sources":["a.js"],"names":[],"mappings":"AAAA"}},{"offset":{"line":1,"column":0},"map":{"version":3,"sources":["b.js"],"names":[],"mappings":"AAAA"}}]}"#;
        let map = parse(m, Limits::default()).unwrap();
        assert_eq!(map.original_positions(1, 3)[0].source.as_deref(), Some("b.js"));
        assert_eq!(map.all_sources(), vec!["a.js", "b.js"]);
    }

    #[test]
    fn validates_index_map_shape_and_offsets() {
        let inline = serde_json::json!({"version":3,"mappings":"A"});
        for sections in [
            serde_json::json!({}),
            serde_json::json!([{"map":inline}]),
            serde_json::json!([{"offset":{"line":-1,"column":0},"map":inline}]),
            serde_json::json!([{"offset":{"line":0,"column":4294967296_u64},"map":inline}]),
            serde_json::json!([{"offset":{"line":0,"column":0}}]),
            serde_json::json!([{"offset":{"line":0,"column":0},"map":inline,"url":"later.map"}]),
            serde_json::json!([{"offset":{"line":0,"column":2},"map":inline},{"offset":{"line":0,"column":1},"map":inline}]),
        ] {
            let json = serde_json::json!({"version":3,"sections":sections}).to_string();
            assert_eq!(parse(json.as_bytes(), Limits::default()), Err(SourceMapError::BadSections));
        }
    }

    #[test]
    fn an_external_section_ends_the_previous_inline_mapping() {
        let inline = serde_json::json!({"version":3,"sources":["first.js"],"mappings":"AAAA"});
        let sections = serde_json::json!([
            {"offset":{"line":0,"column":0},"map":inline},
            {"offset":{"line":0,"column":4},"url":"external.map"},
            {"offset":{"line":1,"column":0},"map":inline}
        ]);
        let json = serde_json::json!({"version":3,"sections":sections}).to_string();
        let map = parse(json.as_bytes(), Limits::default()).unwrap();
        assert_eq!(map.original_positions(0, 3)[0].source.as_deref(), Some("first.js"));
        assert!(map.original_positions(0, 4).is_empty());
        assert_eq!(map.original_positions(1, 0)[0].generated_column, 0);
    }

    #[test]
    fn finds_comment_and_data_urls() {
        assert_eq!(find_source_mapping_url("x()\n//# sourceMappingURL=app.js.map\n").as_deref(), Some("app.js.map"));
        assert_eq!(base64_decode("aGVsbG8=").unwrap(), b"hello");
        assert_eq!(decode_data_url("data:application/json;base64,e30=").unwrap(), b"{}");
    }

    #[test]
    fn vlq_signed_extremes_and_overflow() {
        for (field, value) in [
            ("B", Some(i64::from(i32::MIN))),
            ("+/////D", Some(i64::from(i32::MAX))),
            ("//////D", Some(-i64::from(i32::MAX))),
            ("ggggggE", None),
            ("ggggggggggA", Some(0)),
        ] {
            assert_eq!(vlq(field.as_bytes(), &mut 0), value, "{field}");
        }
        let map = parse(br#"{"version":3,"mappings":"+/////D,C,B"}"#, Limits::default()).unwrap();
        assert_eq!(map.lines[0].iter().map(|s| s.generated_column).collect::<Vec<_>>(), [0, 2147483647, 2147483648]);
        assert!(parse(br#"{"version":3,"mappings":"B"}"#, Limits::default()).is_err());
    }

    #[test]
    fn hostile_mappings_do_not_panic() {
        for m in ["", ";;;", "A", "gggggggggggggggggggggggggg", "AAAA,", "////", "AAAAAAAAAAAAAAA", "D", "AD,AA", "+", "!!"] {
            let j = format!(r#"{{"version":3,"sources":[],"names":[],"mappings":"{m}"}}"#);
            if let Ok(map) = parse(j.as_bytes(), Limits::default()) {
                let _ = map.original_positions(0, 0);
                let _ = map.original_positions(u32::MAX, u32::MAX);
            }
        }
        let tight = Limits { max_segments: 1, ..Limits::default() };
        assert!(parse(br#"{"version":3,"sources":[],"names":[],"mappings":"A,A"}"#, tight).is_err());
        assert!(parse(b"[]", Limits::default()).is_err());
    }
}
