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
            "nothing can be written until something goes",
            "delete something",
        );
        assert_eq!(fault.remedies, vec!["delete something".to_owned()]);
        assert_eq!(fault.caused_by, None);
        assert_eq!(fault.kind, "storage.full");
    }

    #[test]
    fn a_fault_says_what_it_costs_as_well_as_what_happened_and_what_to_do() {
        // The middle part, and the one that is quietly dropped: an event and an
        // instruction leave whoever reads them to work out for themselves whether
        // this is worth getting up for.
        let fault = Fault::new(
            "vpn.egress.leaking",
            Severity::Critical,
            "the download client's traffic is not going through the tunnel",
            "every peer it talks to can see this connection's own address",
            "stop the download client",
        );
        assert_eq!(
            fault.meaning,
            "every peer it talks to can see this connection's own address"
        );
        assert!(!fault.summary.is_empty());
        assert!(!fault.remedies.is_empty());
    }

    #[test]
    fn what_a_fault_means_is_withheld_on_the_same_rules_as_the_rest_of_it() {
        // The consequence of a failed login is the obvious place to quote the
        // credential that failed, and a sentence nobody thought of as evidence is
        // the one that reaches a phone unredacted.
        let quoted = format!("nothing will authenticate while {}=hunter2 stands", "TOKEN");
        let fault = Fault::new(
            "service.refused",
            Severity::Error,
            "the login failed",
            &quoted,
            "check the key",
        );
        assert!(!fault.meaning.contains("hunter2"), "{}", fault.meaning);
        assert!(fault.meaning.contains("nothing will authenticate"));
    }

    #[test]
    fn further_remedies_keep_the_order_they_were_offered() {
        // Most likely first, as everywhere else remedies are listed.
        let fault = Fault::new(
            "storage.full",
            Severity::Error,
            "the disk is full",
            "nothing can be written until something goes",
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
            "it stays out of the library until it lands",
            "retry the import",
        )
        .caused_by("storage.space");
        assert_eq!(fault.caused_by.as_deref(), Some("storage.space"));
    }

    #[test]
    fn a_credential_a_service_repeated_back_never_reaches_a_fault() {
        // A service that fails while authenticating says so with the credential in
        // hand, and that message becomes a summary, a stored condition, a digest,
        // and a push to somebody's phone.
        let leaked = format!("sonarr refused: {}=abcdef123456", "api_key");
        let fault = Fault::new(
            "service.refused",
            Severity::Error,
            &leaked,
            "nothing that needs it is working",
            "check the key",
        );
        assert!(!fault.summary.contains("abcdef123456"), "{}", fault.summary);
        assert!(
            fault.summary.contains("sonarr refused"),
            "{}",
            fault.summary
        );
    }

    #[test]
    fn a_remedy_is_redacted_on_the_same_rules_as_the_summary() {
        // A remedy that quotes the offending line is the obvious way for one to get
        // out, and the least obvious place to look for it.
        let quoted = format!("set {}=hunter2 in the environment file", "PASSWORD");
        let fault = Fault::new(
            "config.wrong",
            Severity::Error,
            "the login failed",
            "the service will not answer until it is right",
            &quoted,
        )
        .or_else(&quoted);
        assert!(
            fault
                .remedies
                .iter()
                .all(|remedy| !remedy.contains("hunter2")),
            "{:?}",
            fault.remedies
        );
    }

    #[test]
    fn a_credential_written_after_a_colon_is_withheld_too() {
        // The other shape a service quotes one back in. Both are ordinary; catching
        // only the one with an equals sign would be a rule that holds until the
        // next service words its error differently.
        let leaked = format!("sonarr refused: {}: abcdef123456", "api_key");
        let fault = Fault::new(
            "service.refused",
            Severity::Error,
            &leaked,
            "nothing that needs it is working",
            "check the key",
        );
        assert!(!fault.summary.contains("abcdef123456"), "{}", fault.summary);
        assert!(fault.summary.contains("api_key"), "{}", fault.summary);
    }

    #[test]
    fn wording_that_carries_no_credential_is_left_exactly_as_written() {
        // Redaction that mangled ordinary sentences would be its own problem.
        let plain = "sonarr keeps restarting";
        let fault = Fault::new(
            "service.crash-looping",
            Severity::Error,
            plain,
            "nothing that needs it is working",
            "read its logs",
        );
        assert_eq!(fault.summary, plain);
        assert_eq!(fault.remedies, vec!["read its logs".to_owned()]);
    }
}
