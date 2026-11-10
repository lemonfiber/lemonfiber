//! Each new alert owed to every back-end that receives the stack's, delivered, tried again
//! on its schedule, and held where its secret is refused.

use std::sync::Arc;
use std::time::Duration;

use lemonfiber_fixtures::http::{Answer, Fake};

use super::{any_configured, is_due, owe, settled, Fanning};
use crate::alert::{Alert, Moment, Notification};
use crate::app::Ctx;
use crate::config::Settings;
use crate::error::Severity;
use crate::model::{BackendKind, BackendProof, ChosenBy, DeliveryOutcome};
use crate::notify::backends::{self, Backend, Kept, Owed};
use crate::notify::configuring::{asked, Adding, Asked};
use crate::ports::http::{Http, Method};

const NOW: u64 = 1_793_000_000;

fn alert(check: &str, severity: Severity) -> Alert {
    Alert {
        id: Some(Alert::named(check, 1)),
        check: check.to_owned(),
        kind: "service.down".to_owned(),
        notification: Notification::Problem,
        moment: Moment::Onset,
        severity,
        summary: format!("{check} stopped"),
        meaning: "nothing works".to_owned(),
        remedies: Vec::new(),
        affected: vec![check.to_owned()],
        exit: None,
    }
}

fn backend(id: &str, chosen_by: ChosenBy) -> Backend {
    Backend {
        id: id.to_owned(),
        kind: BackendKind::Ntfy,
        url: "http://ntfy.home".to_owned(),
        topic: Some("stack".to_owned()),
        chosen_by,
        hosted: false,
        members: None,
        receivers: Vec::new(),
        attempts: 3,
        delays: vec![1, 5],
        proof: BackendProof::Unproven,
        testing: None,
        last: None,
        failures: Vec::new(),
        retry: None,
        owed: Vec::new(),
        added: NOW,
    }
}

fn owed(summary: &str) -> Owed {
    Owed {
        kind: Notification::Problem,
        summary: summary.to_owned(),
        urgent: false,
        at: NOW,
        attempts: 0,
        next: None,
    }
}

fn first(backend: &Backend) -> Owed {
    backend
        .owed
        .first()
        .cloned()
        .unwrap_or_else(|| owed("nothing owed"))
}

#[test]
fn nothing_is_owed_at_the_first_reading_and_each_new_alert_after_to_the_operators_alone() {
    let mut kept = Kept {
        backends: vec![
            backend("home", ChosenBy::Operator),
            backend("ada-phone", ChosenBy::Member("ada".to_owned())),
        ],
        told: None,
    };

    owe(&mut kept, &[alert("jellyfin", Severity::Warning)], NOW);
    let first: Vec<usize> = kept.backends.iter().map(|one| one.owed.len()).collect();
    owe(
        &mut kept,
        &[
            alert("jellyfin", Severity::Warning),
            alert("sonarr", Severity::Error),
        ],
        NOW,
    );

    assert_eq!(first, [0, 0]);
    let owing: Vec<Vec<(String, bool)>> = kept
        .backends
        .iter()
        .map(|one| {
            one.owed
                .iter()
                .map(|owed| (owed.summary.clone(), owed.urgent))
                .collect()
        })
        .collect();
    assert_eq!(owing.len(), 2);
    assert!(
        matches!(owing.first().map(Vec::as_slice), Some([(summary, true)]) if summary.contains("sonarr stopped")),
        "{owing:?}"
    );
    assert_eq!(owing.get(1).map(Vec::len), Some(0));
}

#[test]
fn a_back_end_owes_no_more_than_its_most_and_a_held_one_is_never_due() {
    let mut kept = Kept {
        backends: vec![backend("home", ChosenBy::Operator)],
        told: Some(Vec::new()),
    };
    let many: Vec<Alert> = (0..60)
        .map(|at| alert(&format!("service-{at}"), Severity::Warning))
        .collect();

    owe(&mut kept, &many, NOW);

    let Some(mut home) = kept.backends.first().cloned() else {
        unreachable!("one back-end is kept")
    };
    assert_eq!(home.owed.len(), super::MOST_OWED);
    assert!(is_due(&home, NOW));
    if let Some(first) = home.owed.first_mut() {
        first.next = Some(NOW + 60);
    }
    assert!(!is_due(&home, NOW));
    assert!(is_due(&home, NOW + 60));
    home.proof = BackendProof::Held("refused".to_owned());
    assert!(!is_due(&home, NOW + 60));
}

#[test]
fn a_failed_delivery_is_tried_again_on_its_schedule_and_then_stands_failed() {
    let mut home = backend("home", ChosenBy::Operator);
    home.owed = vec![owed("first"), owed("second")];
    let failed = DeliveryOutcome::Failed("503 busy".to_owned());

    let sent = first(&home);
    settled(&mut home, &sent, &failed, false, NOW);
    assert_eq!(first(&home).next, Some(NOW + 60));
    assert_eq!(
        home.retry.as_ref().map(|retry| (retry.attempt, retry.of)),
        Some((2, 3))
    );

    let sent = first(&home);
    settled(&mut home, &sent, &failed, false, NOW + 60);
    assert_eq!(first(&home).next, Some(NOW + 60 + 300));

    let sent = first(&home);
    settled(&mut home, &sent, &failed, false, NOW + 400);
    assert_eq!(home.owed.len(), 1);
    assert_eq!(first(&home).summary, "second");
    assert_eq!(home.retry, None);
    assert_eq!(home.failures.len(), 3);

    let sent = first(&home);
    settled(
        &mut home,
        &sent,
        &DeliveryOutcome::Confirmed,
        false,
        NOW + 500,
    );
    assert!(home.owed.is_empty());
    assert!(home.failures.is_empty());
    assert_eq!(
        home.last.map(|last| last.outcome),
        Some(DeliveryOutcome::Confirmed)
    );
}

