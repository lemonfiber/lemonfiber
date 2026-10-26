use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use super::{in_flight, stopped_beside};
use crate::app::Ctx;
use crate::config::{Protocols, Settings};
use crate::ports::http::Http;
use crate::test_support::{a_context, env_without_password, nowhere, torrent, usenet};
use lemonfiber_fixtures::http::Fake;

/// The forms an operator names, as the engine takes them.
fn named(forms: &[&str]) -> Vec<String> {
    forms.iter().map(|form| (*form).to_owned()).collect()
}

/// A context that can reach both download clients: each moved into its plugin where
/// `env_file` gives it somewhere to be recorded, and both protocols in play so a plan
/// holds both.
fn reaching(http: Arc<dyn Http>, env_file: Option<PathBuf>) -> Ctx {
    let settings = Settings {
        protocols: Protocols::both(),
        env_file,
        ..Settings::default()
    };
    usenet::reaching_beside_torrents(
        a_context()
            .settings(settings)
            .build()
            .with_patience(Duration::ZERO)
            .with_http(http),
    )
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
    let fake = usenet::beside_torrents(&[torrent::carrying()], &[usenet::carrying()]);
    let ctx = reaching(
        Arc::clone(&fake) as Arc<dyn Http>,
        Some(env_without_password("in-flight")),
    );

    let found = in_flight(&ctx, &named(&["dl"])).await;

    let names: Vec<&str> = found
        .iter()
        .map(|download| download.name.as_str())
        .collect();
    assert!(names.contains(&torrent::CARRIED), "{names:?}");
    assert!(names.contains(&usenet::CARRIED), "{names:?}");
}

/// Naming a finished download would be warning about work stopping cannot undo.
#[tokio::test]
async fn a_download_that_has_finished_is_not_in_flight() {
    let fake = usenet::beside_torrents(&[torrent::finished()], &[]);
    let ctx = reaching(
        Arc::clone(&fake) as Arc<dyn Http>,
        Some(env_without_password("finished")),
    );

    assert!(in_flight(&ctx, &named(&["dl"])).await.is_empty());
}

/// A client already on its way down cannot be asked, and a teardown blocked on
/// one that is not there would fire exactly when it is least wanted.
#[tokio::test]
async fn a_client_that_will_not_answer_contributes_nothing() {
    let ctx = reaching(Fake::silent(), None);

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

/// A plugin the bundle installs stops with the stack profile its bundle entry names,
/// so its clients are among those a teardown of that profile asks, and a
/// teardown of any other leaves it out.
#[test]
fn a_moved_plugin_is_stopped_with_the_profile_its_bundle_entry_names() {
    let mut ctx = a_context().build();
    ctx.settings.env_file = Some(crate::test_support::env_without_password("in-flight-moved"));
    crate::test_support::media::moved_in(&ctx);
    let named: Vec<String> = crate::test_support::media::moved()
        .map(|installed| crate::plugin::profile_of(&installed))
        .into_iter()
        .collect();

    let stopping: Vec<String> = stopped_beside(&ctx, &named)
        .into_iter()
        .map(|installed| installed.plugin)
        .collect();

    assert_eq!(stopping, vec![crate::test_support::media::PLUGIN]);
    assert!(stopped_beside(&ctx, &["search".to_owned()]).is_empty());
}
