use craft_anything_formats::swift_demangle::{Status, demangle};

fn dm(symbol: &str) -> String {
    let d = demangle(symbol);
    assert_eq!(d.status, Status::Demangled, "{symbol}: {:?}", d.reason);
    d.demangled.unwrap_or_default()
}

/// `Qy` numbers its generic parameter like `q`: `Qy_` is the second parameter `B`. Symbols from
/// macOS 15.7 system binaries, expected text from `swift-demangle`.
#[test]
fn qy_names_the_second_generic_parameter() {
    assert_eq!(dm("_$ss12Zip2SequenceV8IteratorV4next7ElementQz_AFQy_tSgyF"), "Swift.Zip2Sequence.Iterator.next() -> (A.Element, B.Element)?");
    assert_eq!(
        dm("_$s10Algorithms14EitherSequenceO8IteratorV5rightADQy_Sgvpfi"),
        "variable initialization expression of Algorithms.EitherSequence.Iterator.right : B.Iterator?"
    );
    assert_eq!(
        dm("_$s10AppIntents18LessThanComparatorC16mappingTransformACyxq_q0_Gq0_13UnwrappedTypeQy_c_tcfc"),
        "AppIntents.LessThanComparator.init(mappingTransform: (B.UnwrappedType) -> C) -> AppIntents.LessThanComparator<A, B, C>"
    );
}

/// Later parameters and substitutions of `Qy` members, and `Qz` for `A`. Constructed symbols,
/// checked with `swift-demangle`.
#[test]
fn qy_indexes_follow_the_generic_parameter_order() {
    assert_eq!(dm("$s4main1SV3fooyy7ElementQzF"), "main.S.foo(A.Element) -> ()");
    assert_eq!(dm("$s4main1SV3fooyy7ElementQy0_F"), "main.S.foo(C.Element) -> ()");
    assert_eq!(dm("$s4main1SV3fooyy7ElementQy4_F"), "main.S.foo(G.Element) -> ()");
    assert_eq!(dm("$s4main1SV3fooyy7ElementQy__AFtF"), "main.S.foo(B.Element, B.Element) -> ()");
    assert_eq!(dm("$s4main1SV1x7ElementQy0_mvg"), "main.S.x.getter : C.Element.Type");
    assert_eq!(dm("$S4main1SV3fooyy7ElementQy_F"), "main.S.foo(B.Element) -> ()");
}
