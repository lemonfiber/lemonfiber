//! The credentials the request service held before the gate, taken back once it reaches
//! everything through the gate.

use async_trait::async_trait;
use lemonfiber_ports::filesystem::{Fault, FileSystem, Identity, Ownership, StorageFacts};
use lemonfiber_sidecar::gate::{Accepted, Kind, Tokens, PORT};
use lemonfiber_sidecar::TokenHash;

use super::*;
use crate::baseline::Baseline;
use crate::ports::service::{Endpoint, MediaServerLink, RegisteredTarget};
use crate::test_support::curators::{MUSIC, TELEVISION};
use crate::test_support::{media, requests};

/// What the report calls this connection.
const HELD: &str = "The credentials the request service held";

/// Where the owed curator key is recorded.
const OWED_CURATOR: &str = "held-key:sonarr";

/// The key lemonfiber minted for the television curator before it was taken back.
const OLD_KEY: &str = "old-sonarr-key";

/// The token the gate accepts on the curator's route, and on the media server's.
const CURATOR_TOKEN: &str = "sonarr-token";
const LINK_TOKEN: &str = "link-token";

/// The randomness every run here is handed, and the key a replacement mints from it.
const RANDOM: u8 = 0xcd;

/// A stack running the media server and the gate, its curators and its request service
/// in their plugins.
fn gated() -> Vec<lemonfiber_manifest::Service> {
    vec![media_server_svc(), request_gate()]
}

/// `services` written at `project`, with each of `moved` in its plugin.
fn moving(
    services: Vec<lemonfiber_manifest::Service>,
    project: &std::path::Path,
    moved: &[Option<crate::plugin::Installed>],
) -> crate::wiring::Fillers {
    let installed: Vec<_> = moved.iter().flatten().cloned().collect();
    fillers_beside(services, &installed, project)
}

/// Every part this step reaches moved into its plugin: the media server, both curators
/// and the request service.
fn every_part() -> [Option<crate::plugin::Installed>; 4] {
    [
        media::moved(),
        TELEVISION.installed(),
        MUSIC.installed(),
        REQUESTS.installed(),
    ]
}

/// `services` written at `project`, with every part moved.
fn moved(
    services: Vec<lemonfiber_manifest::Service>,
    project: &std::path::Path,
) -> crate::wiring::Fillers {
    moving(services, project, &every_part())
}

/// The gate's routes and the tokens it accepts, and nothing else on disk.
struct Taking;

#[async_trait]
impl FileSystem for Taking {
    async fn canonicalize(&self, path: &std::path::Path) -> Result<std::path::PathBuf, Fault> {
        Ok(path.to_path_buf())
    }

    async fn touch(&self, _path: &std::path::Path) -> Result<(), Fault> {
        Err(Fault::new("unused"))
    }

    async fn link(&self, _from: &std::path::Path, _to: &std::path::Path) -> Result<(), Fault> {
        Err(Fault::new("unused"))
    }

    async fn identify(&self, _path: &std::path::Path) -> Result<Identity, Fault> {
        Err(Fault::new("unused"))
    }

    async fn remove(&self, _path: &std::path::Path) {}

    async fn read(&self, path: &std::path::Path) -> Option<String> {
        let name = path.to_string_lossy();
        if name.ends_with("upstreams.json") {
            return Some(admitting(&[
                ("sonarr", Kind::Sonarr, OLD_KEY),
                ("jellyfin", Kind::Jellyfin, "gates-key"),
            ]));
        }
        if name.ends_with("tokens.json") {
            return Some(
                Tokens::of(vec![
                    Accepted {
                        route: "sonarr".to_owned(),
                        tokens: vec![TokenHash::of(CURATOR_TOKEN)],
                    },
                    Accepted {
                        route: "jellyfin".to_owned(),
                        tokens: vec![TokenHash::of(LINK_TOKEN)],
                    },
                ])
                .written(),
            );
        }
        None
    }

    async fn write(&self, _path: &std::path::Path, _contents: &str) {}

