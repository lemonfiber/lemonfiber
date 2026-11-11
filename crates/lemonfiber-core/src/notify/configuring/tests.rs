//! Adding, changing, proving and removing the back-ends, and what is kept of each.

use std::sync::Arc;

use lemonfiber_fixtures::http::{Answer, Fake};

use super::{asked, Adding, Asked, Offering, Setting};
use crate::app::{Ctx, Outcome};
use crate::config::Settings;
use crate::error::codes::notify::{
    BACKEND_INCOMPLETE, BACKEND_NAME_TAKEN, HOSTED_UNACKNOWLEDGED, NOT_A_BACKEND_ADDRESS,
    NOT_A_BACKEND_NAME, NOT_A_RETRY_SCHEDULE, NO_SUCH_BACKEND,
};
use crate::error::{Code, Problem};
use crate::model::{BackendKind, BackendProof, NotifyReport, NotifyState};
use crate::ports::http::{Http, Method};

const TOKEN: &str = "tk_kept_apart";

/// A machine keeping its files beneath a directory named for `name`, reaching `http`.
fn machine(name: &str, http: Arc<Fake>) -> Ctx {
    let dir = lemonfiber_fixtures::scratch::Scratch::named(name).kept();
    let _ = std::fs::remove_dir_all(&dir);
    let _ = std::fs::create_dir_all(dir.join("config"));
    crate::test_support::a_context()
        .settings(Settings {
            env_file: Some(dir.join("config").join(".env")),
            stack_dir: Some(dir.join("data").join("stack")),
            ..Settings::default()
        })
        .build()
        .with_http(http as Arc<dyn Http>)
}

fn quiet(name: &str) -> Ctx {
    machine(name, Fake::always(Answer::reply(500, "nothing here")))
}

fn ntfy(id: &str) -> Adding {
    Adding {
        id: id.to_owned(),
        kind: BackendKind::Ntfy,
        url: "http://ntfy.home:2586/".to_owned(),
        topic: Some("stack".to_owned()),
        token: Some(TOKEN.to_owned()),
        reader: None,
        hosted: false,
        attempts: None,
        delays: None,
    }
}

async fn run(ctx: &Ctx, what: Asked) -> Result<NotifyReport, Box<Problem>> {
    asked(ctx, what).await.map(|outcome| match outcome {
        Outcome::Notify(report) => report,
        _ => unreachable!("the back-ends answer with their report"),
    })
}

async fn add(ctx: &Ctx, adding: Adding) -> Result<NotifyReport, Box<Problem>> {
    run(ctx, Asked::Add(Box::new(adding))).await
}

fn code(outcome: &Result<NotifyReport, Box<Problem>>) -> Option<Code> {
    outcome.as_ref().err().map(|problem| problem.code)
}

fn file(at: Option<std::path::PathBuf>) -> String {
    at.and_then(|path| std::fs::read_to_string(path).ok())
        .unwrap_or_default()
}

#[tokio::test]
async fn nothing_is_configured_until_a_back_end_is_added_and_its_secret_is_kept_apart() {
    let ctx = quiet("notify-added");

    let before = run(&ctx, Asked::List).await;
    let added = add(&ctx, ntfy("home-ntfy")).await;

    assert_eq!(
        before.map(|report| report.state).ok(),
        Some(NotifyState::Unconfigured)
    );
    let report = added.unwrap_or_else(|_| unreachable!("an ntfy topic is added"));
    assert_eq!(report.state, NotifyState::Degraded);
    let listed = report.backends.first().cloned();
    assert_eq!(
        listed
            .as_ref()
            .map(|one| (one.reaches.as_str(), &one.proof)),
        Some(("http://ntfy.home:2586/stack", &BackendProof::Unproven))
    );
    let record = file(super::backends::at(&ctx));
    let secrets = file(super::backends::secrets_at(&ctx));
    assert!(!record.contains(TOKEN), "{record}");
    assert!(secrets.contains(TOKEN));
    assert_eq!(
        super::backends::destinations(&ctx),
        [(BackendKind::Ntfy, "http://ntfy.home:2586/stack".to_owned())]
    );
}

