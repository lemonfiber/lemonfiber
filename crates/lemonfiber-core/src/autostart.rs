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

/// Why nothing is to come back at this boot.
///
/// Two answers rather than a bare `false`, because an operator asking why their
/// stack is not running is owed which of the two it is: a machine that was never
/// asked to start anything, and a machine that was asked and then deliberately
/// stopped, look identical from the outside and take opposite things to put right.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Held {
    /// Autostart was never asked for.
    NotAsked,
    /// The stack was stopped on purpose, and a boot does not undo that.
    StoppedOnPurpose,
}

impl Held {
    /// The line an operator reads, in the terms the decision was actually made in.
    #[must_use]
    pub const fn said(self) -> &'static str {
        match self {
            Self::NotAsked => "the stack was not asked to start on its own",
            Self::StoppedOnPurpose => {
                "the stack was stopped on purpose before this machine \
                                       restarted, and a restart does not undo that"
            }
        }
    }
}

/// What is to come back after a restart, kept between runs.
///
/// Named for what it answers rather than for when it is read. `boot` is taken twice
/// over in this workspace — setup has a bootstrap of its own that means something
/// else entirely — and a second record under that word would send every reader to
/// the wrong file first.
///
/// The whole of `autostart.json`, of which the operator's answer is one field. Three
/// facts share the file because they are read together and mean nothing apart: what
/// was asked for, which form it applies to, and whether the last stop was one the
/// operator asked for. A boot that knew the answer and not the form would start the
/// wrong stack, and one that knew both and not the stop would resurrect something
/// somebody deliberately put down.
///
/// The answer is flattened rather than nested, so a record written before the rest of
/// this existed reads back unchanged — and so the field a backup carries keeps the
/// name it has always had.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct Returning {
    /// The operator's answer to the autostart question.
    #[serde(flatten)]
    wanted: Wanted,
    /// The forms the operator pinned, which stand ahead of whatever ran last.
    ///
    /// Empty is the ordinary case: most operators run one stack and never think
    /// about which one comes back. Pinning is for the operator who runs a large form
    /// by hand and wants a small one on an unattended restart.
    #[serde(default)]
    pinned: Vec<String>,
    /// The forms the last start named, which is what comes back where nothing is
    /// pinned.
    ///
    /// Empty means every form, the same way naming no form to `up` does — so a
    /// machine that has started the whole stack and a machine that has started
    /// nothing at all agree about what a restart should bring back, which is the
    /// whole stack in both cases.
    #[serde(default)]
    last_run: Vec<String>,
    /// Whether the last thing that happened to the stack was a stop the operator
    /// asked for.
    ///
    /// False after a start, which is what makes this a record of the *last* thing
    /// rather than of anything that ever happened. A stop lemonfiber decided on by
    /// itself — the data location disappearing out from under a running stack — never
    /// sets it, because the operator did not ask for that and would want it back.
    #[serde(default)]
    halted: bool,
}

impl Returning {
    /// What was recorded, or nothing recorded where the file cannot be read.
    ///
    /// An unreadable record reads as a machine that was never asked, for the reason
    /// a missing one does: falling back to not starting leaves a stack the operator
    /// starts themselves, and falling back the other way starts a media stack nobody
    /// asked to have started.
    #[must_use]
    pub fn at(path: &std::path::Path) -> Self {
        std::fs::read_to_string(path)
            .ok()
            .and_then(|text| serde_json::from_str(&text).ok())
            .unwrap_or_default()
    }

    /// The same record, carrying the operator's answer to the autostart question.
    ///
    /// The rest is kept, so running setup a second time does not forget which form
    /// was pinned or lose the fact that the stack was deliberately stopped.
    #[must_use]
    pub fn answering(mut self, on_boot: bool) -> Self {
        self.wanted = Wanted::answered(on_boot);
        self
    }

    /// The operator's answer as they gave it.
    #[must_use]
    pub const fn wanted(&self) -> Wanted {
        self.wanted
    }

    /// The forms pinned, empty where none are.
    #[must_use]
    pub fn pinned(&self) -> &[String] {
        &self.pinned
    }

    /// Whether the last thing asked of the stack was a stop.
    #[must_use]
    pub const fn halted(&self) -> bool {
        self.halted
    }

    /// Record that these forms were started.
    ///
    /// Which also un-stops the record: a start is the operator saying they want it
    /// running, and a deliberate stop three days ago has been answered by it.
    pub fn started(&mut self, forms: &[String]) {
        self.last_run = forms.to_vec();
        self.halted = false;
    }

    /// Record that the operator asked for the stack to stop.
    ///
    /// What was last run is left exactly as it was, because it is still the answer to
    /// which form comes back once they start it again.
    pub fn stopped(&mut self) {
        self.halted = true;
    }

    /// Pin these forms, or unpin by naming none.
    pub fn pin(&mut self, forms: &[String]) {
        self.pinned = forms.to_vec();
    }

