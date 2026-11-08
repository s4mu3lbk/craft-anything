// Craft Anything accessibility helper.
//
// Reads a JSON request on stdin and writes a JSON result on stdout. It walks the macOS
// accessibility (AX) tree of a running application and, for walkthroughs, performs AX actions
// between reads. It is our own clean-room code; it names no third-party product.
//
// Request shapes:
//   {"command":"observe","pid":1234,"window_id":null,"screenshot":false,"accessibility":true,"max_nodes":5000}
//   {"command":"walkthrough","pid":1234,"steps":[{"action":"AXPress","element":"Save"}],"max_nodes":5000}
//
// Compiled lazily on the host with `xcrun swiftc`; never shipped as a binary.

import Foundation
import ApplicationServices
import ScreenCaptureKit
import ImageIO
import AppKit

func screenshotFailure(_ code: String, _ message: String) -> [String: Any] {
    return ["screenshot_error": ["code": code, "message": message]]
}

// Never capture a display or fall back to a different process's window. Enumeration needs
// existing Screen Recording permission; preflight avoids opening a permission dialog.
@MainActor
func captureScreenshot(pid: Int32, windowID: UInt64?) async -> [String: Any] {
    guard #available(macOS 14.0, *) else {
        return screenshotFailure("screenshot_unsupported", "Screenshots need macOS 14 or later.")
    }
    // A command-line helper has no AppKit application yet. ScreenCaptureKit needs its
    // WindowServer connection initialized, or CoreGraphics can abort in CGS_REQUIRE_INIT.
    _ = NSApplication.shared
    guard CGPreflightScreenCaptureAccess() else {
        return screenshotFailure("screen_recording_denied", "Screenshot unavailable: grant Screen Recording permission to the invoking app in System Settings, then retry.")
    }
    do {
        let content = try await SCShareableContent.excludingDesktopWindows(true, onScreenWindowsOnly: true)
        let candidates = content.windows.filter {
            $0.owningApplication?.processID == pid && $0.windowLayer == 0 &&
                $0.frame.width > 0 && $0.frame.height > 0 &&
                (windowID == nil || UInt64($0.windowID) == windowID)
        }
        guard let window = candidates.first else {
            return screenshotFailure("screenshot_window_not_found", "No capturable on-screen window matches this pid and window_id. Open the target window and retry.")
        }
        let width = window.frame.width
        let height = window.frame.height
        guard width.isFinite && height.isFinite else {
            return screenshotFailure("screenshot_invalid_size", "The target window has invalid dimensions.")
        }
        // Bound pixels before capture and encoded bytes before base64/JSON allocation.
        let scale = min(1.0, 2048.0 / max(width, height))
        let config = SCStreamConfiguration()
        config.width = max(1, Int((width * scale).rounded(.down)))
        config.height = max(1, Int((height * scale).rounded(.down)))
        config.showsCursor = false
        let image = try await SCScreenshotManager.captureImage(
            contentFilter: SCContentFilter(desktopIndependentWindow: window), configuration: config)
        let png = NSMutableData()
        guard let destination = CGImageDestinationCreateWithData(png, "public.png" as CFString, 1, nil) else {
            return screenshotFailure("screenshot_encoding_failed", "Could not create a PNG encoder for the screenshot.")
        }
        CGImageDestinationAddImage(destination, image, nil)
        guard CGImageDestinationFinalize(destination), png.length <= 6 * 1024 * 1024 else {
            return screenshotFailure("screenshot_encoding_failed", "Screenshot PNG encoding failed or exceeded the 6 MiB limit.")
        }
        return ["screenshot": ["mime_type": "image/png", "base64": (png as Data).base64EncodedString(),
            "window_id": window.windowID, "width": image.width, "height": image.height,
            "scaled": scale < 1.0]]
    } catch {
        return screenshotFailure("screenshot_capture_failed", "The window screenshot could not be captured. Check Screen Recording permission and that the target window is still open, then retry.")
    }
}

struct Request: Decodable {
    var command: String
    var pid: Int32?
    var window_id: UInt64?
    var screenshot: Bool?
    var accessibility: Bool?
    var max_nodes: Int?
    var steps: [Step]?
}

struct Step: Decodable {
    var action: String
    var element: String
}

func attr(_ element: AXUIElement, _ name: String) -> String? {
    var value: CFTypeRef?
    if AXUIElementCopyAttributeValue(element, name as CFString, &value) == .success,
       let v = value {
        return (v as? String) ?? String(describing: v)
    }
    return nil
}

func children(_ element: AXUIElement) -> [AXUIElement] {
    var value: CFTypeRef?
    if AXUIElementCopyAttributeValue(element, kAXChildrenAttribute as CFString, &value) == .success,
       let arr = value as? [AXUIElement] {
        return arr
    }
    return []
}

