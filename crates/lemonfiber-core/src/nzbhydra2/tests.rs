use lemonfiber_fixtures::http::{Answer, Fake};
use lemonfiber_ports::http::Method;

use super::{indexers, Credential, Nzbhydra2, Unguarded};
use crate::ports::service::Failure;

/// A client over `http`.
fn client(http: std::sync::Arc<Fake>) -> Nzbhydra2 {
    Nzbhydra2::new(http, "http://127.0.0.1:5076", "nzbhydra2")
}

/// The administrator the tests present.
const ADMIN: Credential<'static> = Credential {
    username: "admin",
    password: "kept",
};

/// A read of the configuration answered with the configuration is exposed, one refused
/// is not, and anything else is the service's own refusal rather than either.
#[tokio::test]
async fn whether_the_configuration_is_exposed_is_read_off_the_answer() {
    let exposed = client(Fake::always(Answer::reply(200, "{}")))
        .exposed()
        .await;
    let guarded = client(Fake::always(Answer::reply(401, ""))).exposed().await;
    let forbidden = client(Fake::always(Answer::reply(403, ""))).exposed().await;
    let broken = client(Fake::always(Answer::reply(500, "boom")))
        .exposed()
        .await;

    assert_eq!(exposed.ok(), Some(true));
    assert_eq!(guarded.ok(), Some(false));
    assert_eq!(forbidden.ok(), Some(false));
    assert!(matches!(broken, Err(Failure::Refused { .. })));
}

/// The configuration read with a credential presents it as the service's authentication
/// takes it, and one read with none presents nothing.
#[tokio::test]
async fn a_credential_is_presented_only_where_one_is_given() {
    let http = Fake::always(Answer::reply(200, r#"{"indexers":[]}"#));

    let presented = client(http.clone()).config(Some(ADMIN)).await;
    let bare = client(http.clone()).config(None).await;

    assert!(presented.is_ok() && bare.is_ok());
    let headers: Vec<Vec<(String, String)>> = http
        .requests()
        .into_iter()
        .map(|asked| asked.headers)
        .collect();
    assert_eq!(
        headers.first().and_then(|first| first
            .iter()
            .find(|(name, _)| name == "Authorization")
            .map(|(_, value)| value.clone())),
        Some("Basic YWRtaW46a2VwdA==".to_owned())
    );
    assert!(headers
        .get(1)
        .is_some_and(|second| second.iter().all(|(name, _)| name != "Authorization")));
}

/// Guarding hands back everything the configuration held, with authentication turned on
/// for the administrator alone and every part of the service guarded.
#[tokio::test]
async fn guarding_keeps_everything_else_and_guards_every_part() {
    let http = Fake::by_route(vec![(
        Method::Put,
        "/internalapi/config",
        Answer::reply(200, r#"{"ok":true}"#),
    )]);
    let config = serde_json::json!({
        "auth": { "authType": "NONE", "users": [] },
        "indexers": [{ "name": "Dummy" }],
        "main": { "apiKey": "unchanged" },
    });

    let guarded = client(http.clone()).guard(config, ADMIN).await;

    assert!(guarded.is_ok());
    let sent: serde_json::Value = http
        .request()
        .and_then(|asked| asked.body)
        .and_then(|body| serde_json::from_str(&body).ok())
        .unwrap_or_default();
    let at = |pointer: &str| sent.pointer(pointer).cloned();
    assert_eq!(at("/auth/authType"), Some("BASIC".into()));
    for restricted in super::RESTRICTED {
        assert_eq!(
            at(&format!("/auth/{restricted}")),
            Some(true.into()),
            "{restricted}"
        );
    }
    assert_eq!(at("/auth/users/0/username"), Some("admin".into()));
    assert_eq!(at("/auth/users/0/maySeeAdmin"), Some(true.into()));
    assert_eq!(at("/indexers/0/name"), Some("Dummy".into()));
    assert_eq!(at("/main/apiKey"), Some("unchanged".into()));
}

/// A configuration with nothing to turn on is refused before anything is sent, and one
/// the service rejects or says it will not take is refused in its own words: nothing
/// changed. One sent and answered unreadably, or not answered, may have been taken.
#[tokio::test]
async fn a_configuration_that_cannot_be_guarded_is_refused() {
    let http = Fake::always(Answer::reply(
        200,
        r#"{"ok":false,"errorMessages":["first","second"]}"#,
    ));

    let unguardable = client(http.clone())
        .guard(serde_json::json!({}), ADMIN)
        .await;
    let unsent = http.requests().is_empty();
    let untaken = client(http)
        .guard(serde_json::json!({ "auth": {} }), ADMIN)
        .await;
    let unread = client(Fake::always(Answer::reply(200, "not json")))
        .guard(serde_json::json!({ "auth": {} }), ADMIN)
        .await;
    let rejected = client(Fake::always(Answer::reply(400, "bad request")))
        .guard(serde_json::json!({ "auth": {} }), ADMIN)
        .await;
    let unanswered = client(Fake::silent())
        .guard(serde_json::json!({ "auth": {} }), ADMIN)
        .await;

    assert!(
        matches!(unguardable, Err(Unguarded::Untaken(Failure::Refused { detail, .. }))
        if detail.contains("no authentication section"))
    );
    assert!(unsent);
    assert!(
        matches!(untaken, Err(Unguarded::Untaken(Failure::Refused { detail, .. }))
        if detail == "first; second")
    );
    assert!(matches!(rejected, Err(Unguarded::Untaken(_))));
    assert!(matches!(unread, Err(Unguarded::Unknown(_))));
    assert!(matches!(unanswered, Err(Unguarded::Unknown(_))));
}

/// How the service is guarded is read off what it says, and a restart is asked for and
/// taken as done only where the service says so.
#[tokio::test]
async fn access_and_the_restart_are_read_off_the_answer() {
    let access = client(Fake::always(Answer::reply(
        200,
        r#"{"authConfigured":true,"authType":"BASIC"}"#,
    )))
    .access()
    .await;
    let unread = client(Fake::always(Answer::reply(200, "not json")))
        .access()
        .await;
    let restarted = client(Fake::always(Answer::reply(200, "{}")))
        .restart()
        .await;
    let refused = client(Fake::always(Answer::reply(401, ""))).restart().await;

    assert_eq!(access.ok().map(|access| access.auth_configured), Some(true));
    assert!(unread.is_err());
    assert!(restarted.is_ok());
    assert!(matches!(refused, Err(Failure::Unauthorised { .. })));
}

/// Every indexer a configuration holds is named; one holding an empty list names none,
/// and one holding no list is not read as holding none.
#[test]
fn the_indexers_are_named_in_the_order_held() {
    assert_eq!(
        indexers(
            &serde_json::json!({ "indexers": [{ "name": "A" }, { "host": "x" }, { "name": "B" }] })
        ),
        Some(vec!["A".to_owned(), "B".to_owned()])
    );
    assert_eq!(
        indexers(&serde_json::json!({ "indexers": [] })),
        Some(Vec::new())
    );
    assert_eq!(indexers(&serde_json::json!({})), None);
}
