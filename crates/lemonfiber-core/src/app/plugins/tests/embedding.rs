//! Installing a plugin the bundle installs: read from the tree this build carries, with
//! nothing fetched, and recorded at the revision the bundle pins.

use super::fetching::{from_git, served, Serving};
use super::*;
use crate::ports::docker::{Health, Lifecycle};
use crate::test_support::Reporting;

/// The plugin the bundle installs.
const BUNDLED: &str = "jellyfin";

/// What the bundle pins for it.
fn pinned() -> Option<&'static lemonfiber_bundled::Pinned> {
    super::super::embedding::pinned(BUNDLED)
}

/// A context over the shipped stack with the bundled plugin's own service taken out.
fn moved(name: &str, serving: &Arc<Serving>) -> Ctx {
    let mut ctx = served(name, serving)
        .with_http(lemonfiber_fixtures::http::Fake::silent())
        .with_patience(std::time::Duration::ZERO);
    let fork = lemonfiber_fixtures::scratch::Scratch::named(&format!("{name}-fork")).kept();
    let _ = std::fs::remove_dir_all(&fork);
    lemonfiber_fixtures::stack::manifest_without(&fork, BUNDLED);
    ctx.stack = crate::stack::Source::External(Box::leak(fork.into_boxed_path()));
    ctx
}

#[tokio::test]
async fn a_plugin_the_bundle_installs_is_read_from_this_build_at_its_pin() {
    let serving = Arc::new(Serving::listing(""));
    let ctx = moved("embedded-installed", &serving);

    let would = report(from_git(&ctx, BUNDLED).await)
        .and_then(|one| one.install)
        .map(|install| install.would);

    assert_eq!(
        would
            .as_ref()
            .map(|one| (one.from.as_str(), one.revision.as_str(), one.tree.as_str())),
        pinned().map(|one| (one.origin, one.revision, one.first_party.tree))
    );
    assert!(serving.checkouts().is_empty(), "nothing was checked out");
    assert!(!serving
        .asked()
        .iter()
        .any(|one| one.contains(&"fetch".to_owned())));
    let checkouts =
        crate::app::targets::layout(&ctx).map(|paths| paths.data_dir().join("checkouts"));
    assert!(
        checkouts.is_some_and(|at| !at.exists()),
        "the tree written out is removed again, with the directory made for it"
    );
}

/// What the bundled plugin's adapter says of itself: the contracts it speaks and the
/// upstream release it fronts, as its manifest pins them.
const ABOUT: &str = r#"{"speaks":["media.serve@1","identity.source@1"],"upstream":"Jellyfin","releases":[{"version":"10.11.11","digest":"sha256:aefb67e6a7ff1debdd154a78a7bbb780fd0c873d8639210a7f6a2016ad2b35db"}]}"#;

/// A context over the shipped stack, which still runs the bundled plugin's own service,
/// with its server and adapter answering every proof it declares and published where
/// the engine says.
fn unmoved(name: &str, serving: &Arc<Serving>) -> Ctx {
    use lemonfiber_fixtures::http::Answer;
    let http = lemonfiber_fixtures::http::Fake::by_path(vec![
        (
            "/System/Info/Public",
            Answer::reply(
                200,
                r#"{"ProductName":"Jellyfin Server","Version":"10.11.11","StartupWizardCompleted":true}"#,
            ),
        ),
        ("/lemonfiber/adapter/v1/about", Answer::reply(200, ABOUT)),
        ("", Answer::reply(401, "")),
    ]);
    let mut ctx = served(name, serving)
        .with_http(http)
        .with_patience(std::time::Duration::ZERO);
    ctx.seams.engine = Arc::new(
        Reporting::holding(&[BUNDLED, ADAPTER], Lifecycle::Running, Health::Healthy)
            .publishing(&[(BUNDLED, "127.0.0.1", 8095), (ADAPTER, "127.0.0.1", 8080)]),
    );
    ctx.with_images(lemonfiber_fixtures::pulled::Pulled::holding(Vec::new()))
}

/// The bundled plugin's adapter.
const ADAPTER: &str = "jellyfin-adapter";