    async fn ownership(&self, _path: &std::path::Path) -> Option<Ownership> {
        None
    }
}

#[async_trait]
impl Storage for Taking {
    async fn describe(&self, _path: &std::path::Path) -> StorageFacts {
        StorageFacts {
            point: std::path::PathBuf::new(),
            kind: lemonfiber_ports::filesystem::FsKind::Linking("test".to_owned()),
            removable: false,
            available: 0,
            total: 0,
        }
    }
}

/// Where the request service reaches a service: at `host` and `port` under `base`.
fn reached(host: &str, port: u16, base: &str) -> Endpoint {
    Endpoint {
        host: host.to_owned(),
        port,
        base: base.to_owned(),
    }
}

/// The curator as the request service lists it, at `at` under `key`.
fn listing(at: Endpoint, key: &str) -> Answer {
    requests::answered(&[RegisteredTarget {
        id: "1".to_owned(),
        at,
        key: key.to_owned(),
        kind: crate::ports::media::Kind::Tv,
    }])
}

/// The curator held by the request service at its own address.
fn curator_direct() -> Answer {
    listing(reached("sonarr", 8989, ""), OLD_KEY)
}

/// The curator held by the request service at the gate, under the token it accepts there.
fn curator_gated() -> Answer {
    listing(reached("request-gate", PORT, "/sonarr"), CURATOR_TOKEN)
}

/// The request service's media-server link, at the gate under the token it accepts.
fn linked() -> Answer {
    link_at(reached("request-gate", PORT, "/jellyfin"))
}

/// The request service's media-server link at `at`, under the token the gate accepts.
fn link_at(at: Endpoint) -> Answer {
    requests::answered(&MediaServerLink {
        at,
        key: LINK_TOKEN.to_owned(),
    })
}

/// The keys the media server files under the request service's name: its own where
/// `minted`.
fn keys(minted: bool) -> Answer {
    let filed: Vec<&str> = minted.then_some(MINTED).into_iter().collect();
    media::answered(&filed)
}

/// The media server key the request service minted itself.
const MINTED: &str = "seerr-minted";

/// A household whose request service holds the curator as `listed`, answers its own test
/// of it with `tested`, and whose media server answers what it files under the request
/// service's name with `keys` and a revoke with `revoked`.
fn household(listed: Answer, tested: Answer, keys: Answer, revoked: Answer) -> Arc<Fake> {
    linked_household(linked(), listed, tested, keys, revoked)
}

/// The same, with the request service's media-server link as `link`.
fn linked_household(
    link: Answer,
    listed: Answer,
    tested: Answer,
    keys: Answer,
    revoked: Answer,
) -> Arc<Fake> {
    Fake::by_path_in_turn(vec![
        (requests::intake("test_fulfilment_target"), vec![tested]),
        (requests::intake("fulfilment_targets"), vec![listed]),
        (requests::intake("media_server_link"), vec![link]),
        (media::serve("filed_as"), vec![keys]),
        (media::serve("revoke"), vec![revoked]),
        (
            media::serve("declared"),
            vec![media::answered(&{
                let mut declared = lemonfiber_fixtures::declaring::enforceable();
                declared.ports.internal_tls = Some(8920);
                declared
            })],
        ),
    ])
}

/// A run over `http` holding the administrator's password, in a scratch project, every
/// part recorded as moved and the television curator holding [`OLD_KEY`] as the key
/// lemonfiber minted for it, with randomness to replace it where `random`.
fn taking_with(
    name: &str,
    http: &Arc<Fake>,
    rehearsing: bool,
    random: bool,
) -> (Ctx, std::path::PathBuf) {
    let project = lemonfiber_fixtures::scratch::Scratch::named(&format!("taken-{name}")).kept();
    let _ = std::fs::remove_dir_all(&project);
    let _ = std::fs::create_dir_all(&project);
    let mut ctx = seed_ctx(true, None, Some(recorded_admin(name)))
        .with_http(http.clone())
        .with_filesystem(Arc::new(Taking))
        .with_random(Arc::new(FixedRandom(
            random.then(|| vec![RANDOM; crate::secret::SECRET_BYTES]),
        )));
    ctx.settings.stack_dir = Some(project.clone());
    for curator in [TELEVISION, MUSIC, REQUESTS] {
        curator.recorded(&ctx);
    }
    TELEVISION.minted(&ctx, OLD_KEY);
    ctx.dry_run = rehearsing;
    (ctx, project)
}

