use std::sync::Arc;

use lemonfiber_contract::capabilities::{invite::decline, request::gate};
use lemonfiber_contract::wire;
use lemonfiber_core::ports::docker::{Engine, LogQuery};
use lemonfiber_core::ports::http::{Http, Method, Request};
use lemonfiber_core::ports::service::{
    Admitted, Claimed, Declinable, Declined, Declining, GateEntry, GateOutcome, Gated, Gating,
    Invitations, Lapsed, TakenBack, Target,
};
use lemonfiber_fixtures::http::{Answer, Fake};
use lemonfiber_fixtures::support::Reporting;

use super::super::adapted::KEY;
use super::super::context::{a_context, unguarded_embedded, unguarded_stack};
use super::{
    both_installed, moved, responded, with_decline, with_gate, Alongside, Decline, Gate, Serving,
    BOTH, DECLINE, GATE, PAGE,
};

/// A route to the media server under `path`, admitting `tokens`.
fn route(path: &str, tokens: &[&str]) -> Gated {
    Gated {
        target: Target::MediaServer { pins: Vec::new() },
        path: path.to_owned(),
        address: "https://media:8920".to_owned(),
        majors: vec![10],
        credential: "media-key".to_owned(),
        tokens: tokens.iter().map(|token| (*token).to_owned()).collect(),
    }
}

/// An entry of the gate's record at `seq`.
fn entry(seq: u64) -> GateEntry {
    GateEntry {
        seq,
        at: seq,
        route: "media".to_owned(),
        method: "DELETE".to_owned(),
        path: "/Users/9".to_owned(),
        outcome: GateOutcome::Refused,
    }
}

/// The invitations of one unclaimed offer.
fn invitations() -> Invitations {
    Invitations {
        claimed: Claimed::HasPassword,
        invitations: vec![Declinable {
            token: "hash".to_owned(),
            account: "9".to_owned(),
            name: "ana".to_owned(),
            issued: 1,
            lapses: 2,
        }],
    }
}

/// A transport answering both guards' contracts from `gate` and `decline`, and
/// everything else with `beside`.
fn serving(gate: &Arc<Gate>, decline: &Arc<Decline>) -> Arc<Serving> {
    Arc::new(
        Serving::beside(Fake::by_path(vec![("/elsewhere", Answer::reply(204, ""))]))
            .gate(gate)
            .decline(decline),
    )
}

/// A call to `operation` of `capability` carrying no key.
fn unkeyed(capability: &str, major: u32, operation: &str) -> Request {
    Request {
        method: Method::Post,
        url: format!(
            "http://127.0.0.1:8080{}",
            lemonfiber_contract::path(capability, major, operation)
        ),
        headers: Vec::new(),
        body: Some("{}".to_owned()),
        pinned: None,
    }
}

#[tokio::test]
async fn the_gate_admits_what_it_was_told_and_reads_its_record_after_a_point() {
    let gate = Gate::admitting(vec![route("media", &[])]).recording(vec![entry(3), entry(4)]);

    assert!(gate
        .admit_tokens("media", &["one".to_owned()])
        .await
        .is_ok());
    assert!(gate
        .admit_tokens("films", &["one".to_owned()])
        .await
        .is_err());
    assert_eq!(gate.tokens("media"), Some(vec!["one".to_owned()]));
    assert_eq!(gate.tokens("films"), None);
    assert_eq!(
        gate.admitted().await.ok(),
        Some(gate.routes().iter().map(Admitted::of).collect())
    );
    let read = gate.record(1).await.ok();
    assert_eq!(read.as_ref().map(|record| record.lost), Some(1));
    assert_eq!(read.map(|record| record.entries.len()), Some(2));
    assert_eq!(gate.record(9).await.ok().map(|record| record.lost), Some(0));
    assert!(gate.admit(&[]).await.is_ok());
    assert!(gate.routes().is_empty());
}

#[tokio::test]
async fn a_gate_answering_a_fixed_admission_or_refusing_says_so() {
    let fixed = vec![Admitted::of(&route("fixed", &[]))];
    let answering = Gate::admitting(Vec::new()).answering(fixed.clone());
    assert_eq!(answering.admitted().await.ok(), Some(fixed));

    let refusing = Gate::admitting(vec![route("media", &[])]).refusing(&[
        "admit",
        "admitted",
        "admit_tokens",
        "record",
    ]);
    assert!(refusing.admit(&[]).await.is_err());
    assert!(refusing.admitted().await.is_err());
    assert!(refusing.admit_tokens("media", &[]).await.is_err());
    assert!(refusing.record(0).await.is_err());
}