#[tokio::test]
async fn a_back_end_is_refused_for_its_name_address_kind_host_or_an_empty_secret() {
    let ctx = quiet("notify-refused");
    let _ = add(&ctx, ntfy("home-ntfy")).await;
    let cases: Vec<(Adding, Code)> = vec![
        (ntfy("Not A Name"), NOT_A_BACKEND_NAME),
        (
            Adding {
                url: "https://me:pw@ntfy.home".to_owned(),
                ..ntfy("logged")
            },
            NOT_A_BACKEND_ADDRESS,
        ),
        (
            Adding {
                url: "ftp://ntfy.home".to_owned(),
                ..ntfy("ftp")
            },
            NOT_A_BACKEND_ADDRESS,
        ),
        (
            Adding {
                topic: None,
                ..ntfy("topicless")
            },
            BACKEND_INCOMPLETE,
        ),
        (
            Adding {
                kind: BackendKind::Gotify,
                token: None,
                ..ntfy("tokenless")
            },
            BACKEND_INCOMPLETE,
        ),
        (
            Adding {
                kind: BackendKind::Unifiedpush,
                ..ntfy("phone")
            },
            BACKEND_INCOMPLETE,
        ),
        (
            Adding {
                token: Some(String::new()),
                ..ntfy("unset")
            },
            BACKEND_INCOMPLETE,
        ),
        (
            Adding {
                url: "https://ntfy.sh".to_owned(),
                ..ntfy("away")
            },
            HOSTED_UNACKNOWLEDGED,
        ),
        (ntfy("home-ntfy"), BACKEND_NAME_TAKEN),
        (
            Adding {
                attempts: Some(3),
                delays: Some(vec![1]),
                ..ntfy("uneven")
            },
            NOT_A_RETRY_SCHEDULE,
        ),
        (
            Adding {
                attempts: Some(11),
                ..ntfy("many")
            },
            NOT_A_RETRY_SCHEDULE,
        ),
    ];

    for (adding, refused) in cases {
        let id = adding.id.clone();
        assert_eq!(code(&add(&ctx, adding).await), Some(refused), "{id}");
    }
    let listed = run(&ctx, Asked::List)
        .await
        .map(|report| report.backends.len());
    assert_eq!(listed.ok(), Some(1));
}

#[tokio::test]
async fn a_hosted_back_end_is_added_once_accepted_and_says_what_leaves_for_it() {
    let ctx = quiet("notify-hosted");

    let added = add(
        &ctx,
        Adding {
            url: "https://ntfy.sh".to_owned(),
            hosted: true,
            ..ntfy("away")
        },
    )
    .await;

    let hosted = added
        .ok()
        .and_then(|report| report.backends.into_iter().next())
        .and_then(|one| one.hosted);
    assert_eq!(
        hosted.as_ref().map(|one| one.host.as_str()),
        Some("ntfy.sh")
    );
    assert!(hosted.is_some_and(|one| !one.leaves.is_empty()));
}

#[tokio::test]
async fn a_changed_secret_holds_a_back_end_and_the_household_is_offered_or_withdrawn() {
    let ctx = quiet("notify-set");
    let _ = add(&ctx, ntfy("home-ntfy")).await;
    let set = |setting: Setting| run(&ctx, Asked::Set(Box::new(setting)));

    let offered = set(Setting {
        id: "home-ntfy".to_owned(),
        members: Some(Offering::Offer("Phones".to_owned())),
        attempts: Some(2),
        delays: Some(vec![3]),
        ..Setting::default()
    })
    .await;
    let rekeyed = set(Setting {
        id: "home-ntfy".to_owned(),
        token: Some("tk_new".to_owned()),
        reader: Some("rd_new".to_owned()),
        ..Setting::default()
    })
    .await;
    let withdrawn = set(Setting {
        id: "home-ntfy".to_owned(),
        members: Some(Offering::Withdraw),
        ..Setting::default()
    })
    .await;
    let unknown = set(Setting {
        id: "nowhere".to_owned(),
        ..Setting::default()
    })
    .await;
    let empty = set(Setting {
        id: "home-ntfy".to_owned(),
        reader: Some(" ".to_owned()),
        ..Setting::default()
    })
    .await;

    assert!(offered.is_ok());
    assert!(matches!(
        rekeyed
            .ok()
            .and_then(|report| report.backends.into_iter().next())
            .map(|one| one.proof),
        Some(BackendProof::Held(_))
    ));
    assert!(withdrawn.is_ok());
    let kept = super::backends::at(&ctx)
        .and_then(|path| super::backends::kept(&path).ok())
        .and_then(|kept| kept.backends.into_iter().next());
    assert_eq!(
        kept.as_ref().map(|one| (one.attempts, one.delays.clone())),
        Some((2, vec![3]))
    );
    assert_eq!(kept.and_then(|one| one.members), None);
    let secrets = file(super::backends::secrets_at(&ctx));
    assert!(secrets.contains("tk_new") && secrets.contains("rd_new"));
    assert_eq!(code(&unknown), Some(NO_SUCH_BACKEND));
    assert_eq!(code(&empty), Some(BACKEND_INCOMPLETE));
}

