//! The request service as the household tests drive it: one scripted transport, and
//! the installs that reach it.
//!
//! Shared because two seams drive the same service. What a household may ask for is
//! settings and quotas; what becomes of a refusal's words is a decision and a message.
//! Both need the whole exchange answered — the sign-in, the member, the counts, the
//! decision and the reading afterwards — and a second copy of that table would be a
//! second idea of what the service says.
//!
//! The request service has moved into its plugin, so the table answers `request.intake`
//! as its adapter would. **Answered by route rather than in turn.** Several of these
//! calls read before they write, so a queue would prove only that the right number of
//! requests went out.

use std::sync::Arc;

use lemonfiber_core::app::Ctx;
use lemonfiber_core::config::{Reaching, Settings};
use lemonfiber_core::ports::service::{
    Address, Asking, Certificate, Headroom, HouseholdRequest, Left, MediaStatus, NamedLibrary,
    RequestStatus, Requesting,
};
use lemonfiber_core::stack::Source;
use lemonfiber_fixtures::http::{Answer, Fake};
use lemonfiber_fixtures::support::Reporting;
use lemonfiber_ports::docker::{Health, Lifecycle};
use lemonfiber_ports::media::Kind;
use lemonfiber_testing::media;
use lemonfiber_testing::requests::{self, REQUESTS};

/// The identifier the request service files the household's one member under.
pub const MEMBER: &str = "4";

/// Where the member who asked already hears from the request service.
///
/// Both agents, because a member who gave two addresses and asked to hear on both is the
/// case where every arm of the sending runs. The one who switched an agent off is a case
/// of its own below.
#[must_use]
pub fn reached_at() -> Vec<Address> {
    let mut both = half_reached_at();
    both.push(Address::Pushbullet {
        token: "the-access-token".to_owned(),
    });
    both
}

/// The same member, with one of the two agents left switched off.
#[must_use]
pub fn half_reached_at() -> Vec<Address> {
    vec![Address::Pushover {
        user: "the-user-key".to_owned(),
        application: "the-application-token".to_owned(),
        sound: Some("bike".to_owned()),
    }]
}

/// A scratch environment file holding the media server's recorded password.
#[track_caller]
pub fn recorded_admin(name: &str) -> std::path::PathBuf {
    let dir = lemonfiber_fixtures::scratch::Scratch::named(name).kept();
    let _ = std::fs::create_dir_all(&dir);
    let env = dir.join(".env");
    let _ = lemonfiber_core::config::store::set(
        &env,
        lemonfiber_core::config::IDENTITY_SOURCE_ADMIN_PASSWORD_KEY,
        &["minted", "-earlier"].concat(),
    );
    env
}

/// The shipped stack, in a directory of its own.
///
/// Leaked because `Source::External` holds a `&'static Path`.
#[track_caller]
pub fn stack_written(name: &str) -> Source {
    let dir = lemonfiber_fixtures::scratch::Scratch::named(&format!("{name}-stack")).kept();
    lemonfiber_fixtures::stack::manifest_into(&dir);
    Source::External(Box::leak(dir.into_boxed_path()))
}

/// [`stack_written`], named after the scratch directory `env` sits in, for the
/// tests that already keep one per case.
#[track_caller]
pub fn stack_beside(env: &std::path::Path) -> Source {
    let name = env
        .parent()
        .and_then(std::path::Path::file_name)
        .and_then(std::ffi::OsStr::to_str)
        .unwrap_or("beside");
    stack_written(name)
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

/// The same, with one operation answering a refusal instead.
///
/// One rule rather than a whole transport per case: every write here reaches the
/// service more than once, and what each of these holds is that the *later* calls
/// leave the household as it was — which a fixture that refused everything could not
/// tell apart from never having been asked.
pub fn refusing(name: &str, route: &'static str) -> Ctx {
    with(name, vec![(route, requests::refusing())])
}

/// The transport these run against, with any broken rule ahead of the working ones.
pub fn with(name: &str, broken: Vec<(&'static str, Answer)>) -> Ctx {
    context(name, &table(broken))
}

/// The household's one request, as the request service records it.
#[must_use]
pub fn the_request() -> HouseholdRequest {
    HouseholdRequest {
        id: 7,
        made: Some("2026-08-17T21:04:09.000Z".to_owned()),
        member: "Alex".to_owned(),
        member_id: None,
        kind: Some(Kind::Movies),
        title_id: None,
        item: Some(3),
        arrived: None,
        shelf_id: None,
        request_status: Some(RequestStatus::Pending),
        media_status: Some(MediaStatus::Pending),
    }
}

/// The routes, with any broken rule ahead of the working ones.
pub fn table(broken: Vec<(&'static str, Answer)>) -> Arc<Fake> {
    let mut routes = broken;
    routes.extend(vec![
        (
            media::identity("household"),
            media::answered(&[media::member("a1", "Alex", true)]),
        ),
        (
            media::identity("libraries"),
            media::answered(&Vec::<NamedLibrary>::new()),
        ),
        (
            media::identity("ratings"),
            media::answered(&Vec::<Certificate>::new()),
        ),
        requests::answering(
            "asking",
            &Asking {
                approves_own: true,
                quota: None,
            },
        ),
        requests::answering("member_for", &Some(MEMBER)),
        requests::answering(
            "requesting",
            &Some(Requesting {
                id: MEMBER.to_owned(),
                approves_own: true,
            }),
        ),
        requests::answering(
            "left",
            &Headroom {
                films: Left {
                    limit: Some(5),
                    used: 5,
                    days: Some(7),
                },
                television: Left {
                    limit: None,
                    used: 0,
                    days: Some(7),
                },
            },
        ),
        requests::answering("reachable", &reached_at()),
        requests::answering("requests", &vec![the_request()]),
        (requests::intake("answers"), requests::taken()),
        (requests::intake("decide"), requests::taken()),
        (requests::intake("set_asking"), requests::taken()),
        (requests::intake("set_quota"), requests::taken()),
        (requests::intake("approves_own"), requests::taken()),
        (requests::intake("set_notices"), requests::taken()),
        ("", Answer::reply(200, "[]")),
    ]);
    Fake::by_path(routes)
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
    let ctx = lemonfiber_testing::a_context()
        .over(stack_written(name))
        .engine(Arc::new(
            Reporting::holding(&["jellyfin", "seerr"], Lifecycle::Running, Health::Healthy)
                .alongside(media::running())
                .alongside(REQUESTS.running()),
        ))
        .settings(Settings {
            env_file: Some(recorded_admin(name)),
            reaching: allowed,
            ..Settings::default()
        })
        .build()
        .with_http(transport.clone());
    media::moved_in(&ctx);
    REQUESTS.moved_in(&ctx);
    ctx
}

/// The shipped stack with one service's block taken out.
///
/// Leaked because `Source::External` holds a `&'static Path`, and kept per-service so
/// two tests wanting different omissions do not share one directory.
pub fn stack_without(service: &str, tag: &str) -> Source {
    let to = lemonfiber_fixtures::scratch::Scratch::named(&format!("without-{tag}")).kept();
    lemonfiber_fixtures::stack::manifest_without(&to, service);
    Source::External(Box::leak(to.into_boxed_path()))
}
