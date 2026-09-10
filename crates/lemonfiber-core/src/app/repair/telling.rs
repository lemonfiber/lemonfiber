//! What a reversal says it did, once it has done it.
//!
//! The account rather than the instruction. Everything a reversal needs in order to put
//! something back is in the undo it was handed; what comes out the other side is a list
//! somebody reads — printed on a terminal, served at `/api/undo`, carried by
//! `Outcome::Undo`. The two are the same shape and must not hold the same things.
//!
//! Here rather than beside the run for the reason `consent` is next door: what a run is
//! allowed to do and what it may say about having done it are two questions, and the
//! second one is short enough to read whole.

use crate::config::store::is_secret;
use crate::journal::{Action, Undo};
use crate::ports::withheld::REDACTED;

/// One reversal as it is safe to report.
///
/// The values are what a reversal is carried out *with*, and they must not survive the
/// carrying out. This list is what [`Reversal`] holds and `Outcome::Undo` carries — a
/// terminal prints it and `/api/undo` serves it — so a credential left in it is the
/// record sealed and the door held open. Withheld here rather than where the undo is
/// built, because the undo is the instruction and this is the account of it: blanking
/// the instruction would leave the reversal with nothing to put back.
///
/// [`REDACTED`] rather than nothing, because nothing already means something in both of
/// these: a restore carrying no value is one that takes the setting away again. A
/// credential reported that way would read as an instruction to remove it, which is a
/// different act from the one that happened.
///
/// Asked of the name in both, so a service field spelled like a credential is treated
/// as one — `apiKey` is withheld and `tvCategory` is not.
pub(super) fn told(undo: Undo) -> Undo {
    let action = match undo.action {
        Action::Restore { key, value, .. } if is_secret(&key) => Action::Restore {
            key,
            value: value.map(|_| REDACTED.to_owned()),
            wrote: REDACTED.to_owned(),
        },
        Action::Reconfigure {
            resource,
            id,
            field,
            value,
        } if is_secret(&field) => Action::Reconfigure {
            resource,
            id,
            field,
            value: value.map(|_| REDACTED.to_owned()),
        },
        other => other,
    };
    Undo { action, ..undo }
}