    /// Which forms a boot would start, or why it would start none.
    ///
    /// The pinned forms stand ahead of the last-run ones, which is the whole of what
    /// pinning means — and a machine that has run nothing yet asks for every form,
    /// which is what naming none means everywhere else in this product.
    ///
    /// # Errors
    ///
    /// Returns [`Held`] where autostart was not asked for, or where the stack was
    /// stopped on purpose.
    pub fn at_boot(&self) -> Result<&[String], Held> {
        if !self.wanted.on_boot() {
            return Err(Held::NotAsked);
        }
        if self.halted {
            return Err(Held::StoppedOnPurpose);
        }
        Ok(if self.pinned.is_empty() {
            &self.last_run
        } else {
            &self.pinned
        })
    }
}

#[cfg(test)]
mod tests {
    use super::{Held, Returning, Wanted, DECLINE_CONSEQUENCE};

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
    fn what_comes_back_is_the_last_form_run_unless_one_is_pinned() {
        // The whole of the rule: an operator who ran `up films` last night gets
        // films back, and one who pinned `tv` gets tv whatever they ran by hand.
        let mut returning = Returning::default().answering(true);
        returning.started(&["films".to_owned()]);
        assert_eq!(
            returning.at_boot().ok(),
            Some(["films".to_owned()].as_slice())
        );

        returning.pin(&["tv".to_owned()]);
        assert_eq!(
            returning.at_boot().ok(),
            Some(["tv".to_owned()].as_slice()),
            "a pin stands ahead of whatever was run by hand"
        );

        returning.pin(&[]);
        assert_eq!(
            returning.at_boot().ok(),
            Some(["films".to_owned()].as_slice()),
            "and unpinning gives the last-run form back"
        );
    }

    #[test]
    fn a_machine_that_has_run_nothing_yet_asks_for_every_form() {
        // Naming no form means the whole stack everywhere else in this product, and
        // it has to mean the same here: an operator who answered yes during setup and
        // then rebooted before ever running `up` is asking for their stack, not for
        // nothing at all.
        let returning = Returning::default().answering(true);
        assert_eq!(returning.at_boot().ok(), Some([].as_slice()));
    }

    #[test]
    fn a_stack_stopped_on_purpose_is_not_resurrected() {
        // The edge case the requirement is written about. An operator who stopped the
        // stack on Friday and rebooted on Monday did not ask for it back.
        let mut returning = Returning::default().answering(true);
        returning.started(&["tv".to_owned()]);
        returning.stopped();

        assert_eq!(returning.at_boot(), Err(Held::StoppedOnPurpose));
        assert!(returning.halted());
        assert!(
            Held::StoppedOnPurpose.said().contains("on purpose"),
            "and the reason says which of the two it is"
        );
    }

    #[test]
    fn starting_it_again_answers_the_stop() {
        // A deliberate stop is a statement about the stack as it was then. Starting it
        // is the operator saying otherwise, and a record that held the stop for ever
        // would leave a machine that never came back after any reboot.
        let mut returning = Returning::default().answering(true);
        returning.stopped();
        returning.started(&["tv".to_owned()]);

        assert!(!returning.halted());
        assert_eq!(returning.at_boot().ok(), Some(["tv".to_owned()].as_slice()));
    }

    #[test]
    fn a_machine_that_was_never_asked_starts_nothing_however_it_was_left() {
        // And the two refusals are told apart, because putting them right takes
        // opposite things: answering the question, or starting the stack.
        let mut returning = Returning::default();
        returning.started(&["tv".to_owned()]);
        assert_eq!(returning.at_boot(), Err(Held::NotAsked));
        assert!(!Held::NotAsked.said().is_empty());
    }

    #[test]
    fn the_answer_setup_wrote_before_any_of_this_existed_still_reads() {
        // The file format is the one already on operators' machines. A record holding
        // only the answer has to keep meaning what it meant, or an upgrade would
        // quietly forget every answer given so far.
        let read: Option<Returning> = serde_json::from_str(r#"{"on_boot":true}"#).ok();
        assert_eq!(
            read.as_ref().map(Returning::wanted),
            Some(Wanted::answered(true))
        );
        assert_eq!(
            read.and_then(|read| read.at_boot().ok().map(<[String]>::len)),
            Some(0)
        );
    }

    #[test]
    fn the_answer_survives_setup_being_run_a_second_time() {
        // Setup writes the answer and nothing else, so a second run over a machine
        // that has been used must not take the pin and the stop with it.
        let mut returning = Returning::default().answering(true);
        returning.pin(&["tv".to_owned()]);
        returning.stopped();

        let again = returning.clone().answering(false);
        assert_eq!(again.pinned(), ["tv".to_owned()]);
        assert!(again.halted());
        assert_eq!(
            again.at_boot(),
            Err(Held::NotAsked),
            "and the new answer wins"
        );
    }

    #[test]
    fn a_record_that_cannot_be_read_is_a_machine_that_was_never_asked() {
        // The safe direction, the same one a missing answer falls in: a truncated
        // file must not be the reason a metered line starts saturating at boot.
        let nowhere = std::env::temp_dir().join(format!(
            "lemonfiber-returning-{}-absent.json",
            std::process::id()
        ));
        let _ = std::fs::remove_file(&nowhere);
        assert_eq!(Returning::at(&nowhere), Returning::default());

        assert!(std::fs::write(&nowhere, "not json at all").is_ok());
        assert_eq!(Returning::at(&nowhere).at_boot(), Err(Held::NotAsked));
        let _ = std::fs::remove_file(&nowhere);
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