/// [`taking_with`], with randomness to replace a key.
fn taking(name: &str, http: &Arc<Fake>, rehearsing: bool) -> (Ctx, std::path::PathBuf) {
    taking_with(name, http, rehearsing, true)
}

/// The key the store holds for the television curator in `ctx`.
fn curator_key(ctx: &Ctx) -> Option<String> {
    crate::settings::secrets_at(ctx)
        .and_then(|at| crate::settings::Secrets::at(&at).ok())
        .and_then(|held| {
            held.value(&crate::settings::named(TELEVISION.plugin, "upstream-key"))
                .map(str::to_owned)
        })
}

/// A baseline owing the curator's key.
fn owing() -> Baseline {
    let mut baseline = Baseline::new();
    baseline.record("seerr", OWED_CURATOR, "owed", "1");
    baseline
}

/// This step over `ctx`, with the media server moved into its plugin, and the baseline it
/// left.
async fn taken(
    ctx: &Ctx,
    services: &[lemonfiber_manifest::Service],
    project: &std::path::Path,
    baseline: Baseline,
) -> (Option<State>, Baseline) {
    taken_among(
        ctx,
        services,
        &moved(services.to_vec(), project),
        project,
        baseline,
    )
    .await
}

/// This step over `ctx` among `fillers`, and the baseline it left.
async fn taken_among(
    ctx: &Ctx,
    services: &[lemonfiber_manifest::Service],
    fillers: &crate::wiring::Fillers,
    project: &std::path::Path,
    mut baseline: Baseline,
) -> (Option<State>, Baseline) {
    let wiring = super::super::taken_back::seed_taken_back(
        ctx,
        services,
        fillers,
        Some(project),
        &mut baseline,
    )
    .await;
    (
        wiring.map(|one| {
            assert_eq!(one.connection, HELD);
            one.state
        }),
        baseline,
    )
}

/// Whether `asked` asks the media server to revoke `key`.
fn revoking(asked: &lemonfiber_ports::http::Request, key: &str) -> bool {
    asked.url.ends_with(media::serve("revoke"))
        && asked.body.as_deref() == Some(serde_json::json!({ "key": key }).to_string().as_str())
}

#[tokio::test]
async fn a_key_held_at_the_curators_own_address_is_owed_and_one_at_the_gate_is_not() {
    let direct = household(
        curator_direct(),
        requests::taken(),
        keys(false),
        media::taken(),
    );
    let (ctx, project) = taking("noted", &direct, false);
    let mut noted = Baseline::new();
    super::super::taken_back::note_held(
        &ctx,
        &gated(),
        &moved(gated(), &project),
        Some(&project),
        &mut noted,
    )
    .await;
    assert_eq!(noted.expected("seerr", OWED_CURATOR), Some("owed"));

    let through = household(
        curator_gated(),
        requests::taken(),
        keys(false),
        media::taken(),
    );
    let (ctx, project) = taking("not-noted", &through, false);
    let mut clean = Baseline::new();
    super::super::taken_back::note_held(
        &ctx,
        &gated(),
        &moved(gated(), &project),
        Some(&project),
        &mut clean,
    )
    .await;
    assert!(clean.is_empty());

    // A stack without the gate owes nothing: the request service is meant to hold it.
    let ungated: Vec<_> = gated()
        .into_iter()
        .filter(|service| service.id != "request-gate")
        .collect();
    let mut none = Baseline::new();
    super::super::taken_back::note_held(
        &ctx,
        &ungated,
        &moved(ungated.clone(), &project),
        Some(&project),
        &mut none,
    )
    .await;
    assert!(none.is_empty());
}

