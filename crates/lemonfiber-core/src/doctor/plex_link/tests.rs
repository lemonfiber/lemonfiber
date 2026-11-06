use super::{Linked, PlexLinkCheck, CHECK};
use crate::doctor::{Check, Verdict};

/// The link with `recorded` as the mode, the request service's own account named `own`
/// and Plex claimed by `claimed_by`.
fn linked(recorded: Option<&str>, own: Option<&str>, claimed_by: Option<&str>) -> Linked {
    Linked {
        requests: "seerr".to_owned(),
        recorded: recorded.map(str::to_owned),
        own: own.map(str::to_owned),
        claimed_by: claimed_by.map(str::to_owned),
    }
}

/// The one verdict the check gives over `linked`.
async fn verdict(linked: Option<Linked>) -> Option<Verdict> {
    let findings = PlexLinkCheck::new(linked).run().await;
    assert_eq!(findings.len(), 1, "{findings:?}");
    findings.into_iter().next().map(|finding| {
        assert_eq!(finding.check, CHECK);
        finding.verdict
    })
}

/// The note a passing verdict carries.
fn noted(verdict: Option<Verdict>) -> String {
    match verdict {
        Some(Verdict::Pass { note }) => note.unwrap_or_default(),
        other => format!("not a pass: {other:?}"),
    }
}

#[tokio::test]
async fn managed_says_the_owner_is_its_own_account_and_members_ask_through_lemonfiber() {
    let note = noted(verdict(Some(linked(None, Some("lemonfiber-gate"), None))).await);

    assert!(note.contains("PLEX_LINK_MODE is managed"), "{note}");
    assert!(
        note.contains("its own Plex account, lemonfiber-gate"),
        "{note}"
    );
    assert!(
        note.contains("members cannot sign in to its page with Plex"),
        "{note}"
    );
}

#[tokio::test]
async fn fronted_says_members_can_sign_in_and_plex_tv_is_answered_by_the_gate() {
    let note = noted(verdict(Some(linked(Some("fronted"), None, None))).await);

    assert!(note.contains("PLEX_LINK_MODE is fronted"), "{note}");
    assert!(
        note.contains("the request service's owner is its own Plex account,"),
        "{note}"
    );
    assert!(
        note.contains("members can sign in to its page with Plex"),
        "{note}"
    );
}

#[tokio::test]
async fn owner_is_a_standing_notice_of_what_holding_the_operators_token_costs() {
    let warned = verdict(Some(linked(Some("owner"), None, Some("4815162342")))).await;

    assert!(matches!(warned, Some(Verdict::Warn(_))), "{warned:?}");
    let Some(Verdict::Warn(problem)) = warned else {
        return;
    };
    assert_eq!(problem.code.as_str(), "SETTING-10");
    assert!(
        problem.summary.contains("plex.tv id 4815162342"),
        "{problem:?}"
    );
    assert!(
        problem.summary.contains("members can sign in"),
        "{problem:?}"
    );
    assert!(
        problem.meaning.contains(crate::plex_link::OWNER_COST),
        "{problem:?}"
    );
}

#[tokio::test]
async fn a_mode_that_names_none_is_unverified_and_no_plex_is_skipped() {
    assert!(matches!(
        verdict(Some(linked(Some("shared"), None, None))).await,
        Some(Verdict::Unverified { reason, .. }) if reason.contains("shared is none of them")
    ));
    assert!(matches!(verdict(None).await, Some(Verdict::Skipped { .. })));
}