func walk(_ element: AXUIElement, depth: Int, budget: inout Int, out: inout [[String: Any]]) {
    if budget <= 0 { return }
    budget -= 1
    var node: [String: Any] = ["depth": depth]
    if let role = attr(element, kAXRoleAttribute as String) { node["role"] = role }
    if let title = attr(element, kAXTitleAttribute as String) { node["title"] = title }
    if let value = attr(element, kAXValueAttribute as String) { node["value"] = value }
    if let ident = attr(element, "AXIdentifier") { node["identifier"] = ident }
    out.append(node)
    for child in children(element) {
        walk(child, depth: depth + 1, budget: &budget, out: &out)
    }
}

func snapshot(pid: Int32, maxNodes: Int) -> [[String: Any]] {
    let app = AXUIElementCreateApplication(pid)
    var out: [[String: Any]] = []
    var budget = maxNodes
    walk(app, depth: 0, budget: &budget, out: &out)
    return out
}

func findElement(_ root: AXUIElement, _ needle: String, _ budget: inout Int) -> AXUIElement? {
    if budget <= 0 { return nil }
    budget -= 1
    if attr(root, kAXTitleAttribute as String) == needle || attr(root, "AXIdentifier") == needle {
        return root
    }
    for child in children(root) {
        if let found = findElement(child, needle, &budget) { return found }
    }
    return nil
}

// Match the runner's stdout cap, including the newline added by emit. Measure JSON bytes:
// base64 slashes and AX strings can expand when escaped by JSONSerialization.
func boundedObservation(_ observation: [String: Any]) -> [String: Any] {
    let stdoutCap = 16 * 1024 * 1024
    var result = observation
    var nodes = result["nodes"] as? [[String: Any]] ?? []
    while true {
        if let data = try? JSONSerialization.data(withJSONObject: result, options: []),
           data.count < stdoutCap {
            return result
        }
        if !nodes.isEmpty {
            // Keep a prefix of the tree and the image. Halving bounds serialization attempts
            // even when one AX value alone is larger than the entire output budget.
            nodes = Array(nodes.prefix(nodes.count / 2))
            result["nodes"] = nodes
            result["truncated"] = true
        } else if result.removeValue(forKey: "screenshot") != nil {
            result.merge(screenshotFailure("screenshot_output_limit",
                "The screenshot exceeded the helper output budget.")) { _, new in new }
        } else {
            // Serialization failure must also produce a small, valid response.
            return ["error": "Could not serialize the observation within the output budget."]
        }
    }
}

func emit(_ obj: [String: Any]) {
    if let data = try? JSONSerialization.data(withJSONObject: obj, options: []),
       let text = String(data: data, encoding: .utf8) {
        print(text)
    }
}

let input = FileHandle.standardInput.readDataToEndOfFile()
guard let req = try? JSONDecoder().decode(Request.self, from: input), let pid = req.pid else {
    emit(["error": "invalid request"])
    exit(2)
}

// Observation may explicitly omit the tree. Walkthroughs still need AX permission because
// they read and drive controls, regardless of an observe-only flag in the request.
let readAccessibility = req.command != "observe" || (req.accessibility ?? true)
if readAccessibility && !AXIsProcessTrusted() {
    // A typed reason so the caller can report a permission problem, not an empty tree.
    emit(["error": "accessibility permission not granted", "reason": "accessibility_not_granted", "nodes": []])
    exit(0)
}

switch req.command {
case "observe":
    let nodes = readAccessibility ? snapshot(pid: pid, maxNodes: req.max_nodes ?? 5000) : []
    var result: [String: Any] = ["nodes": nodes, "truncated": readAccessibility && nodes.count >= (req.max_nodes ?? 5000)]
    if req.screenshot ?? false {
        result.merge(await captureScreenshot(pid: pid, windowID: req.window_id)) { _, new in new }
    }
    emit(boundedObservation(result))
case "walkthrough":
    let app = AXUIElementCreateApplication(pid)
    var steps: [[String: Any]] = []
    let before = snapshot(pid: pid, maxNodes: req.max_nodes ?? 5000)
    for step in req.steps ?? [] {
        var budget = req.max_nodes ?? 5000
        var outcome = "element_not_found"
        if let target = findElement(app, step.element, &budget) {
            let result = AXUIElementPerformAction(target, step.action as CFString)
            outcome = result == .success ? "dispatched" : "failed"
        }
        steps.append(["action": step.action, "element": step.element, "outcome": outcome])
    }
    let after = snapshot(pid: pid, maxNodes: req.max_nodes ?? 5000)
    emit(["steps": steps, "nodes_before": before, "nodes_after": after])
default:
    emit(["error": "unknown command"])
    exit(2)
}