#[tokio::test]
async fn a_request_service_that_will_not_say_what_it_holds_owes_nothing_yet() {
    let silent = Fake::by_path(Vec::new());
    let (ctx, project) = taking("unsaid", &silent, false);
    let mut noted = Baseline::new();
    super::super::taken_back::note_held(
        &ctx,
        &gated(),
        &moved(gated(), &project),
        Some(&project),
        &mut noted,
    )
    .await;
    assert!(noted.is_empty());
}

/// **Only the stack's own request service ran before the gate.** A first-party plugin's
/// request service speaking `request.intake` in its place owes nothing, is asked nothing,
/// and nothing recorded under its id is taken back.
#[tokio::test]
async fn a_plugins_request_service_owes_nothing_and_is_asked_nothing() {
    let http = household(
        curator_direct(),
        requests::taken(),
        keys(true),
        media::taken(),
    );
    let (ctx, project) = taking("plugin", &http, false);
    let services = gated();
    let fillers = fillers_trusting(
        services.clone(),
        &[contracted(
            "intake",
            crate::test_support::CONTRACTED_REQUESTS,
            "request.intake",
        )],
        &project,
        &first_party("intake"),
    );
    let mut noted = Baseline::new();
    super::super::taken_back::note_held(&ctx, &services, &fillers, Some(&project), &mut noted)
        .await;
    let mut owing = Baseline::new();
    owing.record(
        crate::test_support::CONTRACTED_REQUESTS,
        OWED_CURATOR,
        "owed",
        "1",
    );

    let wiring = super::super::taken_back::seed_taken_back(
        &ctx,
        &services,
        &fillers,
        Some(&project),
        &mut owing,
    )
    .await;

    assert!(noted.is_empty());
    assert!(wiring.is_none(), "{wiring:?}");
    assert_eq!(
        owing.expected(crate::test_support::CONTRACTED_REQUESTS, OWED_CURATOR),
        Some("owed")
    );
    assert!(http.requests().is_empty(), "{:?}", http.requests());
}

#[tokio::test]
async fn nothing_owed_and_nothing_minted_is_not_a_connection() {
    let http = household(
        curator_gated(),
        requests::taken(),
        keys(false),
        media::taken(),
    );
    let (ctx, project) = taking("nothing", &http, false);
    let (state, _) = taken(&ctx, &gated(), &project, Baseline::new()).await;
    assert_eq!(state, None);
}

#[tokio::test]
async fn a_rehearsal_counts_what_is_held_and_replaces_nothing() {
    let http = household(
        curator_gated(),
        requests::taken(),
        keys(true),
        media::taken(),
    );
    let (ctx, project) = taking("rehearsed", &http, true);
    let (state, baseline) = taken(&ctx, &gated(), &project, owing()).await;
    assert_eq!(
        state,
        Some(State::WouldWire {
            yours: Some("2 credentials held".to_owned()),
            ours: Some("none".to_owned()),
        })
    );
    assert_eq!(baseline.expected("seerr", OWED_CURATOR), Some("owed"));
    assert_eq!(curator_key(&ctx).as_deref(), Some(OLD_KEY));
}

#[tokio::test]
async fn one_held_credential_is_counted_as_one() {
    let http = household(
        curator_gated(),
        requests::taken(),
        keys(true),
        media::taken(),
    );
    let (ctx, project) = taking("rehearsed-one", &http, true);
    let (state, _) = taken(&ctx, &gated(), &project, Baseline::new()).await;
    assert_eq!(
        state,
        Some(State::WouldWire {
            yours: Some("1 credential held".to_owned()),
            ours: Some("none".to_owned()),
        })
    );
}

