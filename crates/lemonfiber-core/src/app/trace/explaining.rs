//! Why it stopped, where the record plainly shows it.
//!
//! Never over-claiming: an \*arr's own history can show a grab that failed or an import
//! that never came, and those are said. What it cannot see — whether an indexer found
//! anything, whether the client took it — is left to the services that can.

use super::reading::{Reads, Searched};
use crate::model::{TraceReport, TraceStage};
use crate::trace::{Confidence, Presence, Stage};

/// Why a wanted item that nothing has been grabbed for has stopped, where nobody asked
/// for a search.
///
/// Three causes look identical from an \*arr's own record — the indexers carry nothing,
/// the indexers carry only releases the quality in force rejects, and no search has been
/// made at all — and only a live search tells them apart. So this names none of them and
/// says which question is unanswered, beside the form of the trace that answers it.
const NOT_SEARCHED: &str = "monitored, but nothing has been grabbed for it yet — no search was \
     run, so whether the indexers carry nothing for it or the quality in force wants none of \
     what they do carry is not known; ask for a search to tell the two apart";

/// Why it stopped, where a search ran cleanly and the indexers carried nothing.
///
/// Said as the absence it is, and said as not the preset's doing: an operator who eases
/// the quality here changes nothing, because there was nothing to reject.
const NOTHING_CARRIED: &str = "monitored, and a search found nothing — the indexers carry \
     nothing for it at all, which is not the quality preset's doing";

/// Why it stopped, where a search was made and settled nothing.
///
/// The search itself could not be run, or what came back was about other content. Either
/// way the question the search was spent on is still open, which is a different sentence
/// from never having asked it.
const SEARCH_SETTLED_NOTHING: &str = "monitored, but nothing has been grabbed for it yet — the \
     search settled nothing about it, so why it stopped here is still not known";

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
        // Monitored and nothing since — but a claim about why is a claim about an empty
        // history, so only where the history was actually read, not where it could not be.
        // What it stopped for is a question no \*arr can answer alone, so what is said is
        // that it is unanswered and what would answer it.
        Stage::Monitored if reads.history => Some(NOT_SEARCHED.to_owned()),
        // Grabbed and not in the queue — a claim about an empty queue, so only where the
        // queue was actually read.
        Stage::Grabbed if reads.queue => furthest.stall().map(str::to_owned),
        Stage::Imported if library == Some(Presence::Absent) => {
            Stage::Imported.stall().map(str::to_owned)
        }
        _ => None,
    }
}

/// What a live search adds to a trace whose item is wanted and has been carried nowhere.
///
/// The one reading no \*arr can reach on its own. Releases every profile rejects and no
/// releases at all leave the same silence in its history, and only a search against the
/// indexers tells them apart — so where the search says the quality in force wants none
/// of what is out there, the item did reach `Found`, and the stage, the stages it passed
/// through and the reason it stopped all move together.
///
/// Applied only to a trace resting at `Monitored` with a reason that stands. A search
/// says nothing about an item already grabbed, and a history that could not be read is
/// not one proving nothing was.
pub(crate) fn searched(report: &mut TraceReport, service: &str, said: Searched) {
    match said {
        Searched::NoneAtTheQuality => {
            report.furthest = Stage::Found;
            report.stages.push(TraceStage {
                stage: Stage::Found,
                service: service.to_owned(),
                at: None,
            });
            report.stall = Stage::Found.stall().map(str::to_owned);
        }
        Searched::Nothing => report.stall = Some(NOTHING_CARRIED.to_owned()),
        Searched::Unsettled => report.stall = Some(SEARCH_SETTLED_NOTHING.to_owned()),
    }
}

/// Whether a live search could still tell this trace anything.
///
/// Only an item somebody is asking for that its service has carried nowhere, and only
/// where the record proving that was actually read. Anything else already has its
/// answer, and a search spent on it would cost the operator an indexer request for a
/// question that is not open.
pub(crate) fn unexplained(report: &TraceReport) -> bool {
    report.furthest == Stage::Monitored && report.stall.is_some()
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
