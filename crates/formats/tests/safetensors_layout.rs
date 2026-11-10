use craft_anything_formats::ai_model::{self, ModelReport};
use serde_json::{Value, json};

fn report(header: Value, payload_bytes: u64) -> ModelReport {
    let json = serde_json::to_vec(&header).unwrap();
    let mut header = (json.len() as u64).to_le_bytes().to_vec();
    header.extend(json);
    ai_model::parse(&header, header.len() as u64 + payload_bytes).unwrap()
}

fn tensor(dtype: &str, elements: u64, start: u64, end: u64) -> Value {
    json!({"dtype": dtype, "shape": [elements], "data_offsets": [start, end]})
}

#[test]
fn only_string_metadata_is_reported_and_model_titles_share_the_display_bound() {
    let long = "x".repeat(4096);
    let r = report(json!({"__metadata__": {"name": long, "architecture": 7}}), 0);
    assert_eq!((r.declared_metadata, r.metadata_read), (2, 1));
    assert!(r.architecture.is_none());
    assert!(r.metadata[0].shortened);
    assert_eq!(r.name.as_deref(), r.metadata[0].value.as_str());
    assert!(r.warnings.iter().any(|w| w.contains("not a string")));
}

#[test]
fn reports_coverage_in_offset_order_including_empty_tensors() {
    let check = |a, b, size| report(json!({"a": a, "b": b}), size).warnings;
    assert!(check(tensor("U8", 2, 0, 2), tensor("U8", 1, 1, 2), 2).iter().any(|w| w.contains("overlap")));
    assert!(check(tensor("U8", 1, 2, 3), tensor("U8", 1, 0, 1), 3).iter().any(|w| w.contains("gap")));
    assert!(check(tensor("U8", 1, 0, 1), tensor("U8", 1, 1, 2), 3).iter().any(|w| w.contains("after the last")));
    assert!(check(tensor("U8", 1, 0, 1), tensor("U8", 0, 0, 0), 1).is_empty());
    assert!(check(tensor("U8", 1, 1, 2), tensor("U8", 1, 0, 1), 2).is_empty());
}

#[test]
fn checks_fp8_and_packed_four_and_six_bit_tensor_sizes() {
    for (ty, n, bytes) in [("F4", 2, 1), ("F6_E3M2", 4, 3), ("F6_E2M3", 4, 3), ("F8_E8M0", 3, 3), ("F8_E4M3FNUZ", 4, 4), ("F8_E5M2", 4, 4)] {
        assert!(report(json!({"t":tensor(ty,n,0,bytes)}), bytes).warnings.is_empty());
        let invalid = report(json!({"t":tensor(ty,n,0,bytes+1)}), bytes + 1);
        assert!(invalid.warnings.iter().any(|w| w.contains("shape and type")), "{ty}");
    }
    assert!(report(json!({"t":tensor("F4",1,0,1)}), 1).warnings.iter().any(|w| w.contains("shape and type")));
}

#[test]
fn rejects_headers_over_the_format_limit_before_requesting_more_bytes() {
    let mut prefix = 100_000_001u64.to_le_bytes().to_vec();
    prefix.push(b'{');
    let err = ai_model::parse(&prefix, 200_000_000).unwrap_err();
    assert!(err.to_string().contains("100 MB"));
}
