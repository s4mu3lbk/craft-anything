use craft_anything_formats::swift_demangle::{Status, demangle};

fn dm(symbol: &str) -> String {
    let d = demangle(symbol);
    assert_eq!(d.status, Status::Demangled, "{symbol}: {:?}", d.reason);
    d.demangled.unwrap_or_default()
}

// Symbols from macOS 15.7 and Xcode 26 system binaries, expected text from `swift-demangle`.

/// `Ml` names the cache the runtime fills once when it lays out a type's metadata.
#[test]
fn ml_reads_as_the_singleton_initialization_cache() {
    assert_eq!(dm("_$ss20__BridgingHashBufferCMl"), "type metadata singleton initialization cache for Swift.__BridgingHashBuffer");
    assert_eq!(dm("_$s10RealityKit21BodyTrackingComponentVMl"), "type metadata singleton initialization cache for RealityKit.BodyTrackingComponent");
    assert_eq!(dm("_$s10RealityKit17__REAnchoringTypeOMl"), "type metadata singleton initialization cache for RealityKit.__REAnchoringType");
}

/// The type before `Ml` can be nested in another type.
#[test]
fn ml_after_a_nested_type_reads_as_the_singleton_initialization_cache() {
    assert_eq!(dm("_$s10RealityKit15TextureResourceC9LoadErrorOMl"), "type metadata singleton initialization cache for RealityKit.TextureResource.LoadError");
    assert_eq!(dm("_$s10RealityKit18AnchoringComponentV6TargetOMl"), "type metadata singleton initialization cache for RealityKit.AnchoringComponent.Target");
    // Constructed with `swiftc -enable-library-evolution` and checked with `swift-demangle`.
    assert_eq!(dm("_$s4main5OuterV5InnerVMl"), "type metadata singleton initialization cache for main.Outer.Inner");
}

/// Types declared in an extension: `swift-demangle` prints `(extension in …):` before the name,
/// which this demangler leaves out by design.
#[test]
fn ml_after_a_type_in_an_extension_leaves_out_the_extension_prefix() {
    assert_eq!(
        dm("_$s10RealityKit18AnchoringComponentV0A10FoundationE05ImageC6SourceV0G0OMl"),
        "type metadata singleton initialization cache for RealityKit.AnchoringComponent.ImageAnchoringSource.Source"
    );
    // Constructed with `swiftc -enable-library-evolution` and checked with `swift-demangle`.
    assert_eq!(dm("_$sSi4mainE3ExtVMl"), "type metadata singleton initialization cache for Swift.Int.Ext");
}

/// `ML` keeps its own name: the lazy cache variable for a type's metadata.
#[test]
fn capital_ml_still_reads_as_the_lazy_cache_variable() {
    assert_eq!(dm("_$s10Foundation3URLVSgML"), "lazy cache variable for type metadata for Foundation.URL?");
    assert_eq!(dm("_$s10Foundation3URLV_ACtSgML"), "lazy cache variable for type metadata for (Foundation.URL, Foundation.URL)?");
}

/// `swift-demangle` decodes none of these: nothing before `Ml`, or a name cut short after it.
#[test]
fn ml_without_a_whole_type_is_invalid() {
    for symbol in ["$sMl", "$s4main1AVMl1", "$s4main1AVM"] {
        assert_eq!(demangle(symbol).status, Status::Invalid, "{symbol}");
    }
}
