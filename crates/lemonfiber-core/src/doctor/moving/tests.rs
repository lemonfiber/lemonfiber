use super::{MovesCheck, CHECK};
use crate::app::plugins::MoveOffered;
use crate::doctor::{Check, Verdict};
use crate::error::codes::plugin::MOVE_OFFERED;

fn jellyfin() -> MoveOffered {
    MoveOffered {
        plugin: "jellyfin".to_owned(),
        service: "jellyfin".to_owned(),
    }
}

/// Nothing on offer is a pass.
#[tokio::test]
async fn nothing_on_offer_passes() {
    let found = MovesCheck::new(Vec::new()).run().await;
    let [finding] = found.as_slice() else {
        unreachable!("one finding: {found:?}");
    };
    assert_eq!(finding.check, CHECK);
    assert!(matches!(finding.verdict, Verdict::Pass { .. }));
}

/// A move on offer is named, with the reading that takes it.
#[tokio::test]
async fn a_move_on_offer_is_named_with_the_reading_that_takes_it() {
    let found = MovesCheck::new(vec![jellyfin()]).run().await;
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
        Some("lemonfiber plugin install jellyfin")
    );
}
