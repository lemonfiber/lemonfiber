use std::sync::Arc;

use async_trait::async_trait;

use super::{Decline, DeclineKeyCheck, KeyDates, MARGIN, UNEXPLAINED};
use crate::doctor::{Category, Check, Verdict};
use crate::ports::digest::sha256;
use crate::ports::service::{Dated, Declined, Lapsed, TakenBack};

/// 2026-10-05T00:00:00Z, in seconds since the Unix epoch.
const MIDNIGHT: u64 = 1_791_158_400;

/// `seconds` after midnight, as the media server writes a moment.
fn moment(seconds: u64) -> String {
    i64::try_from(MIDNIGHT + seconds)
        .ok()
        .and_then(|at| jiff::Timestamp::from_second(at).ok())
        .map(|at| at.to_string())
        .unwrap_or_default()
}

/// A media server that dates the decline keys as given, or will not answer.
struct Server(Result<Vec<Dated>, ()>);

#[async_trait]
impl KeyDates for Server {
    async fn decline_keys(&self) -> Result<Vec<Dated>, ()> {
        self.0.clone()
    }
}

/// A key made at `made` and last used at `used`, each seconds after midnight.
fn key(made: u64, used: Option<u64>) -> Dated {
    Dated {
        created: Some(moment(made)),
        last_used: used.map(moment),
    }
}

/// A decline service answering a refusal at each of `refused` and a lapse at each of
/// `taken_back`, in seconds after midnight.
fn recorded(refused: &[u64], taken_back: &[u64]) -> crate::test_support::guards::Decline {
    crate::test_support::guards::Decline::default()
        .declining(
            refused
                .iter()
                .map(|when| Declined {
                    token: sha256(&format!("r{when}")),
                    account: "9".to_owned(),
                    at: MIDNIGHT + when,
                })
                .collect(),
        )
        .lapsing(
            taken_back
                .iter()
                .map(|when| Lapsed {
                    token: sha256(&format!("l{when}")),
                    account: "9".to_owned(),
                    name: "ana".to_owned(),
                    issued: MIDNIGHT,
                    at: MIDNIGHT + when,
                    outcome: TakenBack::Removed,
                })
                .collect(),
        )
}

/// What the check says over what `decline` recorded, the server answering `server`.
async fn verdict(
    decline: crate::test_support::guards::Decline,
    server: Option<Result<Vec<Dated>, ()>>,
) -> Verdict {
    let check = DeclineKeyCheck::new(Some(Decline {
        service: Some(Arc::new(decline)),
        keys: server.map(|answer| Arc::new(Server(answer)) as Arc<dyn KeyDates>),
    }));
    check.run().await.into_iter().next().map_or(
        Verdict::Skipped {
            reason: String::new(),
        },
        |one| one.verdict,
    )
}

fn warned_unexplained(verdict: &Verdict) -> bool {
    matches!(verdict, Verdict::Warn(problem) if problem.code == UNEXPLAINED)
}

#[tokio::test]
async fn a_stack_without_the_decline_service_has_no_key_to_account_for() {
    let check = DeclineKeyCheck::new(None);

    let found = check.run().await;

    assert_eq!(check.category(), Category::Services);

    assert!(matches!(
        found.first().map(|one| &one.verdict),
        Some(Verdict::Skipped { .. })
    ));
    assert_eq!(
        found.first().map(|one| one.check.as_str()),
        Some("services.decline-key")
    );
}

#[tokio::test]
async fn a_key_last_used_for_a_lapse_the_service_recorded_is_explained() {
    let decline = recorded(&[], &[3_600]);

    let said = verdict(decline, Some(Ok(vec![key(0, Some(3_600))]))).await;

    assert!(matches!(said, Verdict::Pass { .. }), "{said:?}");
}

#[tokio::test]
async fn a_key_last_used_for_a_refusal_the_service_recorded_is_explained() {
    let decline = recorded(&[7_200], &[]);

    let said = verdict(decline, Some(Ok(vec![key(0, Some(7_200))]))).await;

    assert!(matches!(said, Verdict::Pass { .. }), "{said:?}");
}

#[tokio::test]
async fn a_key_used_only_when_it_was_made_and_proved_is_explained() {
    let decline = recorded(&[], &[]);

    let said = verdict(decline, Some(Ok(vec![key(100, Some(160))]))).await;

    assert!(matches!(said, Verdict::Pass { .. }), "{said:?}");
}

#[tokio::test]
async fn a_use_later_than_every_record_is_warned_of() {
    let decline = recorded(&[600], &[3_600]);

    let said = verdict(decline, Some(Ok(vec![key(0, Some(3_600 + MARGIN + 60))]))).await;

    assert!(warned_unexplained(&said), "{said:?}");
}

#[tokio::test]
async fn a_use_with_no_record_at_all_long_after_the_key_was_made_is_warned_of() {
    let decline = recorded(&[], &[]);

    let said = verdict(decline, Some(Ok(vec![key(0, Some(86_400))]))).await;

    assert!(warned_unexplained(&said), "{said:?}");
}

