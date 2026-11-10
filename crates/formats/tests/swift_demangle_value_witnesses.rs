use craft_anything_formats::swift_demangle::{Status, demangle};

fn dm(symbol: &str) -> String {
    let d = demangle(symbol);
    assert_eq!(d.status, Status::Demangled, "{symbol}: {:?}", d.reason);
    d.demangled.unwrap_or_default()
}

// Real symbols are from macOS 15.7 and Xcode 26 system binaries; constructed ones are marked.
// Every expected string is what `swift-demangle` prints.

/// The value witnesses of a struct: copy, move and destroy a value.
#[test]
fn struct_value_witnesses_decode() {
    assert_eq!(dm("_$s10Foundation4DataVwxx"), "destroy value witness for Foundation.Data");
    assert_eq!(dm("_$ss5SIMD2Vwtk"), "initializeWithTake value witness for Swift.SIMD2");
    assert_eq!(dm("_$sSDwCP"), "initializeBufferWithCopyOfBuffer value witness for Swift.Dictionary");
    assert_eq!(dm("_$s10Foundation6LocaleVwca"), "assignWithCopy value witness for Foundation.Locale");
    assert_eq!(dm("_$s10Foundation6LocaleVwcp"), "initializeWithCopy value witness for Foundation.Locale");
    assert_eq!(dm("_$s10Foundation6LocaleVwta"), "assignWithTake value witness for Foundation.Locale");
    assert_eq!(dm("_$sSo4statVwet"), "getEnumTagSinglePayload value witness for __C.stat");
    assert_eq!(dm("_$sSo4statVwst"), "storeEnumTagSinglePayload value witness for __C.stat");
}

/// The enum-only value witnesses read, project and set the case of a value.
#[test]
fn enum_value_witnesses_decode() {
    assert_eq!(dm("_$ss6ResultOwug"), "getEnumTag value witness for Swift.Result");
    assert_eq!(dm("_$ss6ResultOwup"), "destructiveProjectEnumData value witness for Swift.Result");
    assert_eq!(dm("_$ss6ResultOwui"), "destructiveInjectEnumTag value witness for Swift.Result");
    assert_eq!(dm("_$sSD5IndexV8_VariantOwCP"), "initializeBufferWithCopyOfBuffer value witness for Swift.Dictionary.Index._Variant");
}

/// A value witness merged with identical ones of other types keeps the `merged` prefix.
#[test]
fn merged_value_witnesses_decode() {
    assert_eq!(dm("_$s3SIL7OperandVwetTm"), "merged getEnumTagSinglePayload value witness for SIL.Operand");
    assert_eq!(dm("_$s7Testing5EventVwxxTm"), "merged destroy value witness for Testing.Event");
}

/// A type declared in an extension: `swift-demangle` prints `(extension in SWBUtil):` before the
/// name, which this demangler leaves out by design.
#[test]
fn value_witness_of_a_type_in_an_extension_leaves_out_the_extension_prefix() {
    assert_eq!(dm("_$sSS7SWBUtilE7PadSideOwet"), "getEnumTagSinglePayload value witness for Swift.String.PadSide");
}

/// Every two-letter code `swift-demangle` accepts, on a constructed `main.A`, and value witnesses
/// of constructed non-nominal types.
#[test]
fn every_value_witness_code_has_its_name() {
    let codes = [
        ("al", "allocateBuffer"),
        ("ca", "assignWithCopy"),
        ("cp", "initializeWithCopy"),
        ("de", "deallocateBuffer"),
        ("et", "getEnumTagSinglePayload"),
        ("pr", "projectBuffer"),
        ("st", "storeEnumTagSinglePayload"),
        ("ta", "assignWithTake"),
        ("tk", "initializeWithTake"),
        ("tT", "initializeArrayWithTakeBackToFront"),
        ("ug", "getEnumTag"),
        ("ui", "destructiveInjectEnumTag"),
        ("up", "destructiveProjectEnumData"),
        ("xg", "getExtraInhabitantIndex"),
        ("xs", "storeExtraInhabitant"),
        ("xx", "destroy"),
        ("Cc", "initializeArrayWithCopy"),
        ("Cp", "initializeBufferWithCopy"),
        ("CP", "initializeBufferWithCopyOfBuffer"),
        ("Tk", "initializeBufferWithTake"),
        ("Tt", "initializeArrayWithTakeFrontToBack"),
        ("TK", "initializeBufferWithTakeOfBuffer"),
        ("Xx", "destroyArray"),
        ("XX", "destroyBuffer"),
    ];
    for (code, name) in codes {
        assert_eq!(dm(&format!("$s4main1AVw{code}")), format!("{name} value witness for main.A"));
    }
    assert_eq!(dm("$sSay4main1AVGwxx"), "destroy value witness for [main.A]");
    assert_eq!(dm("$s4main1AVSgwet"), "getEnumTagSinglePayload value witness for main.A?");
    assert_eq!(dm("$s4main1AV_SitwCP"), "initializeBufferWithCopyOfBuffer value witness for (main.A, Swift.Int)");
}

/// `swift-demangle` decodes none of these: a code it doesn't know, a code cut short, nothing
/// before `w`, a module or a bare name before it, or another global or a member before it.
#[test]
fn value_witness_without_a_known_code_or_a_type_is_invalid() {
    for symbol in [
        "$s4main1AVwzz",
        "$s4main1AVwxa",
        "$s4main1AVw",
        "$s4main1AVwx",
        "$swxx",
        "$s4mainwxx",
        "$s4main1Awxx",
        "$sSi4mainEwxx",
        "$s4main1AVMawxx",
        "$s4main1AVWVwxx",
        "$s4main1AVwxxwxx",
        "$s4main1AV5valueSivgwxx",
        "$s4main1AVAA1PAAMcwxx",
    ] {
        assert_eq!(demangle(symbol).status, Status::Invalid, "{symbol}");
    }
}
