use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use super::in_flight;
use crate::app::Ctx;
use crate::config::{Protocols, Settings};
use crate::ports::http::Http;
use crate::test_support::{a_context, a_password, env_at, nowhere, SeedFs};
use lemonfiber_fixtures::downloads::{
    downloads, QBIT_FINISHED, QBIT_TORRENTS, SAB_EMPTY, SAB_KEY_INI, SAB_QUEUE,
};
use lemonfiber_fixtures::http::Fake;

/// The forms an operator names, as the engine takes them.
fn named(forms: &[&str]) -> Vec<String> {
    forms.iter().map(|form| (*form).to_owned()).collect()
}

/// A context that can reach both download clients: `SABnzbd`'s key on a fake
/// filesystem, qBittorrent's password in a scratch env file, and both protocols
/// in play so a plan holds both.
fn reaching(http: Arc<dyn Http>, env_file: Option<PathBuf>) -> Ctx {
    let settings = Settings {
        protocols: Protocols::both(),
        env_file,
        ..Settings::default()
    };
    a_context()
        .settings(settings)
        .build()
        .waiting(Duration::ZERO)
        .with_filesystem(Arc::new(SeedFs::keyed(None, Some(SAB_KEY_INI))))
        .with_http(http)
}

/// The whole reason this is safe to put on the teardown path: a form with no
/// download client in it does not go to the network to find that out.
#[tokio::test]
async fn a_form_holding_no_client_asks_nothing_at_all() {
    let fake = Fake::silent();
    let ctx = reaching(Arc::clone(&fake) as Arc<dyn Http>, None);

    let found = in_flight(&ctx, &named(&["search"])).await;

    assert!(found.is_empty());
    assert!(
        fake.requests().is_empty(),
        "a form with no download client made requests anyway"
    );
}

#[tokio::test]
async fn both_clients_say_what_they_are_working_on() {
    let fake = downloads(QBIT_TORRENTS, SAB_QUEUE);
    let ctx = reaching(
        Arc::clone(&fake) as Arc<dyn Http>,
        Some(env_at("in-flight", &a_password())),
    );

    let found = in_flight(&ctx, &named(&["dl"])).await;

    let names: Vec<&str> = found
        .iter()
        .map(|download| download.name.as_str())
        .collect();
    assert!(names.contains(&"Ubuntu.iso"), "{names:?}");
    assert!(names.contains(&"Linux.nzb"), "{names:?}");
}

/// Naming a finished download would be warning about work stopping cannot undo.
#[tokio::test]
async fn a_download_that_has_finished_is_not_in_flight() {
    let fake = downloads(QBIT_FINISHED, SAB_EMPTY);
    let ctx = reaching(
        Arc::clone(&fake) as Arc<dyn Http>,
        Some(env_at("finished", &a_password())),
    );

    assert!(in_flight(&ctx, &named(&["dl"])).await.is_empty());
}

/// A client already on its way down cannot be asked, and a teardown blocked on
/// one that is not there would fire exactly when it is least wanted.
#[tokio::test]
async fn a_client_that_will_not_answer_contributes_nothing() {
    let ctx = reaching(Fake::scripted(Vec::new()), None);

    assert!(in_flight(&ctx, &named(&["dl"])).await.is_empty());
}

#[tokio::test]
async fn a_stack_that_cannot_be_read_finds_nothing() {
    let settings = Settings {
        protocols: Protocols::both(),
        ..Settings::default()
    };
    let ctx = a_context()
        .over(nowhere())
        .settings(settings)
        .build()
        .with_http(Fake::silent());

    assert!(in_flight(&ctx, &named(&["dl"])).await.is_empty());
}

#[tokio::test]
async fn a_form_the_stack_does_not_declare_finds_nothing() {
    let ctx = reaching(Fake::silent(), None);

    assert!(in_flight(&ctx, &named(&["no-such-form"])).await.is_empty());
}
