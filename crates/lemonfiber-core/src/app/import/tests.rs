use std::sync::Arc;

use lemonfiber_fixtures::http::{Answer, Fake};
use lemonfiber_fixtures::serving::Serves;
use lemonfiber_ports::docker::{Container, Health, Lifecycle, Published};

use super::{carry, ACCOUNTED, NOT_FIRST_PARTY, UNCONFIGURED, UNRECORDED};
use crate::config::Settings;
use crate::model::{ImportReport, MigrationReport, OccupantReport, StandingReport};
use crate::test_support::curators::TELEVISION;
use crate::test_support::{a_context, SeedFs};

/// The project their stack runs under.
const THEIR_PROJECT: &str = "media";

/// The port their copy of the television curator publishes on the host.
const THEIR_PORT: u16 = 18_989;

/// The port the engine says a lent adapter is published on.
const PUBLISHED: u16 = 41_234;

/// What a survey finds: their stack, running a copy of the television curator.
fn survey() -> MigrationReport {
    MigrationReport {
        read: true,
        standing: vec![StandingReport {
            project: THEIR_PROJECT.to_owned(),
            services: vec![OccupantReport {
                service: "sonarr".to_owned(),
                running: true,
                ports: vec![THEIR_PORT],
                adoptable: true,
            }],
        }],
        ..MigrationReport::default()
    }
}

/// Their copy of the television curator, published on the host and mounting its own
/// configuration.
fn theirs() -> Container {
    Container {
        id: "theirs".to_owned(),
        project: THEIR_PROJECT.to_owned(),
        service: "sonarr".to_owned(),
        lifecycle: Lifecycle::Running,
        health: Health::Healthy,
        published: vec![Published {
            address: std::net::IpAddr::from([0, 0, 0, 0]),
            port: THEIR_PORT,
            private: 8989,
        }],
        mounts: vec![std::path::PathBuf::from("/their/sonarr")],
        exit: None,
    }
}

/// A machine running their stack beside lemonfiber's, whose television curator has moved
/// into its plugin, their copy's key on disk, and an engine that serves as `serves` does.
fn importing(name: &str, serves: &Arc<Serves>) -> crate::app::Ctx {
    let dir = lemonfiber_fixtures::scratch::Scratch::named(&format!("import-{name}")).kept();
    let mut ctx = a_context()
        .settings(Settings {
            env_file: Some(dir.join(".env")),
            ..Settings::default()
        })
        .build()
        .with_filesystem(Arc::new(SeedFs::keyed(Some(
            "<Config><ApiKey>their-key</ApiKey></Config>",
        ))))
        .with_http(Fake::by_path(vec![(
            "/lemonfiber/adapter/v1/ready",
            Answer::reply(200, "true"),
        )]));
    ctx.seams.serving = Arc::clone(serves) as Arc<dyn lemonfiber_ports::serving::Serving>;
    let ctx = TELEVISION.reaching(ctx);
    TELEVISION.kept(&ctx);
    ctx
}

/// What an unconfirmed import comes to in `ctx`, `trusted` holding the plugins that are
/// first-party.
async fn imported(
    ctx: &crate::app::Ctx,
    trusted: &[crate::plugin::first_party::FirstParty],
) -> ImportReport {
    imported_from(ctx, theirs(), trusted).await
}

/// What an unconfirmed import of `running` comes to in `ctx`, `trusted` holding the
/// plugins that are first-party.
async fn imported_from(
    ctx: &crate::app::Ctx,
    running: Container,
    trusted: &[crate::plugin::first_party::FirstParty],
) -> ImportReport {
    carry(ctx, &survey(), &[running], false, trusted)
        .await
        .unwrap_or_default()
}

/// Whether `report` says sonarr was not carried for `why`.
fn refused_for(report: &ImportReport, why: &str) -> bool {
    report
        .not_carried
        .iter()
        .any(|one| one.what == "sonarr" && one.because.ends_with(why))
}

