//! What a proposed change would do, worked out before anything is written.
//!
//! The catalogue beside this says what revising an answer costs. This says what one
//! particular revision *is*: the setting it names, what that setting holds now, what it
//! would hold, and whether the proposal has cleared everything standing between it and
//! the file.
//!
//! A proposal is a thing in its own right rather than a step inside a write, because
//! three of the four ways it can end write nothing at all. A change nobody has said yes
//! to is staged; a replacement credential the service refused is turned away; a setting
//! that already holds what was asked for has nothing to do. Only the fourth reaches the
//! file, and the report says which of the four happened rather than leaving the operator
//! to infer it from a value they would have to read back.
//!
//! Values are withheld here on the way in, by the rule the settings listing already
//! withholds by, so a diff of a password is a diff of two redactions. The comparison
//! that decides whether anything differs is made on what was given, before the
//! withholding — two different secrets are two different values, and a diff that read
//! them after redaction would call every password change a no-op.

use serde::Serialize;

use super::{consequential, cost, Cost, Findings};
use crate::config::store::showing;
use crate::validate::Validation;

/// Where a proposed change stands once it has been read against what is in force.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, schemars::JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum Stance {
    /// The setting already holds what was asked for. Nothing to apply, nothing to
    /// confirm, and the file is not touched — a write of the same value would move
    /// the file's own timestamp and read afterwards as an edit somebody made.
    Unchanged,
    /// Staged and not applied: nothing has been written. Either it is consequential
    /// and nobody has said yes to it yet, or it was rehearsed.
    Pending,
    /// Turned away. It cannot be applied safely, the file is exactly as it was, and
    /// the refusal says why.
    Blocked,
    /// Written.
    Applied,
}

/// The difference between the configuration in force and the one proposed.
///
/// One setting, because one call changes one setting. What makes it a diff rather than
/// a value is `from`: an operator deciding whether to go ahead is deciding between two
/// things, and a report that showed only the new one would be asking them to remember
/// the old one correctly.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, schemars::JsonSchema)]
pub struct Change {
    /// The setting the change names.
    pub key: String,
    /// What it holds now, withheld where it is a credential, and absent where the
    /// setting holds nothing yet.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub from: Option<String>,
    /// What it would hold, withheld the same way.
    pub to: String,
    /// Whether applying it is cheap or consequential.
    pub cost: Cost,
}

/// A proposed change, read against what is in force, and where it stands.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, schemars::JsonSchema)]
pub struct Review {
    /// The difference, as it would be applied.
    pub change: Change,
    /// Where the proposal stands.
    pub stance: Stance,
    /// Why nothing was written, where nothing was and the reason is not simply that
    /// somebody has yet to say yes.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub refusal: Option<String>,
    /// What proving the replacement credential came to, where one was proven.
    ///
    /// A replacement is proven against its live service before the credential it
    /// replaces is discarded, so this is present for exactly those settings and
    /// absent everywhere else.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub proof: Option<Validation>,
    /// What the change comes to on this machine, beyond the value it changes.
    ///
    /// Filled by whoever went and asked — the services where they file, the clients
    /// what they are fetching — and empty on every change that comes to nothing
    /// beyond its value. Carried on a staged proposal as well as an applied one: a
    /// review that withheld this until after the yes would be asking for a yes to
    /// something unstated.
    #[serde(default)]
    pub findings: Findings,
}

impl Review {
    /// The proposal as it stands before anything has been asked of a service.
    ///
    /// `held` is what the setting holds now and `wanted` what it would hold, both as
    /// they were given rather than as they will be shown; `consent` is what the
    /// operator has said about it.
    #[must_use]
    pub fn proposed(key: &str, held: Option<&str>, wanted: &str, consent: &Consent) -> Self {
        let stance = if held == Some(wanted) {
            Stance::Unchanged
        } else if consent.rehearsing || (consequential(key) && !consent.settled) {
            Stance::Pending
        } else {
            Stance::Applied
        };
        Self {
            change: Change {
                key: key.to_owned(),
                from: held.map(|value| shown(key, value)),
                to: shown(key, wanted),
                cost: cost(key),
            },
            stance,
            refusal: None,
            proof: None,
            findings: Findings::default(),
        }
    }

    /// Whether this proposal reaches the file.
    #[must_use]
    pub const fn writes(&self) -> bool {
        matches!(self.stance, Stance::Applied)
    }

    /// Whether this proposal changes, or would change, what is in force.
    ///
    /// True of a staged change as well as an applied one: staged means it has not
    /// happened yet, not that it would do nothing. False of a refusal, which is a
    /// change that is not going to happen at all.
    #[must_use]
    pub const fn differs(&self) -> bool {
        matches!(self.stance, Stance::Pending | Stance::Applied)
    }

    /// The same proposal, carrying what proving the replacement credential came to.
    #[must_use]
    pub fn proven(mut self, proof: Validation) -> Self {
        self.proof = Some(proof);
        self
    }

    /// The same proposal, turned away with the file left as it was.
    ///
    /// The sentence carries the way past it where there is one — a replacement
    /// nothing could reach, or work still in flight, are the operator's own call and
    /// each says how to make it. A refusal nobody may override says what to do
    /// instead, because a refusal with no way past it is what people go and edit the
    /// file to get around.
    #[must_use]
    pub fn blocked(mut self, refusal: String) -> Self {
        self.stance = Stance::Blocked;
        self.refusal = Some(refusal);
        self
    }

    /// The same proposal, carrying what was found about it on this machine.
    #[must_use]
    pub fn finding(mut self, findings: Findings) -> Self {
        self.findings = findings;
        self
    }
}

