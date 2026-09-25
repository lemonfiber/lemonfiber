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

pub(crate) mod run;

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
    pub(crate) const fn on_boot(self) -> bool {
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
mod tests;
