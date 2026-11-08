use craft_anything_analysis::standin::{Answer, BodyLeftOut, BuildOptions, NoMatchReason, StandIn, Turns, answer, build_from_bytes};
use serde_json::{Value, json};

fn entry(url: &str, status: u16, body: &str, headers: Value) -> Value {
    json!({
        "request": {"method": "GET", "url": url},
        "response": {"status": status, "headers": headers, "content": {"mimeType": "text/plain", "text": body}}
    })
}

fn operation(name: &str, status: u16, body: &str) -> Value {
    let mut value = entry("https://example.test/graphql", status, body, json!([]));
    value["request"]["method"] = json!("POST");
    value["request"]["postData"] = json!({"text": json!({"operationName": name}).to_string()});
    value
}

fn capture(entries: Vec<Value>, opts: &BuildOptions) -> StandIn {
    build_from_bytes(&serde_json::to_vec(&json!({"log": {"entries": entries}})).unwrap(), opts).unwrap()
}

fn get(standin: &StandIn, turns: &mut Turns, path: &str, query: &str) -> Answer {
    answer(standin, turns, "GET", path, query, b"")
}

fn assert_unavailable(answer: Answer) {
    let Answer::NoMatch(missing) = answer else { panic!("a missing cached body must not use another request's answer") };
    assert_eq!(missing.reason, NoMatchReason::CachedBodyUnavailable);
}

#[test]
fn equivalent_headers_do_not_consume_sample_capacity() {
    let first = entry(
        "https://example.test/resource",
        200,
        "same",
        json!([{"name": "X-Mode", "value": "first"}, {"name": "X-Other", "value": "other"}, {"name": "x-MODE", "value": "second"}]),
    );
    let equivalent = entry(
        "https://example.test/resource",
        200,
        "same",
        json!([{"name": "x-other", "value": "other"}, {"name": "x-mode", "value": "first"}, {"name": "X-MODE", "value": "second"}]),
    );
    let later = entry("https://example.test/resource", 200, "later", json!([]));
    let opts = BuildOptions { max_samples_per_route: 2, ..Default::default() };
    let standin = capture(vec![first.clone(), equivalent, first, later], &opts);
    let route = &standin.routes[0];
    assert_eq!((route.requests, route.samples.len(), route.samples_left_out), (4, 2, 0));
    assert_eq!(route.samples[0].headers, vec![("X-Mode".into(), "first".into()), ("X-Other".into(), "other".into()), ("x-MODE".into(), "second".into())]);
    let mut turns = Turns::default();
    for expected in ["same", "later"] {
        let Answer::Found(found) = get(&standin, &mut turns, "/resource", "") else { panic!("missing recorded answer") };
        assert_eq!(found.body, expected.as_bytes());
    }
}

#[test]
fn repeated_header_values_keep_their_order_and_count() {
    let headers = [
        json!([{"name": "X-Mode", "value": "first"}, {"name": "X-Other", "value": "other"}, {"name": "x-mode", "value": "second"}]),
        json!([{"name": "x-MODE", "value": "second"}, {"name": "x-other", "value": "other"}, {"name": "X-MODE", "value": "first"}]),
        json!([{"name": "x-mode", "value": "first"}, {"name": "x-other", "value": "other"}, {"name": "X-Other", "value": "other"}]),
    ];
    let standin = capture(headers.into_iter().map(|h| entry("https://example.test/resource", 200, "same", h)).collect(), &BuildOptions::default());
    assert_eq!(standin.routes[0].samples.len(), 3);
    let mut turns = Turns::default();
    for expected in [vec!["first", "second"], vec!["second", "first"], vec!["first"]] {
        let Answer::Found(found) = get(&standin, &mut turns, "/resource", "") else { panic!("missing distinct header values") };
        let values: Vec<_> = found.headers.iter().filter(|(name, _)| name.eq_ignore_ascii_case("x-mode")).map(|(_, value)| value.as_str()).collect();
        assert_eq!(values, expected);
    }
}

