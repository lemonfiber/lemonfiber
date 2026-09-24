//! Reading back everything lemonfiber changed, and whether each could be put back.
//!
//! The record exists already — every apply, seed and reconfigure journals what it did,
//! with the values on both sides. What this adds is a way to *look* at it, and the
//! judgement [`crate::rollback`] makes about each entry, so an operator can see what
//! happened and what could be undone before asking for anything to be.
//!
//! Reading only. Nothing here puts a change back.

use crate::config::store::is_secret;
use crate::journal::{horizon, Change, Kind};
use crate::model::{ChangeReport, HistoryReport};
use crate::rollback::{standing, together};

use super::Ctx;

/// Everything lemonfiber changed, newest first.
///
/// Cannot fail. A machine with nowhere to keep a journal has recorded no changes, which
/// is an empty history rather than a refusal.
pub fn history(ctx: &Ctx) -> HistoryReport {
    let Some(paths) = super::targets::layout(ctx) else {
        return HistoryReport {
            horizon: horizon(&[]),
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

    // And what a file holds, for a region: whether it is still the one written.
    let reads = |path: &str| -> Option<String> { std::fs::read_to_string(path).ok() };

    let mut read: Vec<ChangeReport> = changes
        .iter()
        .enumerate()
        .map(|(at, change)| {
            let later = changes.get(at + 1..).unwrap_or_default();
            told(
                change,
                standing(change, later, &holds, &reads),
                together(changes, &change.operation, &change.at).len(),
            )
        })
        .collect();
    read.reverse();

    HistoryReport {
        changes: read,
        horizon: horizon(changes),
    }
}

/// One change, as an operator reads it.
///
/// A secret says what changed and never what to. The rest of this product refuses to
/// write a credential down a second time; a read that printed one would be the same
/// leak arriving by a different route, and this is the output an operator pastes into
/// a forum thread when they are asking why something broke.
///
/// That it *had* a value before is still said, because that is what separates a
/// setting being given one from a setting being changed, and neither is the value.
fn told(change: &Change, standing: crate::rollback::Standing, alongside: usize) -> ChangeReport {
    let (because, instead) = standing
        .refusal
        .map_or((None, None), |why| (Some(why.because), why.instead));

    ChangeReport {
        at: stamped(&change.at),
        operation: change.operation.clone(),
        target: change.target.clone(),
        did: did(&change.kind),
        reversal: standing.reversal,
        because,
        instead,
        alongside,
    }
}

/// A change's stamp as the report promises it: whole seconds since the epoch.
///
/// Every stamp this build writes is already that. A journal outlives the build that
/// wrote it, though, and an earlier one wrote an empty stamp where its clock would not
/// answer — so anything that is not digits is read as the epoch, which is how this
/// build writes a clock that would not answer, rather than passed on as a promise the
/// report does not keep.
fn stamped(at: &str) -> String {
    if !at.is_empty() && at.bytes().all(|byte| byte.is_ascii_digit()) {
        at.to_owned()
    } else {
        "0".to_owned()
    }
}

/// What a change did, in the operator's terms rather than the journal's.
fn did(kind: &Kind) -> String {
    match kind {
        Kind::Created { resource, .. } => format!("added a {resource}"),
        Kind::Set { key, previous, .. } if is_secret(key) => previous.as_ref().map_or_else(
            || format!("set {key}"),
            |_| format!("changed {key}, and it had a value before"),
        ),
        Kind::Set {
            key,
            previous,
            current,
        } => previous.as_ref().map_or_else(
            || format!("set {key} to {current}"),
            |was| format!("changed {key} from {was} to {current}"),
        ),
        Kind::Made { path } => format!("made {path}"),
        Kind::Region { owner, path, .. } => format!("wrote {owner}'s region into {path}"),
        Kind::Pinned {
            previous, current, ..
        } => format!("moved from {previous} to {current}"),
        Kind::Configured { field, .. } => format!("set {field} on the service"),
    }
}

#[cfg(test)]
mod tests {
    use super::{did, stamped};

    /// A region reads as what it was: something written into a file that was there.
    #[test]
    fn a_region_reads_as_whose_it_is_and_which_file_it_went_into() {
        assert_eq!(
            did(&crate::journal::Kind::Region {
                path: "/stack/config/caddy/Caddyfile".to_owned(),
                key: "config/caddy/Caddyfile".to_owned(),
                owner: "plugin komga".to_owned(),
                written: 0,
            }),
            "wrote plugin komga's region into /stack/config/caddy/Caddyfile"
        );
    }

    /// Every stamp this build writes goes out as it was written.
    #[test]
    fn a_stamp_of_seconds_goes_out_as_written() {
        assert_eq!(stamped("1709287200"), "1709287200");
        assert_eq!(stamped("0"), "0");
    }

    /// An earlier build wrote an empty stamp where its clock would not answer, and a
    /// journal outlives the build that wrote it. The report promises digits, so what is
    /// not digits is read as the epoch — this build's own spelling of that clock —
    /// rather than passed on as a promise it does not keep.
    #[test]
    fn a_stamp_that_is_not_seconds_is_read_as_the_epoch() {
        for unreadable in ["", "t", "2024-03-01T10:00:00Z", "-1", "12a"] {
            assert_eq!(stamped(unreadable), "0", "{unreadable:?}");
        }
    }
}
