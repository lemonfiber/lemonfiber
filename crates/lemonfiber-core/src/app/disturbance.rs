//! What an operation takes away while it runs, said before it takes it.
//!
//! An operator weighing a stop is weighing being without something for a while,
//! and "a while" is the part they cannot look up. A second is a shrug and three
//! minutes is a different decision — and the decision is made before the command
//! runs, which is the only moment saying so is any use.
//!
//! Every length here is one the run is actually held to, never a guess at one. A
//! duration worked out by a surface would be a guess at something this side
//! knows, wrong in exactly the cases somebody most needs it, and wrong silently,
//! because nothing on either side would ever compare it to what happened. So an
//! operation with no clock on it says that instead, and says what it is waiting
//! for.

use std::time::Duration;

use super::{Command, Ctx};

/// What the container engine gives a service to stop in.
///
/// Compose's own default, and named here because it is the length a stop is
/// actually held to rather than an estimate of one. It is not imposed: passing a
/// timeout would override a service that asked for a longer one, and a service
/// flushing to disk asked for that length for a reason. The day the stack
/// declares its own, this becomes the longest of them rather than one number.
const GRACE: Duration = Duration::from_secs(10);

/// How long an operation takes something away for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Disturbance {
    /// It ends, and this is the length it is held to.
    Bounded(Duration),
    /// It does not end on a clock, and this is what it is waiting for.
    Until(Awaiting),
}

/// What an operation with no bound on it is waiting for.
///
/// A case rather than a sentence, so what an operation is waiting for is a value
/// something can be asked about. The words are built from it in one place below;
/// a phrase carried here instead would be a phrase every later surface had to
/// parse back into the fact it came from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, schemars::JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum Awaiting {
    /// Everything still coming down has finished arriving.
    Downloads,
}

/// One of the ways a lifecycle verb puts the stack out.
///
/// Between the command and the length is a situation: several commands put the
/// stack in the same one, and two surfaces spell the same command differently.
/// Naming the situation gives both the length to read off — [`of`] maps a command
/// onto it, [`everything`] lists every one of them — so a verb's bound and a
/// payload's bound are one answer rather than two that agree today.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Situation {
    /// Services are being brought up.
    Starting,
    /// Services are being taken down, interrupting anything still arriving.
    Stopping,
    /// The stack is being taken down once everything still arriving has landed.
    StoppingAfterDownloads,
    /// Services are being restarted.
    Restarting,
    /// The running set is being changed to a different one.
    Switching,
}

impl Situation {
    /// Every situation there is.
    ///
    /// A payload that states all of them reads this rather than listing them a
    /// second time. Kept honest by [`Situation::called`], whose match has no
    /// wildcard: a new situation fails to compile there, and the test that reads
    /// both then refuses any payload that has not grown a field for it.
    pub const EVERY: &[Self] = &[
        Self::Starting,
        Self::Stopping,
        Self::StoppingAfterDownloads,
        Self::Restarting,
        Self::Switching,
    ];

    /// What a payload calls this situation.
    ///
    /// The field name, so that the list above and the shape on the wire can be
    /// held against each other by something that reads them both.
    #[must_use]
    pub const fn called(self) -> &'static str {
        match self {
            Self::Starting => "starting",
            Self::Stopping => "stopping",
            Self::StoppingAfterDownloads => "stopping_after_downloads",
            Self::Restarting => "restarting",
            Self::Switching => "switching",
        }
    }

    /// How long this situation lasts.
    ///
    /// The patience is handed in because it is a knob: an operator on a slow disk
    /// runs with a longer one, and a length stated from a constant would be the
    /// wrong length for exactly the run that needed the knob.
    #[must_use]
    pub const fn takes(self, patience: Duration) -> Disturbance {
        match self {
            // Starting is bounded by how long the run waits for services to
            // settle, which is the same knob that decides when it gives up. A
            // start that has to fetch an image spends that time before this one
            // begins, and it is narrated as it happens rather than promised in
            // advance.
            Self::Starting | Self::Restarting | Self::Switching => Disturbance::Bounded(patience),
            Self::Stopping => Disturbance::Bounded(GRACE),
            // A teardown told to let the downloads finish is the one operation
            // here with nothing to bound it: what it waits for is a torrent at
            // ninety-four per cent, and how long that takes belongs to whoever
            // is seeding it.
            Self::StoppingAfterDownloads => Disturbance::Until(Awaiting::Downloads),
        }
    }
}

/// What this command disturbs, or nothing where this does not yet say.
///
/// `None` means **unstated**, not *nothing*. The verbs that start and stop
/// services are the ones answered here; setup, an uninstall, a restore and a
/// disruptive diagnosis all take something away too, and what bounds each of
/// them is a different subsystem's answer. Saying `None` for them is this side
/// declining to guess rather than claiming they are free, and the list below is
/// what makes the remainder countable instead of invisible.
///
/// The patience is handed in because it is a knob: an operator on a slow disk
/// runs with a longer one, and a length stated from a constant would be the
/// wrong length for exactly the run that needed the knob.
#[must_use]
pub const fn of(command: &Command, patience: Duration) -> Option<Disturbance> {
    let Some(situation) = situation(command) else {
        return None;
    };
    Some(situation.takes(patience))
}

/// Which situation this command puts the stack in, or none where this does not
/// yet say.
///
/// Read from the table that describes every command, [`super::rehearsal::asked`],
/// so a command is described once. How long a situation *takes* is answered here,
/// and a payload listing every situation reads that without going near a command.
#[must_use]
const fn situation(command: &Command) -> Option<Situation> {
    super::rehearsal::asked(command).disturbs
}

/// Say what this is about to take away, before it takes it.
///
/// Said through the narrator rather than returned, because it belongs to the
/// moment before the run rather than to the report after it — a length an
/// operator reads once the thing has already stopped is a length they had no
/// use for.
///
/// A rehearsal says it too. What somebody rehearsing wants to know is what the
/// real run would cost, and a rehearsal silent about it would leave the one
/// question the flag exists to answer unanswered.
pub async fn said(command: &Command, ctx: &Ctx) {
    let Some(disturbance) = of(command, ctx.patience) else {
        return;
    };

    ctx.narrator.say(&sentence(disturbance)).await;
}

/// The one place a disturbance is put into words.
///
/// The bounded arm reaches for the same phrasing a disturbing check uses, so the
/// two kinds of length an operator meets read the same way and neither can drift
/// into a number typed into a sentence.
fn sentence(disturbance: Disturbance) -> String {
    match disturbance {
        Disturbance::Bounded(bound) => format!(
            "this takes services away {}",
            crate::doctor::disturbing_for(bound)
        ),
        Disturbance::Until(Awaiting::Downloads) => {
            "this does not finish until everything still coming down has arrived".to_owned()
        }
    }
}

#[cfg(test)]
mod tests;
