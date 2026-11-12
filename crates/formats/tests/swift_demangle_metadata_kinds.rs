use craft_anything_formats::swift_demangle::{Status, demangle};

fn dm(symbol: &str) -> String {
    let d = demangle(symbol);
    assert_eq!(d.status, Status::Demangled, "{symbol}: {:?}", d.reason);
    d.demangled.unwrap_or_default()
}

fn invalid(symbol: &str) {
    let d = demangle(symbol);
    assert_eq!(d.status, Status::Invalid, "{symbol} should be invalid, got {:?}", d.demangled);
}

// Symbols from macOS 15.7 and Xcode 26.0 beta system binaries, expected text from `swift-demangle`.
// Symbols starting `$s1x` or `$s4main` were built with `swiftc` or written by hand and checked with
// `swift-demangle`.

/// `MK` follows a protocol conformance descriptor and prints it after a short prefix.
#[test]
fn a_metadata_cache_of_a_conformance_descriptor_decodes() {
    assert_eq!(dm("_$s7Testing3TagVSeAAMcMK"), "metadata instantiation cache for protocol conformance descriptor for Testing.Tag : Swift.Decodable in Testing");
    assert_eq!(
        dm("_$sSo10SGDataTypeVSQSCMcMK"),
        "metadata instantiation cache for protocol conformance descriptor for __C.SGDataType : Swift.Equatable in __C_Synthesized"
    );
    assert_eq!(dm("$s1x6SquareVSHAAMcMK"), "metadata instantiation cache for protocol conformance descriptor for x.Square : Swift.Hashable in x");
    // The operand can be any other symbol that is not a marker: another global, or a module.
    assert_eq!(dm("$s4main1SVMnMK"), "metadata instantiation cache for nominal type descriptor for main.S");
    assert_eq!(dm("$s4mainMK"), "metadata instantiation cache for main");
}

/// `MD` is the cache a type's metadata is demangled into; the type is a whole type.
#[test]
fn a_demangling_cache_variable_decodes() {
    assert_eq!(dm("_$sSS_SitMD"), "demangling cache variable for type metadata for (Swift.String, Swift.Int)");
    assert_eq!(dm("_$sShySiGMD"), "demangling cache variable for type metadata for Swift.Set<Swift.Int>");
    assert_eq!(dm("_$sSi3loc_SS4texttMD"), "demangling cache variable for type metadata for (loc: Swift.Int, text: Swift.String)");
    assert_eq!(dm("_$sSo8NSScreenCSgMD"), "demangling cache variable for type metadata for __C.NSScreen?");
    assert_eq!(dm("_$sSo5NSURLCmMD"), "demangling cache variable for type metadata for __C.NSURL.Type");
    assert_eq!(dm("$s1x6HolderCyAA6SquareVGMD"), "demangling cache variable for type metadata for x.Holder<x.Square>");
    assert_eq!(dm("$s1x4KindO_AA3BoxVySdGtMD"), "demangling cache variable for type metadata for (x.Kind, x.Box<Swift.Double>)");
}

/// `MF` and `MB` name a type with no word such as `for` in front of it.
#[test]
fn field_and_builtin_descriptors_decode() {
    assert_eq!(dm("_$ss5SIMD4VMF"), "reflection metadata field descriptor Swift.SIMD4");
    assert_eq!(dm("_$sSj_pMF"), "reflection metadata field descriptor Swift.Numeric");
    assert_eq!(dm("_$ss8_PtrAuthO3KeyVMF"), "reflection metadata field descriptor Swift._PtrAuth.Key");
    assert_eq!(dm("$s1x5Shape_pMF"), "reflection metadata field descriptor x.Shape");
    assert_eq!(dm("_$sBBMB"), "reflection metadata builtin descriptor Builtin.UnsafeValueBuffer");
    assert_eq!(dm("_$sSo6CGSizeVMB"), "reflection metadata builtin descriptor __C.CGSize");
    assert_eq!(dm("_$s12DebuggerLLDB20SessionProgressEventC5ErrorOMB"), "reflection metadata builtin descriptor DebuggerLLDB.SessionProgressEvent.Error");
}

/// `MP` is the pattern the runtime builds a generic type's metadata from.
#[test]
fn a_generic_metadata_pattern_decodes() {
    assert_eq!(dm("_$ss13AnyCollectionVMP"), "generic type metadata pattern for Swift.AnyCollection");
    assert_eq!(dm("_$ss12Zip2SequenceV8IteratorVMP"), "generic type metadata pattern for Swift.Zip2Sequence.Iterator");
    assert_eq!(
        dm("_$s6XCTest3Box33_8A3DF7466C50553FA6980E49458734D6LLCMP"),
        "generic type metadata pattern for XCTest.(Box in _8A3DF7466C50553FA6980E49458734D6)"
    );
}

