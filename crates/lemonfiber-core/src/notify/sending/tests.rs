//! One message to one back-end as each kind speaks it, and whether its history shows it.

use std::sync::Arc;

use lemonfiber_fixtures::http::{Answer, Fake};

use super::{arrived, bearer, said, seen, send, tag, Message, Sent, REASON_MOST};
use crate::alert::Notification;
use crate::app::Ctx;
use crate::config::{Reaching, Settings, REACH_NTFY_KEY};
use crate::endpoint::AUTHORIZATION;
use crate::model::{BackendKind, BackendProof, ChosenBy, DeliveryOutcome};
use crate::notify::backends::{Backend, Held};
use crate::ports::http::Http;

fn reaching(http: Arc<dyn Http>) -> Ctx {
    crate::test_support::a_context().build().with_http(http)
}

fn faked(answer: Answer) -> (Ctx, Arc<Fake>) {
    let http = Fake::always(answer);
    (reaching(Arc::clone(&http) as Arc<dyn Http>), http)
}

fn backend(kind: BackendKind) -> Backend {
    Backend {
        id: "home".to_owned(),
        kind,
        url: "http://alerts.home".to_owned(),
        topic: (kind == BackendKind::Ntfy).then(|| "stack".to_owned()),
        chosen_by: ChosenBy::Operator,
        hosted: false,
        members: None,
        receivers: Vec::new(),
        attempts: 1,
        delays: Vec::new(),
        proof: BackendProof::Unproven,
        testing: None,
        last: None,
        failures: Vec::new(),
        retry: None,
        owed: Vec::new(),
        added: 0,
    }
}

fn held(token: Option<&str>, reader: Option<&str>) -> Held {
    Held {
        token: token.map(str::to_owned),
        reader: reader.map(str::to_owned),
    }
}

fn message(urgent: bool) -> Message {
    Message {
        kind: Notification::Problem,
        summary: "jellyfin stopped".to_owned(),
        urgent,
        at: 7,
    }
}

fn reason(sent: &Sent) -> String {
    match &sent.outcome {
        DeliveryOutcome::Failed(reason) => reason.clone(),
        _ => String::new(),
    }
}

fn field(http: &Fake, name: &str) -> serde_json::Value {
    http.request()
        .and_then(|request| request.body)
        .and_then(|body| serde_json::from_str::<serde_json::Value>(&body).ok())
        .and_then(|body| body.get(name).cloned())
        .unwrap_or_default()
}

fn header(http: &Fake, name: &str) -> Option<String> {
    http.request().and_then(|request| {
        request
            .headers
            .into_iter()
            .find(|(key, _)| key == name)
            .map(|(_, value)| value)
    })
}

#[tokio::test]
async fn a_switched_off_kind_is_never_sent_and_says_which_switch() {
    let http = Fake::always(Answer::reply(200, ""));
    let ctx = crate::test_support::a_context()
        .settings(Settings {
            reaching: Reaching::without(REACH_NTFY_KEY),
            ..Settings::default()
        })
        .build()
        .with_http(Arc::clone(&http) as Arc<dyn Http>);

    let sent = send(
        &ctx,
        &backend(BackendKind::Ntfy),
        &held(None, None),
        &message(false),
    )
    .await;

    assert!(reason(&sent).contains(REACH_NTFY_KEY), "{sent:?}");
    assert!(http.requests().is_empty());
}

#[tokio::test]
async fn what_cannot_be_spoken_to_is_never_sent() {
    let (ctx, http) = faked(Answer::reply(200, ""));
    let topicless = Backend {
        topic: None,
        ..backend(BackendKind::Ntfy)
    };

    for (one, with) in [
        (backend(BackendKind::Unifiedpush), held(None, None)),
        (topicless, held(None, None)),
        (backend(BackendKind::Gotify), held(None, Some("rd"))),
    ] {
        let sent = send(&ctx, &one, &with, &message(false)).await;
        assert_eq!(reason(&sent), "it cannot be spoken to this way");
    }
    assert!(http.requests().is_empty());
}

