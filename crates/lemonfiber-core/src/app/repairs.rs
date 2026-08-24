//! Every repair lemonfiber has carried out, kept between runs.
//!
//! Apart from the change journal, deliberately. The journal is a log of *reversible
//! changes*: every entry carries what undoing exactly that change requires, which is what
//! makes an undo trustworthy. This is a log of *attempts* — including the ones that
//! changed nothing, which are the ones worth reading when a fault keeps coming back and
//! the same repair keeps failing to hold it down.
//!
//! Folding the two together would cost one of them. A journal that also held notes would
//! have to be filtered before an undo could read it, and an audit that only recorded the
//! successful changes would be missing precisely the entries somebody is looking for.
//!
//! An entry is appended as each repair finishes, whatever it came to, carrying the finding
//! it was for, what it did, the time and the outcome. Those four are what a person needs
//! when the same repair keeps failing to hold a fault down, so a repair that changed
//! nothing is written as readily as one that worked.
//!
//! Best-effort, both ways, for the reason the conditions are: a lost history is a worse
//! picture for the next run and never a reason to refuse this one.

use serde::{Deserialize, Serialize};

// The same bound the delivered-alert history uses, borrowed rather than restated: two
// records written between runs and pruned by nobody have the same problem and deserve the
// same answer, and a second number is a second thing to change when that answer moves.
use crate::alert::KEPT;
use crate::repair::Outcome;

use super::Ctx;

/// One repair, as it will be read back long afterwards.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(super) struct Entry {
    /// When it was carried out, as a service writes a moment.
    pub at: String,
    /// The check whose finding it answered.
    pub check: String,
    /// What it set out to do, in the words the operator was shown before agreeing.
    pub did: String,
    /// How it turned out, once the check was asked again.
    pub outcome: Outcome,
}

/// What lemonfiber has tried to put right.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub(super) struct Repairs {
    /// Every attempt, oldest first.
    #[serde(default)]
    pub entries: Vec<Entry>,
}

impl Repairs {
    /// Add one, dropping the oldest where the record is full.
    pub(super) fn record(&mut self, entry: Entry) {
        self.entries.push(entry);
        if self.entries.len() > KEPT {
            self.entries.drain(..self.entries.len() - KEPT);
        }
    }
}

/// The name this record is kept under, beside the environment file.
const NAME: &str = "repairs.json";

/// Read what previous runs recorded, or an empty record where there is none.
#[must_use]
pub(super) fn load(ctx: &Ctx) -> Repairs {
    super::record::beside(ctx, NAME)
}

/// Write the record where the next run will read it.
pub(super) fn save(ctx: &Ctx, repairs: &Repairs) {
    super::record::keep_beside(ctx, NAME, repairs);
}

#[cfg(test)]
mod tests {
    use super::{Entry, Repairs, KEPT};
    use crate::repair::Outcome;

    fn entry(check: &str, outcome: Outcome) -> Entry {
        Entry {
            at: "1000".to_owned(),
            check: check.to_owned(),
            did: "put it right".to_owned(),
            outcome,
        }
    }

    /// The attempts that changed nothing are the ones worth reading when a fault keeps
    /// coming back, so they are kept beside the ones that worked.
    #[test]
    fn what_was_tried_is_kept_whether_or_not_it_worked() {
        let mut repairs = Repairs::default();
        repairs.record(entry("vpn.port-forward-client", Outcome::FixFailed));
        repairs.record(entry("vpn.port-forward-client", Outcome::Fixed));
        repairs.record(entry("something.else", Outcome::Fixed));

        assert_eq!(repairs.entries.len(), 3);
        assert!(repairs
            .entries
            .iter()
            .any(|entry| entry.outcome == Outcome::FixFailed));
    }

    /// Bounded, because a record nobody prunes is a file somebody finds the hard way —
    /// and the oldest go first, since what a repair did last is what explains today.
    #[test]
    fn the_record_keeps_the_most_recent_and_drops_the_rest() {
        let mut repairs = Repairs::default();
        for _ in 0..=KEPT {
            repairs.record(entry("vpn.port-forward-client", Outcome::Fixed));
        }
        repairs.record(entry("the.newest", Outcome::Fixed));

        assert_eq!(repairs.entries.len(), KEPT);
        assert_eq!(
            repairs.entries.last().map(|entry| entry.check.as_str()),
            Some("the.newest")
        );
    }

    /// It is read back long after it was written, so what it holds has to survive the
    /// round trip — including which of the outcomes it was.
    #[test]
    fn a_record_reads_back_as_it_was_written() {
        let mut repairs = Repairs::default();
        repairs.record(entry(
            "vpn.port-forward-client",
            Outcome::Stopped {
                leaving: "half of it".to_owned(),
            },
        ));
        let written = serde_json::to_string(&repairs).unwrap_or_default();

        assert_eq!(
            serde_json::from_str::<Repairs>(&written).ok(),
            Some(repairs)
        );
    }
}