#[tokio::test]
async fn a_target_still_reached_directly_holds_everything_back() {
    let http = household(
        curator_direct(),
        requests::taken(),
        keys(true),
        media::taken(),
    );
    let (ctx, project) = taking("still-direct", &http, false);
    let (state, baseline) = taken(&ctx, &gated(), &project, owing()).await;
    assert_eq!(
        state,
        Some(State::Skipped {
            reason: "Seerr still reaches Sonarr without the gate, so nothing it held is \
                     replaced yet. A later run replaces them once Seerr reaches everything \
                     through the gate."
                .to_owned(),
        })
    );
    assert_eq!(baseline.expected("seerr", OWED_CURATOR), Some("owed"));
    assert_eq!(curator_key(&ctx).as_deref(), Some(OLD_KEY));
    assert!(!http.requests().iter().any(|asked| revoking(asked, MINTED)));
}

#[tokio::test]
async fn a_target_at_the_gate_that_fails_its_test_is_still_direct() {
    let http = household(
        curator_gated(),
        requests::refusing(),
        keys(false),
        media::taken(),
    );
    let (ctx, project) = taking("untested", &http, false);
    let (state, _) = taken(&ctx, &gated(), &project, owing()).await;
    assert!(
        matches!(&state, Some(State::Skipped { reason }) if reason.contains("reaches Sonarr")),
        "{state:?}"
    );
}

#[tokio::test]
async fn a_request_service_that_does_not_answer_is_left_for_a_later_run() {
    let http = Fake::by_path_in_turn(vec![(media::serve("filed_as"), vec![keys(false)])]);
    let (ctx, project) = taking("unread", &http, false);
    let (state, baseline) = taken(&ctx, &gated(), &project, owing()).await;
    assert!(matches!(state, Some(State::Skipped { .. })), "{state:?}");
    assert_eq!(baseline.expected("seerr", OWED_CURATOR), Some("owed"));
}

#[tokio::test]
async fn a_request_service_that_cannot_be_asked_over_its_contract_is_left_for_a_later_run() {
    let http = Fake::by_path_in_turn(vec![(media::serve("filed_as"), vec![keys(false)])]);
    let (mut ctx, project) = taking("unasked", &http, false);
    ctx.seams.engine = Arc::new(
        crate::test_support::adapted::running(&[TELEVISION, MUSIC]).alongside(media::running()),
    );
    let (state, baseline) = taken(&ctx, &gated(), &project, owing()).await;
    assert!(
        matches!(&state, Some(State::Skipped { reason })
            if reason.contains("could not be asked over its contract, so nothing it held is replaced yet")),
        "{state:?}"
    );
    assert_eq!(baseline.expected("seerr", OWED_CURATOR), Some("owed"));
}

/// The curator's replaced key reaches every secrets file that reads it, as a rotation's
/// does, so no reader is left holding the key that was taken back.
#[tokio::test]
async fn a_curator_key_taken_back_reaches_every_file_that_reads_it() {
    let http = household(
        curator_gated(),
        requests::taken(),
        keys(false),
        media::taken(),
    );
    let (ctx, project) = taking("propagated", &http, false);
    let env = ctx.settings.env_file.clone().unwrap_or_default();
    let kept = env.with_file_name(crate::config::paths::PLUGINS);
    let mut register = std::fs::read_to_string(&kept)
        .ok()
        .and_then(|text| crate::plugin::Register::parse(&text).ok())
        .unwrap_or_else(crate::plugin::Register::empty);
    assert!(register
        .record(crate::test_support::a_trusted_crossing_reader())
        .is_ok());
    assert!(std::fs::write(&kept, register.to_json().unwrap_or_default()).is_ok());
    let file = env.with_file_name("env").join("reader.env");
    let _ = std::fs::create_dir_all(env.with_file_name("env"));
    let _ = std::fs::write(&file, format!("SONARR_KEY={OLD_KEY}\n"));

    let _ = taken(&ctx, &gated(), &project, owing()).await;

    let replaced = curator_key(&ctx).unwrap_or_default();
    assert_ne!(replaced, OLD_KEY);
    assert_eq!(
        std::fs::read_to_string(&file).ok(),
        Some(format!("SONARR_KEY={replaced}\n"))
    );
}

