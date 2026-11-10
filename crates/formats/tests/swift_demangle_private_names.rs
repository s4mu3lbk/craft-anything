use craft_anything_formats::swift_demangle::{Status, demangle};

fn dm(symbol: &str) -> String {
    let d = demangle(symbol);
    assert_eq!(d.status, Status::Demangled, "{symbol}: {:?}", d.reason);
    d.demangled.unwrap_or_default()
}

// Symbols from macOS 15.7 and Xcode 26 system binaries, expected text from `swift-demangle`.
// A private name is the declaration's name, then an identifier for its file, then `LL`.

/// Private types, nested or not, and a private protocol.
#[test]
fn a_private_type_prints_its_name_in_its_file() {
    assert_eq!(
        dm("_$s18PackageCollections0aB5ModelO16CollectionSourceV10CodingKeys33_BFE299151AE66B36D103F9A71B02F370LLOMa"),
        "type metadata accessor for PackageCollections.PackageCollectionsModel.CollectionSource.(CodingKeys in _BFE299151AE66B36D103F9A71B02F370)"
    );
    assert_eq!(
        dm("_$s10DVTIconKit19_SingleValueCodable33_D75A75EE41CBEED7A80B34B31B4936EFLLMp"),
        "protocol descriptor for DVTIconKit.(_SingleValueCodable in _D75A75EE41CBEED7A80B34B31B4936EF)"
    );
    // The result type is the private type again, reached through a substitution (`AF`).
    assert_eq!(
        dm("_$s16PreviewsPipeline14PreviewServiceC11ActiveState33_619E025B5E88AE1CFB03291418406E00LLVAFycfC"),
        "PreviewsPipeline.PreviewService.(ActiveState in _619E025B5E88AE1CFB03291418406E00).init() -> PreviewsPipeline.PreviewService.(ActiveState in _619E025B5E88AE1CFB03291418406E00)"
    );
}

/// Private functions and variables, at the top level and inside a type.
#[test]
fn private_functions_and_variables_print_their_name_in_their_file() {
    assert_eq!(
        dm("_$s013CompilerSwiftA21PluginMessageHandling11JSONScanner33_E54560CB6A910FE1C77A49A6E580BBCDLLV8scanNullyyKF"),
        "CompilerSwiftCompilerPluginMessageHandling.(JSONScanner in _E54560CB6A910FE1C77A49A6E580BBCD).scanNull() throws -> ()"
    );
    assert_eq!(
        dm("_$s10DVTIconKit12_isDebugging33_8F51EEEBC836B21F9ED44F4C6D38A8CBLLSbvp"),
        "DVTIconKit.(_isDebugging in _8F51EEEBC836B21F9ED44F4C6D38A8CB) : Swift.Bool"
    );
    assert_eq!(
        dm("_$s11DVTProducts11RateLimiterC08overrideB5Limit33_8C61E6FBB8FD7FB3ED3A9E738BF6A822LLSbvg"),
        "DVTProducts.RateLimiter.(overrideRateLimit in _8C61E6FBB8FD7FB3ED3A9E738BF6A822).getter : Swift.Bool"
    );
    // Constructed with `swiftc -enable-library-evolution` and checked with `swift-demangle`.
    assert_eq!(dm("$s4main7counter029_12232F587A4C5CD8B1EEDF696793D2FCLLSivp"), "main.(counter in _12232F587A4C5CD8B1EEDF696793A4FC) : Swift.Int");
    assert_eq!(dm("$s4main3Box029_12232F587A4C5CD8B1EEDF696793D2FCLLV5valuexvg"), "main.(Box in _12232F587A4C5CD8B1EEDF696793A4FC).value.getter : A");
}

/// A private type in a conformance and in the protocol witness that implements it.
#[test]
fn a_private_type_in_a_conformance_prints_its_name_in_its_file() {
    assert_eq!(
        dm("_$s18PackageCollections0aB5ModelO16CollectionSourceV10CodingKeys33_BFE299151AE66B36D103F9A71B02F370LLOSHAASH13_rawHashValue4seedS2i_tFTW"),
        "protocol witness for Swift.Hashable._rawHashValue(seed: Swift.Int) -> Swift.Int in conformance PackageCollections.PackageCollectionsModel.CollectionSource.(CodingKeys in _BFE299151AE66B36D103F9A71B02F370) : Swift.Hashable in PackageCollections"
    );
    assert_eq!(
        dm("_$s10Foundation27_JSONKeyedDecodingContainer33_12768CA107A31EF2DCE034FD75B541C9LLVyxGs05KeyedcD8ProtocolAAMc"),
        "protocol conformance descriptor for Foundation.(_JSONKeyedDecodingContainer in _12768CA107A31EF2DCE034FD75B541C9)<A> : Swift.KeyedDecodingContainerProtocol in Foundation"
    );
}

