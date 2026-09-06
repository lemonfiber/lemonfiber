//! The request service as the household tests drive it: one scripted transport, and
//! the installs that reach it.
//!
//! Shared because two seams drive the same service. What a household may ask for is
//! settings and quotas; what becomes of a refusal's words is a decision and a message.
//! Both need the whole exchange answered — the sign-in, the member, the counts, the
//! decision and the reading afterwards — and a second copy of that table would be a
//! second idea of what the service says.
//!
//! **Answered by route rather than in turn.** Several of these calls read before they
//! write, so a queue would prove only that the right number of requests went out; the
//! defect worth catching is a narrow body, which a queue cannot see at all.

use std::sync::Arc;

use lemonfiber_core::app::Ctx;
use lemonfiber_core::config::{Reaching, Settings};
use lemonfiber_core::platform::Environment;
use lemonfiber_core::ports::http::Method;
use lemonfiber_core::stack::Source;
use lemonfiber_fixtures::http::{Answer, Fake};
use lemonfiber_fixtures::ports::Stopped;
use lemonfiber_fixtures::support::{spoke, Reporting, Scripted};
use lemonfiber_ports::docker::{Health, Lifecycle};

/// Where the member who asked already hears from the request service.
///
/// Both agents carrying the bit the service files a refusal under, because a member who
/// gave two addresses and asked to hear on both is the case where every arm of the
/// sending runs. The one who switched an agent off is a case of its own below.
pub const REACHED_AT: &str = r#"{"pushoverUserKey":"the-user-key",
    "pushoverApplicationToken":"the-application-token","pushoverSound":"bike",
    "pushbulletAccessToken":"the-access-token",
    "notificationTypes":{"pushover":64,"pushbullet":64}}"#;

/// The same member, with one of the two agents left switched off.
///
/// Nought is what the service stores for anybody who has never chosen, and it is what it
/// treats as telling them nothing on that agent.
pub const HALF_REACHED_AT: &str = r#"{"pushoverUserKey":"the-user-key",
    "pushoverApplicationToken":"the-application-token",
    "pushbulletAccessToken":"the-access-token",
    "notificationTypes":{"pushover":64,"pushbullet":0}}"#;

/// The stack this repository ships.
pub fn stack() -> Source {
    Source::External(std::path::Path::new(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../assets/media-stack"
    )))
}

/// A scratch environment file holding the media server's recorded password.
pub fn recorded_admin(name: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!("lemonfiber-asking-{}-{name}", std::process::id()));
    let _ = std::fs::create_dir_all(&dir);
    let env = dir.join(".env");
    let _ = lemonfiber_core::config::store::set(
        &env,
        lemonfiber_core::config::JELLYFIN_ADMIN_PASSWORD_KEY,
        &["minted", "-earlier"].concat(),
    );
    env
}

/// A context over a transport that answers everything both writes ask.
///
/// The refusing case next door proves the dispatcher reaches these commands; this one
/// proves what they do when the service answers. Both are wanted from *outside* the
/// crate: the app layer is compiled twice, and a branch driven only from the in-crate
/// tests is counted as never run in the copy these binaries link.
pub fn answering(name: &str) -> Ctx {
    with(name, Vec::new())
}

/// The same, with the transport kept so what it was sent can be read back.
///
/// Wanted for one question only — whether a rehearsal writes — and a question about
/// what did *not* go out cannot be asked of a context that swallowed its transport.
pub fn watched(name: &str) -> (Ctx, Arc<Fake>) {
    let transport = table(Vec::new());
    (context(name, &transport), transport)
}

/// The same, with one call answering a refusal instead.
///
/// One rule rather than a whole transport per case: every write here reaches the
/// service more than once, and what each of these holds is that the *later* calls
/// leave the household as it was — which a fixture that refused everything could not
/// tell apart from never having been asked.
pub fn refusing(name: &str, method: Method, route: &'static str) -> Ctx {
    with(name, vec![(Some(method), route, Answer::reply(500, "no"))])
}

