use craft_anything_formats::ai_model;
use serde_json::json;

fn safetensors(shape: &[u64], data_bytes: usize) -> Vec<u8> {
    let header = serde_json::to_vec(&json!({"weight": {"dtype": "F32", "shape": shape, "data_offsets": [0, data_bytes]}})).unwrap();
    let mut data = (header.len() as u64).to_le_bytes().to_vec();
    data.extend_from_slice(&header);
    data.resize(data.len() + data_bytes, 0);
    data
}

fn gguf(shape: &[u64]) -> Vec<u8> {
    let mut data = b"GGUF".to_vec();
    data.extend_from_slice(&3u32.to_le_bytes());
    data.extend_from_slice(&1u64.to_le_bytes()); // One tensor.
    data.extend_from_slice(&0u64.to_le_bytes()); // No metadata.
    data.extend_from_slice(&6u64.to_le_bytes());
    data.extend_from_slice(b"weight");
    data.extend_from_slice(&(shape.len() as u32).to_le_bytes());
    for dimension in shape {
        data.extend_from_slice(&dimension.to_le_bytes());
    }
    data.extend_from_slice(&0u32.to_le_bytes()); // F32.
    data.extend_from_slice(&0u64.to_le_bytes()); // Relative data offset.
    data.resize(data.len().next_multiple_of(32), 0);
    data
}

#[test]
fn an_empty_tensor_has_zero_elements_in_every_axis_order() {
    for shape in [[u64::MAX, 2, 0], [2, u64::MAX, 0], [u64::MAX, 0, 2], [0, u64::MAX, 2]] {
        for data in [safetensors(&shape, 0), gguf(&shape)] {
            let report = ai_model::parse(&data, data.len() as u64).unwrap();
            assert_eq!((report.parameters, report.tensors[0].parameters), (0, 0), "shape {shape:?}, format {:?}", report.format);
            assert_eq!(report.tensors[0].data_bytes, Some(0));
            assert_eq!(report.parameters_by_type["F32"], 0);
            assert!(report.complete);
            assert_eq!(report.tensors_past_end, 0);
            assert!(report.warnings.is_empty(), "{:?}", report.warnings);
        }
    }
}

#[test]
fn scalar_nonempty_and_overflowing_shapes_keep_their_counts() {
    for (shape, bytes, parameters) in [(vec![], 4, 1), (vec![2, 3], 24, 6)] {
        let data = safetensors(&shape, bytes);
        let report = ai_model::parse(&data, data.len() as u64).unwrap();
        assert_eq!(report.parameters, parameters);
        assert!(report.warnings.is_empty());
    }
    let data = safetensors(&[u64::MAX, 2], 0);
    let report = ai_model::parse(&data, data.len() as u64).unwrap();
    assert_eq!(report.parameters, u64::MAX);
    assert!(report.warnings.iter().any(|w| w.contains("shape multiplies")));
}