#[tokio::test]
async fn an_accepted_message_keeps_the_id_it_was_given_however_it_was_written() {
    for (answer, id) in [
        ("{\"id\":\"m7\"}", Some("m7")),
        ("{\"id\":42}", Some("42")),
        ("accepted", None),
    ] {
        let (ctx, _http) = faked(Answer::reply(200, answer));
        let sent = send(
            &ctx,
            &backend(BackendKind::Ntfy),
            &held(None, None),
            &message(false),
        )
        .await;
        assert_eq!(sent.outcome, DeliveryOutcome::Sent);
        assert_eq!(sent.id.as_deref(), id);
        assert!(!sent.refused_key);
    }
}

#[tokio::test]
async fn a_refusal_says_whether_the_key_was_refused_and_is_kept_short() {
    for (status, refused_key) in [(401, true), (403, true), (500, false)] {
        let (ctx, _http) = faked(Answer::reply(status, "no ".repeat(REASON_MOST)));
        let sent = send(
            &ctx,
            &backend(BackendKind::Ntfy),
            &held(None, None),
            &message(false),
        )
        .await;
        assert_eq!(sent.refused_key, refused_key);
        assert!(reason(&sent).starts_with(&status.to_string()));
        assert_eq!(reason(&sent).chars().count(), REASON_MOST);
    }
}

#[tokio::test]
async fn nothing_answering_is_a_failure_in_the_transports_words() {
    let (ctx, _http) = faked(Answer::Silent);
    let sent = send(
        &ctx,
        &backend(BackendKind::Ntfy),
        &held(None, None),
        &message(false),
    )
    .await;
    assert!(matches!(sent.outcome, DeliveryOutcome::Failed(_)));
    assert!(sent.id.is_none());
}

#[tokio::test(start_paused = true)]
async fn a_back_end_that_never_answers_is_given_up_on() {
    let ctx = reaching(Fake::hanging());
    let sent = send(
        &ctx,
        &backend(BackendKind::Ntfy),
        &held(None, None),
        &message(false),
    )
    .await;
    assert_eq!(reason(&sent), "it did not answer in time");
    assert!(
        !arrived(
            &ctx,
            &backend(BackendKind::Ntfy),
            &held(None, None),
            "m7",
            0
        )
        .await
    );
}

#[tokio::test]
async fn gotify_is_posted_its_key_and_a_priority_by_urgency() {
    for (urgent, priority) in [(true, 8), (false, 5)] {
        let (ctx, http) = faked(Answer::reply(200, "{\"id\":3}"));
        let sent = send(
            &ctx,
            &backend(BackendKind::Gotify),
            &held(Some("tk"), None),
            &message(urgent),
        )
        .await;
        assert_eq!(sent.id.as_deref(), Some("3"));
        assert_eq!(header(&http, "X-Gotify-Key").as_deref(), Some("tk"));
        assert_eq!(field(&http, "priority"), priority);
        assert_eq!(field(&http, "message"), "jellyfin stopped");
        assert!(http.asked_for("/message"));
    }
}

#[tokio::test]
async fn apprise_is_posted_a_type_by_urgency_and_no_key() {
    for (urgent, kind) in [(true, "failure"), (false, "info")] {
        let (ctx, http) = faked(Answer::reply(200, ""));
        let sent = send(
            &ctx,
            &backend(BackendKind::Apprise),
            &held(Some("tk"), None),
            &message(urgent),
        )
        .await;
        assert_eq!(sent.outcome, DeliveryOutcome::Sent);
        assert_eq!(field(&http, "type"), kind);
        assert_eq!(field(&http, "body"), "jellyfin stopped");
        assert_eq!(header(&http, AUTHORIZATION), None);
    }
}

