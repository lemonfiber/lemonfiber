//! Reading back everything lemonfiber changed, and whether each could be put back.
//!
//! The record exists already — every apply, seed and reconfigure journals what it did,
//! with the values on both sides. What this adds is a way to *look* at it, and the
//! judgement [`crate::rollback`] makes about each entry, so an operator can see what
//! happened and what could be undone before asking for anything to be.
//!
//! Reading only. Nothing here puts a change back.

use crate::journal::{Change, Kind};
use crate::model::{ChangeReport, HistoryReport};
use crate::rollback::{standing, together, Reversal};

use super::Ctx;

/// How far back the record goes.
///
/// The journal is written by apply and never trimmed, so the horizon is the whole of it.
/// Stated anyway, because a record that has been trimmed and one that has always been
/// short look identical from the entries alone.
const HORIZON: &str = "every change since this machine was set up; nothing is trimmed";

/// Everything lemonfiber changed, newest first.
///
/// Cannot fail. A machine with nowhere to keep a journal has recorded no changes, which
/// is an empty history rather than a refusal.
pub fn history(ctx: &Ctx) -> HistoryReport {
    let Some(paths) = super::targets::layout(ctx) else {
        return HistoryReport {
            horizon: HORIZON.to_owned(),
            ..HistoryReport::default()
        };
    };
    let journal = super::recover::journal_at(&paths.journal());
    let changes = journal.changes();

    // What the environment file holds now, for the drift question. Read once: asking per
    // change would read the same file as many times as there are entries.
    let holds = |key: &str| -> Option<String> {
        let file = ctx.settings.env_file.as_ref()?;
        crate::config::store::read(file)
            .ok()?
            .get(key)
            .map(str::to_owned)
    };

    let mut read: Vec<ChangeReport> = changes
        .iter()
        .enumerate()
        .map(|(at, change)| {
            let later = changes.get(at + 1..).unwrap_or_default();
            told(
                change,
                standing(change, later, &holds),
                together(changes, &change.operation).len(),
            )
        })
        .collect();
    read.reverse();

    HistoryReport {
        changes: read,
        horizon: HORIZON.to_owned(),
    }
}

/// One change, as an operator reads it.
fn told(change: &Change, standing: crate::rollback::Standing, alongside: usize) -> ChangeReport {
    let (because, instead) = standing
        .refusal
        .map_or((None, None), |why| (Some(why.because), why.instead));

    ChangeReport {
        at: change.at.clone(),
        operation: change.operation.clone(),
        target: change.target.clone(),
        did: did(&change.kind),
        reversal: match standing.reversal {
            Reversal::Whole => "whole",
            Reversal::Partial => "partial",
            Reversal::None => "none",
        }
        .to_owned(),
        because,
        instead,
        alongside,
    }
}

/// What a change did, in the operator's terms rather than the journal's.
fn did(kind: &Kind) -> String {
    match kind {
        Kind::Created { resource, .. } => format!("added a {resource}"),
        Kind::Set {
            key,
            previous,
            current,
        } => previous.as_ref().map_or_else(
            || format!("set {key} to {current}"),
            |was| format!("changed {key} from {was} to {current}"),
        ),
        Kind::Made { path } => format!("made {path}"),
        Kind::Configured { field, .. } => format!("set {field} on the service"),
    }
}
