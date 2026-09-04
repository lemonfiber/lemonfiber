//! What is said where what a household may ask for could not be settled.
//!
//! **A limit that refuses out loud is the whole design.** A request quietly dropped and
//! a request refused with the figure beside it are the same outcome for the disk and
//! opposite outcomes for the person who asked, so every refusal here names the thing it
//! turned on — the limit, the word, the request, or the person — and never merely
//! reports that something did not work.
//!
//! **The disk is not one of these.** A machine with no room left refuses an acquisition
//! in [`crate::app`]'s own words, from the one reading of the volumes every command that
//! brings content onto the disk shares. That refusal names the disk and this file names
//! limits, which is exactly the distinction that matters: an operator who read a full
//! disk as somebody's quota would go and raise a quota and watch it happen again.

use crate::error::{Amiss, Code, Problem, Remedy, Severity};

/// Raised where the request service would not answer, so nothing was changed.
pub const UNREACHABLE: Code = Code::new("QUOTA-1");

/// Raised where a policy that lives inside a limit was chosen without one.
pub const NO_LIMIT: Code = Code::new("QUOTA-2");

/// Raised where no policy goes by the word that was given.
pub const NO_SUCH_POLICY: Code = Code::new("QUOTA-3");

/// Raised where the request named is not one that is waiting on anybody.
pub const NOT_WAITING: Code = Code::new("QUOTA-4");

/// Raised where a request was turned down and the reason said nothing.
pub const NO_REASON: Code = Code::new("QUOTA-5");

/// Raised where nobody in the household goes by the name that was given.
pub const NOBODY: Code = Code::new("QUOTA-6");

/// Raised where the request service holds no account for somebody who has one here.
pub const NEVER_HERE: Code = Code::new("QUOTA-7");

/// Said where the request service could not be asked or would not answer.
///
/// Named as nothing having been changed rather than as a failure, because those are
/// different states to be left in: an operator told only that something went wrong does
/// not know whether to set the policy again or to go and check what it now says.
#[must_use]
pub fn unreachable(doing: &str) -> Problem {
    Problem::new(
        UNREACHABLE,
        Severity::Error,
        format!("the request service would not answer, so {doing}"),
        "What the household may ask for is that service's to hold, so nothing here \
         could be changed and nothing was — what it had before is what it still has",
        Remedy::new("Check the request service is running, then run this again"),
    )
}

/// Said where a policy that only means something with a limit was chosen without one.
///
/// Refused rather than given a number of this product's choosing. A limit is the whole
/// of what that policy is, and one invented here would be a household held to a figure
/// nobody in it agreed to.
#[must_use]
pub fn no_limit_named() -> Problem {
    Problem::new(
        NO_LIMIT,
        Severity::Error,
        "living within a limit needs a limit, and none was named",
        "This policy lets everything through until somebody has used up their share of \
         a period, so without a share it would be the same as trusting everybody — \
         which is a policy of its own and would be chosen by name",
        Remedy::new("Say how many requests a period allows, and how long the period is"),
    )
    .lies_in(Amiss::Asking)
}

/// Said where no policy goes by the word that was given, with the ones there are named.
#[must_use]
pub fn no_such_policy(written: &str) -> Problem {
    Problem::new(
        NO_SUCH_POLICY,
        Severity::Error,
        format!("`{written}` is not one of the ways a household may be trusted"),
        "What happens to a request is one of three things: it arrives, it arrives \
         within a limit, or it waits for you",
        Remedy::new("Choose one of the three").with_detail(super::Policy::labels()),
    )
    .lies_in(Amiss::Naming)
}

/// Said where the request named is not one anybody is waiting on.
///
/// The number is the request service's own, and one already ruled on is not a mistake to
/// correct silently: an operator approving something a second time has misread a list,
/// and being told so is worth more than a second approval that changes nothing.
#[must_use]
pub fn nothing_to_decide(request: i64) -> Problem {
    Problem::new(
        NOT_WAITING,
        Severity::Error,
        format!("request {request} is not one that is waiting on anybody"),
        "Only a request nobody has ruled on can be approved or turned down — one \
         already decided keeps the answer it was given",
        Remedy::new("Ask what the household has asked for, to see what is still waiting")
            .with_detail("lemonfiber household"),
    )
    .lies_in(Amiss::Naming)
}

/// Said where a decline carried a reason that says nothing.
///
/// A blank reason is the silent decline this is here to prevent, arriving through the
/// field meant to prevent it.
#[must_use]
pub fn no_reason_given() -> Problem {
    Problem::new(
        NO_REASON,
        Severity::Error,
        "turning a request down needs a reason, and the one given was blank",
        "Somebody asked for this and will see that it was refused; a refusal with \
         nothing beside it is indistinguishable from being ignored, which is the \
         conversation this is here to save you",
        Remedy::new("Say why in a few words, and pass them on to whoever asked"),
    )
    .lies_in(Amiss::Asking)
}

