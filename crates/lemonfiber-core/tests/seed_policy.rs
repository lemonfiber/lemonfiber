//! What seeding decides before it touches anything.
//!
//! Pure over what was observed and what is wanted: which folders to write, which
//! to leave, and which to refuse. No service, no fake — the decisions alone.

use lemonfiber_core::baseline::{Origin, Record};
use lemonfiber_core::seed::{
    intent, reconcile, Assessment, Intent, Observed, Report, State, Wiring,
};

// ---- The policy: pure, decided without a service. ----

#[test]
fn the_policy_follows_from_what_was_observed() {
    assert_eq!(intent(Observed::Unavailable), Intent::Skip);
    assert_eq!(intent(Observed::Absent), Intent::Wire);
    // Idempotent: a connection already correct is left, so a second run writes
    // nothing.
    assert_eq!(intent(Observed::Present), Intent::Leave);
    // Drift-aware: an operator's own change is preserved, never reverted.
    assert_eq!(intent(Observed::Drifted), Intent::Preserve);
    // lemonfiber's own value behind its intent is brought up to date.
    assert_eq!(intent(Observed::Stale), Intent::Update);
    // A two-sided change is presented, never resolved on lemonfiber's own.
    assert_eq!(intent(Observed::Conflicted), Intent::Ask);
    // An adopted value is kept; a pre-existing one with no baseline is adopted.
    assert_eq!(intent(Observed::Adopted), Intent::Keep);
    assert_eq!(intent(Observed::Unmanaged), Intent::Adopt);
}

#[test]
fn the_three_way_comparison_reads_every_row_of_the_merge_table() {
    // The expected leg carries a value and where it came from; a helper builds each.
    let written = |value: &str| Record {
        value: value.to_owned(),
        at: "1".to_owned(),
        origin: Origin::Written,
    };
    let adopted = |value: &str| Record {
        value: value.to_owned(),
        at: "1".to_owned(),
        origin: Origin::Adopted,
    };
    // Actual already at desired: in sync, whoever moved it there.
    assert_eq!(
        reconcile(Some(&written("tv")), Some("tv"), "tv"),
        Observed::Present
    );
    assert_eq!(
        reconcile(Some(&written("old")), Some("tv"), "tv"),
        Observed::Present
    );
    // Actual differs, but lemonfiber's intent is unchanged from the baseline: the
    // operator's edit, preserved.
    assert_eq!(
        reconcile(Some(&written("tv")), Some("mine"), "tv"),
        Observed::Drifted
    );
    // Actual still at the baseline, only lemonfiber's intent moved: stale, its own.
    assert_eq!(
        reconcile(Some(&written("tv")), Some("tv"), "tv-hd"),
        Observed::Stale
    );
    // Baseline matches neither side: both moved away — a conflict.
    assert_eq!(
        reconcile(Some(&written("tv")), Some("mine"), "tv-hd"),
        Observed::Conflicted
    );
    assert_eq!(
        reconcile(Some(&written("tv")), None, "tv-hd"),
        Observed::Conflicted
    );
    // An adopted value the service still holds is kept, even though lemonfiber's
    // desired differs — it is the operator's, not lemonfiber's own to bring up to
    // date; changed again, it is a fresh edit to preserve.
    assert_eq!(
        reconcile(Some(&adopted("mine")), Some("mine"), "tv"),
        Observed::Adopted
    );
    assert_eq!(
        reconcile(Some(&adopted("mine")), Some("other"), "tv"),
        Observed::Drifted
    );
    // An adopted value the operator moved to match lemonfiber's desired is in sync.
    assert_eq!(
        reconcile(Some(&adopted("mine")), Some("tv"), "tv"),
        Observed::Present
    );
    // No baseline to judge against: the value the service holds is the operator's
    // own, unmanaged — adopted rather than overwritten on a guess.
    assert_eq!(reconcile(None, Some("mine"), "tv"), Observed::Unmanaged);
    assert_eq!(reconcile(None, None, "tv"), Observed::Unmanaged);
}

#[test]
fn only_a_wired_preserved_or_stale_connection_is_settled() {
    // Settled: written, already correct, the operator's own edit, an adopted or
    // pre-existing value theirs to keep, or lemonfiber's own value merely behind its
    // intent — all working states.
    for settled in [
        State::Wired,
        State::AlreadyWired,
        State::Drifted,
        State::Stale,
        State::Adopted,
        State::Unmanaged,
    ] {
        assert!(settled.is_settled(), "{settled:?} is settled");
    }
    // Not settled: a re-run or an operator's decision must return to it.
    for unsettled in [
        State::Skipped {
            reason: "lidarr is not in the active form".to_owned(),
        },
        State::Failed {
            detail: "rejected".to_owned(),
        },
        State::Conflicted {
            yours: Some("mine".to_owned()),
            ours: "tv-hd".to_owned(),
        },
    ] {
        assert!(!unsettled.is_settled(), "{unsettled:?} is not settled");
    }
}