#[tokio::test]
async fn a_message_is_seen_only_in_a_history_that_was_read_and_holds_it() {
    let ntfy = backend(BackendKind::Ntfy);
    let gotify = backend(BackendKind::Gotify);
    let topicless = Backend {
        topic: None,
        ..backend(BackendKind::Ntfy)
    };
    let cases = [
        (
            &ntfy,
            held(None, None),
            Answer::reply(200, "{\"id\":\"m7\"}\n"),
            true,
        ),
        (
            &ntfy,
            held(None, None),
            Answer::reply(500, "{\"id\":\"m7\"}\n"),
            false,
        ),
        (&ntfy, held(None, None), Answer::Silent, false),
        (
            &topicless,
            held(None, None),
            Answer::reply(200, "{\"id\":\"m7\"}\n"),
            false,
        ),
        (
            &gotify,
            held(Some("tk"), Some("rd")),
            Answer::reply(200, "{\"messages\":[{\"id\":\"m7\"}]}"),
            true,
        ),
        (
            &gotify,
            held(Some("tk"), None),
            Answer::reply(200, "{\"messages\":[{\"id\":\"m7\"}]}"),
            false,
        ),
    ];
    for (one, with, answer, expected) in cases {
        let (ctx, _http) = faked(answer);
        assert_eq!(arrived(&ctx, one, &with, "m7", 0).await, expected);
    }
    let (ctx, http) = faked(Answer::reply(200, "{\"messages\":[]}"));
    assert!(!arrived(&ctx, &gotify, &held(Some("tk"), Some("rd")), "m7", 0).await);
    assert_eq!(header(&http, "X-Gotify-Key").as_deref(), Some("rd"));
    let (ctx, http) = faked(Answer::reply(200, ""));
    assert!(
        !arrived(
            &ctx,
            &backend(BackendKind::Webhook),
            &held(None, None),
            "m7",
            0
        )
        .await
    );
    assert!(http.requests().is_empty());
}

#[test]
fn only_ntfy_and_gotify_keep_a_history_that_is_read() {
    let line = "{\"id\":\"m7\"}";
    let page = "{\"messages\":[{\"id\":\"m7\"}]}";
    assert!(seen(BackendKind::Ntfy, &format!("not json\n{line}"), "m7"));
    assert!(!seen(BackendKind::Ntfy, line, "m8"));
    assert!(seen(BackendKind::Gotify, page, "m7"));
    assert!(!seen(BackendKind::Gotify, "not json", "m7"));
    for kind in [
        BackendKind::Webhook,
        BackendKind::Apprise,
        BackendKind::Unifiedpush,
    ] {
        assert!(!seen(kind, line, "m7"));
    }
}

#[test]
fn a_login_is_handed_over_as_basic_and_a_token_as_bearer() {
    assert_eq!(
        bearer(Some("lemonfiber:pw")),
        Some((
            AUTHORIZATION.to_owned(),
            "Basic bGVtb25maWJlcjpwdw==".to_owned()
        ))
    );
    assert_eq!(
        bearer(Some("tk")),
        Some((AUTHORIZATION.to_owned(), "Bearer tk".to_owned()))
    );
    assert_eq!(bearer(Some("")), None);
    assert_eq!(bearer(None), None);
}

#[test]
fn every_kind_is_tagged_and_an_id_is_read_however_written() {
    let tags: Vec<&str> = [
        Notification::Problem,
        Notification::Completion,
        Notification::Update,
        Notification::Backup,
        Notification::Request,
        Notification::Member,
        Notification::Ready,
    ]
    .into_iter()
    .map(tag)
    .collect();
    assert_eq!(
        tags,
        [
            "warning",
            "white_check_mark",
            "arrow_up",
            "floppy_disk",
            "inbox_tray",
            "busts_in_silhouette",
            "tada"
        ]
    );
    assert_eq!(said(&serde_json::json!("m7")), "m7");
    assert_eq!(said(&serde_json::json!(42)), "42");
}