/// Said where nobody in the household goes by the name that was given.
#[must_use]
pub fn nobody_called(named: &str, household: &[String]) -> Problem {
    Problem::new(
        NOBODY,
        Severity::Error,
        format!("nobody in this household goes by {named}, so nothing was changed"),
        "A limit is set on somebody the media server holds an account for, matched the \
         way you would say their name rather than exactly",
        Remedy::new("Name somebody who is here").with_detail(household.join(", ")),
    )
    .lies_in(Amiss::Naming)
}

/// Said where somebody has an account here and none on the request service.
///
/// Not a fault and not a member who is missing. The request service learns of somebody
/// when they first sign in to it, so this is an invitation nobody has used yet — and a
/// limit written against nobody would read as a limit that had been applied.
#[must_use]
pub fn never_asked_here(name: &str) -> Problem {
    Problem::new(
        NEVER_HERE,
        Severity::Warning,
        format!(
            "{name} has never signed in to the request service, so there is nobody \
                 there to hold to a limit"
        ),
        "The request service learns of somebody the first time they sign in to it, and \
         until then it holds no account of theirs for a limit to sit on — what the \
         household is held to applies to them in the meantime",
        Remedy::new("Ask them to open the request service once, then set this again"),
    )
    .lies_in(Amiss::Naming)
}

#[cfg(test)]
mod tests {
    use super::{
        never_asked_here, no_limit_named, no_reason_given, no_such_policy, nobody_called,
        nothing_to_decide, unreachable, NEVER_HERE, NOBODY, NOT_WAITING, NO_LIMIT, NO_REASON,
        NO_SUCH_POLICY, UNREACHABLE,
    };
    use crate::error::{Amiss, Severity};

    /// A service that would not answer is said as nothing having changed.
    #[test]
    fn a_service_that_would_not_answer_says_nothing_changed() {
        let problem = unreachable("the policy was not set");

        assert_eq!(problem.code, UNREACHABLE);
        assert!(problem.summary.contains("the policy was not set"));
        assert!(problem.meaning.contains("still has"), "{problem:?}");
    }

    /// The policy that is a limit refuses to be chosen without one.
    #[test]
    fn the_policy_that_is_a_limit_refuses_to_be_chosen_without_one() {
        let problem = no_limit_named();

        assert_eq!(problem.code, NO_LIMIT);
        assert_eq!(problem.amiss, Amiss::Asking);
        assert!(problem.remedies.first().is_some_and(|remedy| {
            remedy.action.contains("how many") && remedy.action.contains("how long")
        }));
    }

    /// A word nobody offers is refused with the ones there are named beside it.
    #[test]
    fn a_word_nobody_offers_is_refused_with_the_ones_there_are() {
        let problem = no_such_policy("generous");

        assert_eq!(problem.code, NO_SUCH_POLICY);
        assert_eq!(problem.amiss, Amiss::Naming);
        assert!(problem.summary.contains("generous"));
        let offered = problem
            .remedies
            .first()
            .and_then(|remedy| remedy.detail.clone())
            .unwrap_or_default();
        assert!(offered.contains("trusted"), "{offered}");
        assert!(offered.contains("everything-waits"), "{offered}");
    }

    /// A request already ruled on is named rather than decided a second time.
    #[test]
    fn a_request_already_ruled_on_is_named_rather_than_decided_again() {
        let problem = nothing_to_decide(42);

        assert_eq!(problem.code, NOT_WAITING);
        assert_eq!(problem.amiss, Amiss::Naming);
        assert!(problem.summary.contains("42"), "{problem:?}");
    }

    /// A blank reason is the silent decline arriving through the field meant to stop it.
    #[test]
    fn a_blank_reason_is_refused_as_the_silence_it_would_be() {
        let problem = no_reason_given();

        assert_eq!(problem.code, NO_REASON);
        assert_eq!(problem.amiss, Amiss::Asking);
        assert!(problem.meaning.contains("ignored"), "{problem:?}");
    }

    /// Nobody by that name is refused with the household named beside it.
    #[test]
    fn nobody_by_that_name_is_refused_with_the_household_named() {
        let problem = nobody_called("sam", &["ana".to_owned(), "bea".to_owned()]);

        assert_eq!(problem.code, NOBODY);
        assert_eq!(problem.amiss, Amiss::Naming);
        assert!(problem.summary.contains("sam"), "{problem:?}");
        let there = problem
            .remedies
            .first()
            .and_then(|remedy| remedy.detail.clone())
            .unwrap_or_default();
        assert!(there.contains("ana") && there.contains("bea"), "{there}");
    }

    /// Somebody who has never signed in is an invitation unused, not a fault.
    ///
    /// It says what holds them in the meantime, because an operator told only that
    /// nothing happened would not know whether they are limited or not.
    #[test]
    fn somebody_who_has_never_signed_in_is_not_a_fault() {
        let problem = never_asked_here("ana");

        assert_eq!(problem.code, NEVER_HERE);
        assert_eq!(problem.severity, Severity::Warning);
        assert!(problem.summary.contains("ana"), "{problem:?}");
        assert!(problem.meaning.contains("in the meantime"), "{problem:?}");
    }
}