#[tokio::test]
async fn the_decline_service_holds_and_answers_what_it_was_handed() {
    let refusals = vec![Declined {
        token: "hash".to_owned(),
        account: "9".to_owned(),
        at: 3,
    }];
    let lapses = vec![Lapsed {
        token: "hash".to_owned(),
        account: "9".to_owned(),
        name: "ana".to_owned(),
        issued: 1,
        at: 2,
        outcome: TakenBack::Removed,
    }];
    let decline = Decline::holding("key", &["pin"])
        .handed(invitations())
        .declining(refusals.clone())
        .lapsing(lapses.clone())
        .paging(PAGE);

    assert_eq!(decline.key().as_deref(), Some("key"));
    assert_eq!(decline.pins(), vec!["pin".to_owned()]);
    assert_eq!(decline.invitations(), Some(invitations()));
    assert_eq!(decline.refusals().await.ok(), Some(refusals));
    assert_eq!(decline.lapses().await.ok(), Some(lapses));
    assert_eq!(decline.page().await.ok().as_deref(), Some(PAGE));
    assert!(decline.hold("other", &[]).await.is_ok());
    assert_eq!(
        decline.held().await.ok().flatten().map(|held| held.key),
        Some(lemonfiber_core::ports::digest::sha256("other"))
    );
    assert!(decline.apply_invitations(&invitations()).await.is_ok());

    let fixed = Decline::holding("key", &[]).answering("fixed");
    assert_eq!(
        fixed.held().await.ok().flatten().map(|held| held.key),
        Some(lemonfiber_core::ports::digest::sha256("fixed"))
    );
    assert_eq!(Decline::default().key(), None);
    assert!(Decline::default().pins().is_empty());
}

#[tokio::test]
async fn a_decline_service_refusing_an_operation_answers_it_with_a_refusal() {
    let decline = Decline::default().refusing(&[
        "hold",
        "held",
        "apply_invitations",
        "refusals",
        "lapses",
        "page",
    ]);
    assert!(decline.hold("key", &[]).await.is_err());
    assert!(decline.held().await.is_err());
    assert!(decline.apply_invitations(&invitations()).await.is_err());
    assert!(decline.refusals().await.is_err());
    assert!(decline.lapses().await.is_err());
    assert!(decline.page().await.is_err());
}

#[tokio::test]
async fn each_guard_is_asked_over_its_contract_and_everything_else_beside_it() {
    let gate = Arc::new(Gate::admitting(Vec::new()).refusing(&["record"]));
    let decline = Arc::new(Decline::default().paging(PAGE));
    let serving = serving(&gate, &decline);

    let gating = gate::Adapter(GATE.contracted(serving.clone()));
    let declining = decline::Adapter(DECLINE.contracted(serving.clone()));
    assert!(gating.admit(&[route("media", &[])]).await.is_ok());
    assert!(gating.record(0).await.is_err());
    assert_eq!(declining.page().await.ok().as_deref(), Some(PAGE));
    assert_eq!(gate.routes(), vec![route("media", &[])]);
    assert_eq!(serving.asked().len(), 3);

    let beside = serving
        .send(&Request {
            method: Method::Get,
            url: "http://127.0.0.1:9/elsewhere".to_owned(),
            headers: Vec::new(),
            body: None,
            pinned: None,
        })
        .await;
    assert_eq!(beside.ok().map(|answer| answer.status), Some(204));
    assert_eq!(serving.asked().len(), 3);
}

#[tokio::test]
async fn a_guard_asked_without_its_key_is_refused_unheard() {
    let gate = Arc::new(Gate::admitting(Vec::new()));
    let decline = Arc::new(Decline::default());
    let serving = serving(&gate, &decline);

    for request in [
        unkeyed(gate::CAPABILITY, gate::MAJOR, "admitted"),
        unkeyed(decline::CAPABILITY, decline::MAJOR, "page"),
    ] {
        let answered = serving.send(&request).await;
        assert_eq!(answered.ok().map(|answer| answer.status), Some(401));
    }
    let alone = Serving::beside(Fake::silent()).decline(&decline);
    let answered = alone
        .send(&unkeyed(decline::CAPABILITY, decline::MAJOR, "page"))
        .await;
    assert_eq!(answered.ok().map(|answer| answer.status), Some(401));
}

