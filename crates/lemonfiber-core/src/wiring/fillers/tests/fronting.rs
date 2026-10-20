use super::super::Fillers;
use super::shipped;
use crate::test_support::{a_placed, an_installed};
use crate::wiring::Chosen;

/// A plugin's adapter, `front`, standing in front of `fronts` where it names one.
fn an_adapter(fronts: Option<&str>) -> crate::plugin::Placed {
    let mut adapter = a_placed("front", &[], None, Some(8080));
    adapter.fronts = fronts.map(str::to_owned);
    adapter
}

/// The plugin that brought the upstream `front` stands in front of, where it stands in
/// front of one at all.
fn fronted(fillers: &Fillers) -> Option<&str> {
    let adapter = fillers.service("front")?;
    fillers
        .fronted_by(adapter)
        .and_then(super::super::Filler::brought_by)
}

/// The upstream an adapter stands in front of is its own plugin's service of that id,
/// even where another plugin brings a service of the same id and is found first.
#[test]
fn an_adapter_fronts_its_own_plugins_service_of_that_id() {
    let installed = [
        an_installed("other", vec![a_placed("back", &[], None, Some(9000))]),
        an_installed(
            "own",
            vec![
                an_adapter(Some("back")),
                a_placed("back", &[], None, Some(9000)),
            ],
        ),
    ];
    let fillers = shipped(&installed, &Chosen::default(), |_| ());

    assert_eq!(
        fillers
            .service("back")
            .and_then(super::super::Filler::brought_by),
        Some("other")
    );
    assert_eq!(fronted(&fillers), Some("own"));
}

/// An adapter fronts nothing where the service of the id it names is another plugin's or
/// the stack's own, or where it names none: its upstream is never found outside its own
/// plugin.
#[test]
fn an_adapter_fronts_nothing_outside_its_own_plugin() {
    let elsewhere = [
        an_installed("own", vec![an_adapter(Some("back"))]),
        an_installed("other", vec![a_placed("back", &[], None, Some(9000))]),
    ];
    let the_stacks = [an_installed("own", vec![an_adapter(Some("jellyfin"))])];
    let unnamed = [an_installed(
        "own",
        vec![an_adapter(None), a_placed("back", &[], None, Some(9000))],
    )];

    for (case, installed, named) in [
        ("another plugin's", &elsewhere[..], "back"),
        ("the stack's", &the_stacks[..], "jellyfin"),
        ("none named", &unnamed[..], "back"),
    ] {
        let fillers = shipped(installed, &Chosen::default(), |_| ());
        assert!(fillers.service("front").is_some(), "{case}");
        assert!(fillers.service(named).is_some(), "{case}");
        assert_eq!(fronted(&fillers), None, "{case}");
    }
}

/// A service of the stack's own naming an upstream fronts nothing, even where a plugin
/// brings a service of that id: only a plugin's adapter has a plugin to look in.
#[test]
fn a_stack_service_naming_an_upstream_fronts_nothing() {
    let installed = [an_installed(
        "own",
        vec![a_placed("back", &[], None, Some(9000))],
    )];
    let fillers = shipped(&installed, &Chosen::default(), |_| ());
    let bundled = fillers.service("jellyfin").cloned().map(|mut one| {
        one.fronts = Some("back".to_owned());
        one
    });

    assert!(fillers.service("back").is_some());
    assert_eq!(
        bundled
            .as_ref()
            .map(|one| (one.brought_by(), fillers.fronted_by(one).is_some())),
        Some((None, false))
    );
}
