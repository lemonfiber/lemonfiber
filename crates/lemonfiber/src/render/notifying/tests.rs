use lemonfiber_core::alert::Notification;
use lemonfiber_core::model::{
    BackendKind, BackendProof, BackendReport, ChosenBy, Delivery, DeliveryOutcome, DeliveryRetry,
    HostedBackend, NotifyReport, NotifyState, UnreadBackend,
};

use super::backends;

const AT: u64 = 1_793_000_000;

fn backend(id: &str, proof: BackendProof) -> BackendReport {
    BackendReport {
        id: id.to_owned(),
        kind: BackendKind::Ntfy,
        reaches: "http://ntfy.home:2586/stack".to_owned(),
        chosen_by: ChosenBy::Operator,
        hosted: None,
        proof,
        last: None,
        failures: Vec::new(),
        retry: None,
    }
}

fn report(state: NotifyState, listed: Vec<BackendReport>) -> NotifyReport {
    NotifyReport {
        state,
        backends: listed,
        unread: Vec::new(),
        rehearsed: false,
    }
}

#[test]
fn no_back_end_says_where_alerts_still_reach() {
    let said = backends(&report(NotifyState::Unconfigured, Vec::new())).text();

    assert!(said.contains("No back-end is configured"), "{said}");
}

#[test]
fn each_back_end_is_drawn_with_its_proof_and_what_became_of_its_deliveries() {
    let failing = BackendReport {
        kind: BackendKind::Gotify,
        chosen_by: ChosenBy::Member("ada".to_owned()),
        hosted: Some(HostedBackend {
            host: "ntfy.sh".to_owned(),
            leaves: vec![
                "the summary".to_owned(),
                "this machine's address".to_owned(),
            ],
        }),
        last: Some(Delivery {
            at: AT,
            kind: Notification::Problem,
            outcome: DeliveryOutcome::Failed("503 down".to_owned()),
        }),
        failures: vec![Delivery {
            at: AT,
            kind: Notification::Problem,
            outcome: DeliveryOutcome::Failed("503 down".to_owned()),
        }],
        retry: Some(DeliveryRetry {
            next: AT + 60,
            attempt: 2,
            of: 5,
        }),
        ..backend(
            "away",
            BackendProof::Held("it refused its token".to_owned()),
        )
    };
    let proven = BackendReport {
        last: Some(Delivery {
            at: AT,
            kind: Notification::Completion,
            outcome: DeliveryOutcome::Confirmed,
        }),
        ..backend("home-ntfy", BackendProof::Proven(AT))
    };
    let mut drawn = report(
        NotifyState::Degraded,
        vec![
            proven,
            failing,
            backend("hook", BackendProof::Unproven),
            BackendReport {
                last: Some(Delivery {
                    at: AT,
                    kind: Notification::Update,
                    outcome: DeliveryOutcome::Sent,
                }),
                ..backend("loose", BackendProof::Unproven)
            },
        ],
    );
    drawn.unread.push(UnreadBackend {
        backend: "broken".to_owned(),
        because: "its record is not JSON".to_owned(),
    });
    drawn.rehearsed = true;

    let said = backends(&drawn).text();

    for expected in [
        "Rehearsed",
        "not to every back-end",
        "home-ntfy  ntfy  http://ntfy.home:2586/stack",
        "proven 2026",
        "(completion) confirmed arriving",
        "away  gotify",
        "chosen by ada",
        "hosted at ntfy.sh",
        "the summary; this machine's address",
        "held, delivering nothing: it refused its token",
        "failed 2026",
        "(problem) failed: 503 down",
        "attempt 2 of 5 at 2026",
        "unproven: run notify test",
        "(update) taken, its arrival not seen",
        "broken could not be read: its record is not JSON",
    ] {
        assert!(said.contains(expected), "{expected}: {said}");
    }
}

#[test]
fn every_state_is_said_in_words() {
    for (state, words) in [
        (NotifyState::Ready, "every back-end is answering"),
        (NotifyState::Unreachable, "No back-end is taking alerts"),
    ] {
        let said = backends(&report(state, Vec::new())).text();
        assert!(said.contains(words), "{said}");
    }
}
