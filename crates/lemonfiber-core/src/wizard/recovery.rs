//! Picking a setup back up after it stopped.
//!
//! An interrupted apply wrote something, and what it wrote is the operator's to keep,
//! undo, or forget — so the ways out are modelled rather than decided for them.

use super::{Change, Journal, Phase, Progress, Undo};

/// What a later run makes of the setup it finds — the whole lifecycle, read from
/// the world rather than trusted from a file.
///
/// Two of these are inferred rather than stored. No saved progress at all is
/// `Absent`; a saved progress still marked applying is `FailedApply`, because the
/// only thing that writes the applying marker is a live apply, so finding it
/// persisted means that apply stopped before it finished. Every other state is
/// its marker read straight back.
///
/// A surface must consult this before it decides whether to offer setup or resume
/// a run: `FailedApply` takes precedence over both. An interrupted apply that got
/// as far as writing configuration looks "already configured" to a naive check,
/// and its half-written state must be recovered rather than mistaken for a
/// finished one.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Status {
    /// No setup has been started; the wizard is offered.
    Absent,
    /// Setup is part-answered and resumable.
    InProgress,
    /// Answers are complete and awaiting confirmation.
    Reviewing,
    /// Setup finished and configuration is on disk.
    Applied,
    /// An apply was interrupted and must be recovered before setup goes on.
    FailedApply,
}

impl Status {
    /// Classify the setup a run finds from its saved progress, if any.
    ///
    /// No progress read back is `Absent`. A saved applying marker is the
    /// interrupted case, surfaced as `FailedApply` for deliberate recovery rather
    /// than resumed as if nothing had gone wrong.
    #[must_use]
    pub const fn of(progress: Option<&Progress>) -> Self {
        match progress {
            None => Self::Absent,
            Some(progress) => match progress.phase {
                Phase::InProgress => Self::InProgress,
                Phase::Reviewing => Self::Reviewing,
                Phase::Applying => Self::FailedApply,
                Phase::Applied => Self::Applied,
            },
        }
    }

    /// Whether this is a setup to pick up where it was left.
    ///
    /// The question every surface asks first, and asks before it asks whether the
    /// machine is configured: an apply that stopped part-way has written settings
    /// that the configured-yet check reads as a finished install, so a run that
    /// asked in the other order would offer reconfiguration for a stack that is
    /// half written.
    #[must_use]
    pub const fn unfinished(self) -> bool {
        matches!(self, Self::InProgress | Self::Reviewing | Self::FailedApply)
    }
}

/// What to do about an interrupted apply, once one is found.
///
/// The three exits the setup wizard promises for its one dangerous state: keep
/// going, walk it back, or drop it entirely. A surface offers these; [`Recovery`]
/// turns the chosen one into the work it means.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Choice {
    /// Carry on from where apply stopped, keeping what was already written.
    Resume,
    /// Undo what was written and return to the reviewed answers, ready to apply
    /// again.
    RollBack,
    /// Undo what was written and discard the answers too, back to a clean slate.
    StartOver,
}

/// The recovery offered for an interrupted apply: what it wrote, and what each
/// choice does about it.
///
/// Built from the change journal — the record of what apply managed to write
/// before it stopped — so the operator is shown the real partial state, not a
/// guess, and a reversal touches exactly what was recorded. Only recorded changes
/// can be reversed: whatever apply writes and wants undone, it must journal.
#[derive(Debug)]
pub struct Recovery<'a> {
    written: &'a Journal,
}

impl<'a> Recovery<'a> {
    /// The recovery for an apply that stopped after writing what `written` holds.
    #[must_use]
    pub const fn of(written: &'a Journal) -> Self {
        Self { written }
    }

    /// What the interrupted apply wrote, in the order it wrote it — the partial
    /// state to report before a choice is offered.
    #[must_use]
    pub fn written(&self) -> &[Change] {
        self.written.changes()
    }

    /// The concrete work a choice resolves to, for a surface to carry out.
    ///
    /// Both walking back and starting over reverse what was written — the one
    /// difference is whether the answers survive, so neither leaves the partial
    /// apply stranded on disk. The reversal is computed here, so the whole of it
    /// stays testable without a service or a disk.
    #[must_use]
    pub fn resolve(&self, choice: Choice) -> Resolution {
        match choice {
            Choice::Resume => Resolution::Resume,
            Choice::RollBack => Resolution::RollBack(self.written.rewind()),
            Choice::StartOver => Resolution::StartOver(self.written.rewind()),
        }
    }
}

/// The work a [`Choice`] resolves to.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Resolution {
    /// Continue the apply, keeping every write already made.
    Resume,
    /// Reverse the recorded writes, most recent first, keeping the answers so the
    /// apply can be reviewed and run again.
    RollBack(Vec<Undo>),
    /// Reverse the recorded writes, then discard the saved progress and journal —
    /// nothing of this attempt left behind.
    StartOver(Vec<Undo>),
}
