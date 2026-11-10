use craft_anything_formats::swift_demangle::{Status, demangle};

fn dm(symbol: &str) -> String {
    let d = demangle(symbol);
    assert_eq!(d.status, Status::Demangled, "{symbol}: {:?}", d.reason);
    d.demangled.unwrap_or_default()
}

// Symbols from macOS 15.7 and Xcode 26 system binaries, expected text from `swift-demangle`.

/// The accessor follows `subscript` and the subscript's type follows a colon.
#[test]
fn accessors_follow_subscript_and_the_type_follows_a_colon() {
    assert_eq!(dm("_$s10Foundation4DataV06InlineB0Vys5UInt8VSiciM"), "Foundation.Data.InlineData.subscript.modify : (Swift.Int) -> Swift.UInt8");
    assert_eq!(dm("_$s10Foundation4DataV06InlineB0Vys5UInt8VSicis"), "Foundation.Data.InlineData.subscript.setter : (Swift.Int) -> Swift.UInt8");
    assert_eq!(dm("_$sSayxSicig"), "Swift.Array.subscript.getter : (Swift.Int) -> A");
    assert_eq!(dm("_$sSayxSicir"), "Swift.Array.subscript.read : (Swift.Int) -> A");
    assert_eq!(dm("_$s9RealityIO9AttributeC2atxSd_tcig"), "RealityIO.Attribute.subscript.getter : (at: Swift.Double) -> A");
}

/// Addressors are accessors too.
#[test]
fn addressors_follow_subscript() {
    assert_eq!(dm("_$sSPyxSicilu"), "Swift.UnsafePointer.subscript.unsafeAddressor : (Swift.Int) -> A");
    assert_eq!(dm("_$sSpyxSiciau"), "Swift.UnsafeMutablePointer.subscript.unsafeMutableAddressor : (Swift.Int) -> A");
}

/// Inside thunks, descriptors, witnesses and after an unmangled suffix the subscript reads the same.
#[test]
fn accessors_inside_other_globals_follow_subscript() {
    assert_eq!(dm("_$sSly7ElementQz5IndexQzcirTq"), "method descriptor for Swift.Collection.subscript.read : (A.Index) -> A.Element");
    assert_eq!(
        dm("_$s6Gluten12FeatureFlagsCySbAA07CurrentbC0OcisTj"),
        "dispatch thunk of Gluten.FeatureFlags.subscript.setter : (Gluten.CurrentFeatureFlags) -> Swift.Bool"
    );
    assert_eq!(
        dm("_$sSf12SIMD8StorageVs11SIMDStoragessACPy6ScalarQzSicisTW"),
        "protocol witness for Swift.SIMDStorage.subscript.setter : (Swift.Int) -> A.Scalar in conformance Swift.Float.SIMD8Storage : Swift.SIMDStorage in Swift"
    );
    assert_eq!(dm("_$sSayxSicir.resume.0"), "Swift.Array.subscript.read : (Swift.Int) -> A with unmangled suffix \".resume.0\"");
    assert_eq!(dm("_$sSDyq_SgxciM.resume.0"), "Swift.Dictionary.subscript.modify : (A) -> B? with unmangled suffix \".resume.0\"");
}

/// A static subscript keeps `static` in front. Constructed with `swiftc -enable-library-evolution`
/// and checked with `swift-demangle`.
#[test]
fn static_subscript_accessors_follow_subscript() {
    assert_eq!(dm("_$s1x1SVySiSScigZ"), "static x.S.subscript.getter : (Swift.String) -> Swift.Int");
    assert_eq!(dm("_$s1x1SVySiSSciMZ.resume.0"), "static x.S.subscript.modify : (Swift.String) -> Swift.Int with unmangled suffix \".resume.0\"");
}

/// In an extension: `swift-demangle` prints `(extension in …):` before the name, which this
/// demangler leaves out by design.
#[test]
fn accessor_in_an_extension_leaves_out_the_extension_prefix() {
    assert_eq!(dm("_$sSo13simd_float2x3a0A0Eys5SIMD3VySfGSiciM"), "__C.simd_float2x3.subscript.modify : (Swift.Int) -> Swift.SIMD3<Swift.Float>");
}

/// A property descriptor names the subscript without an accessor; its type still follows directly.
#[test]
fn subscript_without_an_accessor_keeps_the_type_after_subscript() {
    assert_eq!(dm("_$sSR10_uncheckedxSi_tcipMV"), "property descriptor for Swift.UnsafeBufferPointer.subscript(_unchecked: Swift.Int) -> A");
    // Constructed with `swiftc -enable-library-evolution` and checked with `swift-demangle`.
    assert_eq!(dm("_$s1x1SVySiSScipZMV"), "property descriptor for static x.S.subscript(Swift.String) -> Swift.Int");
}

/// `swift-demangle` decodes none of these: an unknown letter after the accessor, a cut-short
/// operator after it, and a subscript with no type.
#[test]
fn broken_subscript_accessors_are_invalid() {
    for symbol in ["_$s1x1SVyS2iciX", "_$s1x1SVyS2icigq", "_$s1x1SVcig"] {
        assert_eq!(demangle(symbol).status, Status::Invalid, "{symbol}");
    }
}