/// The record of what is installed in `ctx`, with the television curator's plugin as
/// `shape` leaves it.
fn reshaped(ctx: &crate::app::Ctx, shape: fn(&mut crate::plugin::Installed)) {
    let Some(at) = ctx
        .settings
        .env_file
        .as_deref()
        .map(|env| env.with_file_name(crate::config::paths::PLUGINS))
    else {
        return;
    };
    let mut register = std::fs::read_to_string(&at)
        .ok()
        .and_then(|text| crate::plugin::Register::parse(&text).ok())
        .unwrap_or_else(crate::plugin::Register::empty);
    register.forget(TELEVISION.plugin);
    for installed in TELEVISION.installed().into_iter().map(|mut one| {
        shape(&mut one);
        one
    }) {
        let _ = register.record(installed);
    }
    let _ = std::fs::write(&at, register.to_json().unwrap_or_default());
}

/// **Their copy's key is handed only to a first-party plugin's adapter.** A plugin this
/// build does not hold as first-party is refused before their key is read, and nothing is
/// started, so the key never reaches it — not even on an unconfirmed import.
#[tokio::test]
async fn a_plugin_that_is_not_first_party_is_never_lent_their_key() {
    let serves = Arc::new(Serves::on(PUBLISHED));
    let ctx = importing("untrusted", &serves);

    let report = imported(&ctx, &[]).await;

    assert!(serves.asked().is_empty(), "{:?}", serves.asked());
    assert!(
        report
            .not_carried
            .iter()
            .any(|one| one.what == "sonarr" && one.because.ends_with(NOT_FIRST_PARTY)),
        "{report:?}"
    );
}

/// The first-party plugin's adapter is lent to their copy, pointed at where this machine
/// reaches it, and taken away again.
#[tokio::test]
async fn the_first_party_plugins_adapter_is_lent_to_their_copy() {
    let serves = Arc::new(Serves::on(PUBLISHED));
    let ctx = importing("trusted", &serves);

    let report = imported(&ctx, crate::plugin::first_party::EMBEDDED).await;

    let lent: Vec<Vec<String>> = serves
        .asked()
        .into_iter()
        .map(|asked| asked.command)
        .collect();
    assert_eq!(
        lent,
        vec![vec![
            "--upstream".to_owned(),
            format!("http://theirs:{THEIR_PORT}")
        ]],
        "{report:?}"
    );
    assert_eq!(serves.removed().len(), 1, "{report:?}");
    assert!(
        !report
            .not_carried
            .iter()
            .any(|one| one.because.ends_with(NOT_FIRST_PARTY)),
        "{report:?}"
    );
}

/// A stack that cannot be read now has no services to look up, so nothing is carried and
/// nothing is lent.
#[tokio::test]
async fn a_stack_that_cannot_be_read_carries_nothing() {
    let serves = Arc::new(Serves::on(PUBLISHED));
    let mut ctx = importing("unread", &serves);
    ctx.stack = crate::stack::Source::External(std::path::Path::new("/lemonfiber/no/such/stack"));

    let report = imported(&ctx, crate::plugin::first_party::EMBEDDED).await;

    assert!(report.refusal.is_some(), "{report:?}");
    assert!(serves.asked().is_empty());
}

/// lemonfiber's own copy whose adapter the engine is not running cannot be asked over its
/// contract, so nothing is carried into it and nothing is lent.
#[tokio::test]
async fn an_own_copy_that_cannot_be_asked_carries_nothing_into_it() {
    let serves = Arc::new(Serves::on(PUBLISHED));
    let mut ctx = importing("unasked", &serves);
    ctx.seams.engine = Arc::new(crate::test_support::Reporting::absent());

    let report = imported(&ctx, crate::plugin::first_party::EMBEDDED).await;

    assert!(
        refused_for(
            &report,
            "lemonfiber's own copy could not be asked over its contract"
        ),
        "{report:?}"
    );
    assert!(serves.asked().is_empty());
}

/// An own copy whose plugin the record of what is installed no longer holds has no
/// adapter to lend.
#[tokio::test]
async fn an_own_copy_whose_plugin_is_not_recorded_lends_nothing() {
    let serves = Arc::new(Serves::on(PUBLISHED));
    let ctx = importing("unrecorded", &serves);
    reshaped(&ctx, |installed| installed.plugin = "elsewhere".to_owned());

    let report = imported(&ctx, crate::plugin::first_party::EMBEDDED).await;

    assert!(refused_for(&report, UNRECORDED), "{report:?}");
    assert!(serves.asked().is_empty());
}

