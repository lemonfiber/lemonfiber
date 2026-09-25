//! Whether the people in the house will hear back about what they asked for.
//!
//! The read-only half of the seeding step that switches the request service's
//! telling on. Seeding does the comparison when it writes; this makes it again,
//! changing nothing, so an operator running a diagnosis is told when the household
//! has quietly stopped being notified.
//!
//! **Three values, not two** — what lemonfiber recorded, what the service holds now,
//! and what lemonfiber would write — read through the same `reconcile` the seeding
//! uses, so the two cannot come to different opinions about the same field.
//!
//! What each outcome is worth is the judgement here, and it is not the same
//! judgement seeding makes. Seeding decides whether to write; this decides whether
//! anybody needs telling. **An operator who switched notifications off does not need
//! telling** — they know, and a diagnosis that warned about it every run would be
//! training them to ignore it. What is worth raising is lemonfiber's own value
//! fallen behind what it now intends, which nobody chose and nobody can see.

use std::sync::Arc;

use async_trait::async_trait;

use super::{Category, Check, Finding, Verdict};
use crate::baseline::Record;
use crate::error::codes::telling::BEHIND;
use crate::error::{Problem, Remedy, Severity};
use crate::ports::service::Requests;
use crate::seed::drift::Observed;
use crate::seed::observed_telling;

/// The name this check and anything answering it share.
const CHECK: &str = "config.household-telling";

/// The heading an operator reads this under.
const TITLE: &str = "What the household is told";

/// Reports whether the request service will tell the household what became of what
/// they asked for.
pub struct TellingCheck {
    /// The request service, absent where the stack has none to ask.
    seerr: Option<Arc<dyn Requests>>,
    /// What lemonfiber last recorded having set the telling to.
    recorded: Option<Record>,
}

impl TellingCheck {
    /// A check over the request service given, against what was last recorded for it.
    #[must_use]
    pub fn new(seerr: Option<Arc<dyn Requests>>, recorded: Option<Record>) -> Self {
        Self { seerr, recorded }
    }
}

#[async_trait]
impl Check for TellingCheck {
    fn category(&self) -> Category {
        Category::Services
    }

    async fn run(&self) -> Vec<Finding> {
        ran(self).await
    }
}

/// Whether the operator can be told anything, and by which route.
async fn ran(check: &TellingCheck) -> Vec<Finding> {
    let Some(seerr) = check.seerr.as_ref() else {
        return vec![finding(Verdict::Skipped {
            reason: "this stack has no request service, so there is nothing to ask \
                     and nobody asking"
                .to_owned(),
        })];
    };

    let held = match seerr.telling().await {
        Ok(held) => held,
        // Nobody could find out. Said as its own thing rather than dressed as a
        // verdict about the household, which is the distinction between not
        // knowing and knowing something is wrong.
        Err(failure) => {
            return vec![finding(Verdict::Unverified {
                reason: format!(
                    "the request service could not be asked what it tells the \
                     household: {failure}"
                ),
                remedy: Remedy::new("Check the service is up and has finished starting")
                    .with_detail("lemonfiber status"),
            })]
        }
    };

    vec![finding(verdict(
        observed_telling(check.recorded.as_ref(), held),
        held.enabled,
    ))]
}

/// The one finding this check produces, under the name anything answering it shares.
fn finding(verdict: Verdict) -> Finding {
    Finding::in_category(Category::Services, CHECK, TITLE, verdict)
}

/// What one observation of the telling is worth to somebody reading a diagnosis.
fn verdict(observed: Observed, sending: bool) -> Verdict {
    match observed {
        // Nothing has ever been set here. Switching it on is seeding's errand, and
        // reporting it as a fault would be a second voice on the same thing.
        //
        // `Unavailable` cannot arrive: it is what a seed pass says about a service
        // that would not answer, and one that would not answer was reported
        // unverified above.
        Observed::Absent | Observed::Unavailable => Verdict::Skipped {
            reason: "the household's notifications have not been set up yet".to_owned(),
        },
        Observed::Present => Verdict::Pass {
            note: Some(
                "the household is told when a request arrives, is decided, and lands".to_owned(),
            ),
        },
        // Theirs, and they know. Reported as what it is rather than as a fault, and
        // said differently depending on which way they set it — "you turned this
        // off" is the useful sentence, and it is only true when they did.
        Observed::Drifted | Observed::Conflicted | Observed::Adopted | Observed::Unmanaged => {
            Verdict::Pass {
                note: Some(
                    if sending {
                        "the household is told what you set it to tell them, not what \
                         lemonfiber would have chosen"
                    } else {
                        "the household is told nothing, because that is how you set it"
                    }
                    .to_owned(),
                ),
            }
        }
        Observed::Stale => Verdict::Warn(behind()),
    }
}

/// lemonfiber's own value, fallen behind what lemonfiber now intends.
///
/// The one outcome worth raising. Nobody chose it and nobody can see it: the service
/// still holds exactly what lemonfiber last wrote, so nothing looks edited — while
/// the occasions lemonfiber has since added go unsent, and the household hears about
/// less than it should with no sign that anything is missing.
fn behind() -> Problem {
    Problem::new(
        BEHIND,
        Severity::Warning,
        "the household is told about less than lemonfiber now sets out to tell them",
        "Somebody asks for something and hears nothing back on one of the occasions \
         this is meant to close the loop on, so they come and ask you instead",
        Remedy::new("Bring what the household is told up to what lemonfiber now sends")
            .with_detail(crate::repair::ASK_FOR_REPAIRS),
    )
}

#[cfg(test)]
mod tests;
