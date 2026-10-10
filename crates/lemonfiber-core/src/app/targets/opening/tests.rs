use std::path::Path;
use std::sync::Arc;

use lemonfiber_fixtures::http::{Answer, Fake};
use lemonfiber_fixtures::scratch::Scratch;
use lemonfiber_manifest::{Api, ApiKind, KeySource, Manifest};

use super::{request_service, requests_as_owner, requests_from};
use crate::app::Ctx;
use crate::plugin::first_party::FirstParty;
use crate::plugin::Installed;
use crate::ports::docker::{Health, Lifecycle};
use crate::test_support::{
    a_context, a_placed, an_installed, Reporting, SeedFs, CONTRACTED_REQUESTS,
    CONTRACTED_REQUESTS_AT, SEERR_SETTINGS,
};
use crate::wiring::{Chosen, Fillers};

/// The digest the plugin bringing a request service was installed from.
const MANIFEST: &str = "intake-manifest";

/// The plugin bringing a request service, as first-party.
const FIRST_PARTY: [FirstParty; 1] = [FirstParty {
    plugin: "intake",
    manifest: MANIFEST,
}];

/// The request service's own adapter, reading its key from the settings it writes.
fn seerr_api() -> Api {
    Api {
        kind: ApiKind::Seerr,
        key_source: KeySource::ConfigJson,
        path: Some("/config/settings.json".to_owned()),
        version: None,
    }
}

/// A plugin whose `service` fills `request.intake`, speaking `speaks` and through `api`.
fn bringing(service: &str, speaks: &[&str], api: Option<Api>) -> Installed {
    let mut placed = a_placed(service, &["request.intake"], api, Some(8080));
    placed.speaks = speaks.iter().map(|one| (*one).to_owned()).collect();
    let mut installed = an_installed("intake", vec![placed]);
    installed.manifest = MANIFEST.to_owned();
    installed
}

/// The shipped stack written to `project` with its own request service where `own`,
/// `installed` beside it, and [`FIRST_PARTY`] trusted.
fn beside(installed: &[Installed], own: bool, project: &Path) -> Fillers {
    crate::test_support::stack()
        .manifest()
        .map(|mut manifest: Manifest| {
            if !own {
                manifest.services.retain(|service| service.id != "seerr");
            }
            Fillers::trusting(
                &manifest,
                installed,
                &Chosen::default(),
                Some(project),
                &FIRST_PARTY,
            )
        })
        .unwrap_or_default()
}

/// A project holding the plugin's key where `keyed`, and a context publishing it on
/// loopback, reading the stack's request service's settings, and answering every call
/// with nothing.
fn reaching(tag: &str, keyed: bool) -> (Scratch, Ctx, Arc<Fake>) {
    let project = Scratch::new(&format!("opening-{tag}"));
    if keyed {
        let at = crate::plugin::key_file(&project, CONTRACTED_REQUESTS);
        let _ = std::fs::create_dir_all(at.parent().unwrap_or(&at));
        let _ = std::fs::write(&at, "contracted-key");
    }
    let engine = Reporting::holding(&[CONTRACTED_REQUESTS], Lifecycle::Running, Health::Healthy)
        .publishing(&[(CONTRACTED_REQUESTS, "127.0.0.1", 8080)]);
    let fake = Fake::always(Answer::reply(204, ""));
    let mut ctx = a_context()
        .engine(Arc::new(engine))
        .build()
        .with_http(fake.clone())
        .with_filesystem(Arc::new(
            SeedFs::keyed(None, None).with_seerr(SEERR_SETTINGS),
        ));
    ctx.settings.env_file = Some(project.join(".env"));
    (project, ctx, fake)
}

/// Every URL asked, with the key each presented.
fn asked(fake: &Fake) -> Vec<(String, Option<String>)> {
    fake.requests()
        .into_iter()
        .map(|one| {
            let key = one
                .headers
                .iter()
                .find(|(name, _)| name.eq_ignore_ascii_case("x-api-key"))
                .map(|(_, value)| value.clone());
            (one.url, key)
        })
        .collect()
}