/// The plugin the bundle installs moves in place of the stack's own service of its id:
/// that service's name and ports hold nothing against it, and the plugin is recorded.
#[tokio::test]
async fn a_plugin_the_bundle_installs_moves_in_place_of_the_stack_service_it_replaces() {
    let serving = Arc::new(Serving::listing(""));
    let ctx = unmoved("embedded-held", &serving);

    let recorded = report(from_git(&ctx, BUNDLED).await)
        .and_then(|one| one.install)
        .map(|install| (install.would.plugin, install.recorded));

    assert_eq!(recorded, Some((BUNDLED.to_owned(), true)));
    assert_eq!(counted(reading(&ctx).await), Some(1));
}

/// An update of `service`, or of every service where absent, confirmed or only shown.
fn updating(service: Option<&str>, confirm: bool) -> crate::update::run::Asked {
    crate::update::run::Asked {
        service: service.map(str::to_owned),
        confirm,
        wait: crate::app::Waiting::Never,
        offer: None,
    }
}

/// A rehearsal of an update names every move the bundle offers, the media server's, each
/// download client's, the subtitle finder's, the listening server's, every curator's, both
/// indexer aggregators' and the request service's, and takes none.
#[tokio::test]
async fn a_rehearsal_of_an_update_names_every_move_on_offer() {
    let serving = Arc::new(Serving::listing(""));
    let ctx = unmoved("embedded-rehearsed", &serving);

    let read = crate::update::run::update(&ctx, updating(None, false))
        .await
        .ok();

    let mut offered: Vec<String> = read
        .iter()
        .flat_map(|report| report.moves.iter().map(|one| one.plugin.clone()))
        .collect();
    offered.sort();
    assert_eq!(
        offered,
        vec![
            "audiobookshelf".to_owned(),
            "bazarr".to_owned(),
            "bindery".to_owned(),
            "caddy".to_owned(),
            "calibre-web-automated".to_owned(),
            "door".to_owned(),
            "gluetun".to_owned(),
            "homepage".to_owned(),
            BUNDLED.to_owned(),
            "lidarr".to_owned(),
            "navidrome".to_owned(),
            "nzbhydra2".to_owned(),
            "prowlarr".to_owned(),
            "qbittorrent".to_owned(),
            "radarr".to_owned(),
            "recyclarr".to_owned(),
            "sabnzbd".to_owned(),
            "seerr".to_owned(),
            "sonarr".to_owned(),
            "unpackerr".to_owned()
        ]
    );
    assert_eq!(counted(reading(&ctx).await), Some(0));
}

/// A confirmed update of the moved service takes its move, answering the move's own
/// install offer, and leaves the other moves on offer; a rehearsal of it takes nothing.
#[tokio::test]
async fn a_confirmed_update_takes_the_move_and_a_rehearsal_only_names_it() {
    let serving = Arc::new(Serving::listing(""));
    let ctx = unmoved("embedded-update", &serving);

    let read = crate::update::run::update(&ctx, updating(Some(BUNDLED), false))
        .await
        .ok();
    assert_eq!(
        read.as_ref()
            .map(|report| (report.moves.len(), report.moved.len())),
        Some((20, 0))
    );
    assert_eq!(counted(reading(&ctx).await), Some(0));

    let confirmed = crate::update::run::update(&ctx, updating(Some(BUNDLED), true)).await;

    let report = confirmed.as_ref().ok();
    assert_eq!(
        report.map(|report| report.moved.clone()),
        Some(vec![crate::update::run::Moved {
            plugin: BUNDLED.to_owned(),
            service: BUNDLED.to_owned(),
            taken: true,
            why: None,
        }]),
        "{confirmed:?}"
    );
    assert_eq!(
        report.map(|report| report.moves.iter().map(|one| one.plugin.as_str()).collect()),
        Some(vec![
            "audiobookshelf",
            "bazarr",
            "bindery",
            "caddy",
            "calibre-web-automated",
            "door",
            "gluetun",
            "homepage",
            "lidarr",
            "navidrome",
            "nzbhydra2",
            "prowlarr",
            "qbittorrent",
            "radarr",
            "recyclarr",
            "sabnzbd",
            "seerr",
            "sonarr",
            "unpackerr"
        ])
    );
    assert_eq!(counted(reading(&ctx).await), Some(1));
}