/// An adapter that signs in with an account is lent no key in its place.
#[tokio::test]
async fn an_adapter_that_signs_in_with_an_account_is_not_lent() {
    let serves = Arc::new(Serves::on(PUBLISHED));
    let ctx = importing("accounted", &serves);
    reshaped(&ctx, |installed| {
        for placed in &mut installed.services {
            if placed.fronts.is_some() {
                placed.upstream.insert(
                    lemonfiber_contract::adapter::UPSTREAM_USERNAME.to_owned(),
                    "account".to_owned(),
                );
            }
        }
    });

    let report = imported(&ctx, crate::plugin::first_party::EMBEDDED).await;

    assert!(refused_for(&report, ACCOUNTED), "{report:?}");
    assert!(serves.asked().is_empty());
}

/// Their copy mounting nothing has no configuration to read its key from.
#[tokio::test]
async fn their_copy_mounting_nothing_is_not_read() {
    let serves = Arc::new(Serves::on(PUBLISHED));
    let ctx = importing("unmounted", &serves);

    let report = imported_from(
        &ctx,
        Container {
            mounts: Vec::new(),
            ..theirs()
        },
        crate::plugin::first_party::EMBEDDED,
    )
    .await;

    assert!(refused_for(&report, UNCONFIGURED), "{report:?}");
    assert!(serves.asked().is_empty());
}

/// A service lemonfiber does not run is passed over: nothing is lent to it and nothing is
/// said about it.
#[tokio::test]
async fn a_service_lemonfiber_does_not_run_is_passed_over() {
    let serves = Arc::new(Serves::on(PUBLISHED));
    let ctx = importing("not-ours", &serves);

    let report = imported_from(
        &ctx,
        Container {
            service: "ombi".to_owned(),
            ..theirs()
        },
        crate::plugin::first_party::EMBEDDED,
    )
    .await;

    assert!(report.not_carried.is_empty(), "{report:?}");
    assert!(serves.asked().is_empty());
}

/// Their copy answering on this machine's loopback alone is one a lent adapter cannot
/// reach.
#[tokio::test]
async fn their_copy_on_loopback_alone_is_not_lent_to() {
    let serves = Arc::new(Serves::on(PUBLISHED));
    let ctx = importing("loopback", &serves);

    let report = imported_from(
        &ctx,
        Container {
            published: vec![Published {
                address: std::net::IpAddr::from([127, 0, 0, 1]),
                port: THEIR_PORT,
                private: 8989,
            }],
            ..theirs()
        },
        crate::plugin::first_party::EMBEDDED,
    )
    .await;

    assert!(
        refused_for(
            &report,
            "it answers on no address but this machine's loopback, which a container cannot reach"
        ),
        "{report:?}"
    );
    assert!(serves.asked().is_empty());
}

/// Their copy that has written no key is not read.
#[tokio::test]
async fn their_copy_with_no_key_written_is_not_read() {
    let serves = Arc::new(Serves::on(PUBLISHED));
    let ctx = importing("keyless", &serves).with_filesystem(Arc::new(SeedFs::keyed(None)));

    let report = imported(&ctx, crate::plugin::first_party::EMBEDDED).await;

    assert!(
        refused_for(
            &report,
            "their copy has written no key lemonfiber could read"
        ),
        "{report:?}"
    );
    assert!(serves.asked().is_empty());
}

/// An adapter the engine will not serve is said to have not started.
#[tokio::test]
async fn an_adapter_the_engine_will_not_serve_is_said_not_to_have_started() {
    let serves = Arc::new(Serves::refusing(
        lemonfiber_ports::program::Unran::Unreachable {
            reason: "no engine".to_owned(),
        },
    ));
    let ctx = importing("unserved", &serves);

    let report = imported(&ctx, crate::plugin::first_party::EMBEDDED).await;

    assert!(
        report.not_carried.iter().any(|one| one.what == "sonarr"
            && one
                .because
                .contains("the adapter lent to their copy did not start")),
        "{report:?}"
    );
}

#[test]
fn a_service_that_declares_no_media_has_only_its_indexers_carried() {
    assert_eq!(
        super::kinds_for(None),
        [lemonfiber_ports::service::Record::Indexer]
    );
}
