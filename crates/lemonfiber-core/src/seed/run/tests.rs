use crate::ports::filesystem::Storage;
use std::sync::Arc;

use super::applications::{application_kind, prowlarr_source, syncable_arrs};
use super::arrs::servarr_arrs;
use super::baseline::escalate_broken_roots;
use super::clients::{category_for, download_clients, read_sabnzbd_key, sabnzbd_config_path};
use super::withheld;
use crate::app::targets::{project_directory, recorded_qbittorrent_password, servarr_targets};
use crate::app::{dispatch, Command, Ctx, Outcome};
use crate::config::{store, Settings};
use crate::model::VersionReport;
use crate::ports::docker::{Health, Lifecycle};
use crate::ports::service::RootFolder;
use crate::seed::{Severity, State, Wiring};
use crate::stack::Source;
use crate::test_support::{a_context, seeding, seeding_with, FixedRandom, Reporting, SeedFs};
use lemonfiber_fixtures::http::{Answer, Fake};
use lemonfiber_ports::http::Method;

/// A manifest service with the few fields the credential resolver reads, and
/// filler for the rest, so a test can vary the shape, port and key file.
fn manifest_service(
    id: &str,
    api: Option<lemonfiber_manifest::Api>,
    port: Option<u16>,
) -> lemonfiber_manifest::Service {
    lemonfiber_manifest::Service {
        id: id.to_owned(),
        name: format!("{id} the app"),
        profile: "media".to_owned(),
        image: "example/image".to_owned(),
        tag: "1".to_owned(),
        port,
        bind: None,
        health: None,
        api,
        criticality: lemonfiber_manifest::Criticality::Core,
        license: "MIT".to_owned(),
        upstream: "https://example.test".to_owned(),
        last_release: "2026-01-01".to_owned(),
        describes: "an example service".to_owned(),
        without_it: "nothing works".to_owned(),
        media_types: Vec::new(),
        provides: Vec::new(),
        depends_on: Vec::new(),
        grants: Vec::new(),
        host_managed: false,
        memory_mib: None,
        asks_for: None,
        reaches: None,
    }
}

/// A Servarr-shape API declaration naming the given key file, or none, at the
/// v3 most tests need; the version-specific tests set their own.
fn servarr_api(path: Option<&str>) -> lemonfiber_manifest::Api {
    servarr_api_at(path, Some(3))
}

/// The same, at a given API version — `None` to omit it entirely.
fn servarr_api_at(path: Option<&str>, version: Option<u32>) -> lemonfiber_manifest::Api {
    lemonfiber_manifest::Api {
        kind: lemonfiber_manifest::ApiKind::Servarr,
        key_source: lemonfiber_manifest::KeySource::ConfigXml,
        path: path.map(str::to_owned),
        version,
    }
}

/// The seed report an outcome carried, if it was a seed outcome.
fn seeded(outcome: Result<Outcome, Box<super::Problem>>) -> Option<crate::seed::Report> {
    match outcome {
        Ok(Outcome::Seed(report)) => Some(report),
        _ => None,
    }
}

/// Whether a wiring was skipped, on one line so it holds no phantom coverage.
fn is_skipped(wiring: &crate::seed::Wiring) -> bool {
    matches!(wiring.state, crate::seed::State::Skipped { .. })
}

