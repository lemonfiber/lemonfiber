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
/// the same answer a year later. Every code is declared in the error crate's `codes`
/// module, and a code is never recycled.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, schemars::JsonSchema)]
pub struct Code(&'static str);

impl Code {
    /// A code as the registry declares it.
    pub(crate) const fn declared(id: &'static str) -> Self {
        Self(id)
    }

    /// A code no problem lemonfiber raises carries, for a test that needs one.
    ///
    /// Present only in a build with the `testing` feature, which the workspace's
    /// crates take as a development dependency, so a code declared anywhere but
    /// [`crate::codes`] does not build.
    #[cfg(any(test, feature = "testing"))]
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
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, schemars::JsonSchema,
)]
#[serde(rename_all = "kebab-case")]
#[schemars(rename = "ProblemSeverity")]
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

/// Where a problem lies: in what a request named, in how it asked, or in the
/// answering of it.
///
/// Nothing else here carries this. Severity is how much a problem matters and
/// state is whether there is a remedy, and a word this product does not explain
/// and a container engine that is not running can agree on both — so a surface
/// holding only those two cannot tell a caller which of them it met. This is
/// what tells them apart, and a surface that answers requests needs it: one of
/// them is worth asking again, and the other never will be.
///
/// Not blame. Asking about a word with no entry is a reasonable thing to have
/// done and the wording says so; where the answer would have to come from is a
/// separate question from whose mistake it was.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub enum Amiss {
    /// The answering. Nothing about the request was wrong.
    #[default]
    Answering,
    /// What the request named, which is not one of the things there are.
    Naming,
    /// How the request asked, which cannot be answered as it stands.
    Asking,
}

/// Where a problem stands with respect to being fixed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, schemars::JsonSchema)]
#[serde(rename_all = "kebab-case")]
#[schemars(rename = "ProblemState")]
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
#[derive(Debug, Clone, PartialEq, Eq, Serialize, schemars::JsonSchema)]
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
        self.detail = Some(crate::withheld::withheld_text(&detail.into()));
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
#[derive(Debug, Clone, PartialEq, Eq, Serialize, schemars::JsonSchema)]
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
    /// Where the problem lies.
    ///
    /// Not carried in the document. What a surface does with this is say it in
    /// its own terms — a status, an exit code — and writing it into the body as
    /// well would be the same fact stated twice, which is two things to keep
    /// agreeing.
    #[serde(skip)]
    #[schemars(skip)]
    pub amiss: Amiss,
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
            amiss: Amiss::Answering,
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

    /// Record where the problem lies, when it is not in the answering.
    ///
    /// Said at the point the problem is raised, because that is the only place
    /// that knows. A surface reading the code afterwards would be keeping a
    /// second list of which codes mean what, and a list kept away from the thing
    /// it describes is a list that goes stale without anybody noticing.
    #[must_use]
    pub const fn lies_in(mut self, amiss: Amiss) -> Self {
        self.amiss = amiss;
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
        self.detail = Some(crate::withheld::withheld_text(&detail.into()));
        self
    }

    /// Attribute this problem to a root cause.
    #[must_use]
    pub fn caused_by(mut self, cause: Self) -> Self {
        self.cause = Some(Box::new(cause));
        self
    }
}

/// A problem's schema, written out rather than referred to.
///
/// A tagged variant that carries a problem writes the problem's fields beside
/// its tag, so the schema for that variant has to describe them in the same
/// object. Referring to `Problem` there leaves `$ref` as a sibling of
/// `properties`, and readers disagree about that shape: draft-07 discards the
/// siblings, 2020-12 composes them, and the generators reading this contract
/// split the same way — one keeps the tag and loses the diagnosis, the other
/// loses the tag. Written out, there is only one object to read.
pub fn problem_schema(generator: &mut schemars::SchemaGenerator) -> schemars::Schema {
    <Problem as schemars::JsonSchema>::json_schema(generator)
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
    pub(crate) const fn is_repeated(&self) -> bool {
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
mod tests;
