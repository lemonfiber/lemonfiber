use std::sync::Arc;

use async_trait::async_trait;

use super::{MovesCheck, Offers, CHECK};
use crate::app::plugins::MoveOffered;
use crate::doctor::{Category, Check, Verdict};
use crate::error::codes::plugin::MOVE_OFFERED;

/// Offers the moves given, or cannot read what is installed.
struct Offering(Result<Vec<MoveOffered>, ()>);

#[async_trait]
impl Offers for Offering {
    async fn moves(&self) -> Result<Vec<MoveOffered>, ()> {
        self.0.clone()
    }
}

fn checking(moves: Result<Vec<MoveOffered>, ()>) -> MovesCheck {
    MovesCheck::new(Arc::new(Offering(moves)))
}

fn jellyfin() -> MoveOffered {
    MoveOffered {
        plugin: "jellyfin".to_owned(),
        service: "jellyfin".to_owned(),
    }
}

/// Nothing on offer is a pass.
#[tokio::test]
async fn nothing_on_offer_passes() {
    let check = checking(Ok(Vec::new()));
    let found = check.run().await;
    assert_eq!(check.category(), Category::Services);
    let [finding] = found.as_slice() else {
        unreachable!("one finding: {found:?}");
    };
    assert_eq!(finding.check, CHECK);
    assert!(matches!(finding.verdict, Verdict::Pass { .. }));
}

/// A move on offer is named, with the reading that takes it.
#[tokio::test]
async fn a_move_on_offer_is_named_with_the_reading_that_takes_it() {
    let found = checking(Ok(vec![jellyfin()])).run().await;
    let [finding] = found.as_slice() else {
        unreachable!("one finding: {found:?}");
    };
    let Verdict::Warn(problem) = &finding.verdict else {
        unreachable!("a move on offer is a warning: {:?}", finding.verdict);
    };
    assert_eq!(problem.code, MOVE_OFFERED);
    assert!(problem.summary.contains("jellyfin"), "{}", problem.summary);
    assert_eq!(
        problem
            .remedies
            .first()
            .and_then(|remedy| remedy.detail.as_deref()),
        Some("lemonfiber update stack")
    );
}

/// A record of what is installed that cannot be read leaves the offer unknown, never a pass.
#[tokio::test]
async fn an_unreadable_record_of_what_is_installed_is_unverified_never_a_pass() {
    let found = checking(Err(())).run().await;
    let [finding] = found.as_slice() else {
        unreachable!("one finding: {found:?}");
    };
    let Verdict::Unverified { remedy, .. } = &finding.verdict else {
        unreachable!("an unreadable record is unverified: {:?}", finding.verdict);
    };
    assert_eq!(
        remedy.detail.as_deref(),
        Some("lemonfiber plugin installed")
    );
}