#[tokio::test]
async fn an_owed_key_of_a_curator_the_stack_no_longer_runs_is_forgotten() {
    let http = household(
        curator_gated(),
        requests::taken(),
        keys(false),
        media::taken(),
    );
    let (ctx, project) = taking("gone", &http, false);
    let mut baseline = owing();
    baseline.record("seerr", "held-key:radarr", "owed", "1");
    let (state, baseline) = taken(&ctx, &gated(), &project, baseline).await;
    assert_eq!(
        baseline.expected("seerr", "held-key:radarr"),
        None,
        "{state:?}"
    );
}

#[tokio::test]
async fn a_media_server_link_not_at_the_gate_holds_everything_back() {
    let http = linked_household(
        link_at(reached("jellyfin", 8096, "")),
        curator_gated(),
        requests::taken(),
        keys(true),
        media::taken(),
    );
    let (ctx, project) = taking("link-direct", &http, false);
    let (state, _) = taken(&ctx, &gated(), &project, owing()).await;
    assert!(
        matches!(&state, Some(State::Skipped { reason }) if reason.starts_with("Seerr still reaches Jellyfin without")),
        "{state:?}"
    );
}

#[tokio::test]
async fn targets_that_cannot_be_listed_leave_everything_for_a_later_run() {
    let http = linked_household(
        linked(),
        requests::refusing(),
        requests::taken(),
        keys(false),
        media::taken(),
    );
    let (ctx, project) = taking("unlisted", &http, false);
    let (state, baseline) = taken(&ctx, &gated(), &project, owing()).await;
    assert!(
        matches!(state, Some(State::Failed { .. } | State::Skipped { .. })),
        "{state:?}"
    );
    assert_eq!(baseline.expected("seerr", OWED_CURATOR), Some("owed"));
}

#[tokio::test]
async fn a_stack_without_the_request_service_or_the_media_server_has_nothing_to_take_back() {
    let http = household(
        curator_gated(),
        requests::taken(),
        keys(true),
        media::taken(),
    );
    let (ctx, project) = taking("no-seerr", &http, false);
    let (state, _) = taken_among(
        &ctx,
        &gated(),
        &moving(
            gated(),
            &project,
            &[media::moved(), TELEVISION.installed(), MUSIC.installed()],
        ),
        &project,
        owing(),
    )
    .await;
    assert_eq!(state, None);
    let serverless: Vec<_> = gated()
        .into_iter()
        .filter(|service| service.id != "jellyfin")
        .collect();
    let (state, _) = taken_among(
        &ctx,
        &serverless,
        &moving(
            serverless.clone(),
            &project,
            &[
                TELEVISION.installed(),
                MUSIC.installed(),
                REQUESTS.installed(),
            ],
        ),
        &project,
        Baseline::new(),
    )
    .await;
    assert_eq!(state, None);
}

/// A media server that cannot be asked over its contracts revokes nothing, and the curator
/// keys are replaced all the same.
#[tokio::test]
async fn a_media_server_that_cannot_be_asked_leaves_the_curator_keys_still_replaced() {
    let http = household(
        curator_gated(),
        requests::taken(),
        keys(true),
        media::taken(),
    );
    let (ctx, project) = taking("unasked-server", &http, false);
    let (state, baseline) = taken_among(
        &ctx,
        &gated(),
        &moving(
            gated(),
            &project,
            &[
                TELEVISION.installed(),
                MUSIC.installed(),
                REQUESTS.installed(),
            ],
        ),
        &project,
        owing(),
    )
    .await;
    assert_eq!(baseline.expected("seerr", OWED_CURATOR), None, "{state:?}");
    assert_ne!(curator_key(&ctx).as_deref(), Some(OLD_KEY));
    assert!(!http.requests().iter().any(|asked| revoking(asked, MINTED)));
}

mod resetting;
