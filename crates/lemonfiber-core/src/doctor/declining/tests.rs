use std::path::{Path, PathBuf};
use std::sync::Arc;

use async_trait::async_trait;
use lemonfiber_sidecar::decline::{Lapse, Lapses, Outcome, Refusal, Refusals, TokenHash};

use super::{Decline, DeclineKeyCheck, KeyDates, MARGIN, UNEXPLAINED};
use crate::config::store;
use crate::doctor::{Check, Verdict};
use crate::jellyfin::Dated;

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

/// A scratch directory holding a refusal at each of `refused` and a lapse at each of
/// `lapsed`, in seconds after midnight.
fn scene(name: &str, refused: &[u64], taken_back: &[u64]) -> PathBuf {
    let at = lemonfiber_fixtures::scratch::Scratch::named(name).kept();
    let _ = std::fs::remove_dir_all(&at);
    let _ = std::fs::create_dir_all(&at);
    let refusals = refused.iter().fold(Refusals::default(), |all, when| {
        all.with(Refusal {
            token: TokenHash::of(&format!("r{when}")),
            account: "9".to_owned(),
            at: MIDNIGHT + when,
        })
    });
    let lapses = taken_back.iter().fold(Lapses::default(), |all, when| {
        all.with(Lapse {
            token: TokenHash::of(&format!("l{when}")),
            account: "9".to_owned(),
            name: "ana".to_owned(),
            issued: MIDNIGHT,
            at: MIDNIGHT + when,
            outcome: Outcome::Removed,
        })
    });
    if !refused.is_empty() {
        let _ = store::write(&at.join("refusals.json"), &refusals.written());
    }
    if !taken_back.is_empty() {
        let _ = store::write(&at.join("lapses.json"), &lapses.written());
    }
    at
}

fn files() -> Arc<dyn crate::ports::filesystem::FileSystem> {
    crate::test_support::a_context()
        .build()
        .seams
        .filesystem
        .clone()
}

