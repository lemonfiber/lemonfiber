//! One shape for everything that goes wrong.
//!
//! What happened, what it means and what to do are struct fields rather than a
//! convention, so a problem that explains nothing cannot be constructed. The
//! wording lives here because all three surfaces must say the same thing;
//! colour, wrapping and placement are the surface's business.

use serde::{Deserialize, Serialize};

/// A stable identifier for a kind of problem.
///
/// Stability is the whole point: an operator who searches for a code should find
/// the same answer a year later. Codes are declared as constants beside the code
/// that raises them, and are never recycled.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize)]
pub struct Code(&'static str);

impl Code {
    /// Declare a problem code.
    #[must_use]
    pub const fn new(id: &'static str) -> Self {
        Self(id)
    }

    /// The code as it is shown, logged and searched for.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        self.0
    }
}

impl std::fmt::Display for Code {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.0)
    }
}

/// How much a problem matters.
///
/// Four levels, deliberately. More would not be applied consistently, and
/// inconsistent severity is worse than coarse severity.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Severity {
    /// Informational; nothing is required.
    Advisory,
    /// Degraded or risky, still working.
    Warning,
    /// Something is broken.
    Error,
    /// Consequences outside the machine, or data at risk.
    Critical,
}

/// Where a problem stands with respect to being fixed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum State {
    /// A remedy is available here.
    Actionable,
    /// The operator must act, somewhere else.
    Guided,
    /// lemonfiber can fix this itself.
    Remediable,
    /// No known remedy; escalation is offered instead.
    Unknown,
    /// Acknowledged, and not re-shown until it recurs.
    Suppressed,
}

/// One thing the operator can do about a problem.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Remedy {
    /// The action, phrased as something to do rather than something to know.
    pub action: String,
    /// Where to look, when that helps.
    pub detail: Option<String>,
}

impl Remedy {
    /// A remedy the operator can act on.
    #[must_use]
    pub fn new(action: impl Into<String>) -> Self {
        Self {
            action: action.into(),
            detail: None,
        }
    }

    /// Point the remedy at a command, a path or a log.
    #[must_use]
    /// Attach the underlying technical detail, with any credential in it withheld.
    ///
    /// Detail is where a service's own words are quoted verbatim, and a service that
    /// echoes its configuration back in an error message is an ordinary thing rather
    /// than an exotic one. So the withholding happens here rather than at each of the
    /// dozens of call sites: a rule you have to remember at every one of them is a
    /// rule that holds until somebody is in a hurry.
    pub fn with_detail(mut self, detail: impl Into<String>) -> Self {
        self.detail = Some(crate::config::store::withheld_text(&detail.into()));
        self
    }
}

/// The escalation offered when nothing better is known.
///
/// Admitting ignorance costs the operator a support bundle. Confident wrong
/// guidance costs them an afternoon, and costs us the trust that made them
/// believe the guidance in the first place.
fn escalation() -> Remedy {
    Remedy::new("Send a diagnostic bundle so this can be investigated")
        .with_detail("lemonfiber support")
}

/// Something that went wrong, in the form an operator can act on.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Problem {
    /// The stable identifier for this kind of problem.
    pub code: Code,
    /// How much it matters.
    pub severity: Severity,
    /// Where it stands with respect to being fixed.
    pub state: State,
    /// What happened, in one plain sentence.
    pub summary: String,
    /// What it means for the operator.
    pub meaning: String,
    /// What to do, most likely first.
    pub remedies: Vec<Remedy>,
    /// The underlying technical detail, available but never leading.
    pub detail: Option<String>,
    /// The problem that produced this one, where several share a root.
    pub cause: Option<Box<Problem>>,
}

impl Problem {
    /// A problem with a known remedy.
    ///
    /// A remedy is required rather than optional because an error without one is
    /// a dead end, and "I'll add the remedy later" is how a model like this
    /// erodes one message at a time.
    #[must_use]
    pub fn new(
        code: Code,
        severity: Severity,
        summary: impl Into<String>,
        meaning: impl Into<String>,
        remedy: Remedy,
    ) -> Self {
        Self {
            code,
            severity,
            state: State::Actionable,
            summary: summary.into(),
            meaning: meaning.into(),
            remedies: vec![remedy],
            detail: None,
            cause: None,
        }
    }

    /// A problem lemonfiber does not know how to fix.
    ///
    /// Constructing this is the honest path when no remedy is known, and it
    /// still carries somewhere to go, so the four parts hold.
    #[must_use]
    pub fn unknown(
        code: Code,
        severity: Severity,
        summary: impl Into<String>,
        meaning: impl Into<String>,
    ) -> Self {
        Self {
            state: State::Unknown,
            ..Self::new(code, severity, summary, meaning, escalation())
        }
    }