/// **A first-party request service speaking `request.intake` is asked over it.** At its
/// own address with its own key, and never as the bundled request service.
#[tokio::test]
async fn a_first_party_request_service_speaking_the_contract_is_asked_over_it() {
    let (project, ctx, fake) = reaching("contracted", true);
    let fillers = beside(
        &[bringing(CONTRACTED_REQUESTS, &["request.intake@1"], None)],
        false,
        &project,
    );

    let answered = match requests_from(&ctx, &fillers).await {
        Some(access) => access.requests.answers().await.is_ok(),
        None => false,
    };

    assert!(answered, "{:?}", asked(&fake));
    let urls: Vec<String> = asked(&fake).into_iter().map(|(url, _)| url).collect();
    assert_eq!(urls, vec![format!("{CONTRACTED_REQUESTS_AT}answers")]);
}

/// A request service speaking the contract that cannot be asked over it — holding no
/// key, or speaking a major this build does not — is asked nothing, and never as the
/// bundled request service, whatever adapter it also names.
#[tokio::test]
async fn a_contracted_request_service_that_cannot_be_asked_is_asked_nothing() {
    for (tag, keyed, speaks) in [
        ("unkeyed", false, "request.intake@1"),
        ("unspoken", true, "request.intake@2"),
    ] {
        let (project, ctx, fake) = reaching(tag, keyed);
        let fillers = beside(
            &[bringing(CONTRACTED_REQUESTS, &[speaks], Some(seerr_api()))],
            false,
            &project,
        );

        assert!(requests_from(&ctx, &fillers).await.is_none(), "{tag}");
        let owned = match request_service(&fillers) {
            Some(filler) => requests_as_owner(&ctx, filler).await.is_some(),
            None => true,
        };
        assert!(!owned, "{tag}");
        assert!(fake.requests().is_empty(), "{tag}: {:?}", asked(&fake));
    }
}

/// **A plugin's service is never asked as the stack's own request service.** One
/// speaking no contract, under the stack's request service's id and adapter, with a key
/// on file, is asked nothing.
#[tokio::test]
async fn a_plugin_speaking_no_contract_is_never_asked_as_the_bundled_request_service() {
    let (project, ctx, fake) = reaching("impostor", true);
    let fillers = beside(
        &[bringing("seerr", &[], Some(seerr_api()))],
        false,
        &project,
    );

    assert!(requests_from(&ctx, &fillers).await.is_none());
    assert!(fake.requests().is_empty(), "{:?}", asked(&fake));
}

/// The stack's own request service is asked through this build's adapter, presenting
/// the key it wrote for itself; one that has not written it yet is asked nothing.
#[tokio::test]
async fn the_stacks_own_request_service_is_asked_with_its_own_key() {
    let (project, ctx, fake) = reaching("bundled", false);
    let fillers = beside(&[], true, &project);

    let answered = match requests_from(&ctx, &fillers).await {
        Some(access) => access.requests.answers().await.is_ok(),
        None => false,
    };

    assert!(answered, "{:?}", asked(&fake));
    assert_eq!(
        asked(&fake),
        vec![(
            "http://127.0.0.1:5055/api/v1/auth/me".to_owned(),
            Some("seerr-own-key".to_owned())
        )]
    );

    let unwritten = ctx.with_filesystem(Arc::new(SeedFs::keyed(None, None)));
    assert!(requests_from(&unwritten, &fillers).await.is_none());
}

/// The stack's own request service is asked as its owner before it has written its
/// key, holding none, so whatever uses it reports the refusal in its own words.
#[tokio::test]
async fn the_stacks_own_request_service_is_asked_as_its_owner_before_it_writes_its_key() {
    let (project, ctx, fake) = reaching("bundled-unkeyed", false);
    let unwritten = ctx.with_filesystem(Arc::new(SeedFs::keyed(None, None)));
    let fillers = beside(&[], true, &project);

    let answered = match request_service(&fillers) {
        Some(filler) => match requests_as_owner(&unwritten, filler).await {
            Some(requests) => requests.answers().await.is_ok(),
            None => false,
        },
        None => false,
    };

    assert!(answered, "{:?}", asked(&fake));
    assert_eq!(
        asked(&fake),
        vec![("http://127.0.0.1:5055/api/v1/auth/me".to_owned(), None)]
    );
}
