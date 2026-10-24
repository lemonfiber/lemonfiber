use super::{service, written};
use crate::plugin::{Asking, Installed};
use crate::wiring::{settle, unfilled, Chosen, Reaches, Settled, Unfilled};

/// A curator plugin: its adapter provides `library.curate` and fronts its upstream,
/// which talks `servarr` and holds a key lemonfiber minted for it.
fn a_curator() -> Installed {
    let mut adapter = crate::test_support::a_placed("series", &["library.curate"], None, None);
    adapter.fronts = Some("series-upstream".to_owned());
    let mut upstream = crate::test_support::a_placed("series-upstream", &[], None, Some(8989));
    upstream.native = Some("servarr".to_owned());
    let mut curator = crate::test_support::an_installed("series", vec![adapter, upstream]);
    curator.declared.secrets = vec![crate::plugin::Secret {
        id: "upstream-key".to_owned(),
        of: "series-upstream".to_owned(),
        why: "The key the adapter signs in with".to_owned(),
        minted: true,
        serves: None,
    }];
    curator
}

/// A plugin whose one service asks for `library.curate` in `native`.
fn an_asker(native: Option<&str>) -> Installed {
    let mut placed = crate::test_support::a_placed("finder", &[], None, None);
    placed.asks = vec![Asking {
        capability: "library.curate".to_owned(),
        each: false,
        native: native.map(str::to_owned),
    }];
    crate::test_support::an_installed("finder", vec![placed])
}

/// What the finder's ask reaches beside the curator.
fn reached(native: Option<&str>) -> Option<Reaches> {
    let manifest = written(&service("asker", ""))?;
    settle(
        &manifest,
        &[a_curator(), an_asker(native)],
        &Chosen::default(),
    )
    .into_iter()
    .find(|wired| wired.by == "finder")
    .map(|wired| wired.reaches)
}

#[test]
fn an_ask_in_the_upstreams_own_api_reaches_the_upstream_and_one_without_the_adapter() {
    let services = |reaches: Option<Reaches>| match reaches {
        Some(Reaches::Asked {
            services, native, ..
        }) => Some((services, native)),
        _ => None,
    };

    assert_eq!(
        services(reached(Some("servarr"))),
        Some((
            vec!["series-upstream".to_owned()],
            Some("servarr".to_owned())
        ))
    );
    assert_eq!(
        services(reached(None)),
        Some((vec!["series".to_owned()], None))
    );
}

#[test]
fn an_ask_in_an_api_no_upstream_talks_is_unfilled_naming_the_api() {
    let Some(manifest) = written(&service("asker", "")) else {
        return;
    };
    let wired = settle(
        &manifest,
        &[a_curator(), an_asker(Some("sabnzbd"))],
        &Chosen::default(),
    );

    let left = unfilled(&wired);

    assert!(wired.iter().any(|one| one.by == "finder"
        && matches!(&one.reaches, Reaches::Asked { settled: Settled::Unfilled, services, .. } if services.is_empty())));
    assert_eq!(
        left,
        vec![Unfilled {
            by: "finder".to_owned(),
            capability: "library.curate".to_owned(),
            native: Some("sabnzbd".to_owned()),
        }]
    );
    assert_eq!(
        left.first().map(ToString::to_string),
        Some(
            "finder asks for library.curate in sabnzbd and nothing filling it talks sabnzbd"
                .to_owned()
        )
    );
}

#[test]
fn an_upstream_is_reached_with_the_key_lemonfiber_minted_for_it_where_it_crosses() {
    let Some(manifest) = written(&service("asker", "")) else {
        return;
    };
    let installed = [a_curator(), an_asker(Some("servarr"))];
    let trusted = crate::wiring::Fillers::trusting(
        &manifest,
        &installed,
        &Chosen::default(),
        None,
        &crate::test_support::first_party("series"),
    );
    let upstream = trusted.service("series-upstream");

    assert_eq!(
        upstream.and_then(|one| one.minted.clone()),
        Some("series:upstream-key".to_owned())
    );
    assert_eq!(
        upstream.and_then(crate::wiring::Filler::talks_in),
        Some(lemonfiber_manifest::ApiKind::Servarr)
    );
    assert_eq!(
        upstream.and_then(|one| trusted.credential(one)),
        Some("series:upstream-key".to_owned())
    );
}