/// `MA` names a type, a protocol and a module, as a conformance descriptor does.
#[test]
fn an_associated_type_descriptor_decodes() {
    assert_eq!(dm("_$sSSSKsMA"), "reflection metadata associated type descriptor Swift.String : Swift.BidirectionalCollection in Swift");
    assert_eq!(dm("_$sSDyxq_GSlsMA"), "reflection metadata associated type descriptor [A : B] : Swift.Collection in Swift");
    assert_eq!(dm("_$s3AST9TypeArrayVSlAAMA"), "reflection metadata associated type descriptor AST.TypeArray : Swift.Collection in AST");
    assert_eq!(dm("_$sSo10NLLanguageaSYSCMA"), "reflection metadata associated type descriptor __C.NLLanguage : Swift.RawRepresentable in __C_Synthesized");
    assert_eq!(dm("$s1x3BoxVyxGAA5ShapeAAMA"), "reflection metadata associated type descriptor x.Box<A> : x.Shape in x");
}

/// `MU`, `Ms` and `Mt` are the Objective-C interop pieces of a Swift class.
#[test]
fn objc_class_descriptors_decode() {
    assert_eq!(dm("_$s6IDEKit12ActivityViewCMU"), "ObjC metadata update function for IDEKit.ActivityView");
    assert_eq!(dm("_$s8DeltaKit22DiscardChangesMenuItemCMs"), "ObjC resilient class stub for DeltaKit.DiscardChangesMenuItem");
    assert_eq!(dm("_$s8DeltaKit22DiscardChangesMenuItemCMt"), "full ObjC resilient class stub for DeltaKit.DiscardChangesMenuItem");
    assert_eq!(
        dm("_$s14InstrumentsKit17ActionPopoverItem33_34B2968A1CE000BE7DAA892A48C2D125LLCMt"),
        "full ObjC resilient class stub for InstrumentsKit.(ActionPopoverItem in _34B2968A1CE000BE7DAA892A48C2D125)"
    );
}

/// `swift-demangle` prints `(extension in Foundation):` before the name of a type declared in an
/// extension, which this demangler leaves out by design.
#[test]
fn a_type_declared_in_an_extension_leaves_out_the_extension_prefix() {
    assert_eq!(dm("_$s10Foundation4DataV13MachObjectLibE13ScanningErrorOMB"), "reflection metadata builtin descriptor Foundation.Data.ScanningError");
    assert_eq!(
        dm("_$s10Foundation17AsyncLineSequenceVySo12NSURLSessionCAAE0B5BytesVGMD"),
        "demangling cache variable for type metadata for Foundation.AsyncLineSequence<__C.NSURLSession.AsyncBytes>"
    );
    assert_eq!(
        dm("_$s10Foundation15AttributeScopesO14XCUIAutomationE28SyntaxHighlightingAttributesVMF"),
        "reflection metadata field descriptor Foundation.AttributeScopes.SyntaxHighlightingAttributes"
    );
}

/// A kind that describes one type refuses a module, a function, a conformance, another global or
/// a bare name, as `swift-demangle` does.
#[test]
fn a_type_kind_after_something_that_is_not_a_type_is_invalid() {
    for kind in ["MD", "MF", "MP", "MB", "MU", "Mt", "Ms"] {
        invalid(&format!("$s4main{kind}"));
        invalid(&format!("$s4main1fyyF{kind}"));
        invalid(&format!("$s4main1SVMa{kind}"));
        invalid(&format!("$s4main1SVMn{kind}"));
        invalid(&format!("$s4main1SVSQAAMc{kind}"));
        invalid(&format!("$s4main1SV4mainE{kind}"));
        invalid(&format!("$s4main1SV1x{kind}"));
        invalid(&format!("$s{kind}"));
    }
}

/// `MA` needs a type, a protocol and a module, in that order; a missing part, a function or another
/// global as the type, or something that is not a module in the last place is refused.
#[test]
fn an_associated_type_descriptor_needs_a_type_a_protocol_and_a_module() {
    invalid("$s4main1SVMA");
    invalid("$s4main1SVSQMA");
    invalid("$s4mainSQAAMA");
    invalid("$s4main1fyyFSQAAMA");
    invalid("$s4main1SVMaSQAAMA");
    invalid("$s4main1SVSQAAMcMA");
    invalid("$sSiSgSQAAMA");
    invalid("$sMA");
}

/// `MK` takes one thing; with nothing before it, or only a marker, there is nothing to cache.
#[test]
fn a_metadata_cache_needs_something_to_cache() {
    invalid("$sMK");
    invalid("$s4main1SVMKMD");
}
