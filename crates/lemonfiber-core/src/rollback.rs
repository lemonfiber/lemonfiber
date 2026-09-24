//! Whether a journalled change can be put back, and what putting it back would meet.
//!
//! The journal already holds what each change did and what undoing it needs. What this
//! adds is the judgement an operator has to be given *before* anything is undone: whether
//! a change can be reversed at all, whether a later change depends on it, and whether the
//! value has been edited by hand since — because a rollback that quietly overwrites
//! somebody's own edit is worse than one that refuses.
//!
//! Nothing here writes. It is given the journal and what the machine currently holds, and
//! returns what a reversal would come to, so every arrangement can be exercised without a
//! disk.

use crate::config::store::is_secret;
use crate::journal::{is_sealed, Change, Kind};

/// How far a change can be put back.
///
/// Published as the closed set it is, rather than as a word a reader has to trust will
/// be one of three: a surface that lays out a history branches on it, and a set the
/// contract names is one a generated reader can match exhaustively.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, schemars::JsonSchema)]
#[serde(rename_all = "lowercase")]
#[schemars(rename = "ChangeReversal")]
pub enum Reversal {
    /// It can be put back exactly.
    Whole,
    /// Some of it can, and the rest is stated rather than attempted.
    Partial,
    /// None of it can.
    None,
}

/// Why a change cannot be put back, where it cannot.
///
/// Carried beside the verdict rather than folded into it: an operator told no needs the
/// reason, and the reason is what tells them whether to reach for a backup instead.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Refusal {
    /// What stands in the way, in the operator's terms.
    pub because: String,
    /// What to do instead, where there is something.
    pub instead: Option<String>,
}

/// What putting one change back would come to.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Standing {
    /// How far it can go.
    pub reversal: Reversal,
    /// Why it cannot go further, where it cannot.
    pub refusal: Option<Refusal>,
}

impl Standing {
    /// A change that can be put back exactly.
    fn whole() -> Self {
        Self {
            reversal: Reversal::Whole,
            refusal: None,
        }
    }

    /// A change that cannot be put back, and why.
    fn refused(because: &str, instead: Option<&str>) -> Self {
        Self {
            reversal: Reversal::None,
            refusal: Some(Refusal {
                because: because.to_owned(),
                instead: instead.map(str::to_owned),
            }),
        }
    }

    /// A change only partly reversible, and what the rest leaves.
    fn partly(because: &str, instead: Option<&str>) -> Self {
        Self {
            reversal: Reversal::Partial,
            refusal: Some(Refusal {
                because: because.to_owned(),
                instead: instead.map(str::to_owned),
            }),
        }
    }
}

/// The setting a change is about, the value it left there, and the value it replaced —
/// where it is about a setting at all.
///
/// One question rather than three. A change that names a setting is the same change that
/// wrote a value into it and the same one that replaced whatever was there, so asking any
/// of them separately would match the same variant again and leave an arm nothing could
/// reach.
fn touched(change: &Change) -> Option<(&str, &str, Option<&str>)> {
    match &change.kind {
        Kind::Set {
            key,
            current,
            previous,
        } => Some((key, current, previous.as_deref())),
        _ => None,
    }
}

/// The setting a change is about, where it is about one.
#[must_use]
pub fn setting(change: &Change) -> Option<&str> {
    touched(change).map(|(key, ..)| key)
}

/// Why a service that has been started on a newer version cannot be pinned back.
///
/// The domain rule rather than a statement about lemonfiber: it is the service's own
/// binary that migrates its database, on first start, and nothing about lemonfiber
/// having recorded the move changes what that leaves behind.
///
/// The rule itself is recorded in [`crate::migration::version`], which is where it is
/// *acted* on — it is why a pin behind what a project already runs is refused rather
/// than attempted. This is the same rule said to an operator instead of to the
/// comparison, and the two are deliberately linked: if the reason a downgrade is
/// refused there ever stops being true, the sentence here stops being true with it.
fn migrated(target: &str, previous: &str, current: &str) -> String {
    format!(
        "{target} was started on {current} and migrated its database doing it — a \
         database is carried forward by whichever version opened it last, so \
         {previous} can no longer open what it left behind"
    )
}

