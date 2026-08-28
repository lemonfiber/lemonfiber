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
use crate::error::{Code, Problem, Remedy, Severity};
use crate::ports::service::Requests;
use crate::seed::drift::Observed;
use crate::seed::observed_telling;

/// Raised when the household is told about less than lemonfiber now sets out to tell
/// them, through no choice of the operator's.
pub const BEHIND: Code = Code::new("TELLING-1");

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
        let Some(seerr) = self.seerr.as_ref() else {
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
            observed_telling(self.recorded.as_ref(), held),
            held.enabled,
        ))]
    }
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
mod tests {
    use super::{TellingCheck, BEHIND};
    use crate::baseline::Baseline;
    use crate::doctor::{Category, Check, Verdict};
    use crate::seed::{said, wanted_telling, TELLING};
    use crate::seerr::Seerr;
    use lemonfiber_fixtures::http::{Answer, Fake};
    use std::sync::Arc;

    /// A telling that is on, but not for the occasions lemonfiber would choose.
    const SOME: &str = r#"{"enabled":true,"types":8}"#;

    /// The real client over a scripted service, so the check reads the request this
    /// product actually sends.
    fn asking(answer: Answer) -> Arc<dyn crate::ports::service::Requests> {
        let http = Fake::by_path_in_turn(vec![("/settings/notifications/webpush", vec![answer])]);
        Arc::new(Seerr::new(http, "http://seerr:5055", "seerr"))
    }

    fn recorded(value: &str) -> Baseline {
        let mut baseline = Baseline::new();
        baseline.record("seerr", TELLING, value, "2026-08-28T00:00:00Z");
        baseline
    }

    async fn verdict_for(answer: Answer, baseline: &Baseline) -> Verdict {
        let check = TellingCheck::new(
            Some(asking(answer)),
            baseline.entry("seerr", TELLING).cloned(),
        );
        check
            .run()
            .await
            .into_iter()
            .next()
            .map_or(Verdict::Pass { note: None }, |finding| finding.verdict)
    }

    #[test]
    fn the_household_is_a_question_about_the_services() {
        let check = TellingCheck::new(None, None);
        assert_eq!(check.category(), Category::Services);
    }

    #[tokio::test]
    async fn a_stack_with_no_request_service_has_nothing_to_ask() {
        let check = TellingCheck::new(None, None);
        let found = check.run().await;
        let verdict = found.first().map(|finding| &finding.verdict);

        assert!(
            matches!(verdict, Some(Verdict::Skipped { .. })),
            "{found:?}"
        );
    }

    #[tokio::test]
    async fn a_service_that_will_not_answer_is_unverified_rather_than_a_fault() {
        let verdict = verdict_for(Answer::Silent, &Baseline::new()).await;

        assert!(
            matches!(verdict, Verdict::Unverified { .. }),
            "not knowing was reported as knowing: {verdict:?}"
        );
    }

    #[tokio::test]
    async fn a_telling_nobody_has_set_up_is_seedings_errand_rather_than_a_fault() {
        let verdict = verdict_for(
            Answer::reply(200, r#"{"enabled":false,"types":0}"#),
            &Baseline::new(),
        )
        .await;

        assert!(matches!(verdict, Verdict::Skipped { .. }), "{verdict:?}");
    }

    #[tokio::test]
    async fn a_household_told_everything_passes_and_says_so() {
        let held = format!(
            r#"{{"enabled":true,"types":{}}}"#,
            wanted_telling().occasions
        );
        let verdict =
            verdict_for(Answer::reply(200, held), &recorded(&said(wanted_telling()))).await;

        assert!(
            matches!(&verdict, Verdict::Pass { note } if note.as_deref().is_some_and(|said| said.contains("decided"))),
            "{verdict:?}"
        );
    }

    /// Their choice is reported as theirs, both ways round.
    ///
    /// Two cases rather than one, because the sentence differs and the wrong one is
    /// worse than none: telling an operator the household hears nothing when they had
    /// merely narrowed it would send them looking for a fault that is not there.
    #[tokio::test]
    async fn a_setting_the_operator_chose_is_reported_as_theirs_whichever_way_they_set_it() {
        // Narrowed: still sending, but not what lemonfiber would choose.
        let narrowed =
            verdict_for(Answer::reply(200, SOME), &recorded(&said(wanted_telling()))).await;
        assert!(
            matches!(&narrowed, Verdict::Pass { note } if note.as_deref().is_some_and(|said| said.contains("not what"))),
            "{narrowed:?}"
        );

        // Switched off entirely: the household hears nothing, and they chose that.
        let silent = verdict_for(
            Answer::reply(200, r#"{"enabled":false,"types":0}"#),
            &recorded(&said(wanted_telling())),
        )
        .await;
        assert!(
            matches!(&silent, Verdict::Pass { note } if note.as_deref().is_some_and(|said| said.contains("nothing"))),
            "{silent:?}"
        );
    }

    /// The one outcome worth raising.
    ///
    /// The service holds exactly what lemonfiber last wrote, so nothing looks edited
    /// — while what lemonfiber now sends has moved on, and the household hears about
    /// less than it should with no sign anything is missing.
    #[tokio::test]
    async fn lemonfibers_own_value_fallen_behind_is_the_thing_that_warns() {
        let verdict = verdict_for(Answer::reply(200, SOME), &recorded("on:8")).await;

        assert!(
            matches!(&verdict, Verdict::Warn(problem) if problem.code == BEHIND),
            "an operator is not told the household hears less than it should: {verdict:?}"
        );
    }
}