#[test]
fn full_routes_replace_matching_304s_in_recording_order() {
    let opts = BuildOptions::default();
    assert_eq!(opts.max_samples_per_route, 16);
    let mut entries: Vec<_> =
        (0..16).map(|i| entry("https://example.test/resource", 304, "", json!([{"name": "X-Request-Id", "value": format!("poll-{i}")}]))).collect();
    entries.push(entry("https://example.test/resource", 200, "first", json!([])));
    entries.push(entry("https://example.test/resource", 200, "second", json!([])));
    let standin = capture(entries, &opts);
    let route = &standin.routes[0];
    assert_eq!((route.requests, route.samples.len(), route.samples_left_out), (18, 16, 2));
    assert_eq!(route.statuses, vec![200, 304]);
    for (sample, i) in route.samples.iter().take(14).zip(2..16) {
        assert_eq!(sample.status, 304);
        assert_eq!(sample.headers, vec![("X-Request-Id".into(), format!("poll-{i}"))]);
    }
    assert_eq!(route.samples[14].text.as_deref(), Some("first"));
    assert_eq!(route.samples[15].text.as_deref(), Some("second"));
    let mut turns = Turns::default();
    for expected in ["first", "second", "first"] {
        let Answer::Found(found) = get(&standin, &mut turns, "/resource", "") else { panic!("a usable answer was displaced by unavailable 304s") };
        assert_eq!((found.status, found.body), (200, expected.as_bytes().to_vec()));
    }
}

#[test]
fn full_routes_preserve_missing_queries_and_resources() {
    for (missing_url, ready_url, missing_path, missing_query) in [
        ("https://example.test/items/1?view=missing", "https://example.test/items/1?view=ready", "/items/1", "view=missing"),
        ("https://example.test/items/1", "https://example.test/items/2", "/items/1", ""),
    ] {
        let opts = BuildOptions { max_samples_per_route: 2, ..Default::default() };
        let standin =
            capture(vec![entry(missing_url, 304, "", json!([])), entry(ready_url, 304, "", json!([])), entry(ready_url, 200, "ready", json!([]))], &opts);
        let route = &standin.routes[0];
        assert_eq!((route.requests, route.samples.len(), route.samples_left_out), (3, 2, 1));
        assert_eq!(route.samples[0].status, 304);
        assert_eq!(route.samples[1].text.as_deref(), Some("ready"));
        assert_unavailable(get(&standin, &mut Turns::default(), missing_path, missing_query));
    }
}