/// A private type as a generic argument, an array element and an existential.
#[test]
fn a_private_type_as_a_generic_argument_prints_its_name_in_its_file() {
    assert_eq!(
        dm("_$s4X50918OCSPVerifierPolicyVy14PackageSigning14_OCSPRequester33_00ED8E7A93EE1F8B929EBDEA940959EDLLVGMa"),
        "type metadata accessor for X509.OCSPVerifierPolicy<PackageSigning.(_OCSPRequester in _00ED8E7A93EE1F8B929EBDEA940959ED)>"
    );
    assert_eq!(
        dm("_$s7SWBUtil11LockedValueCySayAA7WeakRefVyAA15_HeavyCacheBase33_FD19BBC2EEF67A88200D5046212D0611LL_pGGGMa"),
        "type metadata accessor for SWBUtil.LockedValue<[SWBUtil.WeakRef<SWBUtil.(_HeavyCacheBase in _FD19BBC2EEF67A88200D5046212D0611)>]>"
    );
}

/// The file identifier can use word substitutions, and later private names reuse it through a
/// substitution (`ACLL`).
#[test]
fn the_file_identifier_can_be_shortened_and_reused() {
    assert_eq!(
        dm("_$s19SymbolCacheIndexing8APINotesV12ProtocolNoteV10CodingKeys031_E240ADFCB339E2DB1824183592F1B6M0LLOMa"),
        "type metadata accessor for SymbolCacheIndexing.APINotes.ProtocolNote.(CodingKeys in _E240ADFCB339E2DB1824183592F1B6F1)"
    );
    // Constructed with `swiftc -enable-library-evolution` and checked with `swift-demangle`.
    assert_eq!(
        dm("$s4main8freeFunc029_12232F587A4C5CD8B1EEDF696793E2FCLLyAA4ModeACLLOSgAA3BoxACLLVyAEGF"),
        "main.(freeFunc in _12232F587A4C5CD8B1EEDF696793A4FC)(main.(Box in _12232F587A4C5CD8B1EEDF696793A4FC)<main.(Mode in _12232F587A4C5CD8B1EEDF696793A4FC)>) -> main.(Mode in _12232F587A4C5CD8B1EEDF696793A4FC)?"
    );
    assert_eq!(
        dm("$s4main10takesShape029_12232F587A4C5CD8B1EEDF696793E2FCLLySayAA4NodeACLLCGAA0C0ACLL_pF"),
        "main.(takesShape in _12232F587A4C5CD8B1EEDF696793A4FC)(main.(Shape in _12232F587A4C5CD8B1EEDF696793A4FC)) -> [main.(Node in _12232F587A4C5CD8B1EEDF696793A4FC)]"
    );
}

/// A private method in an extension: `swift-demangle` prints `(extension in main):` first, which
/// this demangler leaves out by design.
#[test]
fn a_private_method_in_an_extension_leaves_out_the_extension_prefix() {
    // Constructed with `swiftc -enable-library-evolution` and checked with `swift-demangle`.
    assert_eq!(dm("$sSi4mainE5twice029_12232F587A4C5CD8B1EEDF696793D2FCLLSiyF"), "Swift.Int.(twice in _12232F587A4C5CD8B1EEDF696793A4FC)() -> Swift.Int");
}

/// `swift-demangle` decodes none of these: `LL` after something other than two identifiers
/// (nothing, a type, another private name), a private name where a context or a type belongs,
/// or `L` cut short.
#[test]
fn a_private_name_without_two_names_is_invalid() {
    for symbol in ["$sLL", "$s4mainSiLLV", "$s4main1aSiLLV", "$s4main1a1bLLLLV", "$s4main1a1bLLSiLLV", "$s4main1a1bLL1cV", "$s4main1a1bLLSi_tF", "$s4main1a1bL"]
    {
        let d = demangle(symbol);
        assert_eq!(d.status, Status::Invalid, "{symbol}: {:?}", d.demangled);
    }
}
