//! What the request gate refused and what removals it passed on, since the last time a
//! diagnosis looked.
//!
//! The gate writes every call it refuses and every removal it forwards to a record in
//! its configuration directory, with when, where and how, and never a token, a query
//! string or a body. This reads that record from the entry after the last one it read,
//! and keeps the place it reached, so each run reports only what is new.
//!
//! **A refusal warns and a removal does not.** A refused call is one the request service
//! has no use for, which is either a setting somebody changed or something running in it
//! that should not be. A removal is somebody's own action in the request service,
//! forwarded because they chose it there, so it is told rather than raised.

use std::path::PathBuf;
use std::sync::Arc;

use async_trait::async_trait;
use lemonfiber_sidecar::gate::{Entry, Outcome, Record};

use super::{Category, Check, Finding, Verdict};
use crate::error::codes::gate::{LOST, REFUSED};
use crate::error::{Problem, Remedy};
use crate::ports::filesystem::FileSystem;

/// The name this check and anything answering it share.
const CHECK: &str = "services.request-gate-record";

/// The heading an operator reads this under.
const TITLE: &str = "What the request gate refused and removed";

/// Where the record is, and where the last place read is kept.
pub struct Gate {
    /// The gate's record.
    pub record: PathBuf,
    /// The last entry read, where there is anywhere to keep it.
    pub read: Option<PathBuf>,
}

/// Reports what the request gate refused and removed since the last diagnosis.
pub struct GateRecordCheck {
    /// The filesystem the record and the place last read are read through.
    files: Arc<dyn FileSystem>,
    /// The gate, absent where the stack runs none.
    gate: Option<Gate>,
}

impl GateRecordCheck {
    /// A check over the gate given, read through `files`.
    #[must_use]
    pub fn new(files: Arc<dyn FileSystem>, gate: Option<Gate>) -> Self {
        Self { files, gate }
    }
}

#[async_trait]
impl Check for GateRecordCheck {
    fn category(&self) -> Category {
        Category::Services
    }

    async fn run(&self) -> Vec<Finding> {
        ran(self.files.as_ref(), self.gate.as_ref()).await
    }
}

/// What the record says since the last place read.
async fn ran(files: &dyn FileSystem, gate: Option<&Gate>) -> Vec<Finding> {
    let Some(gate) = gate else {
        return vec![finding(Verdict::Skipped {
            reason: "this stack has no request gate, so there is nothing it could have refused"
                .to_owned(),
        })];
    };
    let within = crate::within::directory_of(&gate.record);
    let Some(text) = crate::app::targets::read_owned(files, &gate.record, within).await else {
        return vec![finding(Verdict::Pass {
            note: Some("the request gate has refused nothing and removed nothing".to_owned()),
        })];
    };
    let record = match Record::read(&text) {
        Ok(record) => record,
        Err(unreadable) => {
            return vec![finding(Verdict::Unverified {
                reason: format!("the request gate's record could not be read: {unreadable}"),
                remedy: Remedy::new("Check the request gate is running")
                    .with_detail("lemonfiber status"),
            })]
        }
    };
    let reached = match &gate.read {
        Some(read) => files
            .read(read)
            .await
            .and_then(|text| text.trim().parse::<u64>().ok())
            .unwrap_or_default(),
        None => 0,
    };
    let (unread, lost) = record.since(reached);
    if let (Some(read), Some(newest)) = (&gate.read, unread.last()) {
        let _ = crate::config::store::write(read, &format!("{}\n", newest.seq));
    }
    findings(&unread, lost)
}

/// The findings for the entries read, and the count that fell off unread.
fn findings(unread: &[&Entry], lost: u64) -> Vec<Finding> {
    let refused: Vec<&Entry> = unread
        .iter()
        .copied()
        .filter(|entry| entry.outcome == Outcome::Refused)
        .collect();
    let removed: Vec<&Entry> = unread
        .iter()
        .copied()
        .filter(|entry| entry.outcome == Outcome::Removed)
        .collect();
    let mut found = Vec::new();
    if !refused.is_empty() {
        found.push(finding(Verdict::Warn(refusals(&refused))));
    }
    if !removed.is_empty() {
        found.push(finding(Verdict::Pass {
            note: Some(format!(
                "removed through the request service since the last check: {}",
                listed(&removed)
            )),
        }));
    }
    if lost > 0 {
        found.push(finding(Verdict::Warn(dropped(lost))));
    }
    if found.is_empty() {
        found.push(finding(Verdict::Pass {
            note: Some("nothing refused or removed since the last check".to_owned()),
        }));
    }
    found
}

/// The entries, each as when, how and where.
fn listed(entries: &[&Entry]) -> String {
    entries
        .iter()
        .map(|entry| {
            format!(
                "{} {} {}",
                crate::app::ctx::instant(i64::try_from(entry.at).unwrap_or(i64::MAX)),
                entry.method,
                entry.path
            )
        })
        .collect::<Vec<_>>()
        .join("; ")
}

/// The calls the gate refused.
fn refusals(refused: &[&Entry]) -> Problem {
    let calls = if refused.len() == 1 { "call" } else { "calls" };
    Problem::new(
        REFUSED,
        format!(
            "the request gate refused {} {calls} since the last check: {}",
            refused.len(),
            listed(refused)
        ),
        "Something reaching the request service asked a curator or the media server for more \
         than requests need, and the gate stopped it",
        Remedy::new(
            "The request service made calls it has no use for. If nobody changed its \
             settings, check what it is running.",
        ),
    )
}

/// The entries that fell off the record before they were read.
fn dropped(lost: u64) -> Problem {
    Problem::new(
        LOST,
        format!(
            "{lost} entries were dropped before they were read, because more than {} arrived \
             between checks",
            lemonfiber_sidecar::gate::Kept::standard().entries()
        ),
        "What those entries were cannot be told now",
        Remedy::new("Run the diagnosis more often while this keeps happening"),
    )
}

/// One finding of this check.
fn finding(verdict: Verdict) -> Finding {
    Finding::in_category(Category::Services, CHECK, TITLE, verdict)
}

#[cfg(test)]
mod tests;
