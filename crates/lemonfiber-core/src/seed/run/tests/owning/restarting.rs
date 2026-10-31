//! A part whose upstream takes its claim only once it restarts.

use std::sync::atomic::{AtomicUsize, Ordering};

use super::*;
use crate::ports::service::{Claim, Failure, Owner, Ownership};
use crate::seed::run::owning::{claimed, Reaching, Restarts};

/// A part that takes its claim on restart, answering who holds it as `after` once it has.
struct Restarting {
    after: Owner,
    restarted: AtomicUsize,
    refuses: bool,
}

#[async_trait::async_trait]
impl Ownership for Restarting {
    async fn owner(&self) -> Result<Owner, Failure> {
        if self.restarted.load(Ordering::Relaxed) == 0 {
            Ok(Owner::Open)
        } else {
            Ok(self.after)
        }
    }

    async fn claim(&self) -> Result<Claim, Failure> {
        Ok(Claim::ON_RESTART)
    }
}

#[async_trait::async_trait]
impl Restarts for Restarting {
    async fn restarted(&self) -> Result<(), Failure> {
        if self.refuses {
            return Err(Failure::Refused {
                service: SERVICE.to_owned(),
                detail: "the engine refused".to_owned(),
            });
        }
        self.restarted.fetch_add(1, Ordering::Relaxed);
        Ok(())
    }
}

/// A part answering `after` once restarted, its restart refused where `refuses`.
fn restarting(after: Owner, refuses: bool) -> Restarting {
    Restarting {
        after,
        restarted: AtomicUsize::new(0),
        refuses,
    }
}

/// What claiming `part` comes to.
async fn claiming(part: &Restarting) -> Result<Owner, Failure> {
    claimed(Reaching {
        part,
        restarts: part,
    })
    .await
}

#[tokio::test]
async fn a_claim_taken_on_restart_restarts_the_upstream_and_asks_who_holds_it_again() {
    let part = restarting(Owner::Ours, false);
    assert!(matches!(claiming(&part).await, Ok(Owner::Ours)));
    assert_eq!(part.restarted.load(Ordering::Relaxed), 1);
}

#[tokio::test]
async fn a_claim_taken_on_restart_counts_only_once_the_part_answers_ours() {
    let part = restarting(Owner::Open, false);
    assert!(matches!(claiming(&part).await, Ok(Owner::Open)));
    let unclaimed = super::super::super::owning::unclaimed("Plex", Owner::Open, &[]);
    assert!(unclaimed.is_some_and(|said| said.contains("anybody may still take it")));
}

#[tokio::test]
async fn an_upstream_that_cannot_be_started_again_fails_the_claim_naming_it() {
    let part = restarting(Owner::Ours, true);
    let failed = claiming(&part).await;
    assert!(
        matches!(&failed, Err(Failure::Refused { service, .. }) if service == SERVICE),
        "{failed:?}"
    );
    assert_eq!(part.restarted.load(Ordering::Relaxed), 0);
}

#[tokio::test]
async fn a_seed_whose_claim_is_taken_on_restart_starts_the_part_again_through_the_stack() {
    let project = lemonfiber_fixtures::scratch::Scratch::new("owning-restarting");
    let http = Fake::by_path(vec![(CLAIM, Answer::reply(205, ""))]);
    let ctx = contracted_ctx(&project, SERVICE, true, http.clone());

    let wirings = super::super::super::owning::owned(
        &ctx,
        &beside(&project, &first_party(PLUGIN)),
        &[claimer()],
        &mut Baseline::new(),
    )
    .await;

    assert!(
        wirings.first().is_some_and(|one| matches!(&one.state,
            crate::seed::State::Failed { detail } if detail.contains(SERVICE))),
        "{wirings:?}"
    );
    assert_eq!(asked_at(&http, CLAIM), 1);
}
