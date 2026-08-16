//! Whether there is room, and how much is left in human terms.
//!
//! A byte count means nothing to the person reading it; the floor and the phrasing that
//! turns one into the other belong together.

use crate::bytes::humanize;

use super::{finding, Code, Finding, Problem, Remedy, Severity, State, StorageFacts, Verdict};

/// The free-space finding for the volume the data root sits on.
///
/// A volume that reports no size at all could not be measured rather than being
/// empty, so it is unverified rather than reported as full. Otherwise the finding
/// is a projection: the free space left once the download clients' committed
/// content has landed, warned on when that projected figure falls under the floor
/// rather than only when the volume is already nearly full. A committed figure of
/// zero — the clients quiet or unreachable — and `None` — no projection supplied at
/// all — both subtract nothing, so the same floor guards the raw free space, the
/// behaviour before a client could be read.
pub(super) fn space(facts: &StorageFacts, committed: Option<u64>) -> Finding {
    if facts.total == 0 {
        return finding(
            "storage.space",
            "Free space",
            Verdict::Unverified {
                reason: "the volume's free space could not be read".to_owned(),
                remedy: Remedy::new("Run the storage check again once the location is reachable"),
            },
        );
    }

    let underway = committed.unwrap_or(0);
    let projected = facts.available.saturating_sub(underway);
    let verdict = if projected >= LOW_SPACE_FLOOR {
        Verdict::Pass {
            note: Some(free_note(facts, underway)),
        }
    } else if underway > 0 {
        // Room now, but the queue will consume it: exhaustion the operator is
        // warned of before it happens rather than once the disk is already full.
        Verdict::Warn(
            Problem::new(
                SPACE_LOW,
                Severity::Warning,
                format!(
                    "Storage is projected to run out — {} of downloads still to land, {} free",
                    humanize(underway),
                    humanize(facts.available),
                ),
                "Downloads already queued will not fit alongside the room an import and its \
                 unpacking need, so the disk fills partway through and leaves half a file behind.",
                Remedy::new(
                    "Free space on the data location, thin the download queue, or move it to a \
                     larger volume",
                ),
            )
            .in_state(State::Guided),
        )
    } else {
        // No committed content to project from, but the volume is already low.
        Verdict::Warn(
            Problem::new(
                SPACE_LOW,
                Severity::Warning,
                format!("Free space is low — {} left", humanize(facts.available)),
                "A disk that fills partway through an import leaves half a file behind and \
                 stalls the queue, so what is left has to cover what is still to come.",
                Remedy::new("Free space on the data location, or move it to a larger volume"),
            )
            .in_state(State::Guided),
        )
    };
    finding("storage.space", "Free space", verdict)
}

/// The passing note: what is free of the whole, and — where downloads are underway
/// — what is still to land, so a pass that is comfortable only because the queue is
/// small still says so.
pub(super) fn free_note(facts: &StorageFacts, underway: u64) -> String {
    let free_of_total = format!(
        "{} free of {}",
        humanize(facts.available),
        humanize(facts.total)
    );
    if underway > 0 {
        format!("{free_of_total}, {} still to land", humanize(underway))
    } else {
        free_of_total
    }
}

/// Raised when the volume holding the data root is nearly full.
pub const SPACE_LOW: Code = Code::new("STORAGE-4");

/// The free space a volume must keep clear once the queue has landed.
///
/// The check projects exhaustion from committed downloads — the free space *minus*
/// what the download clients still have to write — and warns when that projected
/// figure falls under this floor. Where no download client is
/// reachable there is nothing to subtract, so the same floor guards the raw free
/// space instead. A single large import can be tens of gigabytes and unpacking
/// needs room beside the file, so the floor sits well above one file.
///
/// Public so the quality-headroom check can defer to this one below it: a disk this
/// low is a free-space problem this check already reports, not a quality-fit one.
pub const LOW_SPACE_FLOOR: u64 = 10 * 1024 * 1024 * 1024;
