//! Why it stopped, where the record plainly shows it.
//!
//! Never over-claiming: an \*arr's own history can show a grab that failed or an import
//! that never came, and those are said. What it cannot see — whether an indexer found
//! anything, whether the client took it — is left to the services that can.

use super::reading::*;
use crate::model::TraceReport;
use crate::trace::{Confidence, Presence, Stage};

/// The disagreements and unreadable-fragment notes a trace surfaces on their own, apart
/// from the linear pipeline: a media server holding what nothing monitors, and each read
/// that failed reported as unavailable rather than inferred as nothing — the honesty the
/// trace keeps about a silence it did not actually hear.
pub(crate) fn trace_findings(unmanaged_but_present: bool, reads: Reads) -> Vec<String> {
    let mut findings = Vec::new();
    if unmanaged_but_present {
        findings.push(
            "the media server has this, but no service is monitoring it — it will not be \
             maintained, upgraded, or repaired if it is lost"
                .to_owned(),
        );
    }
    if !reads.history {
        findings.push(
            "this service's history could not be read, so how far the item got may be \
             understated — reported as unavailable, not read as nothing happened"
                .to_owned(),
        );
    }
    if !reads.queue {
        findings.push(
            "the download queue could not be read, so whether it is downloading now is \
             unknown — reported as unavailable, not read as stopped"
                .to_owned(),
        );
    }
    if !reads.parts {
        findings.push(
            "the episodes could not be read, so how much of this is here is unknown — \
             reported as unavailable, not read as a series with nothing in it"
                .to_owned(),
        );
    }
    findings
}

/// Why the item stopped where it did, in plain language — or `None` where nothing proves
/// it stopped. The generic reason a resting stage carries, sharpened by what only the live
/// reads can settle: a stuck queue names the download client; an import confirmed absent
/// from the library names the missing scan; downloading and beyond are otherwise either in
/// progress or beyond what the \*arr alone can judge.
pub(crate) fn stall_reason(
    furthest: Stage,
    queue_stuck: bool,
    library: Option<Presence>,
    reads: Reads,
) -> Option<String> {
    if queue_stuck {
        // The C7 signal: queued but not progressing — a real problem the operator can act
        // on, distinct from a download merely still running.
        return Some(
            "the download is in the queue but not progressing — the download client needs \
             attention"
                .to_owned(),
        );
    }
    // A stall claimed from an absence stands only where that absence was actually read.
    // Imported but confirmed absent from the library is provably awaiting a scan — a reason
    // only the media server can supply, so it stands only on a confirmed absence.
    match furthest {
        // Nobody asked — settled from the monitored flag alone, always known.
        Stage::NotMonitored => furthest.stall().map(str::to_owned),
        // Monitored and nothing since — but a "never found" is a claim about an empty
        // history, so only where the history was actually read, not where it could not be.
        Stage::Monitored if reads.history => furthest.stall().map(str::to_owned),
        // Grabbed and not in the queue — a claim about an empty queue, so only where the
        // queue was actually read.
        Stage::Grabbed if reads.queue => furthest.stall().map(str::to_owned),
        Stage::Imported if library == Some(Presence::Absent) => {
            Stage::Imported.stall().map(str::to_owned)
        }
        _ => None,
    }
}

/// The trace for a term no monitored item matches — nobody has asked for it.
pub(crate) fn not_matched(term: &str) -> TraceReport {
    TraceReport {
        item: term.to_owned(),
        matched: false,
        furthest: Stage::NotMonitored,
        stall: Stage::NotMonitored.stall().map(str::to_owned),
        stages: Vec::new(),
        history: Vec::new(),
        coverage: None,
        confidence: Confidence::Certain,
        findings: Vec::new(),
    }
}
