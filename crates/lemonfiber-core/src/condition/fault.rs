//! What a check found wrong, at the moment it ran.
//!
//! The counterpart to [`super::Condition`], which is what gets remembered of it. A
//! fault is what the check says now; the condition is the history that accumulates
//! around it.
//!
//! A meaning and a remedy are both required to construct one, for the reason
//! [`crate::error::Problem`] requires them: a fault an operator can do nothing
//! about is a dead end, one whose consequence they have to work out for themselves
//! is a notification, and "I'll add it later" is how a model like this erodes one
//! message at a time. Everything that raises a condition therefore has to have
//! thought about what it costs the operator and what they should do, at the point
//! of raising it.
//!
//! Every word of one is redacted on the way in, on the support bundle's own
//! rules. A fault's summary is frequently a service's own message repeated back,
//! and a service that fails while authenticating will say so with the credential
//! in hand. Redacting here rather than at each surface is the point: a condition
//! is written to disk, read back next run, folded into a digest and pushed to a
//! phone, and a rule applied at only some of those places is a rule that holds
//! until somebody adds the next surface.

use serde::{Deserialize, Serialize};

use crate::error::Severity;

/// One line of a fault, as it is safe to keep and to send.
fn withheld(text: &str) -> String {
    crate::config::store::withheld_text(text)
}

/// Something a check found wrong.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Fault {
    /// What kind of thing this is — `service.stopped`, `vpn.egress.leaking`.
    ///
    /// Distinct from the check that raised it, which names the *instance*: four
    /// services stopping raise four checks and one kind. That is what lets four
    /// alerts be one, and what an operator turns off when they turn off an event
    /// rather than a machine.
    pub kind: String,
    /// How bad it is.
    pub severity: Severity,
    /// What is wrong, in one line.
    pub summary: String,
    /// What it costs the operator, in their terms rather than the machine's.
    ///
    /// Distinct from the summary, which is the event: "the tunnel dropped" is what
    /// happened and "nothing is downloading, and nothing leaked" is what that is
    /// worth knowing for. A fault carrying only the first is a notification.
    pub meaning: String,
    /// What to do about it, most likely first. Never empty.
    pub remedies: Vec<String>,
    /// The check whose fault this one is downstream of, where it is known to be.
    ///
    /// A disk that filled and the nine imports that then failed are one problem;
    /// naming the root is what lets a summary say so instead of counting ten.
    pub caused_by: Option<String>,
}

impl Fault {
    /// A fault: what happened, what it means, and the one thing an operator
    /// should do about it.
    #[must_use]
    pub fn new(kind: &str, severity: Severity, summary: &str, meaning: &str, remedy: &str) -> Self {
        Self {
            kind: kind.to_owned(),
            severity,
            summary: withheld(summary),
            meaning: withheld(meaning),
            remedies: vec![withheld(remedy)],
            caused_by: None,
        }
    }

    /// A further thing to try, after the ones already offered.
    #[must_use]
    pub fn or_else(mut self, remedy: &str) -> Self {
        self.remedies.push(withheld(remedy));
        self
    }

    /// Name the check this fault is downstream of.
    #[must_use]
    pub fn caused_by(mut self, check: &str) -> Self {
        self.caused_by = Some(check.to_owned());
        self
    }
}

#[cfg(test)]
mod tests;