/// The transport these run against, with any broken rule ahead of the working ones.
pub fn with(name: &str, broken: Vec<(Option<Method>, &'static str, Answer)>) -> Ctx {
    context(name, &table(broken))
}

/// The routes, with any broken rule ahead of the working ones.
pub fn table(broken: Vec<(Option<Method>, &'static str, Answer)>) -> Arc<Fake> {
    let mut routes = broken;
    routes.extend(vec![
        // Ahead of `/Users`, whose text it contains: a route matched by prefix would
        // answer the sign-in with the list of accounts.
        (
            None,
            "/Users/AuthenticateByName",
            Answer::reply(200, r#"{"AccessToken":"token"}"#),
        ),
        (
            None,
            "/Library/MediaFolders",
            Answer::reply(200, r#"{"Items":[]}"#),
        ),
        (
            None,
            "/Localization/ParentalRatings",
            Answer::reply(200, "[]"),
        ),
        (
            None,
            "/Users",
            Answer::reply(
                200,
                r#"[{"Id":"a1","Name":"Alex","HasPassword":true,
                    "Policy":{"EnableAllFolders":true}}]"#,
            ),
        ),
        (None, "/auth/jellyfin", Answer::reply(200, "{}")),
        (
            None,
            "/settings/main",
            Answer::reply(
                200,
                r#"{"defaultPermissions":160,"defaultQuotas":{"movie":{},"tv":{}}}"#,
            ),
        ),
        (
            None,
            "/user/jellyfin/",
            Answer::reply(200, r#"{"id":4,"permissions":160}"#),
        ),
        (
            None,
            "/user/4/quota",
            // At their limit, so the line saying so — and the sentence that says what
            // they have left and when there is room again — is built here too.
            Answer::reply(
                200,
                r#"{"movie":{"days":7,"limit":5,"used":5},"tv":{"days":7,"limit":0,"used":0}}"#,
            ),
        ),
        (
            None,
            "settings/permissions",
            Answer::reply(200, r#"{"permissions":160}"#),
        ),
        // Where the member who asked is reached. Ahead of the settings write next
        // door only in reading order; what matters is that it is behind nothing whose
        // fragment this URL also contains.
        (
            None,
            "/user/4/settings/notifications",
            Answer::reply(200, REACHED_AT),
        ),
        // The one request's own record, read to find out whose it is. Named by method
        // as well as by path, because the decision below posts to a URL this fragment
        // is a prefix of.
        (
            Some(Method::Get),
            "/request/7",
            Answer::reply(
                200,
                r#"{"id":7,"requestedBy":{"id":4,"displayName":"Alex"}}"#,
            ),
        ),
        (None, "/request/7/", Answer::reply(200, "{}")),
        (
            None,
            "/api/v1/request",
            Answer::reply(
                200,
                r#"{"pageInfo":{"results":1},"results":[{"id":7,
                    "createdAt":"2026-08-17T21:04:09.000Z","status":1,"type":"movie",
                    "media":{"status":2,"externalServiceId":3},
                    "requestedBy":{"displayName":"Alex"}}]}"#,
            ),
        ),
        (None, "", Answer::reply(200, "[]")),
    ]);
    Fake::by_rules(routes)
}

/// An install reached over the given transport.
pub fn context(name: &str, transport: &Arc<Fake>) -> Ctx {
    reaching(name, transport, Reaching::default())
}

/// The same, with the requests this machine allows itself said explicitly.
///
/// Wanted for one question — what a refusal comes to when the operator has switched
/// off reaching the household — and that question cannot be asked of a context whose
/// settings are always the permissive ones.
pub fn reaching(name: &str, transport: &Arc<Fake>, allowed: Reaching) -> Ctx {
    Ctx::new(
        Arc::new(Scripted(Ok(spoke("")))),
        Arc::new(Reporting::holding(
            &["jellyfin", "seerr"],
            Lifecycle::Running,
            Health::Healthy,
        )),
        Stopped::today(),
        Arc::new(lemonfiber_core::adapters::Disk),
        stack(),
        Settings {
            env_file: Some(recorded_admin(name)),
            reaching: allowed,
            ..Settings::default()
        },
        Environment::MacOs,
    )
    .with_http(transport.clone())
}