#[tokio::test]
async fn a_use_inside_the_margin_after_a_record_is_explained() {
    let decline = recorded(&[], &[3_600]);

    let said = verdict(decline, Some(Ok(vec![key(0, Some(3_600 + MARGIN))]))).await;

    assert!(matches!(said, Verdict::Pass { .. }), "{said:?}");
}

#[tokio::test]
async fn a_key_never_used_needs_no_explaining() {
    let decline = recorded(&[], &[]);

    let said = verdict(decline, Some(Ok(vec![key(0, None)]))).await;

    assert!(matches!(said, Verdict::Pass { .. }), "{said:?}");
}

#[tokio::test]
async fn no_key_on_the_server_is_unverified_never_a_pass() {
    let decline = recorded(&[], &[]);

    let said = verdict(decline, Some(Ok(Vec::new()))).await;

    assert!(matches!(said, Verdict::Unverified { .. }), "{said:?}");
}

#[tokio::test]
async fn an_unreadable_key_list_or_no_server_is_unverified_never_a_pass() {
    let decline = recorded(&[], &[]);
    let silent = verdict(decline, Some(Err(()))).await;
    let decline = recorded(&[], &[]);
    let serverless = verdict(decline, None).await;

    assert!(matches!(silent, Verdict::Unverified { .. }), "{silent:?}");
    assert!(
        matches!(serverless, Verdict::Unverified { .. }),
        "{serverless:?}"
    );
}

/// A decline service that cannot be asked over its contract accounts for nothing, so a
/// key the server dates is never said to be explained.
#[tokio::test]
async fn a_decline_service_that_cannot_be_asked_is_unverified_never_a_pass() {
    let check = DeclineKeyCheck::new(Some(Decline {
        service: None,
        keys: Some(Arc::new(Server(Ok(vec![key(0, Some(60))]))) as Arc<dyn KeyDates>),
    }));

    let said = check.run().await.into_iter().next().map(|one| one.verdict);

    assert!(
        matches!(&said, Some(Verdict::Unverified { reason, .. }) if reason.contains("cannot be asked")),
        "{said:?}"
    );
}

#[tokio::test]
async fn a_second_key_under_the_service_s_name_does_not_hide_the_first_one_s_use() {
    let decline = recorded(&[], &[3_600]);
    let used_unexplained = key(0, Some(86_400));
    let newer_unused = key(90_000, None);

    let said = verdict(decline, Some(Ok(vec![newer_unused, used_unexplained]))).await;

    assert!(warned_unexplained(&said), "{said:?}");
}

#[tokio::test]
async fn an_unexplained_use_listed_first_outweighs_every_key_after_it() {
    let decline = recorded(&[], &[3_600]);
    let used_unexplained = key(0, Some(86_400));
    let undated = Dated {
        created: Some(moment(0)),
        last_used: Some("yesterday-ish".to_owned()),
    };
    let newer_unused = key(90_000, None);

    let said = verdict(
        decline,
        Some(Ok(vec![used_unexplained, undated, newer_unused])),
    )
    .await;

    assert!(warned_unexplained(&said), "{said:?}");
}

#[tokio::test]
async fn an_undated_use_listed_first_is_not_hidden_by_an_unused_key_after_it() {
    let decline = recorded(&[], &[]);
    let undated = Dated {
        created: Some(moment(0)),
        last_used: Some("yesterday-ish".to_owned()),
    };
    let newer_unused = key(90_000, None);

    let said = verdict(decline, Some(Ok(vec![undated, newer_unused]))).await;

    assert!(matches!(said, Verdict::Unverified { .. }), "{said:?}");
}

#[tokio::test]
async fn a_last_use_the_server_dates_in_a_form_nobody_reads_is_unverified_never_a_pass() {
    let decline = recorded(&[], &[]);
    let odd = Dated {
        created: Some(moment(0)),
        last_used: Some("yesterday-ish".to_owned()),
    };

    let said = verdict(decline, Some(Ok(vec![odd]))).await;

    assert!(matches!(said, Verdict::Unverified { .. }), "{said:?}");
}

#[tokio::test]
async fn refusals_the_service_will_not_answer_are_unverified_never_a_pass() {
    let decline = recorded(&[], &[]).refusing(&["refusals"]);

    let said = verdict(decline, Some(Ok(vec![key(0, Some(60))]))).await;

    assert!(matches!(said, Verdict::Unverified { .. }), "{said:?}");
}

#[tokio::test]
async fn lapses_the_service_will_not_answer_are_unverified_never_a_pass() {
    let decline = recorded(&[], &[]).refusing(&["lapses"]);

    let said = verdict(decline, Some(Ok(vec![key(0, Some(60))]))).await;

    assert!(matches!(said, Verdict::Unverified { .. }), "{said:?}");
}

#[tokio::test]
async fn a_key_nobody_dates_used_with_nothing_recorded_is_warned_of_saying_so() {
    let decline = recorded(&[], &[]);
    let undated = Dated {
        created: None,
        last_used: Some(moment(60)),
    };

    let said = verdict(decline, Some(Ok(vec![undated]))).await;

    assert!(
        matches!(&said, Verdict::Warn(problem) if problem.code == UNEXPLAINED
            && problem.summary.contains("recorded nothing")),
        "{said:?}"
    );
}