#[test]
fn full_routes_preserve_missing_graphql_operations() {
    let opts = BuildOptions { max_samples_per_route: 2, ..Default::default() };
    let standin = capture(vec![operation("Missing", 304, ""), operation("Ready", 304, ""), operation("Ready", 200, "ready")], &opts);
    let route = &standin.routes[0];
    assert_eq!((route.requests, route.samples.len(), route.samples_left_out), (3, 2, 1));
    assert_eq!(route.samples[0].request_key.as_deref(), Some("op:Missing"));
    assert_unavailable(answer(&standin, &mut Turns::default(), "POST", "/graphql", "", br#"{"operationName":"Missing"}"#));
    let Answer::Found(found) = answer(&standin, &mut Turns::default(), "POST", "/graphql", "", br#"{"operationName":"Ready"}"#) else {
        panic!("missing usable operation");
    };
    assert_eq!(found.body, b"ready");

    let opts = BuildOptions { max_samples_per_route: 1, hidden_values: vec!["Hidden".into()], ..Default::default() };
    let standin = capture(vec![operation("Hidden", 304, ""), operation("Hidden", 200, "ok")], &opts);
    assert_eq!(standin.routes[0].samples[0].request_key, None);
    let Answer::Found(found) = answer(&standin, &mut Turns::default(), "POST", "/graphql", "", br#"{"operationName":"Hidden"}"#) else {
        panic!("replacement used an unsanitized request key");
    };
    assert_eq!(found.body, b"ok");
}

#[test]
fn zero_and_single_sample_limits_remain_bounded() {
    for limit in [0, 1] {
        for status in [200, 304] {
            let opts = BuildOptions { max_samples_per_route: limit, ..Default::default() };
            let standin = capture(
                vec![entry("https://example.test/resource", 304, "", json!([])), entry("https://example.test/resource", status, "ok", json!([]))],
                &opts,
            );
            let route = &standin.routes[0];
            assert_eq!(route.samples.len(), limit);
            assert_eq!(route.samples_left_out, if limit == 0 { 2 } else { 1 });
            if limit == 1 {
                assert_eq!(route.samples[0].text.as_deref(), Some("ok"));
                assert_eq!(route.samples[0].was_not_modified, status == 304);
                let Answer::Found(found) = get(&standin, &mut Turns::default(), "/resource", "") else { panic!("missing replacement") };
                assert_eq!((found.status, found.body), (200, b"ok".to_vec()));
            }
        }
    }
}

#[test]
fn rejected_replacements_leave_budget_for_other_routes() {
    for (url, status, body) in [
        ("https://example.test/resource", 304, "long"),
        ("https://example.test/resource", 200, "long"),
        ("https://example.test/resource?other=true", 200, "no"),
    ] {
        let opts = BuildOptions { max_samples_per_route: 1, max_total_body_bytes: 2, ..Default::default() };
        let standin = capture(
            vec![
                entry("https://example.test/resource", 304, "", json!([{"name": "X-Request-Id", "value": "original"}])),
                entry(url, status, body, json!([])),
                entry("https://example.test/later", 200, "ok", json!([])),
            ],
            &opts,
        );
        let route = standin.routes.iter().find(|route| route.template == "/resource").unwrap();
        assert_eq!((route.requests, route.samples.len(), route.samples_left_out), (2, 1, 1));
        assert_eq!(route.samples[0].status, 304);
        assert_eq!(route.samples[0].body_left_out, Some(BodyLeftOut::NotRecorded));
        assert_eq!(route.samples[0].headers, vec![("X-Request-Id".into(), "original".into())]);
        assert_unavailable(get(&standin, &mut Turns::default(), "/resource", ""));
        let Answer::Found(found) = get(&standin, &mut Turns::default(), "/later", "") else { panic!("rejected sample used the remaining budget") };
        assert_eq!(found.body, b"ok");
    }
}

#[test]
fn a_repeat_at_capacity_preserves_the_304_without_charging_its_body() {
    for budget in [2, 4] {
        let opts = BuildOptions { max_samples_per_route: 2, max_total_body_bytes: budget, ..Default::default() };
        let usable = entry("https://example.test/resource", 200, "ok", json!([]));
        let mut entries = vec![entry("https://example.test/resource", 304, "", json!([])), usable.clone(), usable];
        if budget == 4 {
            entries.push(entry("https://example.test/later", 200, "zz", json!([])));
        }
        let standin = capture(entries, &opts);
        let route = standin.routes.iter().find(|route| route.template == "/resource").unwrap();
        assert_eq!((route.requests, route.samples.len()), (3, 2));
        assert_eq!(route.samples_left_out, 0);
        assert_eq!(route.samples[0].status, 304);
        assert_eq!(route.samples[1].text.as_deref(), Some("ok"));
        let mut turns = Turns::default();
        for _ in 0..3 {
            let Answer::Found(found) = get(&standin, &mut turns, "/resource", "") else { panic!("missing retained answer") };
            assert_eq!(found.body, b"ok");
        }
        if budget == 4 {
            let Answer::Found(found) = get(&standin, &mut Turns::default(), "/later", "") else { panic!("duplicate body was charged") };
            assert_eq!(found.body, b"zz");
        }
    }
}

#[test]
fn refused_bodies_keep_missing_evidence_and_count_headers_once() {
    let mut invalid = entry("https://example.test/resource", 200, "%%%", json!([]));
    invalid["response"]["content"]["encoding"] = json!("base64");
    let private_key_marker = ["PRIVATE ", "KEY-----"].concat();
    let cases = [
        (entry("https://example.test/resource", 200, "large", json!([])), BuildOptions { max_body_bytes: 1, ..Default::default() }),
        (invalid, BuildOptions::default()),
        (entry("https://example.test/resource", 200, &private_key_marker, json!([])), BuildOptions::default()),
    ];
    for (mut refused, mut opts) in cases {
        opts.max_samples_per_route = 1;
        refused["response"]["headers"] = json!([{"name": "Authorization", "value": "Bearer fake-token"}]);
        let standin =
            capture(vec![entry("https://example.test/resource", 304, "", json!([])), refused, entry("https://example.test/later", 200, "z", json!([]))], &opts);
        let route = standin.routes.iter().find(|route| route.template == "/resource").unwrap();
        assert_eq!((route.samples.len(), route.samples_left_out), (1, 1));
        assert_eq!(route.samples[0].body_left_out, Some(BodyLeftOut::NotRecorded));
        assert_eq!(standin.dropped.headers, 1);
        assert_unavailable(get(&standin, &mut Turns::default(), "/resource", ""));
        let Answer::Found(found) = get(&standin, &mut Turns::default(), "/later", "") else { panic!("a refused body consumed the budget") };
        assert_eq!(found.body, b"z");
    }
}