#[test]
fn what_a_guard_served_crosses_the_wire_as_its_contract_says() {
    let json = responded(Ok(wire::Answer::Json(b"[]".to_vec())));
    assert_eq!((json.status, json.body.as_str()), (200, "[]"));
    let reset = responded(Ok(wire::Answer::Reset));
    assert_eq!((reset.status, reset.body.as_str()), (wire::RESET, ""));
    let refused = responded(Err(wire::Refusal::new(
        wire::Kind::Refused,
        "no".to_owned(),
    )));
    assert_eq!(refused.status, wire::Kind::Refused.status());
    assert_eq!(
        refused.headers,
        vec![("Content-Type".to_owned(), wire::PROBLEM.to_owned())]
    );
}

#[tokio::test]
async fn an_engine_alongside_another_lists_both_and_asks_it_everything_else() {
    let alongside = Alongside::new(Arc::new(Reporting::absent()), GATE.running());
    let listed = alongside.list("lemonfiber").await.unwrap_or_default();
    assert!(listed.iter().any(|one| one.service == GATE.adapter));

    let inner = Reporting::absent();
    assert_eq!(
        alongside.exec("x", &[]).await.is_ok(),
        inner.exec("x", &[]).await.is_ok()
    );
    assert_eq!(
        alongside.stats("lemonfiber").await.is_ok(),
        inner.stats("lemonfiber").await.is_ok()
    );
    assert_eq!(
        alongside
            .logs(
                "lemonfiber",
                &[],
                LogQuery {
                    tail: 1,
                    follow: false
                }
            )
            .await
            .is_ok(),
        inner
            .logs(
                "lemonfiber",
                &[],
                LogQuery {
                    tail: 1,
                    follow: false
                }
            )
            .await
            .is_ok()
    );
}

#[test]
fn both_guards_move_in_as_their_plugins_with_their_keys() {
    let scratch = lemonfiber_fixtures::scratch::Scratch::new("guards-moved-in");
    let ctx = a_context()
        .settings(lemonfiber_core::config::Settings {
            env_file: Some(scratch.join(".env")),
            stack_dir: Some(scratch.join("stack")),
            ..lemonfiber_core::config::Settings::default()
        })
        .build();
    let installed: Vec<String> = both_installed().into_iter().map(|one| one.plugin).collect();
    assert_eq!(
        installed,
        BOTH.iter()
            .map(|guard| guard.plugin.to_owned())
            .collect::<Vec<_>>()
    );

    let ctx = moved(ctx);
    let held: Vec<String> =
        std::fs::read_to_string(scratch.join(lemonfiber_core::config::paths::PLUGINS))
            .ok()
            .and_then(|text| lemonfiber_core::plugin::Register::parse(&text).ok())
            .map(|register| {
                register
                    .installed()
                    .iter()
                    .map(|one| one.plugin.clone())
                    .collect()
            })
            .unwrap_or_default();
    assert_eq!(held, installed);
    for guard in BOTH {
        assert!(guard
            .key_at(&ctx)
            .is_some_and(|key| std::fs::read_to_string(key).is_ok_and(|held| held == KEY)));
    }
    assert!(GATE
        .digest()
        .is_some_and(|digest| digest.starts_with("sha256:")));
    assert!(GATE.listens().is_some());
    assert_ne!(GATE.contract(), 0);
}

#[tokio::test]
async fn a_context_reaches_each_guard_over_its_contract_beside_what_it_ran() {
    let gate = Arc::new(Gate::admitting(vec![route("media", &[])]));
    let decline = Arc::new(Decline::default().paging(PAGE));
    let ctx = with_decline(with_gate(a_context().build(), &gate), &decline);

    let listed = ctx
        .seams
        .engine
        .list("lemonfiber")
        .await
        .unwrap_or_default();
    for guard in BOTH {
        assert!(listed.iter().any(|one| one.service == guard.adapter));
    }
    let asked = gate::Adapter(GATE.contracted(Arc::clone(&ctx.seams.http)));
    assert_eq!(
        asked.admitted().await.ok().map(|admitted| admitted.len()),
        Some(1)
    );
}

#[test]
fn a_stack_without_its_guards_declares_neither() {
    for stack in [unguarded_stack(), unguarded_embedded()] {
        let declared: Vec<String> = stack
            .manifest()
            .map(|manifest| manifest.services.into_iter().map(|one| one.id).collect())
            .unwrap_or_default();
        assert!(!declared.is_empty());
        assert!(BOTH
            .iter()
            .all(|guard| !declared.iter().any(|id| id == guard.plugin)));
    }
}
