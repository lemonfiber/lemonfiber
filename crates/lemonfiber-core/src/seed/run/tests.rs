use crate::ports::filesystem::Storage;
use std::sync::Arc;

use super::applications::application_kind;
use super::baseline::escalate_broken_roots;
use super::clients::{category_for, Held};
use super::curating::curators;
use super::{withheld, withheld_brought};
use crate::app::targets::{project_directory, recorded_secret};
use crate::app::{dispatch, Command, Ctx, Outcome};
use crate::config::{store, Settings};
use crate::model::VersionReport;
use crate::ports::docker::{Health, Lifecycle};
use crate::ports::service::{Credential, RootFolder};
use crate::seed::{Severity, State, Wiring};
use crate::stack::Source;
use crate::test_support::curators::curated;
use crate::test_support::indexer::INDEXER;
use crate::test_support::requests::REQUESTS;
use crate::test_support::{
    a_context, contracted, first_party, seeding, seeding_with, FixedRandom, Reporting, SeedFs,
};
use lemonfiber_fixtures::http::{Answer, Fake};

/// A manifest service with the few fields the seed passes read, and filler for the rest,
/// answering on `port` where it publishes one.
pub(super) fn manifest_service(id: &str, port: Option<u16>) -> lemonfiber_manifest::Service {
    lemonfiber_manifest::Service {
        id: id.to_owned(),
        name: format!("{id} the app"),
        profile: "media".to_owned(),
        image: "example/image".to_owned(),
        tag: "1".to_owned(),
        digest: None,
        port,
        bind: None,
        health: None,
        criticality: lemonfiber_manifest::Criticality::Core,
        license: "MIT".to_owned(),
        upstream: "https://example.test".to_owned(),
        last_release: "2026-01-01".to_owned(),
        describes: "an example service".to_owned(),
        without_it: "nothing works".to_owned(),
        dashboard: lemonfiber_manifest::Dashboard::default(),
        media_types: Vec::new(),
        provides: Vec::new(),
        claim: Vec::new(),
        secret: Vec::new(),
        depends_on: Vec::new(),
        grants: Vec::new(),
        host_managed: false,
        memory_mib: None,
        mem_limit_mib: None,
        publish: Vec::new(),
        takes_data: false,
        config_path: None,
        config_ro: false,
        mount: Vec::new(),
        tmpfs: Vec::new(),
        env: std::collections::BTreeMap::new(),
        user_mode: None,
        posture: lemonfiber_manifest::container::Posture::Standard,
        network: None,
        shape: None,
        asks_for: None,
        reaches: None,
        listens: port,
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

/// A context whose engine runs the media server's and the request service's adapters where
/// they are `reachable`, each keyed wherever its configuration is, answering no HTTP and
/// its randomness from `bytes`.
fn seed_ctx(reachable: bool, bytes: Option<Vec<u8>>, env: Option<std::path::PathBuf>) -> Ctx {
    let engine = if reachable {
        crate::test_support::media::running().alongside(REQUESTS.running())
    } else {
        Reporting::absent()
    };
    let settings = Settings {
        env_file: env,
        ..Settings::default()
    };
    let mut ctx = a_context()
        .engine(Arc::new(engine))
        .settings(settings)
        .build()
        .with_http(Fake::silent())
        .with_random(Arc::new(FixedRandom(bytes)));
    ctx.seams.confined = crate::test_support::media::Keyed::holding(
        Arc::clone(&ctx.seams.confined),
        &[crate::test_support::media::ADAPTER, REQUESTS.adapter],
    );
    ctx
}

/// Each secret `installed` mints, kept in `ctx`'s secret store holding what `value` says
/// for the part of an adapter's upstream credential it holds, or for nothing where it
/// holds none.
fn keeping_minted(
    ctx: &Ctx,
    installed: Option<crate::plugin::Installed>,
    value: fn(Option<&str>) -> &'static str,
) {
    let Some(installed) = installed else {
        return;
    };
    let part = |secret: &str| {
        installed.services.iter().find_map(|placed| {
            placed
                .upstream
                .iter()
                .find(|(_, held)| held.as_str() == secret)
                .map(|(part, _)| part.as_str())
        })
    };
    let minted: Vec<(String, &str)> = installed
        .declared
        .secrets
        .iter()
        .filter(|secret| secret.minted)
        .map(|secret| {
            (
                crate::settings::named(&installed.plugin, &secret.id),
                value(part(&secret.id)),
            )
        })
        .collect();
    let keeping: Vec<crate::settings::Keeping<'_>> = minted
        .iter()
        .map(|(name, value)| crate::settings::Keeping {
            name,
            value,
            from: crate::settings::Source::Minted,
            at: 1,
        })
        .collect();
    let at = crate::settings::secrets_at(ctx).unwrap_or_default();
    assert!(crate::settings::kept(&at, &keeping, &|_| Ok(())).is_ok());
}

/// A context as [`seed_ctx`] reaching every service builds it, its settings beside a
/// scratch env file of the test `name`, holding `key` as the key lemonfiber minted for each
/// of `parts`.
fn minted_ctx(name: &str, parts: &[crate::test_support::adapted::Adapted], key: &str) -> Ctx {
    let env = lemonfiber_fixtures::scratch::Scratch::new(&format!("minted-{name}"))
        .kept()
        .join(".env");
    let _ = std::fs::write(&env, "");
    let ctx = seed_ctx(true, None, Some(env));
    for part in parts {
        part.minted(&ctx, key);
    }
    ctx
}

/// The key lemonfiber minted for the Usenet client moved into its plugin.
const MINTED_USENET_KEY: &str = "the-minted-usenet-key";

/// `ctx` with its Usenet client moved into its plugin, and [`MINTED_USENET_KEY`] kept in
/// its secret store as the key lemonfiber minted for it.
fn minting_usenet(ctx: Ctx) -> Ctx {
    crate::test_support::usenet::moved_in(&ctx);
    crate::test_support::usenet::MOVED.minted(&ctx, MINTED_USENET_KEY);
    ctx
}

/// The key lemonfiber minted for each curator moved into its plugin.
const MINTED_CURATOR_KEY: &str = "the-minted-curator-key";

/// `ctx` with every curator moved into its plugin, each holding [`MINTED_CURATOR_KEY`] in
/// its secret store as the key lemonfiber minted for it.
fn minting_curators(ctx: Ctx) -> Ctx {
    for curator in crate::test_support::curators::ALL {
        curator.moved_in(&ctx);
        curator.minted(&ctx, MINTED_CURATOR_KEY);
    }
    ctx
}

/// `ctx` with every curator moved into its plugin, the engine saying each one's adapter
/// runs beside the media server's, each adapter keyed wherever its configuration is where
/// `keyed` and nowhere where not.
fn curating(ctx: Ctx, keyed: bool) -> Ctx {
    use crate::test_support::curators::ALL;
    for curator in ALL {
        curator.recorded(&ctx);
        curator.minted(&ctx, MINTED_CURATOR_KEY);
    }
    let mut ctx = ctx;
    ctx.seams.engine = Arc::new(
        crate::test_support::adapted::running(&ALL)
            .alongside(crate::test_support::media::running()),
    );
    if keyed {
        let adapters: Vec<&str> = ALL.iter().map(|curator| curator.adapter).collect();
        ctx.seams.confined = crate::test_support::adapted::Keyed::holding(
            Arc::clone(&ctx.seams.confined),
            &adapters,
        );
    }
    ctx
}

/// What every moved curator answers a whole seed run with: each media root folder and both
/// download clients already held, the clients answering their test, and every write taken.
fn curated_routes() -> Vec<(&'static str, Answer)> {
    use crate::ports::service::{ClientProbe, RegisteredClient, RegisteredFolder};
    use crate::test_support::curators::{answered, taken};
    let folder = |id: &str, path: &str| RegisteredFolder {
        id: id.to_owned(),
        path: path.to_owned(),
    };
    let client = |id: &str, host: &str, port: u16| RegisteredClient {
        id: id.to_owned(),
        host: host.to_owned(),
        port,
        category: None,
    };
    let probe = |id: &str| ClientProbe {
        id: id.to_owned(),
        reachable: true,
        detail: None,
    };
    vec![
        (
            curated("root_folders"),
            answered(&[
                folder("1", "/data/media/tv"),
                folder("2", "/data/media/movies"),
                folder("3", "/data/media/music"),
            ]),
        ),
        (
            curated("download_clients"),
            answered(&[client("1", "sabnzbd", 8080), client("2", "gluetun", 8081)]),
        ),
        (
            curated("test_download_clients"),
            answered(&[probe("1"), probe("2")]),
        ),
        (curated(""), taken()),
    ]
}

/// A transport answering a whole seed run, every curator's connection already held, with
/// `extra` answered first.
fn seeding_curated(extra: Vec<(&'static str, Answer)>) -> Arc<Fake> {
    let mut routes = extra;
    routes.extend(curated_routes());
    seeding_with(routes)
}

/// The name of the account lemonfiber minted for the torrent client moved into its plugin.
const MINTED_TORRENT_USER: &str = "the-minted-torrent-user";

/// That account's password.
const MINTED_TORRENT_PASSWORD: &str = "the-minted-torrent-password";

/// `ctx` with its torrent client moved into its plugin, and the account
/// [`MINTED_TORRENT_USER`] with [`MINTED_TORRENT_PASSWORD`] kept in its secret store as the
/// one lemonfiber minted for it.
fn minting_torrent(ctx: Ctx) -> Ctx {
    crate::test_support::torrent::moved_in(&ctx);
    keeping_minted(
        &ctx,
        crate::test_support::torrent::moved(),
        |part| match part {
            Some(lemonfiber_contract::adapter::UPSTREAM_USERNAME) => MINTED_TORRENT_USER,
            _ => MINTED_TORRENT_PASSWORD,
        },
    );
    ctx
}

fn config_scratch(name: &str) -> lemonfiber_fixtures::scratch::Scratch {
    lemonfiber_fixtures::scratch::Scratch::unmade(name).within(".env")
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

/// The wirings whose connection registers a download client into a curator.
fn download_client_wirings(report: &crate::seed::Report) -> Vec<&crate::seed::Wiring> {
    report
        .wirings
        .iter()
        .filter(|wiring| {
            wiring.connection.contains("into ")
                && !matches!(wiring.state, crate::seed::State::Unmatched { .. })
        })
        .collect()
}

/// The gate's routes file admitting one route to each of `routes`, named by the curator's
/// id or the media server's, presenting the credential beside it.
fn admitting(routes: &[(&str, lemonfiber_sidecar::gate::Kind, &str)]) -> String {
    lemonfiber_sidecar::gate::Upstreams::of(
        routes
            .iter()
            .map(
                |(route, kind, credential)| lemonfiber_sidecar::gate::Upstream {
                    route: (*route).to_owned(),
                    kind: *kind,
                    address: format!("http://{route}:80"),
                    credential: lemonfiber_sidecar::gate::Credential::new(*credential),
                    majors: Vec::new(),
                },
            )
            .collect(),
    )
    .written()
}

/// The gate under `project` admitting `routes`, as [`admitting`] writes them.
fn routed(project: &std::path::Path, routes: &[(&str, lemonfiber_sidecar::gate::Kind, &str)]) {
    let _ = store::write(
        &crate::app::gating::path(project, lemonfiber_sidecar::gate::File::Upstreams),
        &admitting(routes),
    );
}

/// The stack's request gate, filling `request.gate`.
fn request_gate() -> lemonfiber_manifest::Service {
    let mut gate = manifest_service(
        crate::app::gating::SERVICE,
        Some(lemonfiber_sidecar::gate::PORT),
    );
    gate.provides = vec![lemonfiber_contract::capabilities::request::gate::CAPABILITY.to_owned()];
    gate
}

// ---- Indexer app sync: each media-filing curator registered back. ----

/// A media-filing curator as a manifest service, with the media that makes it
/// syncable — `manifest_service` alone leaves the media empty, which is what
/// marks the indexer aggregator.
fn curator(id: &str, port: u16, media: &str) -> lemonfiber_manifest::Service {
    let mut service = manifest_service(id, Some(port));
    service.media_types = vec![media.to_owned()];
    service.provides = vec!["library.curate".to_owned()];
    service
}

/// Each of the stack's curators among `services` moved into its plugin, in the order
/// `services` lists them.
fn curators_moved(services: &[lemonfiber_manifest::Service]) -> Vec<crate::plugin::Installed> {
    services
        .iter()
        .filter_map(|service| {
            crate::test_support::curators::ALL
                .iter()
                .find(|curator| curator.plugin == service.id)
        })
        .filter_map(|curator| curator.installed())
        .collect()
}

/// `trusted` beside the plugins the bundle pins, which are first-party.
fn beside_the_bundle(
    trusted: &[crate::plugin::first_party::FirstParty],
) -> Vec<crate::plugin::first_party::FirstParty> {
    crate::plugin::first_party::EMBEDDED
        .iter()
        .chain(trusted)
        .copied()
        .collect()
}

/// What the shipped stack's asks come to with these services in place of its own —
/// the services a test declares, asked for as the stack asks.
fn fillers_of(services: Vec<lemonfiber_manifest::Service>) -> crate::wiring::Fillers {
    fillers_at(services, stack_root())
}

/// The same, with the stack's media server moved into its plugin, and its request service
/// where it runs one.
fn moved_fillers(services: Vec<lemonfiber_manifest::Service>) -> crate::wiring::Fillers {
    moved_at(services, stack_root())
}

/// The stack's `services`, written at `project`, with its media server moved into its
/// plugin, and its request service where `services` runs one.
fn moved_at(
    services: Vec<lemonfiber_manifest::Service>,
    project: &std::path::Path,
) -> crate::wiring::Fillers {
    let requesting = services
        .iter()
        .any(|service| service.id == requests_svc().id);
    let installed: Vec<crate::plugin::Installed> = crate::test_support::media::moved()
        .into_iter()
        .chain(REQUESTS.installed().filter(|_| requesting))
        .collect();
    fillers_beside(services, &installed, project)
}

/// The same, with the stack written to disk at `project`.
fn fillers_at(
    services: Vec<lemonfiber_manifest::Service>,
    project: &std::path::Path,
) -> crate::wiring::Fillers {
    fillers_beside(services, &[], project)
}

/// The same, with `installed` beside the stack.
fn fillers_beside(
    services: Vec<lemonfiber_manifest::Service>,
    installed: &[crate::plugin::Installed],
    project: &std::path::Path,
) -> crate::wiring::Fillers {
    fillers_trusting(
        services,
        installed,
        project,
        crate::plugin::first_party::EMBEDDED,
    )
}

/// The same, with `trusted` as the plugins that are first-party.
fn fillers_trusting(
    services: Vec<lemonfiber_manifest::Service>,
    installed: &[crate::plugin::Installed],
    project: &std::path::Path,
    trusted: &[crate::plugin::first_party::FirstParty],
) -> crate::wiring::Fillers {
    crate::test_support::stack_fillers(services, installed, Some(project), trusted)
}

/// The key lemonfiber minted for every part [`contracted_ctx`] holds one for.
const MINTED: &str = "the-key";

/// A context reaching `service` on loopback 8080, holding its key in `project` where
/// `keyed`, its settings beside an env file in `project`, and [`MINTED`] kept as the key
/// lemonfiber minted for every part the bundle pins.
fn contracted_ctx(project: &std::path::Path, service: &str, keyed: bool, http: Arc<Fake>) -> Ctx {
    let env = project.join(".env");
    let _ = std::fs::write(&env, "");
    let ctx = crate::test_support::contracted_context(project, service, keyed)
        .settings(Settings {
            env_file: Some(env),
            ..Settings::default()
        })
        .build()
        .with_http(http);
    minted_for_every_part(&ctx, MINTED);
    ctx
}

/// `ctx` with its secret store gone, so lemonfiber holds no key it minted for anything.
fn forgetting_minted(ctx: Ctx) -> Ctx {
    let _ = std::fs::remove_file(crate::settings::secrets_at(&ctx).unwrap_or_default());
    ctx
}

/// `key` kept in `ctx`'s secret store as the key lemonfiber minted for every part the
/// bundle pins.
fn minted_for_every_part(ctx: &Ctx, key: &str) {
    let names: Vec<String> = lemonfiber_bundled::PINNED
        .iter()
        .filter_map(|pinned| lemonfiber_plugin::Manifest::from_toml(pinned.copy).ok())
        .flat_map(|manifest| {
            let installed = crate::plugin::Installed::of(&manifest);
            installed
                .declared
                .secrets
                .iter()
                .filter(|secret| secret.minted)
                .map(|secret| crate::settings::named(&installed.plugin, &secret.id))
                .collect::<Vec<_>>()
        })
        .collect();
    let keeping: Vec<crate::settings::Keeping<'_>> = names
        .iter()
        .map(|name| crate::settings::Keeping {
            name,
            value: key,
            from: crate::settings::Source::Minted,
            at: 1,
        })
        .collect();
    let at = crate::settings::secrets_at(ctx).unwrap_or_default();
    assert!(crate::settings::kept(&at, &keeping, &|_| Ok(())).is_ok());
}

/// `installed`, each of its services asking for every curator.
fn asking_every_curator(mut installed: crate::plugin::Installed) -> crate::plugin::Installed {
    for placed in &mut installed.services {
        placed.asks = vec![crate::plugin::Asking {
            capability: "library.curate".to_owned(),
            each: true,
            native: None,
        }];
    }
    installed
}

/// A plugin's curator `kept`, filing `media` over `library.curate` and answering in
/// `servarr`.
fn a_stand_in(media: &str) -> crate::plugin::Installed {
    let mut stand_in = crate::test_support::a_placed("kept", &["library.curate"], Some(8990));
    stand_in.media_types = vec![media.to_owned()];
    stand_in.native = Some("servarr".to_owned());
    stand_in.speaks = vec!["library.curate@1".to_owned()];
    crate::test_support::an_installed("kept", vec![stand_in])
}

/// The stack's `services` beside [`a_stand_in`] filing `media`.
fn beside_a_stand_in(
    services: Vec<lemonfiber_manifest::Service>,
    media: &str,
) -> crate::wiring::Fillers {
    fillers_beside(services, &[a_stand_in(media)], stack_root())
}

/// The stack's `services` beside a plugin's service standing where `asker`, one of the
/// stack's own, would ask: the same id, adapter and port, brought by a plugin.
fn asked_by_a_plugin(
    asker: &lemonfiber_manifest::Service,
    services: Vec<lemonfiber_manifest::Service>,
) -> crate::wiring::Fillers {
    let stand_in = crate::test_support::a_placed(&asker.id, &[], asker.listens);
    fillers_beside(
        services,
        &[crate::test_support::an_installed("asking", vec![stand_in])],
        stack_root(),
    )
}

/// The stack's `services` with its indexer moved into its plugin, beside `installed`.
fn indexed(
    services: Vec<lemonfiber_manifest::Service>,
    installed: &[crate::plugin::Installed],
) -> crate::wiring::Fillers {
    let installed: Vec<_> = INDEXER
        .installed()
        .into_iter()
        .chain(installed.iter().cloned())
        .collect();
    fillers_beside(services, &installed, stack_root())
}

/// A context of the test `name` asking the moved indexer over `http`, holding [`MINTED`] as
/// the key lemonfiber minted for the moved TV curator where `keyed`.
fn indexing(name: &str, http: Arc<Fake>, keyed: bool) -> Ctx {
    let parts: &[crate::test_support::adapted::Adapted] = if keyed {
        &[crate::test_support::curators::TELEVISION]
    } else {
        &[]
    };
    crate::test_support::adapted::asking(
        minted_ctx(name, parts, MINTED).with_http(http),
        &[INDEXER],
    )
}

/// The indexer aggregator as a manifest service: a curator shape that files no media.
fn aggregator_svc() -> lemonfiber_manifest::Service {
    manifest_service("prowlarr", Some(9696))
}

// ---- The media server as the request service's identity, with a minted credential. ----

fn requests_svc() -> lemonfiber_manifest::Service {
    requesting(manifest_service("seerr", Some(5055)))
}

/// `service` as the stack's request service: called what the stack calls it and filling
/// `request.intake`.
fn requesting(mut service: lemonfiber_manifest::Service) -> lemonfiber_manifest::Service {
    "Seerr".clone_into(&mut service.name);
    service.provides = vec!["request.intake".to_owned()];
    service
}

/// The stack's media server, called what the stack calls it and serving its identity.
fn media_server_svc() -> lemonfiber_manifest::Service {
    let mut media_server = manifest_service("jellyfin", Some(8096));
    media_server.name = "Jellyfin".to_owned();
    media_server.provides = vec!["identity.source".to_owned()];
    media_server
}

/// The media server the shipped stack's identity ask settles on among `services`, the
/// stack's own moved into its plugin where it is among them.
fn served(services: &[lemonfiber_manifest::Service]) -> Option<crate::app::targets::MediaServer> {
    let own = media_server_svc().id;
    let fillers = if services.iter().any(|service| service.id == own) {
        moved_fillers(services.to_vec())
    } else {
        fillers_of(services.to_vec())
    };
    crate::app::targets::MediaServer::of(&fillers)
}

/// A household that answers the media server's and the request service's setup reads.
///
/// `completed` is what the media server says about its own first run. `signed_in` is
/// whether the request service is already initialised: where it is not, the catch-all
/// answers "no" and then "yes", which is the read-write-read the identity wiring performs.
/// Scripting the change in order rather than flipping a flag says which write is meant to
/// cause it.
fn household(completed: bool, signed_in: bool) -> Arc<Fake> {
    household_changing(completed, signed_in, crate::test_support::media::taken())
}

/// [`household`], with the media server answering the change of its administrator's
/// password with `changed`.
fn household_changing(completed: bool, signed_in: bool, changed: Answer) -> Arc<Fake> {
    use crate::test_support::media;
    use crate::test_support::requests::{answered, intake, taken};
    let initialised = if signed_in {
        vec![answered(&true)]
    } else {
        vec![answered(&false), answered(&true)]
    };
    Fake::by_path_in_turn(vec![
        (
            media::identity("startup_completed"),
            vec![media::answered(&completed)],
        ),
        (media::identity("create_admin"), vec![media::taken()]),
        (media::identity("replace_admin_password"), vec![changed]),
        (intake("establish"), vec![taken()]),
        (intake("initialized"), initialised),
        (intake("configure_identity"), vec![taken()]),
        (
            intake("telling"),
            vec![answered(&crate::ports::service::Telling::default())],
        ),
        (intake("tell"), vec![taken()]),
    ])
}

/// The book curator, as a manifest service whose key lemonfiber mints for it.
fn book_curator_svc() -> lemonfiber_manifest::Service {
    manifest_service("bindery", Some(8787))
}

/// A scratch settings file holding the media server's recorded password, so a
/// client built from it can sign in.
fn recorded_admin(name: &str) -> std::path::PathBuf {
    let dir = lemonfiber_fixtures::scratch::Scratch::named(&format!("seed-{name}")).kept();
    let _ = std::fs::create_dir_all(&dir);
    let env = dir.join(".env");
    let _ = crate::config::store::set(
        &env,
        crate::config::MEDIA_SERVER_ADMIN_PASSWORD_KEY,
        "minted-earlier",
    );
    env
}

/// The stack's own subtitle finder as a manifest service, naming the YAML it writes its key to.
fn subtitle_finder_svc() -> lemonfiber_manifest::Service {
    manifest_service("bazarr", Some(6767))
}

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
mod asking;
mod baseline;
mod contracted_gate;
mod cors;
mod curating;
mod decline;
mod enveloping;
mod extraction;
mod gate;
mod identity;
mod intake;
mod linking;
mod owning;
mod passes;
mod proxies;
mod publishing;
mod quality;
mod replacing;
mod requests;
mod showing;
mod subtitles;
mod taken_back;
mod targets;
mod tokens;
mod unmanaged;

/// Whose a credential is, for a test: one of the stack's own services where `plugin` is
/// nothing, and otherwise the service that plugin brought.
fn holder(plugin: Option<&str>, id: &str) -> super::clients::Holder {
    super::clients::Holder {
        plugin: plugin.map(str::to_owned),
        id: id.to_owned(),
    }
}
