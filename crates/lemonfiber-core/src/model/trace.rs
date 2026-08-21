//! What following one item through the pipeline answers with.
//!
//! One of the report families the machine-readable contract is made of; they live in
//! separate files and are re-exported as one, so `crate::model::X` reads the same as it
//! always did.

use serde::Serialize;

/// One stage a traced item reached, named as the operator would read it: the stage,
/// the service that recorded it, and when.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct TraceStage {
    /// The stage reached.
    pub stage: crate::trace::Stage,
    /// The service that recorded it.
    pub service: String,
    /// When it happened, as the service reported it — absent for a stage inferred
    /// rather than timed, such as being monitored.
    pub at: Option<String>,
}

/// One moment in a traced item's history: what happened and when. Where [`TraceStage`]
/// is the linear progress, this is the log an \*arr kept — the grabs, the failed
/// downloads, the import and any later removal — so a repeated attempt is seen as the
/// pattern it is rather than flattened to a single furthest stage.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct TraceMoment {
    /// What happened.
    pub outcome: crate::trace::Outcome,
    /// When the service reported it.
    pub at: String,
}

/// Where one item is in the pipeline: how far it got, why it stopped if it did, and the
/// stages it passed through — the answer to "where is my show?".
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct TraceReport {
    /// The term the item was searched for by.
    pub item: String,
    /// Whether a monitored item matched the term at all — a false here is itself the
    /// answer: nobody asked for it.
    pub matched: bool,
    /// The furthest stage the item reached.
    pub furthest: crate::trace::Stage,
    /// Why it stopped, where it plainly has — or absent where it is progressing or done.
    pub stall: Option<String>,
    /// The stages it passed through, in order.
    pub stages: Vec<TraceStage>,
    /// The notable events in its history, oldest first — the grabs, failed downloads,
    /// imports and removals. Repeated attempts show here as the pattern they are, which
    /// the single furthest stage cannot.
    pub history: Vec<TraceMoment>,
    /// How much of the item is actually here, season by season — present for an item
    /// made of parts, absent for a film, which is the whole item and has none.
    ///
    /// The furthest stage alone cannot answer this: a series is "imported" the moment one
    /// episode lands, which reads as done while the rest are missing.
    pub coverage: Option<crate::trace::Coverage>,
    /// How sure the trace is of the item it followed.
    pub confidence: crate::trace::Confidence,
    /// Disagreements between the services about this item, each in plain language — a
    /// media server holding what no service is monitoring, and the like. Orthogonal to
    /// the linear pipeline: not where the item got to, but where two services' views of
    /// it contradict, surfaced rather than silently reconciled.
    pub findings: Vec<String>,
}