/// Where to go instead, naming the capture where the record kept one.
///
/// A journal written before the update recorded its capture, or one whose entry lost
/// it, still has the right answer to give — it just cannot give the path, and saying
/// so plainly beats naming a file that may not be there.
fn capture(backup: Option<&str>) -> String {
    match backup {
        Some(path) => format!("restore from the capture taken before the update, at {path}"),
        None => "restore from the capture taken before the update".to_owned(),
    }
}

/// The key whose reversal moves no data, only the pointer to it.
///
/// Named rather than inferred: an operator putting the data location back has to be told
/// that what moved stays where it is, and a rule that guessed which settings were about
/// data would say it about the wrong ones.
const POINTS_AT_DATA: &str = "DATA_ROOT";

/// What putting `change` back would come to, given the journal it sits in and what the
/// machine holds now.
///
/// `later` is every change made after it, `holds` answers what a setting currently
/// holds, and `reads` what a file currently holds — the questions a reversal cannot be
/// judged without.
#[must_use]
pub fn standing(
    change: &Change,
    later: &[Change],
    holds: &dyn Fn(&str) -> Option<String>,
    reads: &dyn Fn(&str) -> Option<String>,
) -> Standing {
    if let Some((key, left, replaced)) = touched(change) {
        // Asked ahead of drift, because a sealed value that will not open is not a value
        // to compare against: what the setting holds now differs from the sealed text
        // whatever it holds, so drift would answer yes and blame the operator for an edit
        // nobody made. The record is there and this machine cannot read it, which is a
        // different sentence and the true one.
        //
        // Either half counts. What was written is what a reversal checks its own work
        // against and what was there before is what it would put back, so a reversal
        // missing either one is a reversal that cannot be carried out honestly.
        if is_sealed(left) || replaced.is_some_and(is_sealed) {
            return Standing::refused(
                &format!(
                    "{key} holds a credential, and the record of what it held is sealed \
                     under a key this machine no longer has — so there is nothing here it \
                     can put back"
                ),
                Some("set it yourself, from wherever the earlier credential came from"),
            );
        }
        if let Some(edited) = drifted(change, holds) {
            // A secret says that it differs and never what it now is. The value here is
            // read live off the environment file, so printing it would put a credential
            // into a refusal — which travels further than the file it came from.
            let holding = if is_secret(key) {
                format!("{key} now holds something else")
            } else {
                format!("{key} now holds {edited}")
            };
            return Standing::refused(
                &format!(
                    "{holding}, which is not what this change left — somebody has set it \
                     since, and putting this back would discard their edit"
                ),
                Some("set it yourself if the older value is the one you want"),
            );
        }
        if depended_on(key, later) {
            return Standing::refused(
                &format!("a later change to {key} would be undone with it"),
                Some("put the later change back first"),
            );
        }
        if key == POINTS_AT_DATA {
            return Standing::partly(
                &format!("{key} goes back and the data does not move with it"),
                Some("move the library yourself if it should follow"),
            );
        }
    }

    match &change.kind {
        // Removing what a service created is that service's own to do, and nothing here
        // asks it to: the reversal of a creation is worked out, set aside as beyond a
        // host's reach, and reported — every time, on every surface. Judging it whole
        // promised an operator a reversal no part of this product carries out.
        Kind::Created { resource, .. } => Standing::refused(
            &format!(
                "removing the {resource} it added is something only {} can be asked to \
                 do, and lemonfiber does not ask it",
                change.target
            ),
            Some("remove it in that service's own interface"),
        ),
        // Not "lemonfiber does not repin", which is true and is the smaller half of
        // the answer. A service recorded this way ran on the newer version, and a
        // database is migrated forward by whichever binary opened it last — so the
        // older one cannot open what it left behind, and moving the pin back would
        // produce a service that will not start rather than the stack they had.
        //
        // Which makes the capture the answer rather than a consolation, and the
        // reason it is named by path: it is the one taken while the stack was down
        // and before anything opened its state on the new version, so it holds the
        // database from before the migration. An operator told only "restore from a
        // backup" has to work out which of five on the machine that is, in the
        // moment they are least able to.
        Kind::Pinned {
            previous,
            current,
            backup,
        } => Standing::refused(
            &migrated(&change.target, previous, current),
            Some(&capture(backup.as_deref())),
        ),
        Kind::Region {
            path,
            owner,
            written,
            ..
        } => bounded(path, owner, *written, reads),
        // A setting, a path, or one field of a service's record — each reversed by
        // something this product actually does.
        Kind::Set { .. } | Kind::Made { .. } | Kind::Configured { .. } => Standing::whole(),
    }
}

