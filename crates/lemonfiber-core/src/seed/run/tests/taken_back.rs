//! The credentials the request service held before the gate, taken back once it reaches
//! everything through the gate.

use async_trait::async_trait;
use lemonfiber_ports::filesystem::{Fault, FileSystem, Identity, Ownership, StorageFacts};
use lemonfiber_sidecar::gate::{Accepted, Tokens, PORT};
use lemonfiber_sidecar::TokenHash;

use super::*;
use crate::baseline::Baseline;

/// What the report calls this connection.
const HELD: &str = "The credentials the request service held";

/// Where the owed Sonarr key is recorded.
const OWED_SONARR: &str = "held-key:sonarr";

/// Sonarr's configuration before and after it replaces its key.
const OLD_CONFIG: &str = "<Config><ApiKey>old-sonarr-key</ApiKey></Config>";
const NEW_CONFIG: &str = "<Config><ApiKey>new-sonarr-key</ApiKey></Config>";

/// The token the gate accepts on Sonarr's route, and on Jellyfin's.
const SONARR_TOKEN: &str = "sonarr-token";
const LINK_TOKEN: &str = "link-token";

/// A stack running Sonarr, the media server, the request service and the gate.
fn gated() -> Vec<lemonfiber_manifest::Service> {
    vec![
        arr("sonarr", 8989, "tv"),
        arr("lidarr", 8686, "music"),
        jellyfin_svc(),
        seerr_with_settings(),
        manifest_service("request-gate", None, Some(PORT)),
    ]
}

/// The configuration each service wrote, the gate's tokens, and Sonarr's key as it
/// stands before and after the transport was asked to reset it.
struct Taking {
    asked: Arc<Fake>,
}

