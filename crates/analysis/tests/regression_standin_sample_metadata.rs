use craft_anything_analysis::standin::{Answer, BodyLeftOut, BuildOptions, StandIn, Turns, answer, build_from_bytes};
use serde_json::{Value, json};

fn entry(status: u16, mime: &str, body: &str, headers: Value) -> Value {
    json!({
        "request": {"method": "GET", "url": "https://example.test/resource"},
        "response": {"status": status, "headers": headers, "content": {"mimeType": mime, "text": body}}
    })
}

fn capture(entries: Vec<Value>, opts: &BuildOptions) -> StandIn {
    build_from_bytes(&serde_json::to_vec(&json!({"log": {"entries": entries}})).unwrap(), opts).unwrap()
}

#[test]
fn redirects_with_different_destinations_remain_separate_samples() {
    let make = |destination| entry(302, "text/plain", "", json!([{"name": "Location", "value": destination}]));
    let standin = capture(vec![make("/first"), make("/second")], &BuildOptions::default());
    assert_eq!(standin.routes[0].samples.len(), 2);
    let mut turns = Turns::default();
    for expected in ["/first", "/second"] {
        let Answer::Found(found) = answer(&standin, &mut turns, "GET", "/resource", "", b"") else {
            panic!("missing redirect");
        };
        let destination = found.headers.iter().find(|(name, _)| name.eq_ignore_ascii_case("location")).map(|(_, value)| value.as_str());
        assert_eq!(destination, Some(expected));
    }
}

#[test]
fn equal_bodies_with_different_content_types_remain_distinct() {
    let standin = capture(vec![entry(200, "application/json", "{}", json!([])), entry(200, "text/plain", "{}", json!([]))], &BuildOptions::default());
    assert_eq!(standin.routes[0].samples.len(), 2);
    assert_eq!(standin.routes[0].samples[0].content_type.as_deref(), Some("application/json"));
    assert_eq!(standin.routes[0].samples[1].content_type.as_deref(), Some("text/plain"));
}

#[test]
fn missing_bodies_keep_their_different_explanations() {
    let options = BuildOptions { max_body_bytes: 1, ..Default::default() };
    let standin = capture(vec![entry(200, "text/plain", "", json!([])), entry(200, "text/plain", "long", json!([]))], &options);
    assert_eq!(standin.routes[0].samples.len(), 2);
    assert_eq!(standin.routes[0].samples[0].body_left_out, Some(BodyLeftOut::NotRecorded));
    assert_eq!(standin.routes[0].samples[1].body_left_out, Some(BodyLeftOut::TooLarge));
}

#[test]
fn cached_304_bodies_keep_their_recorded_status_marker() {
    let standin = capture(vec![entry(200, "application/json", "{}", json!([])), entry(304, "application/json", "{}", json!([]))], &BuildOptions::default());
    let samples = &standin.routes[0].samples;
    assert_eq!(samples.len(), 2);
    assert_eq!((samples[0].status, samples[0].was_not_modified), (200, false));
    assert_eq!((samples[1].status, samples[1].was_not_modified), (200, true));
}

#[test]
fn identical_answers_still_collapse() {
    let one = entry(200, "application/json", "{}", json!([]));
    let standin = capture(vec![one.clone(), one], &BuildOptions::default());
    assert_eq!(standin.routes[0].requests, 2);
    assert_eq!(standin.routes[0].samples.len(), 1);
}
