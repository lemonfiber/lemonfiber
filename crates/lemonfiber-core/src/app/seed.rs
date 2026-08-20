//! Wiring the stack's services to each other, idempotently.
//!
//! One connection lemonfiber mints (qBittorrent's web UI password); the rest it
//! reads and writes. The orchestration lives here so [`super::dispatch`] stays a
//! table of one-line calls rather than carrying the whole graph.

use std::path::{Path, PathBuf};

use super::targets::{project_directory, target_for};
use super::{Ctx, Problem};
use crate::error::Diagnose;
use crate::ports::docker::LogQuery;

mod applications;
mod arrs;
mod baseline;
mod clients;
mod identity;
mod reset;

use applications::{seed_applications, skipped};
use arrs::{arr_download_clients, read_servarr_key, seed_arr, servarr_arrs, ArrSeeding};
use baseline::{
    escalate_broken_roots, load_baseline, save_baseline, wanted_roots, Loaded, DATA_ROOT,
    SCHEMA_VERSION_FIELD,
};
use clients::{
    category_for, download_clients, qbittorrent_target, read_sabnzbd_key, seed_qbittorrent_password,
};
use identity::seed_jellyfin_identity;
pub(super) use reset::reset_connections;

/// Wire the stack's services to each other, idempotently, and report what was
/// wired and what a re-run still owes.
///
/// One connection is unlike the rest: qBittorrent's web UI password, the
/// credential lemonfiber mints rather than reads — its temporary password is
/// read from the container's log, replaced with a generated one, and the
/// generated one recorded where the forwarded-port push reads it. The rest of
/// the graph reads a credential and writes a connection: each media-filing
/// \*arr's root folders, and its download clients (`SABnzbd` and qBittorrent).
/// Prowlarr's app sync registers each of those \*arrs back into Prowlarr, so it
/// pushes them indexers. It then makes Jellyfin the identity source for Seerr, so
/// the household signs in once. Bindery wiring lands next.
pub(super) async fn seed(ctx: &Ctx, adopt: bool) -> Result<crate::seed::Report, Problem> {
    let manifest = ctx
        .stack
        .checked_manifest(ctx.today())
        .map_err(|err| err.problem())?;

    let mut wirings = Vec::new();

    // qBittorrent's password, the one credential lemonfiber mints. Collecting the
    // optional target into a list wires it where the stack has it and does nothing
    // where it does not, without a branch a test could not reach. The generated
    // value is kept to register qBittorrent as a download client below.
    let mut qbittorrent_password = None;
    for target in qbittorrent_target(&manifest.services)
        .into_iter()
        .collect::<Vec<_>>()
    {
        let (wiring, generated) = seed_qbittorrent_password(ctx, &target).await;
        qbittorrent_password = generated.or(qbittorrent_password);
        wirings.push(wiring);
    }

    // A later run mints nothing — the temporary password is long gone — so the
    // value recorded on the run that minted it stands in. Without this an \*arr
    // that came up after the first seed would never learn about qBittorrent,
    // since its password cannot be read back from qBittorrent itself.
    let qbittorrent_password =
        qbittorrent_password.or_else(|| super::targets::recorded_qbittorrent_password(ctx));

    // Root folders and download clients for each \*arr that files media. The
    // download clients' own credentials are read once: SABnzbd's key from its
    // config, qBittorrent's the password minted or recorded above.
    let project = project_directory(&ctx.stack, ctx.settings.stack_dir.as_deref());
    let sabnzbd_key = read_sabnzbd_key(ctx, &manifest.services, project.as_deref()).await;
    // The host data root, read once, so each \*arr's root folders can be checked
    // against the filesystem they file into and a folder pointing nowhere raised as a
    // warning.
    let data_root = super::targets::data_root(ctx);
    let arrs = servarr_arrs(&manifest.services, project.as_deref());
    // A root folder one \*arr wants and another does too is contested: two \*arrs
    // on one folder would each rewrite the other's files, so it is refused rather
    // than wired. Detected across every \*arr up front, before any is wired.
    let root_claims: Vec<(&str, Vec<crate::ports::service::RootFolder>)> = arrs
        .iter()
        .map(|arr| (arr.target.name.as_str(), wanted_roots(&arr.media_types)))
        .collect();
    let contested = crate::seed::contested_roots(
        root_claims
            .iter()
            .map(|(name, roots)| (*name, roots.as_slice())),
    );
    // The expected-state baseline — what seeding last wrote into each service — is
    // loaded so this pass records against what earlier ones set, and saved once at
    // the end. Unlike the per-connection journal, it persists across runs: it is the
    // only memory of what lemonfiber wrote, which a later run reads to tell an
    // operator's edit from lemonfiber's own value.
    // The record may be genuinely absent (a first seed), read, or there but
    // unreadable — lost. A lost record cannot tell an operator's edit from
    // lemonfiber's own, so this pass cannot assess drift; rather than guess against an
    // empty baseline, it says so and offers re-baselining. The deliberate re-baseline
    // is `adopt`, which takes current state on as the new record, so an adopt pass
    // proceeds and re-forms it while an ordinary seed leaves the lost record untouched
    // rather than silently replacing it.
    let loaded = load_baseline(ctx);
    let lost = matches!(loaded, Loaded::Lost);
    let mut baseline = match loaded {
        Loaded::Formed(baseline) => baseline,
        Loaded::Fresh | Loaded::Lost => crate::baseline::Baseline::new(),
    };
    // Each \*arr's wiring is independent of the others, so the \*arrs are seeded at
    // once rather than in series: a pass's time then tracks the slowest \*arr, not
    // their sum. Each records what it wrote into its own baseline, read against the
    // loaded snapshot; the records are folded back into one below, and since a
    // field key carries the service, no two \*arrs collide.
    let seeding = ArrSeeding {
        contested: &contested,
        sabnzbd_key: sabnzbd_key.as_deref(),
        qbittorrent_password: qbittorrent_password.as_deref(),
        data_root: data_root.as_deref(),
        expected: &baseline,
        adopt,
    };
    let seeded =
        futures_util::future::join_all(arrs.iter().map(|arr| seed_arr(ctx, arr, &seeding))).await;
    for (arr_wirings, records) in seeded {
        wirings.extend(arr_wirings);
        baseline.merge(&records);
    }

    // Prowlarr's app sync: register each of those media-filing \*arrs back into
    // Prowlarr, so it pushes them its indexers. Bindery is left out here — it is
    // not one of Prowlarr's applications and is wired via Torznab instead.
    wirings.extend(seed_applications(ctx, &manifest.services, project.as_deref()).await);

    // Jellyfin as Seerr's identity source: one household account, not two.
    // Jellyfin has no key to read, so its admin password is minted and recorded
    // like qBittorrent's, then Seerr is pointed at it.
    wirings.extend(seed_jellyfin_identity(ctx, &manifest.services).await);

    // Persist what this pass recorded as the baseline a later run compares against —
    // unless the record was lost and this is not an adopt pass, in which case the
    // lost record is left as it is rather than silently replaced, and re-baselining is
    // left to the deliberate `adopt`.
    if !lost || adopt {
        save_baseline(ctx, &baseline);
    }

    let assessment = if lost && !adopt {
        crate::seed::Assessment::Unassessable
    } else {
        crate::seed::Assessment::Assessed
    };
    Ok(crate::seed::Report {
        wirings,
        assessment,
    })
}

/// The download-client wirings lemonfiber manages, as a caller that only reads them needs
/// them: each \*arr, the clients lemonfiber would write there, and what it last recorded
/// for each.
///
/// Here rather than where it is used, so the read-only half of drift and the writing half
/// gather their inputs the same way. A diagnosis that worked out the wanted clients for
/// itself would be a second opinion about what lemonfiber intends, and the two would drift
/// apart exactly where an operator most needs them not to.
///
/// Nothing where the baseline could not be read. A record that is there but unreadable
/// cannot tell an operator's edit from lemonfiber's own value, and reporting drift against
/// a baseline that is not there would call every wiring in the stack an edit.
pub(super) async fn managed_wirings(
    ctx: &Ctx,
    services: &[lemonfiber_manifest::Service],
    project: Option<&Path>,
) -> Vec<crate::doctor::wiring::Managed> {
    let Loaded::Formed(baseline) = load_baseline(ctx) else {
        return Vec::new();
    };
    let sabnzbd_key = read_sabnzbd_key(ctx, services, project).await;
    let qbittorrent_password = super::targets::recorded_qbittorrent_password(ctx);
    servarr_arrs(services, project)
        .into_iter()
        .map(|arr| {
            let clients = arr_download_clients(
                &arr,
                sabnzbd_key.as_deref(),
                qbittorrent_password.as_deref(),
            )
            .into_iter()
            .map(|want| crate::doctor::wiring::Wired {
                recorded: baseline
                    .entry(&arr.target.name, &crate::seed::client_field(&want))
                    .cloned(),
                want,
            })
            .collect();
            crate::doctor::wiring::Managed {
                target: arr.target,
                clients,
            }
        })
        .collect()
}