/// A confirmed update narrowed to another service takes no move: every move stays on
/// offer and nothing is installed.
#[tokio::test]
async fn an_update_narrowed_to_another_service_leaves_the_move_on_offer() {
    let serving = Arc::new(Serving::listing(""));
    let ctx = unmoved("embedded-narrowed", &serving);

    let confirmed = crate::update::run::update(&ctx, updating(Some("flaresolverr"), true)).await;

    assert_eq!(
        confirmed
            .as_ref()
            .ok()
            .map(|report| (report.moves.len(), report.moved.len())),
        Some((20, 0)),
        "{confirmed:?}"
    );
    assert_eq!(counted(reading(&ctx).await), Some(0));
}

#[tokio::test]
async fn a_machine_not_set_up_has_nowhere_to_write_the_tree() {
    let ctx = a_context().build();

    assert_eq!(refusal(from_git(&ctx, BUNDLED).await), "CONFIG-3");
}

#[tokio::test]
async fn a_tree_that_cannot_be_written_out_installs_nothing() {
    let serving = Arc::new(Serving::listing(""));
    let ctx = moved("embedded-unwritten", &serving);
    let data = crate::app::targets::layout(&ctx).map(|paths| paths.data_dir().to_path_buf());
    let _ = data.map(|data| {
        let _ = std::fs::create_dir_all(&data);
        std::fs::write(data.join("checkouts"), "in the way")
    });

    assert_eq!(refusal(from_git(&ctx, BUNDLED).await), "PLUGIN-16");
    assert_eq!(counted(reading(&ctx).await), Some(0));
}

/// Every setting, secret and credential name the stack held for a part, each with its
/// value, as the secret store, the settings store and the environment file hold them.
fn names_held(ctx: &Ctx) -> Vec<(String, Option<String>)> {
    let env = ctx.settings.env_file.clone().unwrap_or_default();
    let secrets = crate::settings::secrets_at(ctx)
        .and_then(|at| crate::settings::Secrets::at(&at).ok())
        .unwrap_or_default();
    let file = crate::config::store::read(&env).unwrap_or_default();
    let mut held: Vec<(String, Option<String>)> = secrets
        .names()
        .map(|name| (name.to_owned(), secrets.value(name).map(str::to_owned)))
        .chain(
            file.keys()
                .into_iter()
                .map(|key| (key.to_owned(), file.get(key).map(str::to_owned))),
        )
        .collect();
    held.sort();
    held
}

/// Moving a part in place renames and migrates nothing: every name the stack held for it
/// is held after the move under the same name with the same value, and the move journals
/// no value moved from one name to another.
#[tokio::test]
async fn moving_a_part_in_place_renames_and_migrates_nothing() {
    let serving = Arc::new(Serving::listing(""));
    let ctx = unmoved("embedded-names", &serving);
    assert!(crate::app::targets::record_secret(
        &ctx,
        crate::config::IDENTITY_SOURCE_ADMIN_PASSWORD_KEY,
        "the-admins-password",
        crate::seed::OPERATION,
    )
    .is_ok());
    let credential = |ctx: &Ctx| {
        crate::app::targets::recorded_credential(ctx, BUNDLED, crate::config::ADMIN_PASSWORD_SUFFIX)
    };
    let before = names_held(&ctx);
    let journalled_before = journalled(&ctx).len();
    assert_eq!(credential(&ctx).as_deref(), Some("the-admins-password"));

    let confirmed = crate::update::run::update(&ctx, updating(Some(BUNDLED), true)).await;

    assert!(
        confirmed
            .as_ref()
            .is_ok_and(|report| report.moved.iter().all(|one| one.taken)),
        "{confirmed:?}"
    );
    let after = names_held(&ctx);
    assert!(
        before.iter().all(|one| after.contains(one)),
        "every name is held as it was"
    );
    assert_eq!(credential(&ctx).as_deref(), Some("the-admins-password"));
    let migrated: Vec<crate::journal::Kind> = journalled(&ctx)
        .into_iter()
        .skip(journalled_before)
        .map(|change| change.kind)
        .filter(|kind| {
            matches!(
                kind,
                crate::journal::Kind::Renamed { .. }
                    | crate::journal::Kind::Moved { .. }
                    | crate::journal::Kind::Setting { filed: Some(_), .. }
                    | crate::journal::Kind::Secret { filed: Some(_), .. }
            )
        })
        .collect();
    assert!(migrated.is_empty(), "{migrated:?}");
}