impl Taking {
    fn over(asked: &Arc<Fake>) -> Arc<Self> {
        Arc::new(Self {
            asked: asked.clone(),
        })
    }
}

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
        if name.ends_with("tokens.json") {
            return Some(
                Tokens::of(vec![
                    Accepted {
                        route: "sonarr".to_owned(),
                        tokens: vec![TokenHash::of(SONARR_TOKEN)],
                    },
                    Accepted {
                        route: "jellyfin".to_owned(),
                        tokens: vec![TokenHash::of(LINK_TOKEN)],
                    },
                ])
                .written(),
            );
        }
        if name.contains("seerr") {
            return Some(lemonfiber_fixtures::support::SEERR_SETTINGS.to_owned());
        }
        if name.ends_with("config.xml") {
            let reset = self.asked.asked_for("/command");
            return Some(if reset { NEW_CONFIG } else { OLD_CONFIG }.to_owned());
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

/// Sonarr held by the request service at its own address.
fn sonarr_direct() -> String {
    r#"[{"id":1,"hostname":"sonarr","port":8989,"baseUrl":"","apiKey":"old-sonarr-key"}]"#
        .to_owned()
}

/// Sonarr held by the request service at the gate, under the token it accepts there.
fn sonarr_gated() -> String {
    format!(
        r#"[{{"id":1,"hostname":"request-gate","port":{PORT},"baseUrl":"/sonarr","apiKey":"{SONARR_TOKEN}"}}]"#
    )
}

/// The request service's media-server link, at the gate under the token it accepts.
fn linked() -> String {
    link_at("request-gate", PORT, "/jellyfin")
}

/// The request service's media-server link at `host`, `port` and `base`.
fn link_at(host: &str, port: u16, base: &str) -> String {
    serde_json::json!({
        "name": "Jellyfin",
        "ip": host,
        "port": port,
        "useSsl": false,
        "urlBase": base,
        "apiKey": LINK_TOKEN,
    })
    .to_string()
}

/// The keys the media server lists, with the request service's own under its name
/// where `minted`.
fn keys(minted: bool) -> String {
    let items: Vec<serde_json::Value> = minted
        .then(|| serde_json::json!({ "AppName": "Seerr", "AccessToken": "seerr-minted" }))
        .into_iter()
        .collect();
    serde_json::json!({ "Items": items }).to_string()
}

/// A household whose request service holds Sonarr as `sonarr`, passes its own test of
/// it with `tested`, and whose media server lists `keys` and answers a revoke with
/// `revoked`; Sonarr answers a reset with `reset`.
fn household(sonarr: &str, tested: u16, keys: &str, revoked: u16, reset: u16) -> Arc<Fake> {
    linked_household(&linked(), sonarr, 200, tested, keys, revoked, reset)
}

/// The same, with the request service's media-server link as `link`, and its Sonarr
/// targets listed with status `listed`.
fn linked_household(
    link: &str,
    sonarr: &str,
    listed: u16,
    tested: u16,
    keys: &str,
    revoked: u16,
    reset: u16,
) -> Arc<Fake> {
    let leaked = |text: &str| -> &'static str { Box::leak(text.to_owned().into_boxed_str()) };
    Fake::by_route_in_turn(vec![
        (
            Method::Post,
            "/settings/sonarr/test",
            vec![Answer::reply(tested, "")],
        ),
        (
            Method::Get,
            "/settings/sonarr",
            vec![Answer::reply(listed, leaked(sonarr))],
        ),
        (
            Method::Get,
            "/settings/radarr",
            vec![Answer::reply(200, "[]")],
        ),
        (
            Method::Get,
            "/settings/jellyfin",
            vec![Answer::reply(200, leaked(link))],
        ),
        (
            Method::Post,
            "/Users/AuthenticateByName",
            vec![Answer::reply(
                200,
                r#"{"AccessToken":"token","User":{"Id":"admin-id"}}"#,
            )],
        ),
        (
            Method::Get,
            "/Auth/Keys",
            vec![Answer::reply(200, leaked(keys))],
        ),
        (
            Method::Delete,
            "/Auth/Keys/",
            vec![Answer::reply(revoked, "")],
        ),
        (Method::Post, "/Auth/Keys", vec![Answer::reply(204, "")]),
        (Method::Post, "/command", vec![Answer::reply(reset, "{}")]),
        (
            Method::Get,
            "/system/status",
            vec![Answer::reply(
                200,
                r#"{"instanceName":"Sonarr","version":"4.0.20"}"#,
            )],
        ),
    ])
}

/// A run over `http` holding the administrator's password, in a scratch project.
fn taking(name: &str, http: &Arc<Fake>, rehearsing: bool) -> (Ctx, std::path::PathBuf) {
    let project = lemonfiber_fixtures::scratch::Scratch::named(&format!("taken-{name}")).kept();
    let _ = std::fs::remove_dir_all(&project);
    let _ = std::fs::create_dir_all(&project);
    let mut ctx = seed_ctx(None, true, Vec::new(), None, Some(recorded_admin(name)))
        .with_http(http.clone())
        .with_filesystem(Taking::over(http))
        .with_random(Arc::new(FixedRandom(Some(vec![
            0xcd;
            crate::secret::SECRET_BYTES
        ]))));
    ctx.dry_run = rehearsing;
    (ctx, project)
}

/// A baseline owing Sonarr's key.
fn owing() -> Baseline {
    let mut baseline = Baseline::new();
    baseline.record("seerr", OWED_SONARR, "owed", "1");
    baseline
}

/// This step over `ctx`, and the baseline it left.
async fn taken(
    ctx: &Ctx,
    services: &[lemonfiber_manifest::Service],
    project: &std::path::Path,
    mut baseline: Baseline,
) -> (Option<State>, Baseline) {
    let wiring = super::super::taken_back::seed_taken_back(
        ctx,
        services,
        &fillers_at(services.to_vec(), project),
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

#[tokio::test]
async fn a_key_held_at_the_arrs_own_address_is_owed_and_one_at_the_gate_is_not() {
    let direct = household(&sonarr_direct(), 200, &keys(false), 204, 201);
    let (ctx, project) = taking("noted", &direct, false);
    let mut noted = Baseline::new();
    super::super::taken_back::note_held(
        &ctx,
        &gated(),
        &fillers_at(gated(), &project),
        Some(&project),
        &mut noted,
    )
    .await;
    assert_eq!(noted.expected("seerr", OWED_SONARR), Some("owed"));

    let through = household(&sonarr_gated(), 200, &keys(false), 204, 201);
    let (ctx, project) = taking("not-noted", &through, false);
    let mut clean = Baseline::new();
    super::super::taken_back::note_held(
        &ctx,
        &gated(),
        &fillers_at(gated(), &project),
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
        &fillers_at(ungated.clone(), &project),
        Some(&project),
        &mut none,
    )
    .await;
    assert!(none.is_empty());
}

#[tokio::test]
async fn a_request_service_that_will_not_say_what_it_holds_owes_nothing_yet() {
    let silent = Fake::by_route(Vec::new());
    let (ctx, project) = taking("unsaid", &silent, false);
    let mut noted = Baseline::new();
    super::super::taken_back::note_held(
        &ctx,
        &gated(),
        &fillers_at(gated(), &project),
        Some(&project),
        &mut noted,
    )
    .await;
    assert!(noted.is_empty());
}

#[tokio::test]
async fn nothing_owed_and_nothing_minted_is_not_a_connection() {
    let http = household(&sonarr_gated(), 200, &keys(false), 204, 201);
    let (ctx, project) = taking("nothing", &http, false);
    let (state, _) = taken(&ctx, &gated(), &project, Baseline::new()).await;
    assert_eq!(state, None);
}

#[tokio::test]
async fn a_rehearsal_counts_what_is_held_and_replaces_nothing() {
    let http = household(&sonarr_gated(), 200, &keys(true), 204, 201);
    let (ctx, project) = taking("rehearsed", &http, true);
    let (state, baseline) = taken(&ctx, &gated(), &project, owing()).await;
    assert_eq!(
        state,
        Some(State::WouldWire {
            yours: Some("2 credentials held".to_owned()),
            ours: Some("none".to_owned()),
        })
    );
    assert_eq!(baseline.expected("seerr", OWED_SONARR), Some("owed"));
    assert!(!http.asked_for("/command"));
}

#[tokio::test]
async fn one_held_credential_is_counted_as_one() {
    let http = household(&sonarr_gated(), 200, &keys(true), 204, 201);
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
    let http = household(&sonarr_direct(), 200, &keys(true), 204, 201);
    let (ctx, project) = taking("still-direct", &http, false);
    let (state, baseline) = taken(&ctx, &gated(), &project, owing()).await;
    assert_eq!(
        state,
        Some(State::Skipped {
            reason: "Seerr still reaches sonarr the app without the gate, so nothing it held is \
                     replaced yet. A later run replaces them once Seerr reaches everything \
                     through the gate."
                .to_owned(),
        })
    );
    assert_eq!(baseline.expected("seerr", OWED_SONARR), Some("owed"));
    assert!(!http.asked_for("/command"));
    assert!(!http
        .requests()
        .iter()
        .any(|asked| asked.method == Method::Delete));
}

#[tokio::test]
async fn a_target_at_the_gate_that_fails_its_test_is_still_direct() {
    let http = household(&sonarr_gated(), 500, &keys(false), 204, 201);
    let (ctx, project) = taking("untested", &http, false);
    let (state, _) = taken(&ctx, &gated(), &project, owing()).await;
    assert!(
        matches!(&state, Some(State::Skipped { reason }) if reason.contains("reaches sonarr")),
        "{state:?}"
    );
}

#[tokio::test]
async fn a_request_service_that_does_not_answer_is_left_for_a_later_run() {
    let http = Fake::by_route_in_turn(vec![
        (
            Method::Post,
            "/Users/AuthenticateByName",
            vec![Answer::reply(
                200,
                r#"{"AccessToken":"token","User":{"Id":"admin-id"}}"#,
            )],
        ),
        (
            Method::Get,
            "/Auth/Keys",
            vec![Answer::reply(200, r#"{"Items":[]}"#)],
        ),
    ]);
    let (ctx, project) = taking("unread", &http, false);
    let (state, baseline) = taken(&ctx, &gated(), &project, owing()).await;
    assert!(matches!(state, Some(State::Skipped { .. })), "{state:?}");
    assert_eq!(baseline.expected("seerr", OWED_SONARR), Some("owed"));
}

#[tokio::test]
async fn the_request_services_own_jellyfin_key_is_revoked() {
    let http = household(&sonarr_gated(), 200, &keys(true), 204, 201);
    let (ctx, project) = taking("revoked", &http, false);
    let (state, _) = taken(&ctx, &gated(), &project, Baseline::new()).await;
    assert_eq!(state, Some(State::Wired));
    assert!(http
        .requests()
        .iter()
        .any(|asked| asked.method == Method::Delete
            && asked.url.ends_with("/Auth/Keys/seerr-minted")));
}

#[tokio::test]
async fn a_refused_revoke_is_said_and_left_for_the_next_run() {
    let http = household(&sonarr_gated(), 200, &keys(true), 500, 201);
    let (ctx, project) = taking("unrevoked", &http, false);
    let (state, _) = taken(&ctx, &gated(), &project, Baseline::new()).await;
    assert!(
        matches!(&state, Some(State::Failed { detail })
            if detail.starts_with("Seerr's own Jellyfin key could not be revoked: ")
                && detail.ends_with("It still opens Jellyfin; the next run revokes it.")),
        "{state:?}"
    );
}

#[tokio::test]
async fn a_refused_reset_is_said_and_stays_owed() {
    let http = household(&sonarr_gated(), 200, &keys(false), 204, 500);
    let (ctx, project) = taking("refused", &http, false);
    let (state, baseline) = taken(&ctx, &gated(), &project, owing()).await;
    assert!(
        matches!(&state, Some(State::Failed { detail })
            if detail.contains("API key could not be replaced: the service would not replace its key")),
        "{state:?}"
    );
    assert_eq!(baseline.expected("seerr", OWED_SONARR), Some("owed"));
}

#[tokio::test]
async fn a_landed_reset_is_no_longer_owed() {
    let http = household(&sonarr_gated(), 200, &keys(false), 204, 201);
    let (ctx, project) = taking("landed", &http, false);
    let (state, baseline) = taken(&ctx, &gated(), &project, owing()).await;
    assert_eq!(baseline.expected("seerr", OWED_SONARR), None, "{state:?}");
    assert!(http.asked_for("/command"));
    // Whatever copy could not be given the new key is named, in the rotation's words.
    if let Some(State::Failed { detail }) = &state {
        assert!(detail.contains("could not be given it"), "{detail}");
    }
}

#[tokio::test]
async fn an_owed_key_of_an_arr_the_stack_no_longer_runs_is_forgotten() {
    let http = household(&sonarr_gated(), 200, &keys(false), 204, 201);
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
        &link_at("jellyfin", 8096, ""),
        &sonarr_gated(),
        200,
        200,
        &keys(true),
        204,
        201,
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
    let http = linked_household(&linked(), "[]", 500, 200, &keys(false), 204, 201);
    let (ctx, project) = taking("unlisted", &http, false);
    let (state, baseline) = taken(&ctx, &gated(), &project, owing()).await;
    assert!(
        matches!(state, Some(State::Failed { .. } | State::Skipped { .. })),
        "{state:?}"
    );
    assert_eq!(baseline.expected("seerr", OWED_SONARR), Some("owed"));
}

#[tokio::test]
async fn without_the_administrators_password_only_the_arr_keys_are_taken_back() {
    let http = household(&sonarr_direct(), 200, &keys(true), 204, 201);
    let (mut ctx, project) = taking("no-admin", &http, false);
    ctx.settings.env_file = None;
    let (state, _) = taken(&ctx, &gated(), &project, owing()).await;
    assert!(
        matches!(&state, Some(State::Skipped { reason }) if reason.starts_with("Seerr still reaches sonarr")),
        "{state:?}"
    );
    assert!(!http
        .requests()
        .iter()
        .any(|asked| asked.url.contains("/Auth/Keys")));
}

#[tokio::test]
async fn a_stack_without_the_request_service_or_the_media_server_has_nothing_to_take_back() {
    let http = household(&sonarr_gated(), 200, &keys(true), 204, 201);
    let (ctx, project) = taking("no-seerr", &http, false);
    let without = |id: &str| -> Vec<_> {
        gated()
            .into_iter()
            .filter(|service| service.id != id)
            .collect()
    };
    let (state, _) = taken(&ctx, &without("seerr"), &project, owing()).await;
    assert_eq!(state, None);
    let (state, _) = taken(&ctx, &without("jellyfin"), &project, Baseline::new()).await;
    assert_eq!(state, None);
}

#[tokio::test]
async fn without_the_administrators_password_the_arr_keys_are_still_replaced() {
    let http = household(&sonarr_gated(), 200, &keys(true), 204, 201);
    let (mut ctx, project) = taking("no-admin-landed", &http, false);
    // A settings file to record the replaced key in, holding no administrator password.
    let env = config_scratch("taken-no-admin-landed");
    ctx.settings.env_file = Some(env.to_path_buf());
    let (state, baseline) = taken(&ctx, &gated(), &project, owing()).await;
    assert_eq!(baseline.expected("seerr", OWED_SONARR), None, "{state:?}");
    assert!(http.asked_for("/command"));
    assert!(!http
        .requests()
        .iter()
        .any(|asked| asked.method == Method::Delete));
}
