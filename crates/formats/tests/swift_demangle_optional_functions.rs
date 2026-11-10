use craft_anything_formats::swift_demangle::{Status, demangle};

fn dm(symbol: &str) -> String {
    let d = demangle(symbol);
    assert_eq!(d.status, Status::Demangled, "{symbol}: {:?}", d.reason);
    d.demangled.unwrap_or_default()
}

/// An optional function type keeps its parentheses, so it doesn't read as a function that returns
/// an optional. Symbols from macOS 15.7 system binaries, expected text from `swift-demangle`.
#[test]
fn optional_function_types_are_parenthesised() {
    assert_eq!(dm("_$s10WeatherKit11WDSEndpointV20portOverrideProviderSiycSgvg"), "WeatherKit.WDSEndpoint.portOverrideProvider.getter : (() -> Swift.Int)?");
    assert_eq!(dm("_$s8ShelfKit15TextFieldActionC13cancelClosureAA0E0CSgycSgvg"), "ShelfKit.TextFieldAction.cancelClosure.getter : (() -> ShelfKit.Action?)?");
    assert_eq!(dm("_$s8FMIPCore11FMIPManagerC14refreshBeacons10completionyyycSg_tF"), "FMIPCore.FMIPManager.refreshBeacons(completion: (() -> ())?) -> ()");
    assert_eq!(
        dm("_$s12GameServices0aB11ActorSystemC7invokerAC10Foundation4DataVSgAGYaKcSg_tcfc"),
        "GameServices.GameServicesActorSystem.init(invoker: ((Foundation.Data) async throws -> Foundation.Data?)?) -> GameServices.GameServicesActorSystem"
    );
}

/// `!`, nested optionals and protocol compositions take the parentheses too; other wrapped types
/// don't. Constructed symbols, checked with `swift-demangle`.
#[test]
fn other_optionals_follow_the_same_rule() {
    assert_eq!(dm("$s4main1xs27ImplicitlyUnwrappedOptionalOySiycGvp"), "main.x : (() -> Swift.Int)!");
    assert_eq!(dm("$s4main1xSiycSgSgvp"), "main.x : (() -> Swift.Int)??");
    assert_eq!(dm("$s4main1xAA1PP_AA1QPpSgvp"), "main.x : (main.P & main.Q)?");
    assert_eq!(dm("$s4main1xAA1PP_pSgvp"), "main.x : main.P?");
    assert_eq!(dm("$s4main1xSi_SitSgvp"), "main.x : (Swift.Int, Swift.Int)?");
    assert_eq!(dm("$s4main1xSDySSSiycGSgvp"), "main.x : [Swift.String : () -> Swift.Int]?");
}
