use craft_anything_formats::swift_demangle::{Status, demangle};

fn dm(symbol: &str) -> String {
    let d = demangle(symbol);
    assert_eq!(d.status, Status::Demangled, "{symbol}: {:?}", d.reason);
    d.demangled.unwrap_or_default()
}

// Symbols from macOS 15.7 system binaries, expected text from `swift-demangle`.

/// In a protocol list (`…_p`) a protocol is a context and a name, with no `P` kind letter.
#[test]
fn a_protocol_named_in_a_list_is_a_type() {
    assert_eq!(dm("_$sSS6encode2toys7Encoder_p_tKF"), "Swift.String.encode(to: Swift.Encoder) throws -> ()");
    assert_eq!(dm("_$s20iTunesPodcastLibraryAAC4lockSo9NSLocking_pvg"), "iTunesPodcastLibrary.iTunesPodcastLibrary.lock.getter : __C.NSLocking");
    assert_eq!(
        dm("_$s5Books24ViewBookUpdatesOperationC5errors5Error_pSgvpfi"),
        "variable initialization expression of Books.ViewBookUpdatesOperation.error : Swift.Error?"
    );
}

#[test]
fn a_composition_lists_every_protocol() {
    assert_eq!(
        dm("_$s8ShelfKit13UberViewModelV4itemAA06ModernA4Item_AA0C18BackgroundProtocolpvg"),
        "ShelfKit.UberViewModel.item.getter : ShelfKit.ModernShelfItem & ShelfKit.UberBackgroundProtocol"
    );
}

/// A protocol in a list takes no substitution slot: `AF` and `AE` after one name the types
/// `swift-demangle` names. A protocol named again is its context and an identifier reference (`sAA_p`).
#[test]
fn a_protocol_in_a_list_takes_no_substitution_slot() {
    assert_eq!(
        dm("_$s19OnDeviceStorageCore10ConnectionC6scalaryAA7Binding_pSgSS_SayAFGtKF"),
        "OnDeviceStorageCore.Connection.scalar(Swift.String, [OnDeviceStorageCore.Binding?]) throws -> OnDeviceStorageCore.Binding?"
    );
    assert_eq!(
        dm("_$s8CreateML4showyAA23MLStreamingVisualizable_pAA15MLUntypedColumnV_AEtF"),
        "CreateML.show(CreateML.MLUntypedColumn, CreateML.MLUntypedColumn) -> CreateML.MLStreamingVisualizable"
    );
    assert_eq!(dm("_$ss12_AnyIndexBoxP8_isEqual2toSbsAA_p_tFTj"), "dispatch thunk of Swift._AnyIndexBox._isEqual(to: Swift._AnyIndexBox) -> Swift.Bool");
}
