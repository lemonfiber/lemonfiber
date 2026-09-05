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

    /// Write down why this one was turned down.
    ///
    /// The reason is trimmed on the way in because it is trimmed on the way past the
    /// check that refuses a blank one, and a record holding the untrimmed spelling would
    /// disagree with the line the operator was shown.
    pub fn keep(&mut self, request: i64, reason: &str, at: Option<String>) {
        self.given.insert(
            request,
            Refused {
                reason: reason.trim().to_owned(),
                at,
            },
        );
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
mod tests {
    use super::Reasons;
    use std::collections::BTreeSet;

    /// A moment the calendar holds, for these records to be stamped with.
    const AT: &str = "2026-08-17T21:04:09";

    /// A reason written down comes back under the request it was written for.
    #[test]
    fn a_reason_comes_back_under_the_request_it_was_written_for() {
        let mut held = Reasons::default();
        assert!(held.is_empty());

        held.keep(
            41,
            "we already have it in another form",
            Some(AT.to_owned()),
        );

        assert!(!held.is_empty());
        assert_eq!(
            held.of(41)
                .map(|kept| (kept.reason.as_str(), kept.at.as_deref())),
            Some(("we already have it in another form", Some(AT)))
        );
        assert_eq!(
            held.of(42),
            None,
            "a reason reached a request it is not for"
        );
    }

    /// The record holds the reason as the check that let it past read it.
    ///
    /// A blank reason is refused before this is reached, and one padded either side is
    /// trimmed there — so a record keeping the untrimmed spelling would disagree with
    /// the line the operator was shown it in.
    #[test]
    fn the_reason_is_kept_as_the_operator_was_shown_it() {
        let mut held = Reasons::default();
        held.keep(7, "  the disk is nearly full  ", Some(AT.to_owned()));

        assert_eq!(
            held.of(7).map(|kept| kept.reason.as_str()),
            Some("the disk is nearly full")
        );
    }

    /// Ruling on one request twice keeps the answer it was last given.
    #[test]
    fn a_second_answer_replaces_the_first() {
        let mut held = Reasons::default();
        held.keep(7, "not this month", None);
        held.keep(7, "on second thoughts, the disk", Some(AT.to_owned()));

        assert_eq!(
            held.of(7).map(|kept| kept.reason.as_str()),
            Some("on second thoughts, the disk")
        );
    }

    /// A reason whose request the service no longer holds is forgotten.
    ///
    /// This is a note beside somebody else's list. A note about a line that is no longer
    /// there is only a way to grow a file forever.
    #[test]
    fn a_reason_whose_request_has_gone_is_forgotten() {
        let mut held = Reasons::default();
        held.keep(1, "too new", Some(AT.to_owned()));
        held.keep(2, "we have it", Some(AT.to_owned()));
        held.keep(3, "no room", None);

        held.only(&BTreeSet::from([2, 3, 9]));

        assert_eq!(held.of(1), None);
        assert!(held.of(2).is_some());
        assert!(held.of(3).is_some());
    }

    /// It survives being written down and read back.
    #[test]
    fn it_survives_being_written_down_and_read_back() {
        let mut held = Reasons::default();
        held.keep(11, "the season is only half out", Some(AT.to_owned()));

        let written = serde_json::to_string(&held).unwrap_or_default();
        let read: Reasons = serde_json::from_str(&written).unwrap_or_default();

        assert_eq!(read, held, "{written}");
    }

    /// A record this cannot read is no reasons rather than a failure.
    #[test]
    fn a_record_that_will_not_read_is_no_reasons() {
        let read: Reasons = serde_json::from_str("not a record").unwrap_or_default();

        assert!(read.is_empty());
    }
}