/// What the operator has said about this particular change.
///
/// Two answers rather than one flag, because they are different things: whether this
/// run writes at all, and whether a change with a consequence has been agreed to. A
/// rehearsal of a consequential change is staged for the first reason and would be
/// staged for the second, and collapsing them would make a rehearsal that carried a
/// yes look like a change that had been applied.
pub struct Consent {
    /// Whether the operator has explicitly agreed to what this change costs.
    pub settled: bool,
    /// Whether this run only reports what it would do.
    pub rehearsing: bool,
}

/// A value as a report may show it.
///
/// The settings listing's own rule rather than a second one beside it: a difference
/// that redacted by a rule of its own would be a second list to keep in step with the
/// first, and the two would disagree on exactly the settings nobody thought about.
fn shown(key: &str, value: &str) -> String {
    showing(key, value).value
}

#[cfg(test)]
mod tests {
    use super::{Consent, Review, Stance};
    use crate::config::{
        store::REDACTED, DATA_ROOT_KEY, FRONT_DOOR_KEY, PROVIDER_HOST_KEY, PROVIDER_PASS_KEY,
        PROVIDER_USER_KEY,
    };
    use crate::reconfigure::Cost;

    /// A change nobody has agreed to and that is not a rehearsal.
    const UNSAID: Consent = Consent {
        settled: false,
        rehearsing: false,
    };

    /// The same, agreed to.
    const AGREED: Consent = Consent {
        settled: true,
        rehearsing: false,
    };

    #[test]
    fn a_consequential_change_nobody_agreed_to_is_staged_and_writes_nothing() {
        let review = Review::proposed(DATA_ROOT_KEY, Some("/srv/old"), "/srv/new", &UNSAID);
        assert_eq!(review.stance, Stance::Pending);
        assert!(!review.writes());
        assert!(review.differs());
        assert_eq!(review.change.cost, Cost::Consequential);
        assert_eq!(review.change.from.as_deref(), Some("/srv/old"));
        assert_eq!(review.change.to, "/srv/new");
    }

    #[test]
    fn the_same_change_agreed_to_reaches_the_file() {
        let review = Review::proposed(DATA_ROOT_KEY, Some("/srv/old"), "/srv/new", &AGREED);
        assert_eq!(review.stance, Stance::Applied);
        assert!(review.writes());
    }

    #[test]
    fn a_cheap_change_needs_nobody_to_agree_to_it() {
        let review = Review::proposed(FRONT_DOOR_KEY, None, "jellyfin", &UNSAID);
        assert_eq!(review.stance, Stance::Applied);
        assert_eq!(review.change.cost, Cost::Cheap);
        assert_eq!(review.change.from, None);
    }

    #[test]
    fn a_rehearsal_stages_even_a_cheap_change() {
        let rehearsing = Consent {
            settled: true,
            rehearsing: true,
        };
        let review = Review::proposed(FRONT_DOOR_KEY, None, "jellyfin", &rehearsing);
        assert_eq!(review.stance, Stance::Pending);
        assert!(!review.writes());
    }

    #[test]
    fn a_setting_that_already_holds_what_was_asked_for_has_nothing_to_do() {
        let review = Review::proposed(DATA_ROOT_KEY, Some("/srv/media"), "/srv/media", &UNSAID);
        assert_eq!(review.stance, Stance::Unchanged);
        assert!(!review.writes());
        assert!(!review.differs());
    }

    /// Two different passwords are two different values, and the comparison is made
    /// on what was given rather than on what is shown — a diff read after the
    /// withholding would call every password change a no-op.
    #[test]
    fn a_replaced_credential_is_withheld_on_both_sides_and_still_reads_as_a_change() {
        let review = Review::proposed(PROVIDER_PASS_KEY, Some("old-pass"), "new-pass", &UNSAID);
        assert_eq!(review.stance, Stance::Applied);
        assert_eq!(review.change.from.as_deref(), Some(REDACTED));
        assert_eq!(review.change.to, REDACTED);
    }

    /// A setting emptied is emptied rather than withheld: there is nothing there to
    /// keep back, and a blank shown as a redaction reads as a value still in force.
    #[test]
    fn a_credential_cleared_is_shown_as_empty_rather_than_as_something_withheld() {
        let review = Review::proposed(PROVIDER_PASS_KEY, Some("old-pass"), "", &UNSAID);
        assert_eq!(review.change.to, "");
    }

    /// A setting somebody vouched for is shown as it is, and one nobody has is
    /// withheld whether or not its name sounds like a credential — a Usenet provider
    /// issues account numbers as usernames, and half a paid login is a credential.
    #[test]
    fn what_is_shown_is_what_the_settings_listing_shows() {
        let host = Review::proposed(PROVIDER_HOST_KEY, None, "news.example.net", &UNSAID);
        assert_eq!(host.change.to, "news.example.net");

        let user = Review::proposed(PROVIDER_USER_KEY, None, "someone", &UNSAID);
        assert_eq!(user.change.to, REDACTED);
    }

    #[test]
    fn a_proposal_turned_away_writes_nothing_and_carries_the_reason() {
        let review = Review::proposed(FRONT_DOOR_KEY, None, "jellyfin", &UNSAID)
            .blocked("the service refused it".to_owned());
        assert_eq!(review.stance, Stance::Blocked);
        assert!(!review.writes());
        assert!(!review.differs());
        assert_eq!(review.refusal.as_deref(), Some("the service refused it"));
    }

    #[test]
    fn a_proposal_carries_what_proving_the_replacement_came_to() {
        let proof = crate::validate::Validation::Valid {
            observed: "answered a search".to_owned(),
        };
        let review = Review::proposed(FRONT_DOOR_KEY, None, "jellyfin", &UNSAID).proven(proof);
        assert!(review
            .proof
            .is_some_and(|came| matches!(came, crate::validate::Validation::Valid { .. })));
    }
}
