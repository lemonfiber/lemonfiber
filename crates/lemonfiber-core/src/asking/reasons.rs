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
    pub fn passed_on(&mut self, request: i64, to: Vec<String>, at: Option<String>) {
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

    /// The words are owed until they have been carried, and then never again.
    ///
    /// This is the whole of what stops a household hearing the same thing twice: the
    /// attempt is written down whether it reached anybody or not, so a member with
    /// nowhere to send to is asked about once.
    #[test]
    fn words_are_owed_once_and_then_never_again() {
        let mut held = Reasons::default();
        held.keep(7, "no room this month", Some(AT.to_owned()));
        assert!(held.owed(7), "a fresh refusal owes nobody anything");

        held.passed_on(7, vec!["Pushover".to_owned()], Some(AT.to_owned()));

        assert!(!held.owed(7), "the same words are owed a second time");
        assert_eq!(
            held.of(7).and_then(|kept| kept.told.clone()),
            Some(super::Passed {
                to: vec!["Pushover".to_owned()],
                at: Some(AT.to_owned()),
            })
        );
    }

    /// Nowhere to send is still an attempt made, and still not owed again.
    #[test]
    fn nowhere_to_send_is_still_an_attempt_that_happened() {
        let mut held = Reasons::default();
        held.keep(7, "not this month", None);

        held.passed_on(7, Vec::new(), None);

        assert!(!held.owed(7));
        assert_eq!(
            held.of(7)
                .and_then(|kept| kept.told.clone())
                .map(|passed| passed.to),
            Some(Vec::new())
        );
    }

    /// A request nothing was kept for is owed nothing and records nothing.
    #[test]
    fn a_request_with_no_reason_kept_is_owed_nothing() {
        let mut held = Reasons::default();
        assert!(!held.owed(7), "a request nobody refused here owes words");

        held.passed_on(7, vec!["Pushover".to_owned()], None);

        assert_eq!(held.of(7), None, "a delivery invented a refusal");
    }

    /// A request nobody ruled on is kept apart from one somebody refused.
    ///
    /// The same shape and opposite events. A reading that could not tell them apart
    /// would report a household as having been refused things nobody refused, and would
    /// tell the member they were turned down when what happened is that they ran out.
    #[test]
    fn a_request_that_ran_out_is_kept_apart_from_one_somebody_refused() {
        let mut held = Reasons::default();
        held.keep(7, "we already have it dubbed", Some(AT.to_owned()));
        held.closed(9, "nobody ruled on it within 30 days", Some(AT.to_owned()));

        assert_eq!(held.of(7).map(|kept| kept.expired), Some(false));
        assert_eq!(held.of(9).map(|kept| kept.expired), Some(true));
        assert!(
            held.owed(9),
            "a request that ran out owes its words like any other"
        );
    }

    /// Closing the same request twice owes its words once.
    ///
    /// **The whole of what stops a clock telling a household the same thing every hour**,
    /// and it is here rather than in the clock: a second closure is the same sentence
    /// about the same silence, so the record it would rewrite is left alone and the words
    /// stay carried. An operator's own second answer is the opposite case, below.
    #[test]
    fn closing_the_same_request_twice_owes_its_words_once() {
        let mut held = Reasons::default();
        held.closed(7, "nobody ruled on it within 30 days", Some(AT.to_owned()));
        held.passed_on(7, vec!["Pushover".to_owned()], Some(AT.to_owned()));

        held.closed(7, "nobody ruled on it within 30 days", Some(AT.to_owned()));

        assert!(!held.owed(7), "the same words were owed a second time");
        assert_eq!(
            held.of(7).and_then(|kept| kept.told.clone()),
            Some(super::Passed {
                to: vec!["Pushover".to_owned()],
                at: Some(AT.to_owned()),
            })
        );
    }

    /// An operator turning down what ran out is a decision, and owes its own words.
    ///
    /// The other side of the line above: what is refused a second telling is the same
    /// silence said again, not somebody actually answering.
    #[test]
    fn somebody_answering_after_it_ran_out_owes_their_own_words() {
        let mut held = Reasons::default();
        held.closed(7, "nobody ruled on it within 30 days", None);
        held.passed_on(7, vec!["Pushover".to_owned()], None);

        held.keep(7, "and we already have it dubbed", None);

        assert!(held.owed(7));
        assert_eq!(held.of(7).map(|kept| kept.expired), Some(false));
    }

    /// A second answer to the same request owes its own words afresh.
    ///
    /// Nothing can reach this today — a request already decided is refused by name
    /// before a second answer is taken — and if that ever changed, the new words would
    /// be new words rather than ones already carried.
    #[test]
    fn a_second_answer_owes_its_own_words() {
        let mut held = Reasons::default();
        held.keep(7, "not this month", None);
        held.passed_on(7, vec!["Pushover".to_owned()], None);

        held.keep(7, "on second thoughts, the disk", None);

        assert!(held.owed(7));
    }
}