#[tokio::test]
async fn a_removed_back_end_and_its_secret_are_gone() {
    let ctx = quiet("notify-removed");
    let _ = add(&ctx, ntfy("home-ntfy")).await;

    let removed = run(
        &ctx,
        Asked::Remove {
            id: "home-ntfy".to_owned(),
        },
    )
    .await;
    let again = run(
        &ctx,
        Asked::Remove {
            id: "home-ntfy".to_owned(),
        },
    )
    .await;

    assert_eq!(
        removed.map(|report| report.state).ok(),
        Some(NotifyState::Unconfigured)
    );
    assert_eq!(code(&again), Some(NO_SUCH_BACKEND));
    assert!(!file(super::backends::secrets_at(&ctx)).contains(TOKEN));
}

#[tokio::test]
async fn a_rehearsal_changes_nothing() {
    let ctx = quiet("notify-rehearsed").rehearsing();

    let added = add(&ctx, ntfy("home-ntfy")).await;

    assert_eq!(
        added
            .map(|report| (report.backends.len(), report.rehearsed))
            .ok(),
        Some((0, true))
    );
    assert!(super::backends::at(&ctx).is_some_and(|path| !path.exists()));
}

#[tokio::test]
async fn an_ntfy_back_end_is_proven_once_its_test_is_seen_in_the_topic() {
    let http = Fake::by_route(vec![
        (
            Method::Get,
            "/stack/json",
            Answer::reply(200, "{\"id\":\"m1\"}\n"),
        ),
        (
            Method::Post,
            "/stack",
            Answer::reply(200, "{\"id\":\"m1\"}"),
        ),
    ]);
    let ctx = machine("notify-proven", Arc::clone(&http));
    let _ = add(&ctx, ntfy("home-ntfy")).await;

    let tested = run(
        &ctx,
        Asked::Test {
            id: "home-ntfy".to_owned(),
        },
    )
    .await;

    let report = tested.unwrap_or_else(|_| unreachable!("a back-end is tested"));
    assert!(matches!(
        report.backends.first().map(|one| &one.proof),
        Some(BackendProof::Proven(_))
    ));
    assert_eq!(report.state, NotifyState::Ready);
    let sent = http.requests();
    assert!(sent.iter().any(|one| one
        .headers
        .iter()
        .any(|(name, value)| name.eq_ignore_ascii_case("authorization")
            && value == &format!("Bearer {TOKEN}"))));
}

#[tokio::test]
async fn a_back_end_refusing_its_token_is_held_and_one_never_seen_stays_unproven() {
    let refusing = machine("notify-refusing", Fake::always(Answer::reply(403, "{}")));
    let unseen = machine(
        "notify-unseen",
        Fake::by_route(vec![
            (Method::Get, "/stack/json", Answer::reply(200, "")),
            (
                Method::Post,
                "/stack",
                Answer::reply(200, "{\"id\":\"m1\"}"),
            ),
        ]),
    );
    let hook = machine("notify-hook", Fake::always(Answer::reply(204, "")));
    let _ = add(&refusing, ntfy("home-ntfy")).await;
    let _ = add(&unseen, ntfy("home-ntfy")).await;
    let _ = add(
        &hook,
        Adding {
            kind: BackendKind::Webhook,
            url: "http://hooks.home/alerts".to_owned(),
            topic: None,
            ..ntfy("hook")
        },
    )
    .await;
    let proof = |ctx: Ctx, id: &'static str| async move {
        run(&ctx, Asked::Test { id: id.to_owned() })
            .await
            .ok()
            .and_then(|report| report.backends.into_iter().next())
            .map(|one| one.proof)
    };

    assert!(matches!(
        proof(refusing, "home-ntfy").await,
        Some(BackendProof::Held(_))
    ));
    assert_eq!(
        proof(unseen, "home-ntfy").await,
        Some(BackendProof::Unproven)
    );
    assert_eq!(proof(hook, "hook").await, Some(BackendProof::Unproven));
    let missing = run(
        &quiet("notify-untested"),
        Asked::Test {
            id: "none".to_owned(),
        },
    )
    .await;
    assert_eq!(code(&missing), Some(NO_SUCH_BACKEND));
}

mod refusing;