/// What taking a region back out comes to, given what its file holds now.
///
/// A region is lemonfiber's only while it is exactly what was written, inside the
/// markers it was written with. Edited since, it holds somebody's work; with its
/// markers edited, which lines are lemonfiber's can no longer be told from which are
/// not. Either way taking it out would take something that is not lemonfiber's, so
/// the reversal is refused — the same answer drift in a setting gets. A file that is
/// not there any more has no region left in it to take, and nothing is owed.
fn bounded(
    path: &str,
    owner: &str,
    written: u32,
    reads: &dyn Fn(&str) -> Option<String>,
) -> Standing {
    let Some(text) = reads(path) else {
        return Standing::whole();
    };
    match crate::region::within(&text, owner) {
        None => Standing::refused(
            &format!(
                "{owner}'s region in {path} is no longer marked out the way it was written — \
                 its markers were edited or taken out — so which lines are lemonfiber's can \
                 no longer be told from which are not"
            ),
            Some("take the region out by hand, or put its markers back as they were"),
        ),
        Some(body) if crate::materialised::checksum(body.as_bytes()) != written => {
            Standing::refused(
                &format!(
                    "{owner}'s region in {path} has been edited since it was written, and \
                     taking it out would discard that edit"
                ),
                Some("take it out by hand if the edit is not wanted"),
            )
        }
        Some(_) => Standing::whole(),
    }
}

/// What the setting holds now, where that is not what this change left.
fn drifted(change: &Change, holds: &dyn Fn(&str) -> Option<String>) -> Option<String> {
    let (key, left, _) = touched(change)?;
    let now = holds(key)?;
    (now != left).then_some(now)
}

/// Whether a later change touched the same setting.
fn depended_on(key: &str, later: &[Change]) -> bool {
    later.iter().filter_map(setting).any(|named| named == key)
}

/// Every change of one run of an operation, which rolls back as one unit or not at all.
///
/// An operation is the unit an operator agreed to — a seed, a reconfigure — and undoing
/// half of one leaves a machine in a state nobody chose.
///
/// A run is the operation and the stamp together, never the operation alone. The name
/// says the kind of run and is reused by every run of that kind, so matching on it would
/// gather every apply this machine has ever made into one unit; the surface stamps one
/// time for a whole run, so the pair is what tells two of them apart.
#[must_use]
pub fn together<'a>(changes: &'a [Change], operation: &str, at: &str) -> Vec<&'a Change> {
    changes
        .iter()
        .filter(|change| change.operation == operation && change.at == at)
        .collect()
}

/// Every change an operation ever made, across every run of it.
///
/// [`together`] is one run, and the doc above says why: an operation's name is reused by
/// every run of that kind, so matching on the name alone would gather every apply this
/// machine has ever made into one unit. That is exactly the wrong answer for an undo of
/// a stamp — and exactly the right one where the operation names a thing rather than a
/// kind of run.
///
/// A plugin's id is such a name. It is one plugin, installed once, and every change
/// journalled under it is that plugin's doing — so taking the plugin off the machine
/// means taking all of them back, whether they were written by the install or by
/// something that wrote to it since. A removal that put back only the run that installed
/// it would leave whatever came after standing with nothing to explain it.
#[must_use]
pub fn everything<'a>(changes: &'a [Change], operation: &str) -> Vec<&'a Change> {
    changes
        .iter()
        .filter(|change| change.operation == operation)
        .collect()
}

#[cfg(test)]
mod tests;
