use craft_anything_analysis::web_har::{HarOptions, inspect_har};
use serde_json::json;

#[test]
fn received_and_decoded_sizes_stay_separate() {
    let responses = [
        json!({"status": 200, "bodySize": 25, "content": {"size": 100}}),
        json!({"status": 304, "bodySize": 0, "content": {"size": 120}}),
        json!({"status": 200, "bodySize": -1, "content": {"size": 50}}),
        json!({"status": 200, "bodySize": 8, "content": {"size": -1}}),
        json!({"status": 200, "content": {}}),
    ];
    let entries: Vec<_> =
        responses.into_iter().map(|response| json!({"request": {"method": "GET", "url": "https://example.test/"}, "response": response})).collect();
    let bytes = serde_json::to_vec(&json!({"log": {"entries": entries}})).unwrap();
    let report = inspect_har(&bytes, &HarOptions { max_entries: 10, limit: 10, ..Default::default() }).unwrap();
    let summary = &report["summary"];
    assert_eq!(summary["response_body_bytes"], 33);
    assert_eq!(summary["responses_without_body_size"], 2);
    assert_eq!(summary["response_content_bytes"], 270);
    assert_eq!(summary["responses_without_content_size"], 2);
}