/// The temporary password qBittorrent announced in its log, if it has.
async fn read_temporary_password(ctx: &Ctx, service: &str) -> Option<String> {
    let mut lines = ctx
        .engine
        .logs(
            &ctx.settings.project,
            &[service.to_owned()],
            LogQuery::recent(TEMP_PASSWORD_LOG_LINES),
        )
        .await
        .ok()?;

    let mut log = String::new();
    while let Some(line) = lines.recv().await {
        log.push_str(&line.line);
        log.push('\n');
    }
    crate::qbittorrent::temporary_password(&log)
}

/// Record the generated password where the forwarded-port push reads it — the
/// `QBITTORRENT_PASSWORD` setting in the environment file. Best-effort: a value
/// that could not be written is reported by the push's own missing-password
/// message rather than failing the wiring that did land.
fn record_qbittorrent_password(ctx: &Ctx, password: &str) {
    super::targets::record_secret(ctx, crate::config::QBITTORRENT_PASSWORD_KEY, password);
}

/// How many lines back to read for qBittorrent's start-up announcement. Its
/// temporary password is printed once, early, so a generous tail finds it well
/// after start without pulling the whole log.
const TEMP_PASSWORD_LOG_LINES: u32 = 200;

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use super::applications::{application_kind, prowlarr_source, syncable_arrs};
    use super::arrs::servarr_arrs;
    use super::baseline::escalate_broken_roots;
    use super::clients::{category_for, download_clients, read_sabnzbd_key, sabnzbd_config_path};
    use crate::app::targets::{project_directory, recorded_qbittorrent_password, servarr_targets};
    use crate::app::{dispatch, Command, Ctx, Outcome};
    use crate::config::{store, Settings};
    use crate::model::VersionReport;
    use crate::platform::Environment;
    use crate::ports::docker::{Health, Lifecycle};
    use crate::ports::service::RootFolder;
    use crate::seed::{Severity, State, Wiring};
    use crate::stack::Source;
    use crate::test_support::{
        seeding, seeding_with, spoke, stack, FixedRandom, Reporting, Scripted, SeedFs,
    };
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
            depends_on: Vec::new(),
            capabilities: Vec::new(),
            host_managed: false,
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

    #[test]
    fn the_project_directory_is_the_external_path_or_the_materialise_target() {
        // Any directory does: what is under test is which path is chosen, not what is
        // in it. `adapters` is named because the crate cannot compile without it — the
        // previous choice was a directory that later moved to its own crate, which broke
        // this at a distance with an error naming neither.
        static EMBEDDED: include_dir::Dir<'_> =
            include_dir::include_dir!("$CARGO_MANIFEST_DIR/src/adapters");
        assert_eq!(
            project_directory(&Source::External(std::path::Path::new("/srv/stack")), None)
                .as_deref(),
            Some(std::path::Path::new("/srv/stack")),
            "an external stack is its own project root"
        );
        assert_eq!(
            project_directory(
                &Source::Embedded(&EMBEDDED),
                Some(std::path::Path::new("/opt/lemonfiber/stack"))
            )
            .as_deref(),
            Some(std::path::Path::new("/opt/lemonfiber/stack")),
            "an embedded stack's root is wherever it was materialised"
        );
        assert_eq!(
            project_directory(&Source::Embedded(&EMBEDDED), None),
            None,
            "an embedded stack materialised nowhere has no root to read from"
        );
    }

    #[test]
    fn only_reachable_servarr_services_with_a_config_path_become_targets() {
        let project = std::path::Path::new("/opt/lemonfiber/stack");
        let services = vec![
            manifest_service(
                "sonarr",
                Some(servarr_api(Some("/config/config.xml"))),
                Some(8989),
            ),
            manifest_service(
                "sabnzbd",
                Some(lemonfiber_manifest::Api {
                    kind: lemonfiber_manifest::ApiKind::Sabnzbd,
                    key_source: lemonfiber_manifest::KeySource::ConfigIni,
                    path: Some("/config/sabnzbd.ini".to_owned()),
                    version: None,
                }),
                Some(8080),
            ),
            manifest_service("jellyfin", None, Some(8096)),
            manifest_service(
                "radarr",
                Some(servarr_api(Some("/config/config.xml"))),
                None,
            ),
            manifest_service(
                "lidarr",
                Some(servarr_api(Some("/data/elsewhere.xml"))),
                Some(8686),
            ),
            manifest_service("prowlarr", Some(servarr_api(None)), Some(9696)),
        ];

        let targets = servarr_targets(&services, Some(project));

        assert_eq!(
            targets.len(),
            1,
            "only the reachable Servarr service qualifies"
        );
        let target = targets.first();
        assert!(
            target.is_some_and(|target| target.id == "sonarr"
                && target.base == "http://127.0.0.1:8989"
                && target.config == project.join("config/sonarr/config.xml")),
            "the key is read from where Compose mounts the service's config"
        );
    }

    #[test]
    fn a_sabnzbd_config_path_is_the_config_mount_of_the_one_sabnzbd_service() {
        let project = std::path::Path::new("/opt/lemonfiber/stack");
        let sabnzbd_api = |path: Option<&str>| lemonfiber_manifest::Api {
            kind: lemonfiber_manifest::ApiKind::Sabnzbd,
            key_source: lemonfiber_manifest::KeySource::ConfigIni,
            path: path.map(str::to_owned),
            version: None,
        };
        let services = vec![
            manifest_service("jellyfin", None, Some(8096)),
            manifest_service(
                "sonarr",
                Some(servarr_api(Some("/config/config.xml"))),
                Some(8989),
            ),
            manifest_service(
                "sabnzbd",
                Some(sabnzbd_api(Some("/config/sabnzbd.ini"))),
                Some(8080),
            ),
        ];

        assert_eq!(
            sabnzbd_config_path(&services, project),
            Some(project.join("config/sabnzbd/sabnzbd.ini")),
            "read from where Compose mounts SABnzbd's config"
        );
        assert!(
            sabnzbd_config_path(&[], project).is_none(),
            "no SABnzbd service, no path"
        );
        assert!(
            sabnzbd_config_path(
                &[manifest_service(
                    "sabnzbd",
                    Some(sabnzbd_api(None)),
                    Some(8080)
                )],
                project
            )
            .is_none(),
            "a SABnzbd that declares no config file"
        );
        assert!(
            sabnzbd_config_path(
                &[manifest_service(
                    "sabnzbd",
                    Some(sabnzbd_api(Some("/data/elsewhere.ini"))),
                    Some(8080)
                )],
                project
            )
            .is_none(),
            "a config path outside the /config mount"
        );
    }

    #[tokio::test]
    async fn a_sabnzbd_key_needs_a_project_and_a_sabnzbd_to_read() {
        let ctx = seed_ctx(None, true, Vec::new(), None, None);
        assert!(
            read_sabnzbd_key(&ctx, &[], None).await.is_none(),
            "without a project there is nowhere to read from"
        );
        assert!(
            read_sabnzbd_key(&ctx, &[], Some(std::path::Path::new("/srv/stack")))
                .await
                .is_none(),
            "without a SABnzbd service there is no key"
        );
    }

    #[test]
    fn nothing_can_be_proven_without_a_project_directory() {
        let services = vec![manifest_service(
            "sonarr",
            Some(servarr_api(Some("/config/config.xml"))),
            Some(8989),
        )];
        assert!(servarr_targets(&services, None).is_empty());
    }

    /// The seed report an outcome carried, if it was a seed outcome.
    fn seeded(outcome: Result<Outcome, super::Problem>) -> Option<crate::seed::Report> {
        match outcome {
            Ok(Outcome::Seed(report)) => Some(report),
            _ => None,
        }
    }

    /// Whether a wiring was skipped, on one line so it holds no phantom coverage.
    fn is_skipped(wiring: &crate::seed::Wiring) -> bool {
        matches!(wiring.state, crate::seed::State::Skipped { .. })
    }

    /// Whether a wiring failed, on one line so it holds no phantom coverage.
    fn is_failed(wiring: &crate::seed::Wiring) -> bool {
        matches!(wiring.state, crate::seed::State::Failed { .. })
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
        Ctx::new(
            Arc::new(Scripted(Ok(spoke("")))),
            Arc::new(engine),
            Arc::new(crate::adapters::System),
            Arc::new(crate::adapters::Disk),
            stack(),
            settings,
            Environment::MacOs,
        )
        .with_http(Fake::scripted(replies))
        .with_random(Arc::new(FixedRandom(bytes)))
    }

    /// The three replies a full password exchange expects: log in, set, confirm.
    fn exchange() -> Vec<(u16, &'static str)> {
        vec![(200, "Ok."), (200, ""), (200, "Ok.")]
    }

    /// The line qBittorrent logs its temporary password on.
    const TEMP_LOG: &str = "A temporary password is provided for this session: read-from-log";

    fn config_scratch(name: &str) -> std::path::PathBuf {
        let dir =
            std::env::temp_dir().join(format!("lemonfiber-app-{}-{name}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        dir.join(".env")
    }

    #[test]
    fn a_non_seed_outcome_carries_no_seed_report() {
        let version = Outcome::Version(VersionReport {
            binary: env!("CARGO_PKG_VERSION").to_owned(),
            supported_schema: vec![1],
            stack: "0.1.0".to_owned(),
            compose: None,
        });
        assert!(seeded(Ok(version)).is_none());
    }

    #[tokio::test]
    async fn seed_replaces_and_records_the_qbittorrent_password() {
        let env = config_scratch("seed-records");
        if let Some(parent) = env.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        let _ = std::fs::write(&env, "DATA_ROOT=/srv/media\n");
        let ctx = seed_ctx(
            Some(TEMP_LOG),
            true,
            exchange(),
            Some(vec![0x11; 24]),
            Some(env.clone()),
        );

        let outcome = dispatch(Command::Seed, &ctx).await;

        let json = outcome
            .as_ref()
            .ok()
            .and_then(|outcome| outcome.clone().envelope().to_json());
        assert!(json
            .as_deref()
            .is_some_and(|json| json.contains(r#""kind":"seed""#)));

        let report = seeded(outcome).unwrap_or_default();
        let wired = report
            .wirings
            .iter()
            .any(|wiring| wiring.state == crate::seed::State::Wired);
        assert!(wired, "the password is wired");

        let written = std::fs::read_to_string(&env).unwrap_or_default();
        assert!(written.contains("QBITTORRENT_PASSWORD="));
        let _ = std::fs::remove_dir_all(env.parent().unwrap_or(std::path::Path::new("/")));
    }

    #[tokio::test]
    async fn seed_sets_the_password_even_with_nowhere_to_record_it() {
        let ctx = seed_ctx(Some(TEMP_LOG), true, exchange(), Some(vec![0x11; 24]), None);
        let report = seeded(dispatch(Command::Seed, &ctx).await).unwrap_or_default();
        let wired = report
            .wirings
            .iter()
            .any(|wiring| wiring.state == crate::seed::State::Wired);
        assert!(wired, "the password is set even with nowhere to record it");
    }

    #[tokio::test]
    async fn the_baseline_persists_across_runs() {
        // The baseline is the one seed artifact kept between runs. A value an earlier
        // run recorded is pre-seeded here; two more passes — neither reaching a
        // service, so neither changing it — must load it, leave it, and save it back
        // unchanged, the round trip the drift policy is built to read.
        let env = config_scratch("baseline-across-runs");
        let baseline = env.with_file_name("baseline.json");
        let recorded =
            r#"{"services":{"sonarr":{"downloadclient:sabnzbd:8080":{"value":"tv","at":"1"}}}}"#;
        let _ = crate::config::store::write(&baseline, recorded);

        let first = seed_ctx(None, false, Vec::new(), None, Some(env.clone()));
        let _ = dispatch(Command::Seed, &first).await;
        let second = seed_ctx(None, false, Vec::new(), None, Some(env.clone()));
        let _ = dispatch(Command::Seed, &second).await;

        let read_back = std::fs::read_to_string(&baseline).unwrap_or_default();
        assert!(
            read_back.contains(r#""value":"tv""#) && read_back.contains(r#""at":"1""#),
            "the recorded value and its timestamp survive both passes unchanged: {read_back}"
        );
        let _ = std::fs::remove_dir_all(env.parent().unwrap_or(std::path::Path::new("/")));
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

    #[tokio::test]
    async fn a_wired_root_folder_the_host_cannot_back_is_a_warning() {
        // The *arr files into `/data/media/tv`, but the host directory it resolves to
        // is not there — the operator repointed the data root, or the media directory
        // was never made. The *arr imports into a void, so the folder is raised to a
        // warning naming the missing path.
        let filesystem = SeedFs::keyed(None, None).missing(vec!["media/tv"]);
        let wanted = [root("tv")];
        let mut wirings = vec![Wiring::settled(
            "tv root folder in Sonarr".to_owned(),
            State::Wired,
        )];
        escalate_broken_roots(
            &filesystem,
            Some(std::path::Path::new("/srv/media")),
            &wanted,
            &mut wirings,
        )
        .await;
        assert!(
            broken(&wirings).is_some_and(|breakage| breakage.contains("media/tv")),
            "the warning names the path that resolves to nothing"
        );
    }

    #[tokio::test]
    async fn a_wired_root_folder_backed_on_disk_stays_informational() {
        // The host directory is there, so the folder files where it should — nothing is
        // broken, and it stays the settled connection it is.
        let filesystem = SeedFs::keyed(None, None);
        let wanted = [root("tv")];
        let mut wirings = vec![Wiring::settled(
            "tv root folder in Sonarr".to_owned(),
            State::AlreadyWired,
        )];
        escalate_broken_roots(
            &filesystem,
            Some(std::path::Path::new("/srv/media")),
            &wanted,
            &mut wirings,
        )
        .await;
        assert!(broken(&wirings).is_none());
    }

    #[tokio::test]
    async fn a_root_folder_check_without_a_data_root_escalates_nothing() {
        // Without a data root the host path cannot be resolved, so nothing can be
        // confirmed or denied — the check escalates nothing rather than guessing.
        let filesystem = SeedFs::keyed(None, None).missing(vec!["media/tv"]);
        let wanted = [root("tv")];
        let mut wirings = vec![Wiring::settled(
            "tv root folder in Sonarr".to_owned(),
            State::Wired,
        )];
        escalate_broken_roots(&filesystem, None, &wanted, &mut wirings).await;
        assert!(broken(&wirings).is_none());
    }

    #[tokio::test]
    async fn a_root_folder_not_wired_is_not_warned_even_where_the_path_is_missing() {
        // A skipped folder is not one the *arr files into, so a missing path there is
        // not yet a break — only the folders the *arr actually holds are checked.
        let filesystem = SeedFs::keyed(None, None).missing(vec!["media/tv"]);
        let wanted = [root("tv")];
        let mut wirings = vec![Wiring::settled(
            "tv root folder in Sonarr".to_owned(),
            State::Skipped {
                reason: "later".to_owned(),
            },
        )];
        escalate_broken_roots(
            &filesystem,
            Some(std::path::Path::new("/srv/media")),
            &wanted,
            &mut wirings,
        )
        .await;
        assert!(broken(&wirings).is_none());
    }

    #[tokio::test]
    async fn a_reset_previews_then_reverts_a_drifted_connection() {
        const KEYED: &str = "<Config><ApiKey>the-key</ApiKey></Config>";
        let dir =
            std::env::temp_dir().join(format!("lemonfiber-reset-conn-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let _ = std::fs::create_dir_all(&dir);
        let env = dir.join(".env");
        // A recorded qBittorrent password makes a qBittorrent download client wanted; a
        // baseline recording lemonfiber's category for it, against the categoryless client
        // the service now reports, reads as the operator's drift to revert.
        let _ = std::fs::write(&env, "QBITTORRENT_PASSWORD=pw\n");
        let _ = crate::config::store::write(
            &dir.join("baseline.json"),
            r#"{"services":{"Sonarr":{"downloadclient:gluetun:8081":{"value":"tv","at":"1"}}}}"#,
        );

        let context = Ctx::new(
            Arc::new(Scripted(Ok(spoke("")))),
            Arc::new(Reporting::absent()),
            Arc::new(crate::adapters::System),
            Arc::new(crate::adapters::Disk),
            stack(),
            Settings {
                env_file: Some(env.clone()),
                ..Settings::default()
            },
            Environment::MacOs,
        )
        .with_filesystem(Arc::new(SeedFs::keyed(Some(KEYED), None)))
        .with_http(seeding());

        // Preview: the drifted connection is named, and nothing is written.
        let preview = super::reset_connections(&context, false).await;
        assert!(
            preview
                .iter()
                .any(|wiring| wiring.connection.contains("into Sonarr")),
            "the drifted connection is previewed"
        );

        // Confirm: it is reverted — the category written back in place, so it reads wired.
        let confirmed = super::reset_connections(&context, true).await;
        assert!(
            confirmed
                .iter()
                .any(|wiring| matches!(wiring.state, crate::seed::State::Wired)),
            "the drifted connection is reverted"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// The Servarr routing, but answering `system/status` with a set version and
    /// holding each download client under a drifted category — so a schema change
    /// (version bumped, every client moved) can be driven end to end.
    /// A seed run whose \*arrs report the given major version, and hold a second
    /// download client the ordinary routes do not.
    ///
    /// Three routes over the ordinary table rather than a transport of its own: the
    /// first match wins, so stating what differs is enough and the rest stays shared.
    fn versioned(version: &'static str) -> Arc<Fake> {
        seeding_with(vec![
            (
                "system/status",
                Answer::reply(
                    200,
                    match version {
                        "5" => r#"{"appName":"Sonarr","version":"5"}"#,
                        _ => r#"{"appName":"Sonarr","version":"4"}"#,
                    },
                ),
            ),
            ("/downloadclient/testall", Answer::reply(200, "[]")),
            (
                "/downloadclient",
                Answer::reply(
                    200,
                    r#"[{"id":1,"fields":[{"name":"host","value":"sabnzbd"},{"name":"port","value":8080},{"name":"tvCategory","value":"shows"}]},{"id":2,"fields":[{"name":"host","value":"gluetun"},{"name":"port","value":8081},{"name":"tvCategory","value":"shows"}]}]"#,
                ),
            ),
        ])
    }

    /// A transport answering the client list with `clients`, and success to everything
    /// else — the reset previews turn on what that one list says, including its refusal.
    fn clients_answering(clients: Answer) -> Arc<Fake> {
        Fake::by_path(vec![
            ("/downloadclient", clients),
            ("", Answer::reply(200, "Ok.")),
        ])
    }

    /// A context seeding the real stack over the given transport, with a
    /// qBittorrent password recorded and the given baseline written beside it.
    fn schema_ctx(dir: &std::path::Path, baseline: &str, http: Arc<Fake>) -> Ctx {
        const KEYED: &str = "<Config><ApiKey>the-key</ApiKey></Config>";
        let _ = std::fs::create_dir_all(dir);
        let env = dir.join(".env");
        let _ = std::fs::write(&env, "QBITTORRENT_PASSWORD=pw\n");
        let _ = crate::config::store::write(&dir.join("baseline.json"), baseline);
        Ctx::new(
            Arc::new(Scripted(Ok(spoke("")))),
            Arc::new(Reporting::absent()),
            Arc::new(crate::adapters::System),
            Arc::new(crate::adapters::Disk),
            stack(),
            Settings {
                env_file: Some(env),
                ..Settings::default()
            },
            Environment::MacOs,
        )
        .with_filesystem(Arc::new(SeedFs::keyed(Some(KEYED), None)))
        .with_http(http)
    }

    /// The state of the `qBittorrent into Sonarr` wiring in a seed report.
    fn qbittorrent_into_sonarr(report: &crate::seed::Report) -> Option<&crate::seed::State> {
        report
            .wirings
            .iter()
            .find(|wiring| wiring.connection == "qBittorrent into Sonarr")
            .map(|wiring| &wiring.state)
    }

    #[tokio::test]
    async fn a_schema_change_re_baselines_rather_than_reporting_mass_drift() {
        // Sonarr moved from version 4 to 5, and its one managed download client now
        // reads a different category — every managed value moved at once. That is the
        // upgrade renaming fields, not the operator editing each, so the current shape
        // is adopted as the new baseline and the wiring reads adopted, not drifted.
        let dir =
            std::env::temp_dir().join(format!("lemonfiber-schema-adopt-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let baseline = r#"{"services":{"Sonarr":{"schema:version":{"value":"4","at":"1"},"downloadclient:gluetun:8081":{"value":"tv","at":"1"}}}}"#;
        let ctx = schema_ctx(&dir, baseline, versioned("5"));

        let report = seeded(dispatch(Command::Seed, &ctx).await).unwrap_or_default();
        assert_eq!(
            qbittorrent_into_sonarr(&report),
            Some(&crate::seed::State::Adopted),
            "a schema change adopts the current shape rather than reporting drift"
        );
        // The new version is recorded, so the next run compares against it.
        let saved = std::fs::read_to_string(dir.join("baseline.json")).unwrap_or_default();
        assert!(
            saved.contains(r#""schema:version""#) && saved.contains(r#""value":"5""#),
            "the service's new version is recorded: {saved}"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[tokio::test]
    async fn a_version_change_with_only_some_drift_is_left_as_the_operators_edits() {
        // The version changed, but Sonarr never recorded this client — so it reads as
        // the operator's own, unmanaged, not as drift. Not every managed value moved,
        // so it is not a schema change: it is left as it is rather than re-baselined.
        let dir =
            std::env::temp_dir().join(format!("lemonfiber-schema-partial-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let baseline = r#"{"services":{"Sonarr":{"schema:version":{"value":"4","at":"1"}}}}"#;
        let ctx = schema_ctx(&dir, baseline, versioned("5"));

        let report = seeded(dispatch(Command::Seed, &ctx).await).unwrap_or_default();
        assert_eq!(
            qbittorrent_into_sonarr(&report),
            Some(&crate::seed::State::Unmanaged),
            "a version change alone does not re-baseline a value that did not wholesale-drift"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[tokio::test]
    async fn an_unchanged_version_leaves_a_drift_as_the_drift_it_is() {
        // Sonarr is on the version lemonfiber last recorded, so nothing upgraded — the
        // client that differs is the operator's edit, reported as drift and preserved,
        // not re-baselined.
        let dir =
            std::env::temp_dir().join(format!("lemonfiber-schema-same-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let baseline = r#"{"services":{"Sonarr":{"schema:version":{"value":"5","at":"1"},"downloadclient:gluetun:8081":{"value":"tv","at":"1"}}}}"#;
        let ctx = schema_ctx(&dir, baseline, versioned("5"));

        let report = seeded(dispatch(Command::Seed, &ctx).await).unwrap_or_default();
        assert_eq!(
            qbittorrent_into_sonarr(&report),
            Some(&crate::seed::State::Drifted),
            "an unchanged version leaves a drift as drift"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// A transport whose download-client list is set per test — a body to return, or
    /// nothing to fail the read — with every other call answered plainly. For the
    /// reset-connection edge cases, where what the service holds decides the preview.
    /// A context for the reset-connection edge cases: the real stack, a recorded
    /// qBittorrent password so a client is wanted, keys per `filesystem`, over `http`.
    fn reset_ctx(
        dir: &std::path::Path,
        filesystem: Arc<SeedFs>,
        http: Arc<dyn crate::ports::http::Http>,
    ) -> Ctx {
        let _ = std::fs::create_dir_all(dir);
        let env = dir.join(".env");
        let _ = std::fs::write(&env, "QBITTORRENT_PASSWORD=pw\n");
        Ctx::new(
            Arc::new(Scripted(Ok(spoke("")))),
            Arc::new(Reporting::absent()),
            Arc::new(crate::adapters::System),
            Arc::new(crate::adapters::Disk),
            stack(),
            Settings {
                env_file: Some(env),
                ..Settings::default()
            },
            Environment::MacOs,
        )
        .with_filesystem(filesystem)
        .with_http(http)
    }

    #[tokio::test]
    async fn a_reset_skips_an_arr_that_has_not_written_its_key() {
        // A client is wanted, but the \*arr's key is not readable — it has not finished
        // starting — so there is nothing to open and it is passed over rather than reset.
        let dir =
            std::env::temp_dir().join(format!("lemonfiber-reset-noopen-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let ctx = reset_ctx(&dir, Arc::new(SeedFs::keyed(None, None)), seeding());
        assert!(super::reset_connections(&ctx, false).await.is_empty());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[tokio::test]
    async fn a_reset_preview_passes_over_a_client_the_service_does_not_hold() {
        const KEYED: &str = "<Config><ApiKey>the-key</ApiKey></Config>";
        // The service holds none of the wanted clients, so there is nothing whose drift
        // to preview — each wanted one is passed over rather than reported.
        let dir =
            std::env::temp_dir().join(format!("lemonfiber-reset-absent-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let ctx = reset_ctx(
            &dir,
            Arc::new(SeedFs::keyed(Some(KEYED), None)),
            clients_answering(Answer::reply(200, "[]")),
        );
        assert!(super::reset_connections(&ctx, false).await.is_empty());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[tokio::test]
    async fn a_reset_preview_names_a_client_whose_category_the_operator_changed() {
        const KEYED: &str = "<Config><ApiKey>the-key</ApiKey></Config>";
        // The service holds the wanted client under a category the operator changed from
        // lemonfiber's recorded one — a drift the preview names as one a reset would
        // revert, reading the category the service now holds to judge it.
        let dir =
            std::env::temp_dir().join(format!("lemonfiber-reset-drift-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let held = r#"[{"id":2,"fields":[{"name":"host","value":"gluetun"},{"name":"port","value":8081},{"name":"tvCategory","value":"shows"}]}]"#;
        let ctx = reset_ctx(
            &dir,
            Arc::new(SeedFs::keyed(Some(KEYED), None)),
            clients_answering(Answer::reply(200, held)),
        );
        let _ = crate::config::store::write(
            &dir.join("baseline.json"),
            r#"{"services":{"Sonarr":{"downloadclient:gluetun:8081":{"value":"tv","at":"1"}}}}"#,
        );
        let preview = super::reset_connections(&ctx, false).await;
        assert!(
            preview
                .iter()
                .any(|wiring| wiring.connection.contains("into Sonarr")),
            "a category the operator changed is previewed as a revert"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[tokio::test]
    async fn a_reset_preview_reads_nothing_where_the_client_list_cannot_be_read() {
        const KEYED: &str = "<Config><ApiKey>the-key</ApiKey></Config>";
        // The service will not answer its client list, so the preview has nothing to
        // compare against and reports nothing rather than guessing.
        let dir =
            std::env::temp_dir().join(format!("lemonfiber-reset-unread-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let ctx = reset_ctx(
            &dir,
            Arc::new(SeedFs::keyed(Some(KEYED), None)),
            clients_answering(Answer::Silent),
        );
        assert!(super::reset_connections(&ctx, false).await.is_empty());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[tokio::test]
    async fn a_lost_baseline_is_reported_and_left_for_a_deliberate_re_baseline() {
        // The record is there but does not parse — lost. An ordinary seed cannot judge
        // drift against it, so it reports that drift could not be assessed and leaves
        // the record untouched rather than silently replacing it: re-baselining is the
        // deliberate act of `adopt`, not a side effect of a plain seed.
        let env = config_scratch("baseline-lost");
        let baseline = env.with_file_name("baseline.json");
        let corrupt = "this is not the baseline you are looking for";
        let _ = crate::config::store::write(&baseline, corrupt);

        let ctx = seed_ctx(None, false, Vec::new(), None, Some(env.clone()));
        let report = seeded(dispatch(Command::Seed, &ctx).await).unwrap_or_default();
        assert_eq!(report.assessment, crate::seed::Assessment::Unassessable);

        let read_back = std::fs::read_to_string(&baseline).unwrap_or_default();
        assert_eq!(
            read_back, corrupt,
            "a lost record is left untouched by a plain seed"
        );
        let _ = std::fs::remove_dir_all(env.parent().unwrap_or(std::path::Path::new("/")));
    }

    #[tokio::test]
    async fn an_adopt_pass_re_baselines_over_a_lost_record() {
        // Re-baselining is offered, and `adopt` is how it is taken: over a lost record
        // an adopt pass re-forms the baseline from current state, replacing the
        // unreadable file with a readable one, and assesses cleanly.
        let env = config_scratch("baseline-rebaseline");
        let baseline = env.with_file_name("baseline.json");
        let _ = crate::config::store::write(&baseline, "not parseable");

        let ctx = seed_ctx(None, false, Vec::new(), None, Some(env.clone()));
        let report = seeded(dispatch(Command::Adopt, &ctx).await).unwrap_or_default();
        assert_eq!(report.assessment, crate::seed::Assessment::Assessed);

        let read_back = std::fs::read_to_string(&baseline).unwrap_or_default();
        assert!(
            serde_json::from_str::<crate::baseline::Baseline>(&read_back).is_ok(),
            "an adopt pass re-forms the lost record into a readable one: {read_back}"
        );
        let _ = std::fs::remove_dir_all(env.parent().unwrap_or(std::path::Path::new("/")));
    }

    #[tokio::test]
    async fn a_baseline_whose_file_cannot_be_read_is_a_loss() {
        // Not every loss is a parse failure: a record whose file cannot even be opened
        // — here a directory standing where the file should be — is a loss too, told
        // apart from a first seed's genuinely absent file.
        let env = config_scratch("baseline-unreadable");
        let baseline = env.with_file_name("baseline.json");
        let _ = std::fs::create_dir_all(&baseline);

        let ctx = seed_ctx(None, false, Vec::new(), None, Some(env.clone()));
        let report = seeded(dispatch(Command::Seed, &ctx).await).unwrap_or_default();
        assert_eq!(report.assessment, crate::seed::Assessment::Unassessable);
        let _ = std::fs::remove_dir_all(env.parent().unwrap_or(std::path::Path::new("/")));
    }

    #[tokio::test]
    async fn seed_reports_an_unreadable_stack_rather_than_guessing() {
        let nowhere = Source::External(std::path::Path::new("/lemonfiber/no/such/stack"));
        let ctx = Ctx::new(
            Arc::new(Scripted(Ok(spoke("")))),
            Arc::new(Reporting::default()),
            Arc::new(crate::adapters::System),
            Arc::new(crate::adapters::Disk),
            nowhere,
            Settings::default(),
            Environment::MacOs,
        );
        let outcome = dispatch(Command::Seed, &ctx).await;
        assert_eq!(
            outcome.err().map(|problem| problem.code),
            Some(crate::stack::STACK_UNREADABLE)
        );
    }

    #[tokio::test]
    async fn seed_reports_a_failed_password_change_and_records_nothing() {
        // The temporary password is read but the client rejects it, so the change
        // fails and there is no generated value to record.
        let ctx = seed_ctx(
            Some(TEMP_LOG),
            true,
            vec![(200, "Fails.")],
            Some(vec![0x11; 24]),
            None,
        );
        let report = seeded(dispatch(Command::Seed, &ctx).await).unwrap_or_default();
        let failed = report.wirings.iter().any(is_failed);
        assert!(failed, "a rejected change is reported as failed");
    }

    #[tokio::test]
    async fn seed_skips_qbittorrent_when_no_password_is_announced() {
        let ctx = seed_ctx(None, true, Vec::new(), Some(vec![0x11; 24]), None);
        let report = seeded(dispatch(Command::Seed, &ctx).await).unwrap_or_default();
        assert!(
            !report.wirings.is_empty(),
            "the run produced wirings to judge, not an empty report from an error"
        );
        let all_skipped = report.wirings.iter().all(is_skipped);
        assert!(
            all_skipped,
            "an unannounced password is skipped, not failed"
        );
    }

    #[tokio::test]
    async fn seed_skips_qbittorrent_when_its_log_cannot_be_read() {
        let ctx = seed_ctx(None, false, Vec::new(), Some(vec![0x11; 24]), None);
        let report = seeded(dispatch(Command::Seed, &ctx).await).unwrap_or_default();
        assert!(
            !report.wirings.is_empty(),
            "the run produced wirings to judge, not an empty report from an error"
        );
        let all_skipped = report.wirings.iter().all(is_skipped);
        assert!(all_skipped, "an unreadable log is skipped, not failed");
    }

    /// The wirings whose connection names a root folder.
    fn root_folder_wirings(report: &crate::seed::Report) -> Vec<&crate::seed::Wiring> {
        report
            .wirings
            .iter()
            .filter(|wiring| wiring.connection.contains("root folder"))
            .collect()
    }

    #[tokio::test]
    async fn seed_wires_each_arrs_root_folders() {
        // Each application already holds the folders, so each is left as wired. Each
        // \*arr also reads its version for the schema check first; that read decodes a
        // folder list as no status, so a spare reply per \*arr covers it and the
        // version is simply not learned — the folders are what this test reads.
        const FOLDERS: &str = r#"[{"id":1,"path":"/data/media/tv"},{"id":2,"path":"/data/media/movies"},{"id":3,"path":"/data/media/music"}]"#;
        const KEYED: &str = "<Config><ApiKey>the-key</ApiKey></Config>";
        let ctx = seed_ctx(
            None,
            true,
            vec![(200, FOLDERS); 6],
            Some(vec![0x11; 24]),
            None,
        )
        .with_filesystem(Arc::new(SeedFs::keyed(Some(KEYED), None)));

        let report = seeded(dispatch(Command::Seed, &ctx).await).unwrap_or_default();
        let folders = root_folder_wirings(&report);
        assert_eq!(folders.len(), 3, "one root folder per media-filing arr");
        let all_wired = folders
            .iter()
            .all(|wiring| wiring.state == crate::seed::State::AlreadyWired);
        assert!(all_wired, "a folder already present is left wired");
    }

    #[tokio::test]
    async fn seed_skips_arr_root_folders_when_the_key_is_not_readable() {
        // No configuration to read a key from, so the arrs have not finished
        // starting: their folders are skipped for a re-run, not failed.
        let ctx = seed_ctx(None, true, Vec::new(), Some(vec![0x11; 24]), None)
            .with_filesystem(Arc::new(SeedFs::keyed(None, None)));

        let report = seeded(dispatch(Command::Seed, &ctx).await).unwrap_or_default();
        let folders = root_folder_wirings(&report);
        assert_eq!(folders.len(), 3);
        assert!(folders.iter().all(|wiring| is_skipped(wiring)));
    }

    #[test]
    fn no_project_directory_means_no_arrs_to_wire() {
        assert!(servarr_arrs(&[], None).is_empty());
    }

    #[tokio::test]
    async fn the_read_only_filesystem_fake_is_inert_elsewhere() {
        // The fake answers only `read`; the rest are stubs, exercised here so the
        // fake carries no uncovered lines of its own.
        use crate::ports::filesystem::FileSystem;
        let fs = SeedFs::keyed(None, None);
        let path = std::path::Path::new("/x");
        assert!(fs.canonicalize(path).await.is_ok());
        assert!(fs.touch(path).await.is_err());
        assert!(fs.link(path, path).await.is_err());
        assert!(fs.identify(path).await.is_err());
        fs.remove(path).await;
        assert!(fs.read(path).await.is_none());
        fs.write(path, "unused").await;
        assert!(fs.ownership(path).await.is_none());
        let _ = fs.describe(path).await;
    }

    #[test]
    fn a_category_is_named_by_the_media_the_application_files() {
        assert_eq!(
            category_for("tv").map(|category| category.field),
            Some("tvCategory".to_owned())
        );
        assert_eq!(
            category_for("movies").map(|category| category.field),
            Some("movieCategory".to_owned())
        );
        assert_eq!(
            category_for("music").map(|category| category.field),
            Some("musicCategory".to_owned())
        );
        assert!(category_for("comics").is_none());
    }

    #[test]
    fn a_download_client_is_built_for_each_credential_in_hand() {
        let category = crate::ports::service::Category {
            field: "tvCategory".to_owned(),
            value: "tv".to_owned(),
        };
        assert_eq!(download_clients(Some("k"), Some("p"), &category).len(), 2);
        assert_eq!(download_clients(Some("k"), None, &category).len(), 1);
        assert_eq!(download_clients(None, Some("p"), &category).len(), 1);
        assert!(download_clients(None, None, &category).is_empty());
    }

    #[test]
    fn a_recorded_qbittorrent_password_is_read_back_or_read_as_absent() {
        // Nowhere to read from.
        let ctx = seed_ctx(None, true, Vec::new(), None, None);
        assert!(recorded_qbittorrent_password(&ctx).is_none());

        let path = config_scratch("qbt-readback");
        let ctx = seed_ctx(None, true, Vec::new(), None, Some(path.clone()));
        // A file that holds no password of ours.
        let _ = store::set(&path, "SOMETHING_ELSE", "x");
        assert!(
            recorded_qbittorrent_password(&ctx).is_none(),
            "no password recorded"
        );
        // An empty value is not a password.
        let _ = store::set(&path, crate::config::QBITTORRENT_PASSWORD_KEY, "");
        assert!(
            recorded_qbittorrent_password(&ctx).is_none(),
            "an empty value is absent"
        );
        // The value recorded on an earlier run is handed back.
        let _ = store::set(
            &path,
            crate::config::QBITTORRENT_PASSWORD_KEY,
            "minted-earlier",
        );
        assert_eq!(
            recorded_qbittorrent_password(&ctx).as_deref(),
            Some("minted-earlier")
        );
    }

    /// The wirings whose connection registers a download client into an arr.
    fn download_client_wirings(report: &crate::seed::Report) -> Vec<&crate::seed::Wiring> {
        report
            .wirings
            .iter()
            .filter(|wiring| wiring.connection.contains("into "))
            .collect()
    }

    #[tokio::test]
    async fn seed_leaves_each_arrs_already_present_download_clients() {
        // qBittorrent announces a temporary password, so it is set and its value
        // threaded to the download clients; each arr already holds both clients.
        const SERVARR: &str = "<Config><ApiKey>the-key</ApiKey></Config>";
        const SABNZBD: &str = "[misc]\napi_key = the-sab-key\n";
        let ctx = seed_ctx(Some(TEMP_LOG), true, Vec::new(), Some(vec![0x11; 24]), None)
            .with_http(seeding())
            .with_filesystem(Arc::new(SeedFs::keyed(Some(SERVARR), Some(SABNZBD))));

        let report = seeded(dispatch(Command::Seed, &ctx).await).unwrap_or_default();
        let clients = download_client_wirings(&report);
        assert_eq!(
            clients.len(),
            6,
            "SABnzbd and qBittorrent into each of three arrs"
        );
        // Each client is already registered at its endpoint, so none is written a
        // second time. The service reports no category for them and there is no
        // baseline, so the three-way comparison cannot prove they are lemonfiber's
        // own value: it takes each as the operator's own, pre-existing and unmanaged,
        // and leaves it rather than overwriting it. The point the test guards is that
        // a present client is left, never duplicated.
        let none_rewired = clients
            .iter()
            .all(|wiring| wiring.state == crate::seed::State::Unmanaged);
        assert!(none_rewired, "a present client is left, not re-registered");
    }

    #[tokio::test]
    async fn adopt_runs_the_wiring_and_reports_each_present_client() {
        // The adopt command runs the same wiring as a seed. The mock's present clients
        // report no category and there is no baseline, so each is unmanaged — and with
        // no value to take on, an adopt pass reports it unmanaged just as a seed does,
        // never registering it a second time. The point guarded here is that the adopt
        // command dispatches and reports every present client.
        const SERVARR: &str = "<Config><ApiKey>the-key</ApiKey></Config>";
        const SABNZBD: &str = "[misc]\napi_key = the-sab-key\n";
        let ctx = seed_ctx(Some(TEMP_LOG), true, Vec::new(), Some(vec![0x22; 24]), None)
            .with_http(seeding())
            .with_filesystem(Arc::new(SeedFs::keyed(Some(SERVARR), Some(SABNZBD))));

        let report = seeded(dispatch(Command::Adopt, &ctx).await).unwrap_or_default();
        let clients = download_client_wirings(&report);
        assert_eq!(
            clients.len(),
            6,
            "SABnzbd and qBittorrent into each of three arrs"
        );
        let all_reported = clients
            .iter()
            .all(|wiring| wiring.state == crate::seed::State::Unmanaged);
        assert!(all_reported, "an adopt pass reports each present client");
    }

    #[tokio::test]
    async fn seed_skips_download_clients_when_the_arr_key_is_not_readable() {
        // The clients' own credentials are in hand, but the arrs have not written
        // their keys, so registration is skipped for a re-run rather than failed.
        const SABNZBD: &str = "[misc]\napi_key = the-sab-key\n";
        let ctx = seed_ctx(Some(TEMP_LOG), true, Vec::new(), Some(vec![0x11; 24]), None)
            .with_http(seeding())
            .with_filesystem(Arc::new(SeedFs::keyed(None, Some(SABNZBD))));

        let report = seeded(dispatch(Command::Seed, &ctx).await).unwrap_or_default();
        let clients = download_client_wirings(&report);
        assert_eq!(clients.len(), 6);
        assert!(clients.iter().all(|wiring| is_skipped(wiring)));
    }

    #[tokio::test]
    async fn a_later_seed_offers_qbittorrent_from_its_recorded_password() {
        // The temporary password is gone, so nothing is minted this run; the
        // password recorded earlier stands in and qBittorrent is offered anyway.
        const SERVARR: &str = "<Config><ApiKey>the-key</ApiKey></Config>";
        const SABNZBD: &str = "[misc]\napi_key = the-sab-key\n";
        let path = config_scratch("qbt-later-seed");
        let _ = store::set(
            &path,
            crate::config::QBITTORRENT_PASSWORD_KEY,
            "minted-earlier",
        );
        let ctx = seed_ctx(None, true, Vec::new(), None, Some(path))
            .with_http(seeding())
            .with_filesystem(Arc::new(SeedFs::keyed(Some(SERVARR), Some(SABNZBD))));

        let report = seeded(dispatch(Command::Seed, &ctx).await).unwrap_or_default();
        let clients = download_client_wirings(&report);
        assert_eq!(clients.len(), 6, "both clients into each of three arrs");
        let qbittorrent = clients
            .iter()
            .filter(|wiring| wiring.connection.starts_with("qBittorrent into "))
            .count();
        assert_eq!(
            qbittorrent, 3,
            "qBittorrent is offered to every arr on a later run"
        );
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

    /// The wirings whose connection registers an \*arr into Prowlarr's app sync.
    fn application_wirings(report: &crate::seed::Report) -> Vec<&crate::seed::Wiring> {
        report
            .wirings
            .iter()
            .filter(|wiring| wiring.connection.contains("indexer sync via"))
            .collect()
    }

    #[test]
    fn the_application_kind_follows_from_the_media() {
        use crate::ports::service::ApplicationKind;
        assert_eq!(
            application_kind(&["tv".to_owned()]),
            Some(ApplicationKind::Sonarr)
        );
        assert_eq!(
            application_kind(&["movies".to_owned()]),
            Some(ApplicationKind::Radarr)
        );
        assert_eq!(
            application_kind(&["music".to_owned()]),
            Some(ApplicationKind::Lidarr)
        );
        // Bindery files books but is not one of Prowlarr's applications.
        assert!(application_kind(&["books".to_owned()]).is_none());
        // A service that files no media is not an application at all.
        assert!(application_kind(&[]).is_none());
    }

    #[test]
    fn prowlarr_is_the_servarr_service_that_files_no_media() {
        let project = std::path::Path::new("/opt/lemonfiber/stack");
        // A media-filing *arr is never the source, however reachable.
        assert!(prowlarr_source(&[arr("sonarr", 8989, "tv")], Some(project)).is_none());
        // The Servarr service with no media is, known on the network by its own
        // container name and port.
        let source = prowlarr_source(&[prowlarr()], Some(project));
        assert!(source
            .is_some_and(|source| source.network_url == "http://prowlarr:9696"
                && source.target.id == "prowlarr"));
        // Without a project there is nowhere to read a key from.
        assert!(prowlarr_source(&[prowlarr()], None).is_none());
    }

    #[test]
    fn no_project_directory_means_no_arrs_to_sync() {
        assert!(syncable_arrs(&[arr("sonarr", 8989, "tv")], None).is_empty());
        let project = std::path::Path::new("/opt/lemonfiber/stack");
        let arrs = syncable_arrs(
            &[arr("sonarr", 8989, "tv"), arr("radarr", 7878, "movies")],
            Some(project),
        );
        assert_eq!(arrs.len(), 2, "each media-filing arr is syncable");
        assert!(arrs
            .iter()
            .any(|arr| arr.network_url == "http://sonarr:8989"));
    }

    #[tokio::test]
    async fn app_sync_does_nothing_where_the_stack_has_no_prowlarr() {
        let ctx = seed_ctx(None, true, Vec::new(), None, None);
        let project = std::path::Path::new("/opt/lemonfiber/stack");
        // Only a media-filing arr, so there is no app-sync source at all.
        let wirings =
            super::seed_applications(&ctx, &[arr("sonarr", 8989, "tv")], Some(project)).await;
        assert!(wirings.is_empty(), "no Prowlarr, no app sync");
    }

    #[tokio::test]
    async fn app_sync_skips_every_arr_until_prowlarr_has_written_its_key() {
        let project = std::path::Path::new("/opt/lemonfiber/stack");
        let services = vec![prowlarr(), arr("sonarr", 8989, "tv")];
        // Prowlarr's key is not readable yet, so it is still starting: every
        // application is skipped for a re-run rather than failed.
        let ctx = seed_ctx(None, true, Vec::new(), None, None)
            .with_filesystem(Arc::new(SeedFs::keyed(None, None)));
        let wirings = super::seed_applications(&ctx, &services, Some(project)).await;
        assert_eq!(wirings.len(), 1);
        assert!(wirings.iter().all(is_skipped));
    }

    #[tokio::test]
    async fn app_sync_skips_only_the_arr_that_has_not_written_its_key() {
        const SERVARR: &str = "<Config><ApiKey>the-key</ApiKey></Config>";
        let project = std::path::Path::new("/opt/lemonfiber/stack");
        let services = vec![prowlarr(), arr("sonarr", 8989, "tv")];
        // Prowlarr's key is readable but Sonarr's is not — Sonarr came up after
        // Prowlarr — so Sonarr's application waits while Prowlarr itself proceeds.
        let ctx = seed_ctx(None, true, Vec::new(), None, None)
            .with_http(seeding())
            .with_filesystem(Arc::new(
                SeedFs::keyed(Some(SERVARR), None).only_for_prowlarr(),
            ));
        let wirings = super::seed_applications(&ctx, &services, Some(project)).await;
        assert_eq!(wirings.len(), 1);
        assert!(wirings.iter().all(is_skipped));
    }

    #[tokio::test]
    async fn app_sync_registers_an_arr_whose_keys_are_all_readable() {
        const SERVARR: &str = "<Config><ApiKey>the-key</ApiKey></Config>";
        let project = std::path::Path::new("/opt/lemonfiber/stack");
        let services = vec![prowlarr(), arr("sonarr", 8989, "tv")];
        // The seeding routes report Sonarr already registered — its baseUrl is in the
        // application list — so the connection reads back as already wired.
        let ctx = seed_ctx(None, true, Vec::new(), None, None)
            .with_http(seeding())
            .with_filesystem(Arc::new(SeedFs::keyed(Some(SERVARR), None)));
        let wirings = super::seed_applications(&ctx, &services, Some(project)).await;
        assert_eq!(wirings.len(), 1);
        assert_eq!(
            wirings.first().map(|wiring| &wiring.state),
            Some(&crate::seed::State::AlreadyWired)
        );
    }

    /// A Prowlarr transport that starts with no applications, captures the POST
    /// that registers one, and reports it on the next read — so the orchestrator's
    /// write path runs end to end rather than short-circuiting to already-wired.
    /// Prowlarr holding no applications until one is written, then holding it.
    ///
    /// The registration is a write followed by a read that has to see it, so the read
    /// answers an empty list and then the list with Sonarr in it. What was posted is
    /// read off the transport's own record rather than a captured copy.
    fn registering_prowlarr() -> Arc<Fake> {
        Fake::by_route_in_turn(vec![
            (Method::Post, "", vec![Answer::reply(201, "")]),
            (
                Method::Get,
                "",
                vec![
                    Answer::reply(200, "[]"),
                    Answer::reply(
                        200,
                        r#"[{"id":9,"fields":[{"name":"baseUrl","value":"http://sonarr:8989"}]}]"#,
                    ),
                ],
            ),
        ])
    }

    #[tokio::test]
    async fn app_sync_registers_an_absent_arr_and_reads_it_back() {
        const SERVARR: &str = "<Config><ApiKey>the-key</ApiKey></Config>";
        let project = std::path::Path::new("/opt/lemonfiber/stack");
        let services = vec![prowlarr(), arr("sonarr", 8989, "tv")];
        // Prowlarr holds no applications, so Sonarr is genuinely written and then
        // read back — the write path a pre-populated list would hide.
        let http = registering_prowlarr();
        let ctx = seed_ctx(None, true, Vec::new(), None, None)
            .with_http(http.clone())
            .with_filesystem(Arc::new(SeedFs::keyed(Some(SERVARR), None)));

        let wirings = super::seed_applications(&ctx, &services, Some(project)).await;
        assert_eq!(wirings.len(), 1);
        assert_eq!(
            wirings.first().map(|wiring| &wiring.state),
            Some(&crate::seed::State::Wired),
            "an absent application is written and confirmed by read-back"
        );

        // The orchestrator built the registration for the right *arr, reaching it
        // and Prowlarr on the stack network, and posted it to Prowlarr's v1 API.
        let posted = http
            .requests()
            .into_iter()
            .find(|request| request.method == Method::Post);
        assert!(posted
            .as_ref()
            .is_some_and(|request| request.url.ends_with("/api/v1/applications")));
        let body = posted.and_then(|request| request.body).unwrap_or_default();
        assert!(
            body.contains("http://sonarr:8989"),
            "the *arr's address: {body}"
        );
        assert!(body.contains(r#""implementation":"Sonarr""#), "{body}");
        assert!(
            body.contains("http://prowlarr:9696"),
            "Prowlarr's callback url: {body}"
        );
    }

    #[tokio::test]
    async fn seed_registers_each_arr_into_prowlarr() {
        // The whole command against the real manifest: Prowlarr and the three
        // media-filing arrs, each already registered by the seeding routes.
        const SERVARR: &str = "<Config><ApiKey>the-key</ApiKey></Config>";
        const SABNZBD: &str = "[misc]\napi_key = the-sab-key\n";
        let ctx = seed_ctx(Some(TEMP_LOG), true, Vec::new(), Some(vec![0x11; 24]), None)
            .with_http(seeding())
            .with_filesystem(Arc::new(SeedFs::keyed(Some(SERVARR), Some(SABNZBD))));

        let report = seeded(dispatch(Command::Seed, &ctx).await).unwrap_or_default();
        let applications = application_wirings(&report);
        assert_eq!(
            applications.len(),
            3,
            "Sonarr, Radarr and Lidarr each registered into Prowlarr"
        );
        assert!(applications
            .iter()
            .all(|wiring| wiring.state == crate::seed::State::AlreadyWired));
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
            ("", initialised),
        ])
    }

    #[tokio::test]
    async fn identity_does_nothing_without_both_jellyfin_and_seerr() {
        let ctx = seed_ctx(None, true, Vec::new(), None, None);
        // Seerr present but no Jellyfin, and the other way round: either alone is
        // nothing to wire.
        assert!(super::seed_jellyfin_identity(&ctx, &[seerr_svc()])
            .await
            .is_empty());
        assert!(super::seed_jellyfin_identity(&ctx, &[jellyfin_svc()])
            .await
            .is_empty());
    }

    #[tokio::test]
    async fn identity_leaves_an_already_set_up_household_alone() {
        let env = config_scratch("jellyfin-already");
        if let Some(parent) = env.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        // Jellyfin's password was recorded on an earlier run, and both services are
        // already set up: nothing is minted and nothing re-pointed.
        let _ = store::set(
            &env,
            crate::config::JELLYFIN_ADMIN_PASSWORD_KEY,
            "minted-earlier",
        );
        let ctx = seed_ctx(None, true, Vec::new(), None, Some(env.clone()))
            .with_http(household(true, true));

        let wirings = super::seed_jellyfin_identity(&ctx, &[jellyfin_svc(), seerr_svc()]).await;
        assert_eq!(wirings.len(), 1);
        assert_eq!(
            wirings.first().map(|wiring| &wiring.state),
            Some(&crate::seed::State::AlreadyWired)
        );
        let _ = std::fs::remove_dir_all(env.parent().unwrap_or(std::path::Path::new("/")));
    }

    #[tokio::test]
    async fn identity_mints_records_and_wires_a_fresh_household() {
        let env = config_scratch("jellyfin-fresh");
        if let Some(parent) = env.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        let _ = std::fs::write(&env, "DATA_ROOT=/srv/media\n");
        let ctx = seed_ctx(
            None,
            true,
            Vec::new(),
            Some(vec![0x11; 24]),
            Some(env.clone()),
        )
        .with_http(household(false, false));

        let wirings = super::seed_jellyfin_identity(&ctx, &[jellyfin_svc(), seerr_svc()]).await;
        assert_eq!(wirings.len(), 1);
        assert_eq!(
            wirings.first().map(|wiring| &wiring.state),
            Some(&crate::seed::State::Wired),
            "a fresh household is minted, signed in, and confirmed"
        );
        let written = std::fs::read_to_string(&env).unwrap_or_default();
        assert!(
            written.contains("JELLYFIN_ADMIN_PASSWORD="),
            "the minted password is recorded: {written}"
        );
        let _ = std::fs::remove_dir_all(env.parent().unwrap_or(std::path::Path::new("/")));
    }

    #[tokio::test]
    async fn seed_wires_jellyfin_as_seerrs_identity() {
        // The whole command against the real manifest, which has both services;
        // Jellyfin reports its wizard done and no password was recorded, so the
        // household set it up and the identity is skipped for them to complete.
        let ctx = seed_ctx(None, true, Vec::new(), None, None).with_http(household(true, false));
        let report = seeded(dispatch(Command::Seed, &ctx).await).unwrap_or_default();
        let identity = report
            .wirings
            .iter()
            .find(|wiring| wiring.connection.contains("Seerr's identity"));
        assert!(
            identity.is_some_and(is_skipped),
            "an externally set-up Jellyfin leaves the identity for the household"
        );
    }

    #[test]
    fn a_target_carries_the_servarr_api_version() {
        let project = std::path::Path::new("/opt/lemonfiber/stack");
        // Sonarr answers at v3, Lidarr at v1: the version travels with the target
        // rather than being assumed by the client.
        let sonarr = manifest_service(
            "sonarr",
            Some(servarr_api_at(Some("/config/config.xml"), Some(3))),
            Some(8989),
        );
        assert_eq!(
            super::target_for(&sonarr, project).map(|target| target.version),
            Some(3)
        );
        let lidarr = manifest_service(
            "lidarr",
            Some(servarr_api_at(Some("/config/config.xml"), Some(1))),
            Some(8686),
        );
        assert_eq!(
            super::target_for(&lidarr, project).map(|target| target.version),
            Some(1)
        );
        // A servarr service that names no version cannot be reached at a known
        // path, so it is no target rather than one guessed at the wrong version.
        let versionless = manifest_service(
            "sonarr",
            Some(servarr_api_at(Some("/config/config.xml"), None)),
            Some(8989),
        );
        assert!(super::target_for(&versionless, project).is_none());
    }
}
