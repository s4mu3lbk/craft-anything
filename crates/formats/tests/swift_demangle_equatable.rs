use craft_anything_formats::swift_demangle::{Status, demangle};

fn dm(symbol: &str) -> String {
    let d = demangle(symbol);
    assert_eq!(d.status, Status::Demangled, "{symbol}: {:?}", d.reason);
    d.demangled.unwrap_or_default()
}

/// `SQ` is the standard protocol `Swift.Equatable`. Symbols from macOS 15.7 system binaries,
/// expected text from `swift-demangle`.
#[test]
fn sq_names_the_equatable_protocol() {
    assert_eq!(dm("_$sSQMp"), "protocol descriptor for Swift.Equatable");
    // `swift-demangle` also prints `(extension in PodcastsFoundation):`, which this demangler leaves out.
    assert_eq!(
        dm("_$sSo14NSUserDefaultsC18PodcastsFoundationE14queryPublishery7Combine03AnyF0Vyxs5NeverOGxycSQRzlF"),
        "__C.NSUserDefaults.queryPublisher<A where A: Swift.Equatable>(() -> A) -> Combine.AnyPublisher<A, Swift.Never>"
    );
}

/// Conformances to `Equatable` need `SQ` to be a protocol; they were reported invalid.
#[test]
fn equatable_conformances_decode() {
    assert_eq!(dm("_$s10Foundation4UUIDVSQAAMc"), "protocol conformance descriptor for Foundation.UUID : Swift.Equatable in Foundation");
    assert_eq!(dm("_$sSnyxGSQsMc"), "protocol conformance descriptor for Swift.Range<A> : Swift.Equatable in Swift");
    assert_eq!(dm("_$sSo6CGSizeVSQ12CoreGraphicsMc"), "protocol conformance descriptor for __C.CGSize : Swift.Equatable in CoreGraphics");
    assert_eq!(dm("_$sSiSQsWP"), "protocol witness table for Swift.Int : Swift.Equatable in Swift");
}

/// `swift-demangle` reads `SQ` as `Equatable` in the `$S` and `_T0` manglings too. A spelled-out
/// `ImplicitlyUnwrappedOptional` keeps its `!` form.
#[test]
fn other_manglings_read_sq_the_same_way() {
    assert_eq!(dm("_T0SQMp"), "protocol descriptor for Swift.Equatable");
    assert_eq!(dm("$SSiSQsMc"), "protocol conformance descriptor for Swift.Int : Swift.Equatable in Swift");
    assert_eq!(dm("$S4main3fooyyxSQRzlF"), "main.foo<A where A: Swift.Equatable>(A) -> ()");
    assert_eq!(dm("$s4main1xs27ImplicitlyUnwrappedOptionalOySiGvp"), "main.x : Swift.Int!");
}
