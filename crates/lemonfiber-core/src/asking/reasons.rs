//! Why a request was turned down, kept here because the request service holds nowhere
//! to keep it.
//!
//! **The service's own endpoint carries the decision and nothing else.** `POST
//! /request/{id}/{status}` reads no body at all and `MediaRequest` has no column for a
//! reason — checked against `ghcr.io/seerr-team/seerr:v3.3.0` rather than recalled — so
//! what reaches whoever asked is a bare refusal. A reason said once to the operator and
//! then dropped is the silent decline this whole feature exists to prevent, arriving one
//! step later than the blank field that is refused outright.
//!
//! So it is written down here. **Said to be this program's own record and never the
//! service's**, because a reason presented as having been delivered is worse than one
//! presented as still needing passing on: the first ends the operator's job and the
//! second is the truth.
//!
//! **Kept only for requests that still exist.** The service's own list is read on the way
//! past every decision, so a reason whose request has gone is dropped rather than
//! accumulating — this record is a note beside somebody else's list, and a note about a
//! line that is no longer there is only a way to grow a file forever.

use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};

/// What was said when one request was turned down.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct Refused {
    /// Why, in the words it was turned down in.
    pub reason: String,
    /// When it was turned down, so somebody reading it later knows which answer this
    /// was rather than assuming it is the newest.
    ///
    /// Absent where the machine's clock could not be written as a date, which is a
    /// refusal worth keeping the words of and not worth losing them over.
    pub at: Option<String>,
    /// Whether the words have been carried to whoever asked, and where to.
    ///
    /// Absent until the one attempt has been made. **This is what makes a second one
    /// impossible rather than unlikely**: carrying happens only where this is absent,
    /// and it is written whether anything was reached or not — so a member with nowhere
    /// to send to is asked about once, and a household is never told the same thing
    /// twice by a run that happened to be started twice.
    #[serde(default)]
    pub told: Option<Passed>,
    /// Whether nobody ruled on it and the period the household agreed to closed it,
    /// rather than an operator turning it down.
    ///
    /// The words are then this program's own, which is why the two are told apart here
    /// and not left to be read off the sentence: an operator scanning for what happened
    /// while they were away is looking for exactly the ones nobody answered, and a
    /// member is owed the difference between having been refused and having run out.
    ///
    /// Absent from every record written before a household could arrange this, which is
    /// the right reading of them — they are all somebody's own refusals.
    #[serde(default)]
    pub expired: bool,
}

/// Where a refusal's words were carried, and when.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct Passed {
    /// The services they reached, by the names the member knows them by. Empty where
    /// there was nowhere this could send.
    pub to: Vec<String>,
    /// When the attempt was made, absent where the clock could not be written down.
    pub at: Option<String>,
}

/// Every reason this machine holds, by the request the service files it under.
///
/// Keyed by the service's own number rather than by anything of this program's: it is
/// the only name both sides know a request by, and a second naming would be a second
/// chance to attach a reason to the wrong thing.
#[derive(Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Reasons {
    /// The reasons themselves.
    #[serde(default)]
    given: BTreeMap<i64, Refused>,
}

impl Reasons {
    /// Why that request was turned down, where this machine turned it down.
    ///
    /// Nothing for a request refused elsewhere — somebody using the request service
    /// directly leaves no reason here, and inventing one would put words in their mouth.
    #[must_use]
    pub fn of(&self, request: i64) -> Option<&Refused> {
        self.given.get(&request)
    }

    /// Write down why an operator turned this one down.
    pub fn keep(&mut self, request: i64, reason: &str, at: Option<String>) {
        self.written(request, reason, at, false);
    }

    /// Write down that nobody ruled on it and the household's own period closed it.
    ///
    /// Apart from an operator's refusal in the record as well as in the sentence. The
    /// two are the same shape and opposite events: one is an answer somebody gave, and
    /// the other is an answer nobody gave — and a reading that could not tell them apart
    /// would report a household as having been refused eleven things nobody refused.
    ///
    /// **A request this already closed is left exactly as it is**, which is the other
    /// place the two part company. A second answer from an operator is a second decision
    /// and owes its own words; a second closure is the same sentence about the same
    /// silence, and a record rewritten here would owe it afresh to somebody who has
    /// already had it. So what stops a household hearing this twice is this line, rather
    /// than the request service having moved the request out of the clock's reach.
    pub fn closed(&mut self, request: i64, reason: &str, at: Option<String>) {
        if self.given.get(&request).is_some_and(|kept| kept.expired) {
            return;
        }
        self.written(request, reason, at, true);
    }

    /// Put one refusal in the record, whichever of the two it is.
    ///
    /// The reason is trimmed on the way in because it is trimmed on the way past the
    /// check that refuses a blank one, and a record holding the untrimmed spelling would
    /// disagree with the line the operator was shown.
    fn written(&mut self, request: i64, reason: &str, at: Option<String>, expired: bool) {
        self.given.insert(
            request,
            Refused {
                reason: reason.trim().to_owned(),
                at,
                told: None,
                expired,
            },
        );
    }

    /// Whether whoever asked for this has still to hear the words.
    ///
    /// Nothing kept is nothing owed: a request refused somewhere else leaves no reason
    /// here, and there is nothing to carry.
    #[must_use]
    pub fn owed(&self, request: i64) -> bool {
        self.given
            .get(&request)
            .is_some_and(|kept| kept.told.is_none())
    }

    /// Write down that the words were carried, and which services took them.
    ///
    /// Written even where nothing was reached, because what this records is that the
    /// one attempt happened. Nothing for a request no reason is held for — there was
    /// nothing to carry, so there is nothing to say was carried.
    pub(crate) fn passed_on(&mut self, request: i64, to: Vec<String>, at: Option<String>) {
        if let Some(kept) = self.given.get_mut(&request) {
            kept.told = Some(Passed { to, at });
        }
    }

    /// Forget every reason whose request the service no longer holds.
    ///
    /// Given the numbers that still exist rather than the ones to drop: what this record
    /// is owed is what is still there, and a caller working out the difference would be
    /// the second place that arithmetic lived.
    pub fn only(&mut self, held: &BTreeSet<i64>) {
        self.given.retain(|request, _| held.contains(request));
    }

    /// Whether nothing has been turned down from here.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.given.is_empty()
    }
}

#[cfg(test)]
mod tests;
