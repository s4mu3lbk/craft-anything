use craft_anything_formats::swift_demangle::{Status, demangle};

fn dm(symbol: &str) -> String {
    let d = demangle(symbol);
    assert_eq!(d.status, Status::Demangled, "{symbol}: {:?}", d.reason);
    d.demangled.unwrap_or_default()
}

/// Symbols from macOS 15.7 system binaries, expected text from `swift-demangle`.
#[test]
fn unsafe_mutable_addressors_decode() {
    assert_eq!(dm("_$s5Books19PAGETURNLOG_ENABLEDSbvau"), "Books.PAGETURNLOG_ENABLED.unsafeMutableAddressor : Swift.Bool");
    assert_eq!(dm("_$s11FaceTimeMac8DefaultsC6sharedACvau"), "FaceTimeMac.Defaults.shared.unsafeMutableAddressor : FaceTimeMac.Defaults");
    assert_eq!(
        dm("_$s18TrustedPeersHelper11ResetReasonO8allCasesSayACGvau"),
        "TrustedPeersHelper.ResetReason.allCases.unsafeMutableAddressor : [TrustedPeersHelper.ResetReason]"
    );
}

/// The other kinds, on constructed symbols checked with `swift-demangle`.
#[test]
fn every_addressor_kind_is_named() {
    let kinds = [
        ("aO", "owningMutableAddressor"),
        ("ao", "nativeOwningMutableAddressor"),
        ("aP", "nativePinningMutableAddressor"),
        ("lu", "unsafeAddressor"),
        ("lO", "owningAddressor"),
        ("lo", "nativeOwningAddressor"),
        ("lp", "nativePinningAddressor"),
    ];
    for (code, name) in kinds {
        assert_eq!(dm(&format!("$s4main1xSiv{code}")), format!("main.x.{name} : Swift.Int"));
    }
    assert_eq!(dm("$s4main1AV1xSivauZ"), "static main.A.x.unsafeMutableAddressor : Swift.Int");
    assert_eq!(dm("$s4main1AVyS2iciau"), "main.A.subscript.unsafeMutableAddressor : (Swift.Int) -> Swift.Int");
}

/// `swift-demangle` decodes none of these: no kind letter, or one it doesn't know.
#[test]
fn an_addressor_without_a_known_kind_is_invalid() {
    for symbol in ["$s4main1xSiva", "$s4main1xSivl", "$s4main1xSivap", "$s4main1xSivlP", "$s4main1xSivaX"] {
        assert_eq!(demangle(symbol).status, Status::Invalid, "{symbol}");
    }
}