/// What the check says over the records under `at`, the server answering `server`.
async fn verdict(at: &Path, server: Option<Result<Vec<Dated>, ()>>) -> Verdict {
    let check = DeclineKeyCheck::new(
        files(),
        Some(Decline {
            refusals: at.join("refusals.json"),
            lapses: at.join("lapses.json"),
            keys: server.map(|answer| Arc::new(Server(answer)) as Arc<dyn KeyDates>),
        }),
    );
    let found = check.run().await;
    let _ = std::fs::remove_dir_all(at);
    found.into_iter().next().map_or(
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
    let check = DeclineKeyCheck::new(files(), None);

    let found = check.run().await;

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
    let at = scene("decline-key-lapse", &[], &[3_600]);

    let said = verdict(&at, Some(Ok(vec![key(0, Some(3_600))]))).await;

    assert!(matches!(said, Verdict::Pass { .. }), "{said:?}");
}

#[tokio::test]
async fn a_key_last_used_for_a_refusal_the_service_recorded_is_explained() {
    let at = scene("decline-key-refusal", &[7_200], &[]);

    let said = verdict(&at, Some(Ok(vec![key(0, Some(7_200))]))).await;

    assert!(matches!(said, Verdict::Pass { .. }), "{said:?}");
}

#[tokio::test]
async fn a_key_used_only_when_it_was_made_and_proved_is_explained() {
    let at = scene("decline-key-proof", &[], &[]);

    let said = verdict(&at, Some(Ok(vec![key(100, Some(160))]))).await;

    assert!(matches!(said, Verdict::Pass { .. }), "{said:?}");
}

#[tokio::test]
async fn a_use_later_than_every_record_is_warned_of() {
    let at = scene("decline-key-unexplained", &[600], &[3_600]);

    let said = verdict(&at, Some(Ok(vec![key(0, Some(3_600 + MARGIN + 60))]))).await;

    assert!(warned_unexplained(&said), "{said:?}");
}

#[tokio::test]
async fn a_use_with_no_record_at_all_long_after_the_key_was_made_is_warned_of() {
    let at = scene("decline-key-nothing", &[], &[]);

    let said = verdict(&at, Some(Ok(vec![key(0, Some(86_400))]))).await;

    assert!(warned_unexplained(&said), "{said:?}");
}

#[tokio::test]
async fn a_use_inside_the_margin_after_a_record_is_explained() {
    let at = scene("decline-key-margin", &[], &[3_600]);

    let said = verdict(&at, Some(Ok(vec![key(0, Some(3_600 + MARGIN))]))).await;

    assert!(matches!(said, Verdict::Pass { .. }), "{said:?}");
}

#[tokio::test]
async fn a_key_never_used_needs_no_explaining() {
    let at = scene("decline-key-unused", &[], &[]);

    let said = verdict(&at, Some(Ok(vec![key(0, None)]))).await;

    assert!(matches!(said, Verdict::Pass { .. }), "{said:?}");
}

#[tokio::test]
async fn no_key_on_the_server_is_unverified_never_a_pass() {
    let at = scene("decline-key-none", &[], &[]);

    let said = verdict(&at, Some(Ok(Vec::new()))).await;

    assert!(matches!(said, Verdict::Unverified { .. }), "{said:?}");
}

#[tokio::test]
async fn an_unreadable_key_list_or_no_server_is_unverified_never_a_pass() {
    let at = scene("decline-key-silent", &[], &[]);
    let silent = verdict(&at, Some(Err(()))).await;
    let at = scene("decline-key-serverless", &[], &[]);
    let serverless = verdict(&at, None).await;

    assert!(matches!(silent, Verdict::Unverified { .. }), "{silent:?}");
    assert!(
        matches!(serverless, Verdict::Unverified { .. }),
        "{serverless:?}"
    );
}

#[tokio::test]
async fn a_record_that_cannot_be_read_explains_nothing_and_says_so() {
    let at = scene("decline-key-garbled", &[], &[]);
    let _ = store::write(&at.join("lapses.json"), "not a record");

    let said = verdict(&at, Some(Ok(vec![key(0, Some(60))]))).await;

    assert!(matches!(said, Verdict::Unverified { .. }), "{said:?}");
}

#[tokio::test]
async fn a_second_key_under_the_service_s_name_does_not_hide_the_first_one_s_use() {
    let at = scene("decline-key-second", &[], &[3_600]);
    let used_unexplained = key(0, Some(86_400));
    let newer_unused = key(90_000, None);

    let said = verdict(&at, Some(Ok(vec![newer_unused, used_unexplained]))).await;

    assert!(warned_unexplained(&said), "{said:?}");
}

#[tokio::test]
async fn an_unexplained_use_listed_first_outweighs_every_key_after_it() {
    let at = scene("decline-key-first", &[], &[3_600]);
    let used_unexplained = key(0, Some(86_400));
    let undated = Dated {
        created: Some(moment(0)),
        last_used: Some("yesterday-ish".to_owned()),
    };
    let newer_unused = key(90_000, None);

    let said = verdict(&at, Some(Ok(vec![used_unexplained, undated, newer_unused]))).await;

    assert!(warned_unexplained(&said), "{said:?}");
}

#[tokio::test]
async fn an_undated_use_listed_first_is_not_hidden_by_an_unused_key_after_it() {
    let at = scene("decline-key-undated-first", &[], &[]);
    let undated = Dated {
        created: Some(moment(0)),
        last_used: Some("yesterday-ish".to_owned()),
    };
    let newer_unused = key(90_000, None);

    let said = verdict(&at, Some(Ok(vec![undated, newer_unused]))).await;

    assert!(matches!(said, Verdict::Unverified { .. }), "{said:?}");
}

#[tokio::test]
async fn a_last_use_the_server_dates_in_a_form_nobody_reads_is_unverified_never_a_pass() {
    let at = scene("decline-key-undated", &[], &[]);
    let odd = Dated {
        created: Some(moment(0)),
        last_used: Some("yesterday-ish".to_owned()),
    };

    let said = verdict(&at, Some(Ok(vec![odd]))).await;

    assert!(matches!(said, Verdict::Unverified { .. }), "{said:?}");
}

#[tokio::test]
async fn a_record_of_refusals_that_cannot_be_read_is_unverified_never_a_pass() {
    let at = scene("decline-key-bad-refusals", &[], &[]);
    let _ = std::fs::write(at.join("refusals.json"), "not a record");

    let said = verdict(&at, Some(Ok(vec![key(0, Some(60))]))).await;

    assert!(matches!(said, Verdict::Unverified { .. }), "{said:?}");
}

#[tokio::test]
async fn a_record_of_lapses_that_cannot_be_read_is_unverified_never_a_pass() {
    let at = scene("decline-key-bad-lapses", &[], &[]);
    let _ = std::fs::write(at.join("lapses.json"), "not a record");

    let said = verdict(&at, Some(Ok(vec![key(0, Some(60))]))).await;

    assert!(matches!(said, Verdict::Unverified { .. }), "{said:?}");
}

#[tokio::test]
async fn a_key_nobody_dates_used_with_nothing_recorded_is_warned_of_saying_so() {
    let at = scene("decline-key-undated-making", &[], &[]);
    let undated = Dated {
        created: None,
        last_used: Some(moment(60)),
    };

    let said = verdict(&at, Some(Ok(vec![undated]))).await;

    assert!(
        matches!(&said, Verdict::Warn(problem) if problem.code == UNEXPLAINED
            && problem.summary.contains("recorded nothing")),
        "{said:?}"
    );
}

/// A link the service's container put where one of its records goes is not followed: the
/// record reads as one that cannot be read, and the key's use as unaccounted for.
#[cfg(unix)]
#[tokio::test]
async fn a_record_that_is_a_link_is_not_followed() {
    let at = scene("decline-key-linked", &[], &[]);
    let elsewhere = at.with_file_name("decline-key-linked-elsewhere.json");
    let _ = store::write(&elsewhere, &Lapses::default().written());
    let _ = std::os::unix::fs::symlink(&elsewhere, at.join("lapses.json"));

    let said = verdict(&at, Some(Ok(vec![key(0, Some(3_600))]))).await;
    let _ = std::fs::remove_file(&elsewhere);

    assert!(matches!(said, Verdict::Unverified { .. }), "{said:?}");
}
