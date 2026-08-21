//! What each service holds about one item.
//!
//! The fragments a trace correlates: what the \*arr records, what the media server can
//! play, and — where the trail goes cold at an absence — what the accounts underneath have
//! left. Reading only; nothing here decides what the fragments mean.

use std::sync::Arc;

use crate::app::Ctx;
use crate::doctor::providers::ProvidersCheck;
use crate::doctor::{Check, Finding, Verdict};
use crate::model::TraceReport;
use crate::ports::service::{Indexers, ItemPart, Library, QueueItem, TraceEvent, UsenetAccounts};
use crate::recyclarr::Kind;
use crate::trace::{Presence, Stage};

/// The stall an account underneath the stack could explain, where this trace has one.
///
/// Only two stages it could: nothing found, and nothing taken. An indexer with its
/// allowance spent answers every search with an empty list, and an account that is
/// refusing or empty takes nothing it is handed — and both leave a \*arr holding an
/// absence it cannot explain, which is exactly what those two stalls are.
///
/// Deliberately not the stage where releases were found and none was good enough: that is
/// a preset asking for what the indexers do not carry, and sending its operator to look at
/// their subscriptions would be the wrong half of the answer.
pub(crate) fn account_explainable(report: &TraceReport) -> Option<String> {
    matches!(report.furthest, Stage::Monitored | Stage::Grabbed)
        .then(|| report.stall.clone())
        .flatten()
}

/// What the accounts underneath the stack amount to right now.
///
/// The provider check answers, rather than a second reading of the same services: it is
/// already the judgment, and two paths to one verdict is one way for them to disagree.
/// Reading it costs the providers nothing — every figure in it comes from the services
/// that have been using the accounts.
pub(crate) async fn providers(
    ctx: &Ctx,
    services: &[lemonfiber_manifest::Service],
) -> Vec<Finding> {
    let project =
        crate::app::targets::project_directory(&ctx.stack, ctx.settings.stack_dir.as_deref());
    ProvidersCheck::new(
        crate::app::targets::usenet_client(ctx, services, project.as_deref())
            .await
            .map(|client| Arc::new(client) as Arc<dyn UsenetAccounts>),
        crate::app::targets::indexer_aggregator(ctx, services, project.as_deref())
            .await
            .map(|aggregator| Arc::new(aggregator) as Arc<dyn Indexers>),
        ctx.today(),
        ctx.clock.now(),
    )
    .run()
    .await
}

/// The findings an operator would have to act on, in the check's own words.
///
/// Only those: an account that is fine, or one nothing could be read from, explains
/// nothing about why an item stopped, and saying so beside a stall would bury the reason
/// it is there to sharpen.
pub(crate) fn troubles(findings: Vec<Finding>) -> Vec<String> {
    findings
        .into_iter()
        .filter_map(|finding| match finding.verdict {
            Verdict::Fail(problem) | Verdict::Warn(problem) => {
                Some(format!("{} — {}", finding.title, problem.summary))
            }
            Verdict::Pass { .. } | Verdict::Skipped { .. } | Verdict::Unverified { .. } => None,
        })
        .collect()
}

/// The stall with what the accounts said beside it, where they said anything.
///
/// Beside rather than instead: how far the item got is still the answer to the question
/// that was asked, and the account is why it got no further. Where the accounts are all
/// well, or could not be read, the reason reads exactly as it did before.
pub(crate) fn beside(reason: String, said: &[String]) -> String {
    if said.is_empty() {
        return reason;
    }
    format!("{reason} ({})", said.join("; "))
}

/// Ask the media server whether the item is in the library, as the three-way answer a
/// trace folds in: present, provably absent, or — where there is no media server to ask,
/// or it will not answer — unknown, which the trace reads as "cannot tell" rather than
/// inferring an availability it has not confirmed.
pub(crate) async fn library_presence(
    jellyfin: Option<&crate::jellyfin::Jellyfin>,
    kind: Kind,
    title: &str,
) -> Option<Presence> {
    match jellyfin?.has_item(kind, title).await {
        Ok(true) => Some(Presence::Present),
        Ok(false) => Some(Presence::Absent),
        Err(_) => None,
    }
}

/// What the services could tell about the item: the stage-advancing events, what the queue
/// holds now, whether the media server has it — and, for each read that can fail, whether
/// it was actually read. An unreadable fragment is not an empty one, so the trace can tell
/// "nothing happened" apart from "this could not be read".
pub(crate) struct Fragments {
    pub(crate) events: Vec<TraceEvent>,
    pub(crate) queue: Vec<QueueItem>,
    pub(crate) parts: Vec<ItemPart>,
    pub(crate) library: Option<Presence>,
    pub(crate) reads: Reads,
}

/// Which of the fragments the services actually answered with. They travel together
/// because they mean one thing — what the trace is entitled to conclude from a silence —
/// and because a run of loose booleans is the shape a caller transposes without noticing.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Reads {
    pub(crate) history: bool,
    pub(crate) queue: bool,
    pub(crate) parts: bool,
}

#[cfg(test)]
impl Reads {
    /// Every fragment answered — the ordinary case.
    pub(crate) const ALL: Self = Self {
        history: true,
        queue: true,
        parts: true,
    };
}