    /// Offer a further remedy, less likely than those already listed.
    #[must_use]
    pub fn or_try(mut self, remedy: Remedy) -> Self {
        self.remedies.push(remedy);
        self
    }

    /// Record where this problem stands, when it is not simply actionable.
    #[must_use]
    pub const fn in_state(mut self, state: State) -> Self {
        self.state = state;
        self
    }

    /// Attach the underlying technical detail, verbatim.
    #[must_use]
    /// Attach the underlying technical detail, with any credential in it withheld.
    ///
    /// Detail is where a service's own words are quoted verbatim, and a service that
    /// echoes its configuration back in an error message is an ordinary thing rather
    /// than an exotic one. So the withholding happens here rather than at each of the
    /// dozens of call sites: a rule you have to remember at every one of them is a
    /// rule that holds until somebody is in a hurry.
    pub fn with_detail(mut self, detail: impl Into<String>) -> Self {
        self.detail = Some(crate::config::store::withheld_text(&detail.into()));
        self
    }

    /// Attribute this problem to a root cause.
    #[must_use]
    pub fn caused_by(mut self, cause: Self) -> Self {
        self.cause = Some(Box::new(cause));
        self
    }
}

/// Turns a typed error into the problem an operator sees.
///
/// Every error type in this crate implements it, which is what keeps the error
/// model from being something each subsystem reinvents.
pub trait Diagnose {
    /// The operator-facing form of this error.
    fn problem(&self) -> Problem;
}

/// The same problem, however many times it happened.
///
/// Twenty services failing to start for one reason is one thing wrong, and a screen
/// listing it twenty times reads as twenty — which is both harder to act on and
/// frightening in a way the situation does not warrant. So identical problems are
/// folded, and the count is kept rather than discarded: "this happened twelve times"
/// is information, and losing it would be the opposite mistake.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Repeated {
    /// The problem itself, as it was first reported.
    pub problem: Problem,
    /// How many times it happened. Always at least one.
    pub times: usize,
}

impl Repeated {
    /// Whether this happened more than once.
    #[must_use]
    pub const fn is_repeated(&self) -> bool {
        self.times > 1
    }

    /// How to say the count, or nothing where there is nothing to say — one
    /// occurrence is just the problem, and "1 time" reads as a bug in the product.
    #[must_use]
    pub fn said(&self) -> Option<String> {
        self.is_repeated()
            .then(|| format!("this happened {} times", self.times))
    }
}