#[test]
fn a_refused_secret_holds_the_back_end_and_keeps_what_it_owes() {
    let mut home = backend("home", ChosenBy::Operator);
    home.owed = vec![owed("first")];
    let sent = first(&home);

    settled(
        &mut home,
        &sent,
        &DeliveryOutcome::Failed("403".to_owned()),
        true,
        NOW,
    );

    assert!(matches!(home.proof, BackendProof::Held(_)));
    assert_eq!(home.owed, vec![sent]);
    assert_eq!(first(&home).attempts, 0);
}

#[test]
fn a_message_sent_but_not_seen_is_delivered_and_one_no_longer_first_is_left_alone() {
    let mut home = backend("home", ChosenBy::Operator);
    home.owed = vec![owed("first")];
    let elsewhere = owed("not this one");

    settled(&mut home, &elsewhere, &DeliveryOutcome::Sent, false, NOW);
    assert_eq!(home.owed.len(), 1);
    let sent = first(&home);
    settled(&mut home, &sent, &DeliveryOutcome::Sent, false, NOW);
    assert!(home.owed.is_empty());
}

fn machine(name: &str, http: Arc<Fake>) -> Arc<Ctx> {
    let dir = lemonfiber_fixtures::scratch::Scratch::named(name).kept();
    let _ = std::fs::remove_dir_all(&dir);
    let _ = std::fs::create_dir_all(dir.join("config"));
    Arc::new(
        crate::test_support::a_context()
            .settings(Settings {
                env_file: Some(dir.join("config").join(".env")),
                stack_dir: Some(dir.join("data").join("stack")),
                ..Settings::default()
            })
            .build()
            .with_http(http as Arc<dyn Http>),
    )
}

#[tokio::test]
async fn a_new_alert_is_delivered_to_a_configured_back_end_and_confirmed_in_its_topic() {
    let http = Fake::by_route(vec![
        (
            Method::Get,
            "/stack/json",
            Answer::reply(200, "{\"id\":\"m7\"}\n"),
        ),
        (
            Method::Post,
            "/stack",
            Answer::reply(200, "{\"id\":\"m7\"}"),
        ),
    ]);
    let ctx = machine("notify-fanned", Arc::clone(&http));
    let fanning = Fanning::default();
    assert!(!any_configured(&ctx));
    let _ = asked(
        &ctx,
        Asked::Add(Box::new(Adding {
            id: "home".to_owned(),
            kind: BackendKind::Ntfy,
            url: "http://ntfy.home".to_owned(),
            topic: Some("stack".to_owned()),
            token: None,
            reader: None,
            hosted: false,
            attempts: None,
            delays: None,
        })),
    )
    .await;
    assert!(any_configured(&ctx));

    fanning.owing(&ctx, &[]);
    fanning.owing(&ctx, &[alert("jellyfin", Severity::Error)]);
    let mut last = None;
    for _ in 0..100 {
        last = backends::at(&ctx)
            .and_then(|path| backends::kept(&path).ok())
            .and_then(|kept| kept.backends.into_iter().next())
            .and_then(|one| one.last);
        if last.is_some() {
            break;
        }
        tokio::time::sleep(Duration::from_millis(20)).await;
    }

    assert_eq!(
        last.map(|one| one.outcome),
        Some(DeliveryOutcome::Confirmed)
    );
    let posted = http
        .requests()
        .into_iter()
        .filter(|one| one.method == Method::Post)
        .count();
    assert_eq!(posted, 1);
}

#[tokio::test]
async fn a_member_is_told_only_that_something_is_ready_and_only_on_what_they_chose() {
    let ctx = machine("notify-ready", Fake::always(Answer::reply(500, "")));
    let offered = |id: &str, receivers: &[&str]| Backend {
        members: Some("Phones".to_owned()),
        receivers: receivers.iter().map(|one| (*one).to_owned()).collect(),
        ..backend(id, ChosenBy::Operator)
    };
    let Some(path) = backends::at(&ctx) else {
        unreachable!("the machine keeps its files somewhere")
    };
    let _ = backends::changed(&path, &mut |kept: &mut Kept| {
        kept.backends = vec![
            offered("phones", &["ada"]),
            offered("tablets", &["bo"]),
            backend("operator", ChosenBy::Operator),
        ];
        Ok(())
    });

    super::ready(&ctx, "ada");

    let kept = backends::kept(&path).unwrap_or_default();
    let owing: Vec<(String, Vec<(Notification, String)>)> = kept
        .backends
        .iter()
        .map(|one| {
            (
                one.id.clone(),
                one.owed
                    .iter()
                    .map(|owed| (owed.kind, owed.summary.clone()))
                    .collect(),
            )
        })
        .collect();
    assert_eq!(
        owing,
        [
            (
                "phones".to_owned(),
                vec![(Notification::Ready, super::READY.to_owned())]
            ),
            ("tablets".to_owned(), Vec::new()),
            ("operator".to_owned(), Vec::new()),
        ]
    );
    let mut stack = kept;
    stack.told = Some(Vec::new());
    owe(&mut stack, &[alert("jellyfin", Severity::Error)], NOW);
    let told: Vec<usize> = stack.backends.iter().map(|one| one.owed.len()).collect();
    assert_eq!(told, [1, 0, 1]);
}
