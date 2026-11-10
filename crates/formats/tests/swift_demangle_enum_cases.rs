use craft_anything_formats::swift_demangle::{Status, demangle};

fn dm(symbol: &str) -> String {
    let d = demangle(symbol);
    assert_eq!(d.status, Status::Demangled, "{symbol}: {:?}", d.reason);
    d.demangled.unwrap_or_default()
}

// Symbols from macOS 15.7 system binaries, expected text from `swift-demangle`.

/// A case without a payload is a function from the enum's metatype to the enum.
#[test]
fn a_case_without_a_payload_decodes() {
    assert_eq!(dm("_$s10FindMyBase3BitO3oneyA2CmFWC"), "enum case for FindMyBase.Bit.one(FindMyBase.Bit.Type) -> FindMyBase.Bit");
    assert_eq!(dm("_$s7SwiftUI5ImageV5ScaleO5smallyA2EmFWC"), "enum case for SwiftUI.Image.Scale.small(SwiftUI.Image.Scale.Type) -> SwiftUI.Image.Scale");
}

/// A payload is a second function type after the metatype.
#[test]
fn a_case_with_a_payload_decodes() {
    assert_eq!(
        dm("_$s5TeaUI10DebugModelO5labelyACSSSgcACmFWC"),
        "enum case for TeaUI.DebugModel.label(TeaUI.DebugModel.Type) -> (Swift.String?) -> TeaUI.DebugModel"
    );
    assert_eq!(
        dm("_$s7Sharing7SFErrorO10unexpectedyACSS_SSSitcACmFWC"),
        "enum case for Sharing.SFError.unexpected(Sharing.SFError.Type) -> (Swift.String, Swift.String, Swift.Int) -> Sharing.SFError"
    );
    assert_eq!(
        dm("_$s5TeaUI10DebugModelO6actionyACyyccACmFWC"),
        "enum case for TeaUI.DebugModel.action(TeaUI.DebugModel.Type) -> (() -> ()) -> TeaUI.DebugModel"
    );
    assert_eq!(
        dm("_$s8Dispatch0A9PredicateO7onQueueyACSo17OS_dispatch_queueCcACmFWC"),
        "enum case for Dispatch.DispatchPredicate.onQueue(Dispatch.DispatchPredicate.Type) -> (__C.OS_dispatch_queue) -> Dispatch.DispatchPredicate"
    );
}

/// A generic enum prints its parameter list after the case name.
#[test]
fn a_case_of_a_generic_enum_decodes() {
    assert_eq!(
        dm("_$s8SPShared12LegacyResultO7successyACyxGxcAEmlFWC"),
        "enum case for SPShared.LegacyResult.success<A>(SPShared.LegacyResult<A>.Type) -> (A) -> SPShared.LegacyResult<A>"
    );
    assert_eq!(
        dm("_$s8SPShared12LegacyResultO7failureyACyxGs5Error_pcAEmlFWC"),
        "enum case for SPShared.LegacyResult.failure<A>(SPShared.LegacyResult<A>.Type) -> (Swift.Error) -> SPShared.LegacyResult<A>"
    );
    // Constructed with `swiftc -enable-library-evolution` and checked with `swift-demangle`.
    assert_eq!(dm("$s4main3GenO3labyACyxGx_SitcAEmlFWC"), "enum case for main.Gen.lab<A>(main.Gen<A>.Type) -> (A, Swift.Int) -> main.Gen<A>");
}

/// The enum can be nested, and its type is reached through substitutions (`A2C`, `AE`).
#[test]
fn a_case_of_a_nested_enum_decodes() {
    assert_eq!(dm("_$s8AppState0A0V4KindO5storeyA2EmFWC"), "enum case for AppState.App.Kind.store(AppState.App.Kind.Type) -> AppState.App.Kind");
    assert_eq!(dm("$s4main5OuterV5InnerO1qyAESicAEmFWC"), "enum case for main.Outer.Inner.q(main.Outer.Inner.Type) -> (Swift.Int) -> main.Outer.Inner");
}

/// `swift-demangle` prints `(extension in Dispatch):` before each name of an enum declared in an
/// extension, which this demangler leaves out by design.
#[test]
fn a_case_of_an_imported_type_extension_leaves_out_the_extension_prefix() {
    assert_eq!(
        dm("_$sSo17OS_dispatch_queueC8DispatchE20AutoreleaseFrequencyO8workItemyA2EmFWC"),
        "enum case for __C.OS_dispatch_queue.AutoreleaseFrequency.workItem(__C.OS_dispatch_queue.AutoreleaseFrequency.Type) -> __C.OS_dispatch_queue.AutoreleaseFrequency"
    );
}

/// `WC` follows the same entities as any other function, so a static one and one with argument
/// labels read as `swift-demangle` reads them.
#[test]
fn the_entity_after_wc_prints_like_any_function() {
    assert_eq!(dm("$s4main1AV1ayA2CmFZWC"), "enum case for static main.A.a(main.A.Type) -> main.A");
    assert_eq!(dm("$s4main1f1a1bySi_SitFWC"), "enum case for main.f(a: Swift.Int, b: Swift.Int) -> ()");
}

/// `swift-demangle` decodes none of these: `WC` needs a function or a type before it, not a module
/// or another global symbol.
#[test]
fn wc_after_a_module_or_a_global_is_invalid() {
    for symbol in [
        "$s4mainWC",
        "$s4main1AVMaWC",
        "$s4main1AVNWC",
        "$s4main1AVWVWC",
        "$s4main1PMpWC",
        "$s4main7PayloadO1ayACSicACmFWCWC",
        "$s4main7PayloadO1ayACSicACmFTqWC",
        "$s4main7PayloadO1ayACSicACmFTjWC",
        "$s4main7PayloadO1ayACSicACmFWc",
    ] {
        assert_eq!(demangle(symbol).status, Status::Invalid, "{symbol}");
    }
}
