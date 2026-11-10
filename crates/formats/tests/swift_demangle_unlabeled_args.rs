use craft_anything_formats::swift_demangle::{Status, demangle};

fn dm(symbol: &str) -> String {
    let d = demangle(symbol);
    assert_eq!(d.status, Status::Demangled, "{symbol}: {:?}", d.reason);
    d.demangled.unwrap_or_default()
}

// Symbols from macOS 15.7 and Xcode 26 system binaries, expected text from `swift-demangle`.

/// When some arguments have a label, an argument without one reads `_:`, wherever it stands.
#[test]
fn an_unlabeled_argument_next_to_a_labeled_one_reads_underscore() {
    assert_eq!(
        dm("_$s10Foundation13__DataStorageC6append_6lengthySV_SitF"),
        "Foundation.__DataStorage.append(_: Swift.UnsafeRawPointer, length: Swift.Int) -> ()"
    );
    assert_eq!(
        dm("_$s4simd3mix__1ts5SIMD2VySdGAF_A2FtF"),
        "simd.mix(_: Swift.SIMD2<Swift.Double>, _: Swift.SIMD2<Swift.Double>, t: Swift.SIMD2<Swift.Double>) -> Swift.SIMD2<Swift.Double>"
    );
    assert_eq!(dm("_$ss6HasherV5_hash4seed_S2i_SutFZ"), "static Swift.Hasher._hash(seed: Swift.Int, _: Swift.UInt) -> Swift.Int");
    // Constructed with `swiftc -enable-library-evolution` and checked with `swift-demangle`.
    assert_eq!(dm("_$s4main1SV3mid1a_1cySi_S2itF"), "main.S.mid(a: Swift.Int, _: Swift.Int, c: Swift.Int) -> ()");
}

/// The same holds for initialisers, generic functions, and the argument types `inout`, a tuple
/// and a function.
#[test]
fn initialisers_and_any_argument_type_read_underscore() {
    assert_eq!(
        dm("_$s10Foundation4DataV8IteratorV_2atAeC_SitcfC"),
        "Foundation.Data.Iterator.init(_: Foundation.Data, at: Swift.Int) -> Foundation.Data.Iterator"
    );
    assert_eq!(
        dm("_$s10PreviewsUI12DelayedValueC_9schedulerACyxq_Gx_q_tcfC"),
        "PreviewsUI.DelayedValue.__allocating_init(_: A, scheduler: B) -> PreviewsUI.DelayedValue<A, B>"
    );
    assert_eq!(
        dm("_$s5CASDB8writeInt_6offset5valueSiSays5UInt8VGz_S2itKF"),
        "CASDB.writeInt(_: inout [Swift.UInt8], offset: Swift.Int, value: Swift.Int) throws -> Swift.Int"
    );
    assert_eq!(
        dm("_$ss16_overflowChecked_4file4linexx_Sbt_s12StaticStringVSutlF"),
        "Swift._overflowChecked<A>(_: (A, Swift.Bool), file: Swift.StaticString, line: Swift.UInt) -> A"
    );
    assert_eq!(
        dm("_$ss16AsyncMapSequenceV_9transformAByxq_Gx_q_7ElementQzYactcfC"),
        "Swift.AsyncMapSequence.init(_: A, transform: (A.Element) async -> B) -> Swift.AsyncMapSequence<A, B>"
    );
}

/// Globals that name a function print its arguments the same way.
#[test]
fn witnesses_thunks_descriptors_and_merged_functions_read_underscore() {
    assert_eq!(
        dm("_$s10Foundation4DataVSlAASl5index_8offsetBy5IndexQzAG_SitFTW"),
        "protocol witness for Swift.Collection.index(_: A.Index, offsetBy: Swift.Int) -> A.Index in conformance Foundation.Data : Swift.Collection in Foundation"
    );
    assert_eq!(
        dm("_$s10Foundation11JSONDecoderC6decode_4fromxxm_AA4DataVtKSeRzlFTj"),
        "dispatch thunk of Foundation.JSONDecoder.decode<A where A: Swift.Decodable>(_: A.Type, from: Foundation.Data) throws -> A"
    );
    assert_eq!(
        dm("_$s10PreviewsUI12DelayedValueC_9schedulerACyxq_Gx_q_tcfCTq"),
        "method descriptor for PreviewsUI.DelayedValue.__allocating_init(_: A, scheduler: B) -> PreviewsUI.DelayedValue<A, B>"
    );
    assert_eq!(
        dm("_$s10DVTIconKit26GeneratedIconReferenceSpecV15externalSetName_ACSSSg_SStcfCTm"),
        "merged DVTIconKit.GeneratedIconReferenceSpec.init(externalSetName: Swift.String?, _: Swift.String) -> DVTIconKit.GeneratedIconReferenceSpec"
    );
}

/// With no label at all, or with a single labeled argument, nothing changes.
#[test]
fn arguments_print_bare_when_none_has_a_label() {
    assert_eq!(dm("_$ss27_debuggerTestingCheckExpectyySS_SStF"), "Swift._debuggerTestingCheckExpect(Swift.String, Swift.String) -> ()");
    assert_eq!(dm("_$s10Foundation3URLV4hash4intoys6HasherVz_tF"), "Foundation.URL.hash(into: inout Swift.Hasher) -> ()");
    // Constructed with `swiftc -enable-library-evolution` and checked with `swift-demangle`.
    assert_eq!(dm("_$s4main1SV3twoyySi_SitF"), "main.S.two(Swift.Int, Swift.Int) -> ()");
    assert_eq!(dm("_$s4main1UV2p2yySi_Sit_tF"), "main.U.p2((Swift.Int, Swift.Int)) -> ()");
    // Constructed by hand and checked with `swift-demangle`: a label list of `_` only reads as no
    // labels, and a single argument not wrapped in a tuple prints bare, even with a label.
    assert_eq!(dm("_$s4main1SV3two__ySi_SitF"), "main.S.two(Swift.Int, Swift.Int) -> ()");
    assert_eq!(dm("_$s4main1SV__ACSi_SitcfC"), "main.S.init(Swift.Int, Swift.Int) -> main.S");
    assert_eq!(dm("_$s4main1SV3one_ySiF"), "main.S.one(Swift.Int) -> ()");
    assert_eq!(dm("_$s4main1SV3one1aySiF"), "main.S.one(Swift.Int) -> ()");
}

/// Constructed symbols whose label list doesn't fit the arguments; `swift-demangle` rejects them.
#[test]
fn a_label_list_that_does_not_fit_the_arguments_is_invalid() {
    for symbol in ["_$s4main1SV3one_ySi_SitF", "_$s4main1SV3tup_ySi_SitF", "_$s4main1SV3oneySi_SitF", "_$s4main1SV_Si_SitcfC"] {
        assert_eq!(demangle(symbol).status, Status::Invalid, "{symbol}");
    }
}
