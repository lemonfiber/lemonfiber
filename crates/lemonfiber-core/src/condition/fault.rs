//! What a check found wrong, at the moment it ran.
//!
//! The counterpart to [`super::Condition`], which is what gets remembered of it. A
//! fault is what the check says now; the condition is the history that accumulates
//! around it.
//!
//! A remedy is required to construct one, for the reason [`crate::error::Problem`]
//! requires it: a fault an operator can do nothing about is a dead end, and
//! "I'll add the remedy later" is how a model like this erodes one message at a
//! time. Everything that raises a condition therefore has to have thought about
//! what the operator should do, at the point of raising it.

use serde::{Deserialize, Serialize};

use crate::error::Severity;

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
    /// What to do about it, most likely first. Never empty.
    pub remedies: Vec<String>,
    /// The check whose fault this one is downstream of, where it is known to be.
    ///
    /// A disk that filled and the nine imports that then failed are one problem;
    /// naming the root is what lets a summary say so instead of counting ten.
    pub caused_by: Option<String>,
}

impl Fault {
    /// A fault, with the one thing an operator should do about it.
    #[must_use]
    pub fn new(kind: &str, severity: Severity, summary: &str, remedy: &str) -> Self {
        Self {
            kind: kind.to_owned(),
            severity,
            summary: summary.to_owned(),
            remedies: vec![remedy.to_owned()],
            caused_by: None,
        }
    }

    /// A further thing to try, after the ones already offered.
    #[must_use]
    pub fn or_else(mut self, remedy: &str) -> Self {
        self.remedies.push(remedy.to_owned());
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
mod tests {
    use super::Fault;
    use crate::error::Severity;

    #[test]
    fn a_fault_cannot_be_built_without_something_to_do_about_it() {
        // Not enforced by a check at runtime but by the constructor: there is no way
        // to reach a fault with an empty remedy list.
        let fault = Fault::new(
            "storage.full",
            Severity::Error,
            "the disk is full",
            "delete something",
        );
        assert_eq!(fault.remedies, vec!["delete something".to_owned()]);
        assert_eq!(fault.caused_by, None);
        assert_eq!(fault.kind, "storage.full");
    }

    #[test]
    fn further_remedies_keep_the_order_they_were_offered() {
        // Most likely first, as everywhere else remedies are listed.
        let fault = Fault::new(
            "storage.full",
            Severity::Error,
            "the disk is full",
            "delete something",
        )
        .or_else("move the library to a larger volume");
        assert_eq!(
            fault.remedies,
            vec![
                "delete something".to_owned(),
                "move the library to a larger volume".to_owned()
            ]
        );
    }

    #[test]
    fn a_fault_can_name_the_one_it_is_downstream_of() {
        let fault = Fault::new(
            "import.failed",
            Severity::Error,
            "the import failed",
            "retry the import",
        )
        .caused_by("storage.space");
        assert_eq!(fault.caused_by.as_deref(), Some("storage.space"));
    }
}
