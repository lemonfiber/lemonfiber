use std::sync::Arc;

use async_trait::async_trait;

use super::{OwnedCheck, Part};
use crate::doctor::{Check as _, Verdict};
use crate::ports::service::{Failure, Owner, Ownership};

/// How a part the check asks answers.
#[derive(Clone, Copy)]
enum Asked {
    /// With who holds it.
    Holding(Owner),
    /// Not at all.
    Silent,
    /// It cannot be asked over its contract.
    Unaskable,
}

/// A part answering who holds it with `answer`, not answering where it is nothing, and
/// refusing to be claimed.
struct Answering(Option<Owner>);

#[async_trait]
impl Ownership for Answering {
    async fn owner(&self) -> Result<Owner, Failure> {
        self.0.ok_or_else(|| Failure::Unavailable {
            service: "part".to_owned(),
        })
    }

    async fn claim(&self) -> Result<Owner, Failure> {
        Err(Failure::Refused {
            service: "part".to_owned(),
            detail: "a diagnosis claims nothing".to_owned(),
        })
    }
}

/// The verdicts the check comes to over `parts`, each a part answering as given.
async fn verdicts(parts: Vec<Asked>) -> Vec<Verdict> {
    OwnedCheck::new(
        parts
            .into_iter()
            .enumerate()
            .map(|(at, answer)| {
                (
                    Part {
                        id: format!("part-{at}"),
                        name: format!("Part {at}"),
                        secrets: vec!["CLAIMER_PASSWORD".to_owned()],
                    },
                    match answer {
                        Asked::Holding(owner) => Some(Answering(Some(owner))),
                        Asked::Silent => Some(Answering(None)),
                        Asked::Unaskable => None,
                    }
                    .map(|part| Arc::new(part) as Arc<dyn Ownership>),
                )
            })
            .collect(),
    )
    .run()
    .await
    .into_iter()
    .map(|finding| finding.verdict)
    .collect()
}

fn warned_with(verdict: &Verdict, said: &str) -> bool {
    matches!(verdict, Verdict::Warn(problem)
        if problem.code.as_str() == "CONFIG-11"
            && format!("{problem:?}").contains(said))
}

#[tokio::test]
async fn a_part_held_by_lemonfiber_passes_and_one_taking_up_its_claim_too() {
    let verdicts = verdicts(vec![
        Asked::Holding(Owner::Ours),
        Asked::Holding(Owner::Settling),
    ])
    .await;
    assert!(
        verdicts
            .iter()
            .all(|verdict| matches!(verdict, Verdict::Pass { .. })),
        "{verdicts:?}"
    );
}

#[tokio::test]
async fn a_part_nobody_holds_or_somebody_else_holds_is_a_warning_naming_what_to_do() {
    let verdicts = verdicts(vec![
        Asked::Holding(Owner::Open),
        Asked::Holding(Owner::Theirs),
    ])
    .await;
    assert!(
        verdicts
            .first()
            .is_some_and(|one| warned_with(one, "lemonfiber seed")),
        "{verdicts:?}"
    );
    assert!(
        verdicts
            .get(1)
            .is_some_and(|one| warned_with(one, "CLAIMER_PASSWORD")),
        "{verdicts:?}"
    );
}

#[tokio::test]
async fn a_part_that_will_not_say_is_a_warning_never_a_pass() {
    let verdicts = verdicts(vec![Asked::Silent, Asked::Unaskable]).await;
    assert!(
        verdicts
            .iter()
            .all(|verdict| warned_with(verdict, "could not be asked")),
        "{verdicts:?}"
    );
}

#[tokio::test]
async fn a_machine_with_no_part_claimed_first_skips_the_check() {
    let verdicts = verdicts(Vec::new()).await;
    assert!(
        matches!(verdicts.as_slice(), [Verdict::Skipped { .. }]),
        "{verdicts:?}"
    );
}
