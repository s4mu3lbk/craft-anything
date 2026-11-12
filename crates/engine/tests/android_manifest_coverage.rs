use craft_anything_engine::{CallOptions, Caller, Engine, ToolOutput};
use craft_anything_testkit::{android as fx, electron::zip_stored};
use serde_json::{Value, json};

fn manifest(partial: bool) -> Vec<u8> {
    use fx::{
        AxNode::{End, Start},
        AxVal::Str,
    };
    let mut data = fx::axml(&[
        Start("manifest", vec![fx::AxAttr { name: "package", res_id: None, val: Str("com.example.coverage") }]),
        Start("application", vec![]),
        Start("activity", vec![fx::a("name", 0x0101_0003, Str(".First"))]),
        End("activity"),
        Start("service", vec![fx::a("name", 0x0101_0003, Str(".Second"))]),
        End("service"),
        End("application"),
        End("manifest"),
    ]);
    if partial {
        data.push(0);
        let size = data.len() as u32;
        data[4..8].copy_from_slice(&size.to_le_bytes());
    }
    let doc = craft_anything_formats::axml::parse(&data).unwrap();
    assert_eq!(doc.truncated, partial);
    assert_eq!(craft_anything_formats::axml::manifest(&doc).components.len(), 2);
    data
}

fn apk(manifest: Option<&[u8]>) -> std::path::PathBuf {
    let entries = match manifest {
        Some(data) => vec![("AndroidManifest.xml", data)],
        None => vec![("assets/note.txt", b"fixture".as_slice())],
    };
    craft_anything_testkit::temp_file("coverage.apk", &zip_stored(&entries))
}

fn call(engine: &Engine, id: &str, params: Value) -> Value {
    let output = engine.call(id, params, CallOptions::new(Caller::Mcp)).unwrap();
    let ToolOutput::Evidence(record) = output else { panic!("{id} did not return evidence") };
    assert!(record.verify());
    serde_json::to_value(&*record).unwrap()
}

#[test]
fn package_coverage_discloses_a_recovered_partial_manifest() {
    let engine = Engine::new(craft_anything_engine::testing::session());
    let path = apk(Some(&manifest(true)));
    let record = call(&engine, "android_package_show", json!({"path": path}));
    assert_eq!(record["coverage"]["status"], "partial", "{}", record["coverage"]);
    assert!(record["coverage"]["examined"].is_null());
    assert!(record["coverage"]["total"].is_null());
    assert_eq!(record["result"]["package"], "com.example.coverage");
    assert_eq!(record["result"]["manifest"]["components"].as_array().unwrap().len(), 2);
    assert!(record["result"]["warnings"].as_array().unwrap().iter().any(|warning| warning.as_str().is_some_and(|text| text.contains("unreadable chunk"))));
}

#[test]
fn package_coverage_is_partial_without_a_readable_manifest() {
    let engine = Engine::new(craft_anything_engine::testing::session());
    for manifest in [None, Some(b"unreadable binary XML".as_slice())] {
        let record = call(&engine, "android_package_show", json!({"path": apk(manifest)}));
        assert_eq!(record["coverage"]["status"], "partial", "{}", record["coverage"]);
        assert!(record["coverage"]["total"].is_null());
        assert!(record["result"]["manifest"].is_null());
    }
}

#[test]
fn component_coverage_stays_partial_on_last_and_empty_pages() {
    let engine = Engine::new(craft_anything_engine::testing::session());
    let path = apk(Some(&manifest(true)));
    for offset in [1, 2, 0] {
        let record = call(&engine, "android_components_list", json!({"path": path, "offset": offset, "limit": 1}));
        assert_eq!(record["coverage"]["status"], "partial", "offset {offset}: {}", record["coverage"]);
        assert!(record["coverage"]["total"].is_null());
        assert_eq!(record["coverage"]["next_offset"], if offset == 0 { json!(1) } else { Value::Null });
        assert_eq!(record["result"]["components"].as_array().unwrap().len(), usize::from(offset < 2));
        assert_eq!(record["result"]["total"], 2);
    }
}

#[test]
fn complete_manifests_keep_existing_page_coverage() {
    let engine = Engine::new(craft_anything_engine::testing::session());
    let path = apk(Some(&manifest(false)));
    let record = call(&engine, "android_package_show", json!({"path": path}));
    assert_eq!(record["coverage"]["status"], "complete");
    for offset in [0, 1, 2] {
        let record = call(&engine, "android_components_list", json!({"path": path, "offset": offset, "limit": 1}));
        assert_eq!(record["coverage"]["status"], if offset == 0 { "partial" } else { "complete" });
        assert_eq!(record["coverage"]["total"], 2);
        assert_eq!(record["coverage"]["next_offset"], if offset == 0 { json!(1) } else { Value::Null });
    }
}
