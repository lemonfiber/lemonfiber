use super::{service, written};
use crate::plugin::{Asking, Installed};
use crate::test_support::{a_minting_curator, A_MINTED_KEY};
use crate::wiring::{settle, unfilled, Chosen, Reaches, Settled, Unfilled};

/// A plugin whose one service asks for `library.curate` in `native`.
fn an_asker(native: Option<&str>) -> Installed {
    an_asker_in(native.map(lemonfiber_manifest::container::Apis::one))
}

/// A plugin whose one service asks for `library.curate` in any of `native`.
fn an_asker_in(native: Option<lemonfiber_manifest::container::Apis>) -> Installed {
    let mut placed = crate::test_support::a_placed("finder", &[], None);
    placed.asks = vec![Asking {
        capability: "library.curate".to_owned(),
        each: false,
        native,
        files: None,
        media_types: Vec::new(),
    }];
    crate::test_support::an_installed("finder", vec![placed])
}

/// What the finder's ask reaches beside the curator.
fn reached(native: Option<&str>) -> Option<Reaches> {
    let manifest = written(&service("asker", ""))?;
    settle(
        &manifest,
        &[a_minting_curator(), an_asker(native)],
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
            Some(lemonfiber_manifest::container::Apis::one("servarr"))
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
        &[a_minting_curator(), an_asker(Some("sabnzbd"))],
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
            native: Some(lemonfiber_manifest::container::Apis::one("sabnzbd")),
            files: None,
            media_types: Vec::new(),
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
    let installed = [a_minting_curator(), an_asker(Some("servarr"))];
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
        Some(A_MINTED_KEY.to_owned())
    );
    assert_eq!(
        upstream.and_then(crate::wiring::Filler::talks_in),
        Some(lemonfiber_manifest::ApiKind::Servarr)
    );
    assert_eq!(
        upstream.and_then(|one| trusted.credential(one)),
        Some(A_MINTED_KEY.to_owned())
    );
}

/// A curator brought by itself, fronting nothing, that talks `native` where it says.
fn a_lone_curator(native: Option<&str>) -> Installed {
    let mut placed = crate::test_support::a_placed("curator", &["library.curate"], None);
    placed.native = native.map(str::to_owned);
    crate::test_support::an_installed("curator", vec![placed])
}

#[test]
fn an_ask_in_an_api_reaches_a_filler_fronting_nothing_unless_it_talks_another() {
    let reaching = |curator: Installed| {
        let manifest = written(&service("asker", ""))?;
        settle(
            &manifest,
            &[curator, an_asker(Some("servarr"))],
            &Chosen::default(),
        )
        .into_iter()
        .find(|wired| wired.by == "finder")
        .and_then(|wired| match wired.reaches {
            Reaches::Asked { services, .. } => Some(services),
            Reaches::ByName { .. } => None,
        })
    };

    assert_eq!(
        reaching(a_lone_curator(None)),
        Some(vec!["curator".to_owned()])
    );
    assert_eq!(
        reaching(a_lone_curator(Some("servarr"))),
        Some(vec!["curator".to_owned()])
    );
    assert_eq!(reaching(a_lone_curator(Some("komga"))), Some(Vec::new()));
}

/// An ask naming a list of APIs reaches the upstream declaring any one of them, and is
/// unfilled naming every one where none is declared.
#[test]
fn an_ask_in_any_of_a_list_of_apis_reaches_the_upstream_declaring_one_of_them() {
    let Some(manifest) = written(&service("asker", "")) else {
        return;
    };
    let listed =
        |apis: &str| serde_json::from_str::<lemonfiber_manifest::container::Apis>(apis).ok();
    let reaching = |apis: &str| {
        settle(
            &manifest,
            &[a_minting_curator(), an_asker_in(listed(apis))],
            &Chosen::default(),
        )
    };

    let either = reaching(r#"["sabnzbd","servarr"]"#);
    assert!(either.iter().any(|one| one.by == "finder"
        && matches!(&one.reaches, Reaches::Asked { services, .. } if services == &vec!["series-upstream".to_owned()])));

    let neither = unfilled(&reaching(r#"["sabnzbd","nzbget"]"#));
    assert_eq!(
        neither.first().map(ToString::to_string).as_deref(),
        Some(
            "finder asks for library.curate in sabnzbd or nzbget and nothing filling it talks \
             sabnzbd or nzbget"
        )
    );
}