/// A context whose engine says the given qBittorrent log line, answering
/// seeding's HTTP from `replies` and its randomness from `bytes`.
fn seed_ctx(
    log: Option<&str>,
    reachable: bool,
    replies: Vec<(u16, &'static str)>,
    bytes: Option<Vec<u8>>,
    env: Option<std::path::PathBuf>,
) -> Ctx {
    let mut engine = if reachable {
        Reporting::holding(&["qbittorrent"], Lifecycle::Running, Health::Healthy)
    } else {
        Reporting::absent()
    };
    if let Some(line) = log {
        engine = engine.saying("qbittorrent", line);
    }
    let settings = Settings {
        env_file: env,
        ..Settings::default()
    };
    a_context()
        .engine(Arc::new(engine))
        .settings(settings)
        .build()
        .with_http(Fake::scripted(replies))
        .with_random(Arc::new(FixedRandom(bytes)))
}

/// The line qBittorrent logs its temporary password on.
const TEMP_LOG: &str = "A temporary password is provided for this session: read-from-log";

fn config_scratch(name: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!("lemonfiber-app-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    dir.join(".env")
}

/// A wanted root folder for a media type — the container path `wanted_roots` builds.
fn root(media: &str) -> RootFolder {
    RootFolder {
        path: format!("/data/media/{media}"),
        media_type: media.to_owned(),
    }
}

/// The breakage the first wiring's warning names, or nothing where it is
/// informational or absent — the one severity reader, so both arms are exercised
/// across the escalation tests rather than left dead in either.
fn broken(wirings: &[Wiring]) -> Option<String> {
    match wirings.first().map(|wiring| &wiring.severity) {
        Some(Severity::Warning { breakage, .. }) => Some(breakage.clone()),
        Some(Severity::Informational) | None => None,
    }
}

/// The wirings whose connection registers a download client into an arr.
fn download_client_wirings(report: &crate::seed::Report) -> Vec<&crate::seed::Wiring> {
    report
        .wirings
        .iter()
        .filter(|wiring| wiring.connection.contains("into "))
        .collect()
}

// ---- Prowlarr app sync: register each media-filing arr back into Prowlarr. ----

/// A media-filing \*arr as a manifest service, with the media that makes it
/// syncable — `manifest_service` alone leaves the media empty, which is what
/// marks Prowlarr.
fn arr(id: &str, port: u16, media: &str) -> lemonfiber_manifest::Service {
    let mut service = manifest_service(
        id,
        Some(servarr_api(Some("/config/config.xml"))),
        Some(port),
    );
    service.media_types = vec![media.to_owned()];
    service
}

/// Prowlarr as a manifest service: a Servarr shape that files no media.
fn prowlarr() -> lemonfiber_manifest::Service {
    manifest_service(
        "prowlarr",
        Some(servarr_api(Some("/config/config.xml"))),
        Some(9696),
    )
}

/// What one ask resolves to, as a pass hands it to the connection that asked.
///
/// These fixtures declare services rather than whole stacks, so the resolution is
/// supplied the way the pass supplies it rather than settled again here: what a
/// connection does with an answer is what these are about, and settling it twice
/// would be testing the reader instead.
fn filling(capability: &str, service: &str) -> std::collections::BTreeMap<String, Vec<String>> {
    std::collections::BTreeMap::from([(capability.to_owned(), vec![service.to_owned()])])
}

/// The stack's indexer ask, as the shipped manifest settles it.
fn searched() -> std::collections::BTreeMap<String, Vec<String>> {
    filling("indexer.search", "prowlarr")
}

/// The stack's identity ask, as the shipped manifest settles it.
fn identified() -> std::collections::BTreeMap<String, Vec<String>> {
    filling("identity.source", "jellyfin")
}

// ---- Jellyfin as Seerr's identity: two services and a minted credential. ----

/// A Seerr-shape service declaration.
fn seerr_api() -> lemonfiber_manifest::Api {
    lemonfiber_manifest::Api {
        kind: lemonfiber_manifest::ApiKind::Seerr,
        key_source: lemonfiber_manifest::KeySource::ApiSettings,
        path: None,
        version: None,
    }
}

fn seerr_svc() -> lemonfiber_manifest::Service {
    manifest_service("seerr", Some(seerr_api()), Some(5055))
}

fn jellyfin_api() -> lemonfiber_manifest::Api {
    lemonfiber_manifest::Api {
        kind: lemonfiber_manifest::ApiKind::Jellyfin,
        key_source: lemonfiber_manifest::KeySource::Generated,
        path: None,
        version: None,
    }
}

fn jellyfin_svc() -> lemonfiber_manifest::Service {
    manifest_service("jellyfin", Some(jellyfin_api()), Some(8096))
}

/// A transport standing in for the household pair, routed by path: Jellyfin's
/// public info reports whether its wizard has run, its `/Startup/*` calls
/// succeed, Seerr's sign-in flips it to initialised, and its public settings
/// report that state.
/// A household that answers Jellyfin's and Seerr's setup reads.
///
/// `completed` is what Jellyfin says about its own wizard. `signed_in` is whether
/// Seerr is already initialised: where it is not, the catch-all answers "no" and then
/// "yes", which is the read-write-read the identity wiring performs. Scripting the
/// change in order rather than flipping a flag says which write is meant to cause it.
fn household(completed: bool, signed_in: bool) -> Arc<Fake> {
    let initialised = if signed_in {
        vec![Answer::reply(200, r#"{"initialized":true}"#)]
    } else {
        vec![
            Answer::reply(200, r#"{"initialized":false}"#),
            Answer::reply(200, r#"{"initialized":true}"#),
        ]
    };
    Fake::by_path_in_turn(vec![
        (
            "/System/Info/Public",
            vec![Answer::reply(
                200,
                format!(r#"{{"StartupWizardCompleted":{completed}}}"#),
            )],
        ),
        // Jellyfin's setup calls and Seerr's sign-in succeed, but neither by
        // itself finishes Seerr's setup.
        ("/Startup/", vec![Answer::reply(200, "")]),
        ("/auth/jellyfin", vec![Answer::reply(200, "")]),
        ("/settings/initialize", vec![Answer::reply(200, "")]),
        // Untouched by anybody: what a service that has never had the agent
        // configured answers, which is the case the telling must write into.
        (
            "/settings/notifications/webpush",
            vec![
                Answer::reply(200, r#"{"enabled":false,"types":0}"#),
                Answer::reply(200, ""),
            ],
        ),
        ("", initialised),
    ])
}

/// The request service, declaring the settings file it writes its key to.
fn seerr_with_settings() -> lemonfiber_manifest::Service {
    manifest_service(
        "seerr",
        Some(lemonfiber_manifest::Api {
            kind: lemonfiber_manifest::ApiKind::Seerr,
            key_source: lemonfiber_manifest::KeySource::ConfigJson,
            path: Some("/app/config/settings.json".to_owned()),
            version: None,
        }),
        Some(5055),
    )
}

/// The book \*arr, as a manifest service whose key lemonfiber mints for it.
fn bindery_svc() -> lemonfiber_manifest::Service {
    manifest_service(
        "bindery",
        Some(lemonfiber_manifest::Api {
            kind: lemonfiber_manifest::ApiKind::Bindery,
            key_source: lemonfiber_manifest::KeySource::Generated,
            path: None,
            version: None,
        }),
        Some(8787),
    )
}

/// A scratch settings file holding the media server's recorded password, so a
/// client built from it can sign in.
fn recorded_admin(name: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!("lemonfiber-seed-{}-{name}", std::process::id()));
    let _ = std::fs::create_dir_all(&dir);
    let env = dir.join(".env");
    let _ = crate::config::store::set(
        &env,
        crate::config::JELLYFIN_ADMIN_PASSWORD_KEY,
        "minted-earlier",
    );
    env
}

/// The subtitle finder as a manifest service: a key in a YAML of its own.
fn bazarr_svc() -> lemonfiber_manifest::Service {
    manifest_service(
        "bazarr",
        Some(lemonfiber_manifest::Api {
            kind: lemonfiber_manifest::ApiKind::Bazarr,
            key_source: lemonfiber_manifest::KeySource::ConfigYaml,
            path: Some("/config/config/config.yaml".to_owned()),
            version: None,
        }),
        Some(6767),
    )
}

/// The finder's configuration as it writes it — its own key under `auth`, and
/// one under each \*arr it has been pointed at.
const FINDER_CONFIG: &str = "auth:\n  apikey: finder-key\nsonarr:\n  apikey: someone-elses\n";

fn stack_root() -> &'static std::path::Path {
    std::path::Path::new("/opt/lemonfiber/stack")
}

/// Nothing declared takes nothing out and says nothing, which is every machine
/// where nobody has written anything down.
#[test]
fn nothing_declared_leaves_every_service_in_the_pass() {
    let mut services = crate::test_support::stack()
        .manifest()
        .map(|manifest| manifest.services)
        .unwrap_or_default();
    let counted = services.len();
    // Said rather than left to the assertions below, which an empty list satisfies
    // while proving nothing: no service was taken out of a pass that held none.
    assert!(counted > 0, "the embedded stack declares services");

    assert!(withheld(&mut services, &[]).is_empty());
    assert_eq!(services.len(), counted);
}

mod aggregators;
mod applications;
mod arrs;
mod baseline;
mod identity;
mod passwords;
mod publishing;
mod requests;
mod subtitles;
mod targets;
mod unmanaged;