/// Fold identical problems into one apiece, keeping the count and the order they
/// were first seen.
///
/// Identity is the code and the summary together: the same code with a different
/// summary is the same *kind* of problem about two different things — two root
/// folders, two services — and folding those would report one and hide the other.
#[must_use]
pub fn folded(problems: Vec<Problem>) -> Vec<Repeated> {
    let mut order: Vec<(Code, String)> = Vec::new();
    let mut seen: std::collections::HashMap<(Code, String), Repeated> =
        std::collections::HashMap::new();
    for problem in problems {
        let key = (problem.code, problem.summary.clone());
        if let Some(already) = seen.get_mut(&key) {
            already.times += 1;
        } else {
            order.push(key.clone());
            seen.insert(key, Repeated { problem, times: 1 });
        }
    }
    order
        .into_iter()
        .filter_map(|key| seen.remove(&key))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::{folded, Code, Diagnose, Problem, Remedy, Repeated, Severity, State};

    const TEST: Code = Code::new("TEST-1");

    fn a_problem() -> Problem {
        Problem::new(
            TEST,
            Severity::Error,
            "Something broke",
            "The thing you asked for did not happen",
            Remedy::new("Try it again"),
        )
    }

    #[test]
    fn a_code_shows_as_the_string_it_was_declared_with() {
        assert_eq!(TEST.as_str(), "TEST-1");
        assert_eq!(TEST.to_string(), "TEST-1");
    }

    #[test]
    fn every_problem_carries_a_remedy() {
        assert!(!a_problem().remedies.is_empty());
    }

    #[test]
    fn a_problem_with_no_known_remedy_still_offers_escalation() {
        let problem = Problem::unknown(TEST, Severity::Error, "Something broke", "Unclear");
        assert_eq!(problem.state, State::Unknown);
        assert_eq!(
            problem
                .remedies
                .first()
                .and_then(|remedy| remedy.detail.as_deref()),
            Some("lemonfiber support"),
            "escalation is itself a remedy, and names a command that exists"
        );
    }

    #[test]
    fn remedies_stay_in_the_order_they_were_offered() {
        let problem = a_problem().or_try(Remedy::new("Or do the other thing"));
        let actions: Vec<&str> = problem.remedies.iter().map(|r| r.action.as_str()).collect();
        assert_eq!(actions, vec!["Try it again", "Or do the other thing"]);
    }

    #[test]
    fn detail_and_state_are_recorded_without_displacing_the_plain_words() {
        let problem = a_problem()
            .with_detail("EXDEV: cross-device link")
            .in_state(State::Guided);
        assert_eq!(problem.state, State::Guided);
        assert_eq!(problem.detail.as_deref(), Some("EXDEV: cross-device link"));
        assert_eq!(problem.summary, "Something broke");
    }

    #[test]
    fn a_symptom_can_name_the_cause_it_came_from() {
        let cause = Problem::new(
            Code::new("TEST-2"),
            Severity::Critical,
            "The disk is full",
            "Nothing can be written",
            Remedy::new("Free some space"),
        );
        let problem = a_problem().caused_by(cause);
        assert_eq!(
            problem.cause.as_ref().map(|root| root.code.as_str()),
            Some("TEST-2")
        );
    }

    #[test]
    fn severity_orders_from_advisory_up_to_critical() {
        assert!(Severity::Critical > Severity::Error);
        assert!(Severity::Error > Severity::Warning);
        assert!(Severity::Warning > Severity::Advisory);
    }

    #[test]
    fn a_typed_error_becomes_a_problem() {
        struct Broken;
        impl Diagnose for Broken {
            fn problem(&self) -> Problem {
                a_problem()
            }
        }
        assert_eq!(Broken.problem().code, TEST);
    }

    /// The fixture problem, carrying whatever detail is given.
    fn detailed(detail: &str) -> Problem {
        a_problem().with_detail(detail)
    }

    /// The fixture problem, said differently.
    fn about(summary: &str) -> Problem {
        Problem::new(
            TEST,
            Severity::Error,
            summary,
            "The thing you asked for did not happen",
            Remedy::new("Try it again"),
        )
    }

    #[test]
    fn a_credential_in_a_service_message_never_reaches_the_error() {
        // Detail is where a service's own words are quoted verbatim, and a service
        // echoing its configuration back in an error message is ordinary. A key that
        // reaches a terminal is a key that has to be rotated.
        let problem = detailed("could not apply:\n  SONARR_API_KEY: abc123\n  retries: 3");
        let detail = problem.detail.unwrap_or_default();
        assert!(!detail.contains("abc123"), "{detail}");
        assert!(
            detail.contains("SONARR_API_KEY"),
            "the setting is still named"
        );
        assert!(
            detail.contains("retries: 3"),
            "and the rest survives: {detail}"
        );
    }

    #[test]
    fn detail_that_carries_no_credential_is_untouched() {
        // Withholding is for credentials; detail that redacted everything would be
        // detail that says nothing.
        let said = "connection refused (os error 61)";
        assert_eq!(detailed(said).detail.as_deref(), Some(said));
    }

    #[test]
    fn one_problem_twenty_times_is_one_thing_wrong() {
        // A screen listing it twenty times reads as twenty, which is harder to act on
        // and frightening in a way the situation does not warrant.
        let many = vec![a_problem(); 20];
        let folded = folded(many);
        assert_eq!(folded.len(), 1);
        assert_eq!(folded.first().map(|r| r.times), Some(20));
        assert_eq!(
            folded.first().and_then(Repeated::said).as_deref(),
            Some("this happened 20 times")
        );
    }

    #[test]
    fn one_occurrence_says_nothing_about_a_count() {
        // "1 time" reads as a bug in the product.
        let folded = folded(vec![a_problem()]);
        let only = folded.first();
        assert_eq!(only.map(|r| r.times), Some(1));
        assert!(only.is_some_and(|r| !r.is_repeated()));
        assert_eq!(only.and_then(Repeated::said), None);
    }

    #[test]
    fn the_same_code_about_two_different_things_stays_two_things() {
        // Two root folders failing for one reason are two problems, and folding them
        // would report one and hide the other.
        let folded = folded(vec![a_problem(), about("A different thing broke")]);
        assert_eq!(folded.len(), 2);
        assert!(folded.iter().all(|r| r.times == 1));
    }

    #[test]
    fn folding_keeps_the_order_they_were_first_seen() {
        // The first thing that went wrong is usually the one that caused the rest, so
        // reordering by count would bury the cause under its symptoms.
        let folded = folded(vec![a_problem(), about("Later"), about("Later")]);
        assert_eq!(
            folded
                .iter()
                .map(|r| (r.problem.summary.as_str(), r.times))
                .collect::<Vec<_>>(),
            vec![("Something broke", 1), ("Later", 2)]
        );
    }

    #[test]
    fn folding_nothing_is_nothing() {
        assert!(folded(Vec::new()).is_empty());
    }
}
