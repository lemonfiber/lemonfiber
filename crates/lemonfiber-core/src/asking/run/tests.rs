use std::sync::Arc;

use lemonfiber_fixtures::http::{Answer, Fake};

use super::{allowing, deciding, found, now_reads, settled, theirs, Ctx};
use crate::app::command::{Answer as Ruling, Chosen, Decision};
use crate::asking::Policy;
use crate::ports::http::Method;
use crate::ports::service::{Asking, Quota};
use crate::test_support::{a_context, a_password, SeedFs};

/// A household trusted within five a week.
const FIVE_A_WEEK: Quota = Quota {
    requests: 5,
    days: 7,
};

/// A Servarr config that opens a target, carrying a readable key.
const KEYED: &str = "<Config><ApiKey>the-key</ApiKey></Config>";

/// The two accounts the media server holds — one ordinary, one nobody by that
/// name, so the forgiving match has something to be forgiving about.
const ACCOUNTS: &str = r#"[{"Id":"a1","Name":"Alex","HasPassword":true,
    "Policy":{"EnableAllFolders":true},"LastActivityDate":"2026-08-30T10:00:00Z"}]"#;

/// Where the member who asked already hears from the request service.
///
/// Both agents carrying the bit a refusal is filed under, because a member who gave
/// two addresses and asked to hear on both is the case where every arm of the
/// sending runs. The one who switched an agent off is a case of its own below.
const REACHED_AT: &str = r#"{"pushoverUserKey":"the-user-key",
    "pushoverApplicationToken":"the-application-token","pushoverSound":"bike",
    "pushbulletAccessToken":"the-access-token",
    "notificationTypes":{"pushover":64,"pushbullet":64}}"#;

/// The same member, with one of the two agents left switched off.
const WAITING: &str = r#"{"pageInfo":{"results":1},"results":[{"id":7,
    "createdAt":"2026-08-17T21:04:09.000Z","status":1,"type":"movie",
    "media":{"status":2,"externalServiceId":3},
    "requestedBy":{"displayName":"Alex"}}]}"#;

/// A context over a transport that answers every read and write these take.
///
/// Routed by what each call asks for rather than scripted in turn, because the
/// answer here is the household read *again* — so the same paths are asked twice
/// and a queue would run out halfway through the second reading.
fn a_household(tag: &str) -> Ctx {
    answering(tag, Vec::new())
}

/// The same household, with the named calls answering a refusal instead.
///
/// One rule per call rather than one transport per case: what each of these holds
/// is that a service which will not answer *that* leaves the household as it was,
/// and a fixture that broke everything at once could not say which call it was.
fn refusing(tag: &str, broken: Vec<(Method, &'static str)>) -> Ctx {
    answering(
        tag,
        broken
            .into_iter()
            .map(|(method, route)| (Some(method), route, Answer::reply(500, "no")))
            .collect(),
    )
}

/// The transport these run against, with the broken rules ahead of the working
/// ones — the first rule whose fragment the address holds is the one that answers.
fn answering(tag: &str, broken: Vec<(Option<Method>, &'static str, Answer)>) -> Ctx {
    let mut routes = broken;
    routes.extend(vec![
        // Ahead of `/Users`, whose text it contains: a route matched by prefix
        // would answer the sign-in with the list of accounts.
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
        (None, "/Users", Answer::reply(200, ACCOUNTS)),
        (None, "/auth/jellyfin", Answer::reply(200, "{}")),
        (
            None,
            "/settings/main",
            Answer::reply(
                200,
                r#"{"defaultPermissions":160,
                    "defaultQuotas":{"movie":{},"tv":{}}}"#,
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
            Answer::reply(
                200,
                r#"{"movie":{"days":7,"limit":5,"used":1},"tv":{"days":7,"limit":0,"used":0}}"#,
            ),
        ),
        (
            None,
            "settings/permissions",
            Answer::reply(200, r#"{"permissions":160}"#),
        ),
        (
            None,
            "/user/4/settings/notifications",
            Answer::reply(200, REACHED_AT),
        ),
        // Named by method as well as by path: the decision below posts to an
        // address this fragment is a prefix of.
        (
            Some(Method::Get),
            "/request/7",
            Answer::reply(200, r#"{"id":7,"requestedBy":{"id":4}}"#),
        ),
        (None, "/request/7/", Answer::reply(200, "{}")),
        (None, "/api/v1/request", Answer::reply(200, WAITING)),
        (None, "", Answer::reply(200, "[]")),
    ]);
    let transport = Fake::by_rules(routes);
    let dir = std::env::temp_dir().join(format!("lemonfiber-asking-{tag}-{}", std::process::id()));
    let _ = std::fs::create_dir_all(&dir);
    let mut context = a_context()
        .build()
        .with_filesystem(Arc::new(SeedFs::keyed(Some(KEYED), None)))
        .with_http(transport);
    context.settings.env_file = Some(dir.join(".env"));
    crate::app::targets::record_secret(
        &context,
        crate::config::JELLYFIN_ADMIN_PASSWORD_KEY,
        &a_password(),
    );
    context
}

mod allowing;
mod deciding;