fn wiring(connection: &str, state: State) -> Wiring {
    Wiring::settled(connection.to_owned(), state)
}

#[test]
fn a_pass_where_everything_settled_is_complete() {
    let report = Report {
        assessment: Assessment::Assessed,
        wirings: vec![
            wiring("SABnzbd into Sonarr", State::Wired),
            wiring("root folder in Radarr", State::AlreadyWired),
        ],
    };
    assert!(report.is_complete());
    assert!(report.outstanding().is_empty());
}

#[test]
fn a_skip_or_a_failure_leaves_a_pass_incomplete_and_named() {
    let report = Report {
        assessment: Assessment::Assessed,
        wirings: vec![
            wiring("SABnzbd into Sonarr", State::Wired),
            wiring(
                "SABnzbd into Lidarr",
                State::Skipped {
                    reason: "lidarr is not running".to_owned(),
                },
            ),
            wiring(
                "qBittorrent into Radarr",
                State::Failed {
                    detail: "rejected".to_owned(),
                },
            ),
        ],
    };
    assert!(!report.is_complete());
    let outstanding: Vec<&str> = report
        .outstanding()
        .into_iter()
        .map(|wiring| wiring.connection.as_str())
        .collect();
    assert_eq!(
        outstanding,
        vec!["SABnzbd into Lidarr", "qBittorrent into Radarr"],
        "a re-run is told exactly what it still owes"
    );
}

#[test]
fn a_blocked_connection_is_named_apart_from_the_merely_outstanding() {
    // A refusal and a conflict are both outstanding like a skip, but a re-run will
    // not lift them — the operator must resolve them — so `blocked` names them apart
    // from a skip, which a later run does complete.
    let report = Report {
        assessment: Assessment::Assessed,
        wirings: vec![
            wiring("tv root folder in sonarr", State::Wired),
            wiring(
                "SABnzbd into Lidarr",
                State::Skipped {
                    reason: "lidarr is not running".to_owned(),
                },
            ),
            wiring(
                "movies root folder in radarr",
                State::Refused {
                    reason: "shared with sonarr".to_owned(),
                },
            ),
            wiring(
                "SABnzbd into Sonarr",
                State::Conflicted {
                    yours: Some("mine".to_owned()),
                    ours: "tv-hd".to_owned(),
                },
            ),
        ],
    };
    assert!(!report.is_complete());
    let blocked: Vec<&str> = report
        .blocked()
        .into_iter()
        .map(|wiring| wiring.connection.as_str())
        .collect();
    assert_eq!(
        blocked,
        vec!["movies root folder in radarr", "SABnzbd into Sonarr"],
        "a refusal and a conflict are named as blocked; the skip is not",
    );
}

#[test]
fn the_report_names_each_state_on_the_wire() {
    let report = Report {
        assessment: Assessment::Assessed,
        wirings: vec![
            wiring("a", State::Wired),
            wiring("b", State::AlreadyWired),
            wiring("c", State::Drifted),
            wiring(
                "d",
                State::Skipped {
                    reason: "later".to_owned(),
                },
            ),
            wiring(
                "e",
                State::Failed {
                    detail: "no".to_owned(),
                },
            ),
            wiring("f", State::Stale),
            wiring(
                "g",
                State::Conflicted {
                    yours: Some("mine".to_owned()),
                    ours: "tv-hd".to_owned(),
                },
            ),
            wiring("h", State::Adopted),
            wiring("i", State::Unmanaged),
        ],
    };
    let json = serde_json::to_string(&report).unwrap_or_default();
    for state in [
        "wired",
        "already-wired",
        "drifted",
        "skipped",
        "failed",
        "stale",
        "conflicted",
        "adopted",
        "unmanaged",
    ] {
        assert!(json.contains(&format!(r#""state":"{state}""#)), "{json}");
    }
}

#[test]
fn the_report_draws_out_the_drifts_that_broke_the_stack() {
    // A drift raised to a warning is drawn out on its own, while an ordinary drift and
    // the settled connections are not — so a surface can lead with what must be acted
    // on. The warning also serialises its severity, breakage and remedy.
    let mut broken = wiring("broken", State::Drifted);
    broken.escalate(
        "the root folder points where nothing exists".to_owned(),
        "create the directory".to_owned(),
    );
    let report = Report {
        assessment: Assessment::Assessed,
        wirings: vec![
            broken,
            wiring("ordinary", State::Drifted),
            wiring("fine", State::Wired),
        ],
    };
    let warned: Vec<&str> = report
        .warnings()
        .iter()
        .map(|wiring| wiring.connection.as_str())
        .collect();
    assert_eq!(warned, vec!["broken"], "only the broken drift is a warning");
    let json = serde_json::to_string(&report).unwrap_or_default();
    assert!(json.contains(r#""severity":"warning""#), "{json}");
    assert!(json.contains(r#""severity":"informational""#), "{json}");
}
