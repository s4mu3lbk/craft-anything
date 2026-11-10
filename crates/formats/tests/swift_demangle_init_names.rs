use craft_anything_formats::swift_demangle::{Status, demangle};

fn dm(symbol: &str) -> String {
    let d = demangle(symbol);
    assert_eq!(d.status, Status::Demangled, "{symbol}: {:?}", d.reason);
    d.demangled.unwrap_or_default()
}

/// An allocating initialiser (`fC`) of a struct, an enum or a protocol reads `init`. Symbols from
/// macOS 15.7 system binaries, expected text from `swift-demangle`.
#[test]
fn struct_enum_and_protocol_initialisers_read_init() {
    assert_eq!(dm("_$sSdyS2dcfC"), "Swift.Double.init(Swift.Double) -> Swift.Double");
    assert_eq!(dm("_$sS2ayxGycfC"), "Swift.Array.init() -> [A]");
    assert_eq!(dm("_$s9CryptoKit8InsecureO3MD5VAEycfC"), "CryptoKit.Insecure.MD5.init() -> CryptoKit.Insecure.MD5");
    assert_eq!(dm("_$s9SportsKit5SportO8rawValueACSS_tcfC"), "SportsKit.Sport.init(rawValue: Swift.String) -> SportsKit.Sport");
    assert_eq!(dm("_$s7SwiftUI3AppPxycfCTq"), "method descriptor for SwiftUI.App.init() -> A");
}

/// In an extension, even of a class, it reads `init` too. `swift-demangle` also prints
/// `(extension in NewsTag):`, which this demangler leaves out.
#[test]
fn initialisers_in_class_extensions_read_init() {
    assert_eq!(dm("_$sSo7UIColorC7NewsTagE3hexABSi_tcfC"), "__C.UIColor.init(hex: Swift.Int) -> __C.UIColor");
}

/// A class's own allocating initialiser keeps its name: plain, generic, nested in an enum, and an
/// Objective-C class (constructed, checked with `swift-demangle`).
#[test]
fn class_initialisers_keep_the_allocating_name() {
    assert_eq!(dm("_$s8ShelfKit0A0CACycfC"), "ShelfKit.Shelf.__allocating_init() -> ShelfKit.Shelf");
    assert_eq!(dm("_$s9JetEngine7PromiseCACyxGycfC"), "JetEngine.Promise.__allocating_init() -> JetEngine.Promise<A>");
    assert_eq!(dm("_$s5Books7InfoBarO12MessageQueueCAEycfC"), "Books.InfoBar.MessageQueue.__allocating_init() -> Books.InfoBar.MessageQueue");
    assert_eq!(dm("$sSo8NSObjectCABycfC"), "__C.NSObject.__allocating_init() -> __C.NSObject");
}
