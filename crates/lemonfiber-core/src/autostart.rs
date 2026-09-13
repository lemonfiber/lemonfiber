//! Whether the operator asked for the stack to come back after a restart.
//!
//! The last question setup puts, and the one whose answer reaches furthest. A
//! machine reboots for an operating-system update overnight, the containers do
//! not come back, and nothing anywhere says so — the household finds out days
//! later that nothing has been downloaded since Tuesday, and the operator
//! concludes the software is unreliable. That conclusion is fair on the evidence
//! available to them, which is why the question is asked with its cost attached
//! rather than left as a line in a file somebody would have to know exists.
//!
//! **What is kept here is the answer, and deliberately nothing more.** Whether
//! autostart is actually *in force* is a different question with a different
//! owner: on macOS and Windows the load-bearing half of it is Docker Desktop's
//! own open-at-login setting, which lemonfiber can read and cannot set, and the
//! Linux half is a service the distribution already enabled. A record that
//! conflated the two would be the `enabled-unverified` trap committed to disk —
//! an operator believing they have autostart because they asked for it — and the
//! reason that state is modelled at all is that the belief is the expensive one.
//! So this answers "what was asked for", under a name that cannot be misread as
//! "and it works".

use serde::{Deserialize, Serialize};

/// What declining costs, in the terms the choice is actually about.
///
/// Stated the same way wherever the question is put, for the reason
/// [`crate::storage::COPY_CONSEQUENCE`] is: "start on boot" is a property, and a
/// property is not a thing anybody can weigh. This is what the property is *for*,
/// and it is three things rather than one — the stack stops, nothing brings it
/// back, and nothing tells you either. An operator told only the first two
/// concludes they will notice, and noticing is exactly what does not happen.
///
/// It names the way back as well as the loss, because declining is a reasonable
/// answer for somebody who starts the stack by hand and means to. The sentence
/// has to leave that operator with a decision rather than a warning.
pub const DECLINE_CONSEQUENCE: &str =
    "After a restart — an overnight operating-system update, a power cut — nothing comes back on \
     its own. Downloads stop and the library goes offline, no error is shown and no notification \
     is sent, and it stays that way until you run `lemonfiber up` yourself.";

/// The operator's answer to the autostart question, kept between runs.
///
/// A record of its own rather than a setting in the environment file, for the
/// reason the notification appetite is one: the environment file is handed to
/// Compose as it stands, and this is not a thing Compose has any use for. It sits
/// with the configuration a backup captures, so restoring onto a new machine
/// carries the answer rather than putting the question again.
///
/// A struct around one field rather than a bare `bool`, because this is the start
/// of the record and not the whole of it. Which form comes back — the last one
/// run, unless a specific one is pinned — and what the platform prerequisite was
/// last observed to be are answers belonging to this same file, and a bare `bool`
/// on disk is a shape that cannot grow to hold them without every reader changing
/// with it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct Wanted {
    /// Whether the operator asked for the stack to come back after a restart.
    ///
    /// Absent from a record written before this was kept, and absent from one
    /// that will not parse, both of which read as "not asked for". That is the
    /// safe direction: falling back to *not* starting something on somebody's
    /// machine leaves a stack they start themselves, and falling back the other
    /// way leaves a machine that begins saturating a metered line at boot because
    /// a file was truncated.
    #[serde(default)]
    on_boot: bool,
}

impl Wanted {
    /// The answer as the operator gave it.
    #[must_use]
    pub const fn answered(on_boot: bool) -> Self {
        Self { on_boot }
    }

    /// Whether the operator asked for the stack to come back after a restart.
    ///
    /// Not whether it will. A caller that wants to tell somebody autostart is
    /// working has to establish that separately — this says only that it was asked
    /// for, which is precisely the distinction `enabled-unverified` exists to keep.
    #[must_use]
    pub const fn on_boot(self) -> bool {
        self.on_boot
    }
}

#[cfg(test)]
mod tests {
    use super::{Wanted, DECLINE_CONSEQUENCE};

    #[test]
    fn declining_is_stated_as_what_it_costs_rather_than_what_it_is() {
        // "The stack will not start on boot" is a restatement of the question. An
        // operator weighing the answer is weighing four concrete things: when it
        // happens, what stops, that nothing tells them, and what it takes to undo.
        let said = DECLINE_CONSEQUENCE.to_lowercase();
        assert!(said.contains("restart"), "when it happens: {said}");
        assert!(said.contains("nothing comes back"), "what stops: {said}");
        assert!(
            said.contains("no notification"),
            "and nobody is told: {said}"
        );
        assert!(said.contains("lemonfiber up"), "the way back: {said}");
        // And it never leads with the property itself, which is the sentence that
        // reads as an explanation and explains nothing.
        assert!(!said.contains("autostart"), "a property: {said}");
        assert!(!said.contains("on boot"), "a property: {said}");
    }

    #[test]
    fn the_answer_is_what_comes_back_out_of_the_file_it_is_kept_in() {
        // The whole of what this record is for. Gathering the answer and then
        // losing it is worse than never asking, because the operator believes they
        // have chosen something — which is the belief the feature exists to stop.
        for asked in [true, false] {
            let kept = serde_json::to_string(&Wanted::answered(asked)).unwrap_or_default();
            assert_eq!(
                serde_json::from_str::<Wanted>(&kept).ok(),
                Some(Wanted::answered(asked)),
                "{kept}"
            );
            assert_eq!(Wanted::answered(asked).on_boot(), asked);
        }
    }

    #[test]
    fn a_record_that_is_missing_the_answer_reads_as_not_asked_for() {
        // A record written before this field existed, and a record a later build
        // grew a second field onto, both arrive here. Neither may read as "they
        // asked for it" — starting a media stack nobody asked to start is the one
        // direction this must not fall in.
        assert_eq!(
            serde_json::from_str::<Wanted>("{}").ok(),
            Some(Wanted::default())
        );
        assert!(!Wanted::default().on_boot());
    }
}
