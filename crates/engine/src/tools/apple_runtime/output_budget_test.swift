// Synthetic regression fixture compiled with the production helper functions.
func serializedSize(_ value: Any) -> Int {
    return try! JSONSerialization.data(withJSONObject: value).count
}

let cap = 16 * 1024 * 1024
let image: [String: Any] = ["base64": String(repeating: "A", count: 8 * 1024 * 1024), "mime_type": "image/png"]
// Each component fits separately, but their combined serialized response does not.
let nodes: [[String: Any]] = (0..<16).map { ["depth": $0, "value": String(repeating: "界\\\"\n", count: 100_000)] }
let combined: [String: Any] = ["nodes": nodes, "screenshot": image, "truncated": false]
assert(serializedSize(nodes) < cap)
assert(serializedSize(image) < cap)
assert(serializedSize(combined) >= cap)
let bounded = boundedObservation(combined)
let encoded = try JSONSerialization.data(withJSONObject: bounded)
assert(encoded.count + 1 <= cap)
let decoded = try JSONSerialization.jsonObject(with: encoded) as! [String: Any]
let kept = decoded["nodes"] as! [[String: Any]]
assert(!kept.isEmpty && kept.count < nodes.count)
assert(decoded["truncated"] as? Bool == true)
assert((decoded["screenshot"] as! [String: Any])["base64"] as? String == image["base64"] as? String)
assert(kept.first?["value"] as? String == nodes.first?["value"] as? String)

let small: [String: Any] = ["nodes": [["title": "small"]], "truncated": false]
assert(NSDictionary(dictionary: boundedObservation(small)).isEqual(to: small))
let skipped: [String: Any] = ["nodes": [], "screenshot": image, "truncated": false]
assert(NSDictionary(dictionary: boundedObservation(skipped)).isEqual(to: skipped))
let hugeNode: [String: Any] = ["nodes": [["value": String(repeating: "x", count: cap)]], "truncated": false]
let trimmed = boundedObservation(hugeNode)
assert((trimmed["nodes"] as! [[String: Any]]).isEmpty)
assert(trimmed["truncated"] as? Bool == true)
let hugeImage: [String: Any] = ["nodes": [], "screenshot": ["base64": String(repeating: "/", count: cap)], "truncated": false]
let refused = boundedObservation(hugeImage)
assert(refused["screenshot"] == nil)
assert((refused["screenshot_error"] as! [String: Any])["code"] as? String == "screenshot_output_limit")
assert(serializedSize(refused) + 1 <= cap)
