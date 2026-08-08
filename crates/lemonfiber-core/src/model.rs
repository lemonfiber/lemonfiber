//! The values a surface renders.
//!
//! One set of types, serialised directly. `--json` and the web API are the same
//! values rather than two hand-maintained projections of them, which is what
//! makes the web API and the TUI's interface the same thing by construction —
//! and gives the machine-readable contract exactly one thing to version.

use serde::Serialize;

/// The machine-readable output contract's version.
///
/// Additive change leaves it alone, so a script asserting `== 1` keeps working
/// as features are added. Removing or retyping a field increments it.
pub const API_VERSION: u32 = 1;

/// The wrapper every machine-readable payload arrives in.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Envelope<T> {
    /// The output contract's version.
    pub api_version: u32,
    /// Which payload this is, so a consumer can branch before parsing `data`.
    pub kind: &'static str,
    /// The payload.
    pub data: T,
}

impl<T: Serialize> Envelope<T> {
    /// Render this payload as the machine-readable contract.
    ///
    /// Rendering lives here rather than in a surface so there is one
    /// implementation of the contract rather than one per surface, and so a
    /// surface needs no JSON library to satisfy it.
    ///
    /// `None` only if a payload cannot serialise, which for these types cannot
    /// happen — they are plain data with no maps keyed by anything unusual.
    #[must_use]
    pub fn to_json(&self) -> Option<String> {
        serde_json::to_string(self).ok()
    }
}

impl<T> Envelope<T> {
    /// Wrap a payload for machine-readable output.
    #[must_use]
    pub const fn new(kind: &'static str, data: T) -> Self {
        Self {
            api_version: API_VERSION,
            kind,
            data,
        }
    }
}

/// What versions are in play: the binary, and the stack it can operate.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct VersionReport {
    /// The running binary's version.
    pub binary: String,
    /// The manifest schema generations this build reads.
    pub supported_schema: Vec<u32>,
    /// The version of the stack this build operates.
    pub stack: String,
    /// What the container engine reports, when it could be asked.
    pub compose: Option<String>,
}

/// One setting, as it is safe to show.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct SettingReport {
    /// The setting's name.
    pub key: String,
    /// Its value, or a note that it is set and withheld.
    pub value: String,
    /// Whether the value was withheld.
    pub secret: bool,
}

/// The answer to a configuration command.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ConfigReport {
    /// The settings asked about — one for a lookup, all of them for a listing.
    pub settings: Vec<SettingReport>,
    /// Whether this command changed, or would change, a setting.
    pub changed: bool,
    /// Whether this was a rehearsal, so a change that `changed` reports was one
    /// that *would* be made rather than one that was.
    pub rehearsed: bool,
}

/// What a quality command did to the stored choice.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum Disposition {
    /// The choice was only shown; nothing was asked to change.
    #[default]
    Shown,
    /// The choice was recorded.
    Recorded,
    /// The choice would be recorded; this was a rehearsal, so it was not.
    Rehearsed,
    /// The choice needs transcoding this host cannot do well, so it was held
    /// rather than recorded without explicit confirmation.
    Held,
    /// The recorded preset was re-asserted over the Recyclarr config, overwriting
    /// a hand-edit where an ordinary run would have preserved it.
    Reapplied,
    /// A re-assert that was a rehearsal: it reports whether it would overwrite a
    /// hand-edit, and writes nothing.
    WouldReapply,
}

/// One preset in force, and what it means for the media it applies to — the
/// operator's question answered in their own terms, with no scoring vocabulary.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct PresetChoice {
    /// What this applies to: `everything`, or a specific media type.
    pub scope: String,
    /// The preset's plain-language name.
    pub preset: String,
    /// What it means, in the operator's terms rather than the tool's.
    pub means: String,
    /// The resolution and encode it targets.
    pub resolution: String,
    /// Roughly how much disk an hour of it takes.
    pub size_per_hour: String,
    /// What playback costs, in plain terms.
    pub transcoding: String,
    /// Whether this host would have to transcode it in software — the caution
    /// stated before a choice a household cannot smoothly play.
    pub needs_transcoding_here: bool,
}

/// One audio-format choice in force, for media that has no resolution — the same
/// question as a [`PresetChoice`], answered in format terms rather than resolution.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct MusicChoice {
    /// What this applies to — `music`.
    pub scope: String,
    /// The format's plain-language name.
    pub format: String,
    /// What it means, in the operator's terms.
    pub means: String,
    /// The audio format it targets, in plain terms.
    pub targets: String,
    /// Roughly how much disk an hour of it takes.
    pub size_per_hour: String,
    /// The practical caveat worth knowing — playing it, or finding it.
    pub note: String,
}

/// The operator's quality choice, what each preset means, and what the command
/// did with it.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct QualityReport {
    /// The global choice first, then each media type set apart from it.
    pub choices: Vec<PresetChoice>,
    /// The audio-format choice for music, where one is set — media that has no
    /// resolution, so it is reported apart from the resolution presets rather than
    /// forced into their shape.
    pub music: Option<MusicChoice>,
    /// Whether the Recyclarr config has been hand-edited since lemonfiber wrote it —
    /// the `customised` state, in which the preset is no longer authoritative until
    /// it is deliberately re-asserted. For a reapply, whether an edit was overwritten.
    pub customised: bool,
    /// What became of the choice.
    pub disposition: Disposition,
}

/// What choosing an audio format for music did: the choice, whether it was recorded
/// or only rehearsed, and — once recorded — what became of applying it to the music
/// service.
///
/// Music has no resolution and no community profile to lean on, so unlike a resolution
/// preset the choice is carried straight to the service through its API. The choice is
/// still recorded first, so it is remembered even when the service cannot be reached.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct MusicReport {
    /// The format chosen, what it means, and what it costs.
    pub choice: MusicChoice,
    /// Whether the choice was recorded, or only rehearsed.
    pub disposition: Disposition,
    /// What became of applying it to the music service, or `None` for a rehearsal
    /// that applied nothing.
    pub outcome: Option<Triggered>,
}

/// What became of asking one service to re-search its existing content.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "state", rename_all = "kebab-case")]
pub enum Triggered {
    /// The re-search was accepted and now runs in the service's background.
    Started,
    /// The service had not finished starting — no key yet — so nothing was asked of
    /// it; running the upgrade again once it is up will reach it.
    NotStarted,
    /// The service refused the command or could not be reached.
    Failed {
        /// The service's own account of why.
        detail: String,
    },
}

/// One media type an upgrade covers: its chosen quality, that quality's cost, and —
/// once confirmed — what became of asking its service to re-search.
///
/// Reported per media type rather than as one figure, because each type carries its
/// own preset and so its own cost: film at maximum and television at space-saving are
/// upgraded to different bars, and a single number would misstate one of them.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct UpgradeMedia {
    /// The media type — `tv` or `movies`.
    pub media_type: String,
    /// The preset in force for it.
    pub preset: String,
    /// Roughly what an hour of it costs at that preset.
    pub size_per_hour: String,
    /// What became of the re-search, or `None` where the upgrade was not confirmed
    /// and only the cost was stated.
    pub outcome: Option<Triggered>,
}

/// What upgrading existing content did, or — unconfirmed — would do.
///
/// Upgrading re-acquires the existing library at the chosen quality, which is a
/// large, bandwidth-expensive operation, so it is a separate explicit action whose
/// cost is stated before it runs and which does nothing until confirmed. Each *arr
/// re-searches against its own current cutoff, so the report speaks per media type
/// rather than asserting one preset across the library.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct UpgradeReport {
    /// Whether the operator confirmed; without it nothing was triggered, only the
    /// cost stated.
    pub confirmed: bool,
    /// Per media type: its preset, that preset's cost, and — confirmed — the outcome.
    pub media: Vec<UpgradeMedia>,
}

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

/// What a first-content walkthrough did — the whole of it, narrated line by line as it
/// happened and gathered here so the ending can be rendered, serialised and exited on.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct WalkthroughReport {
    /// Which walk this was.
    pub shape: crate::walkthrough::Shape,
    /// Where it ended up.
    pub state: crate::walkthrough::State,
    /// What it set out to prove, said so the operator knows what they watched.
    pub proves: String,
    /// What it walked, where it got as far as choosing something.
    pub item: Option<String>,
    /// Every line it said, in order — the same lines the operator watched arrive, kept so
    /// a machine-readable run is not a silent one.
    pub lines: Vec<crate::walkthrough::Line>,
    /// Where and why it stopped, where it did.
    pub stopped: Option<crate::walkthrough::Stopped>,
    /// What the import did with the file, where it got that far.
    pub link: Option<crate::walkthrough::Link>,
    /// Where it leaves the operator, where it worked.
    pub handover: Option<crate::walkthrough::Handover>,
    /// What could have been walked instead, where nothing was chosen — the safe first
    /// attempts, so an operator with an empty library is not left guessing.
    pub suggestions: Vec<String>,
    /// Whether the download was handed to the background rather than waited out.
    pub in_background: bool,
    /// Whether what was asked for was already here, and so was not acquired again.
    pub already_here: bool,
}

/// One thing a household member asked for, and where it stands in their words.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct MemberRequest {
    /// What it is called, where the service filing it has been told about it and its
    /// library could be read. Absent for a request no service holds yet — one still
    /// awaiting approval has been handed to nobody, so there is no title to find.
    pub title: Option<String>,
    /// What kind of thing it is — a series, a film — in the household's own words.
    /// Absent where the request service names a kind this build does not know.
    pub media: Option<String>,
    /// Where the request stands, or absent where the request service reports a status
    /// this build does not know rather than guessing it into the nearest word.
    pub state: Option<crate::household::State>,
}

/// One household member and everything they have asked for.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct HouseholdMember {
    /// The member, by the name the request service shows them under.
    pub name: String,
    /// What they asked for, newest first.
    pub requests: Vec<MemberRequest>,
}

/// What the household has asked for, member by member — the simplified view of the same
/// pipeline a trace reports in the services' own terms.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct HouseholdReport {
    /// The members who have asked for something, in name order.
    pub members: Vec<HouseholdMember>,
    /// Whether the requests were read at all. A false here is why the list is empty, and
    /// keeps an unread record from being mistaken for a household that has asked for
    /// nothing — the same honesty a trace keeps about a silence it did not hear.
    pub available: bool,
    /// What could not be read, and anything else worth the operator's attention.
    pub findings: Vec<String>,
}

/// One stuck item queue health found, named so it links straight to its own trace.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct StuckEntry {
    /// The item's title — the term a `trace` searches by.
    pub title: String,
    /// The \*arr whose queue is holding it.
    pub service: String,
    /// The stage its download is stuck at.
    pub stage: crate::trace::Stage,
}

/// The items whose downloads are stuck, across the \*arrs — the landing point for "N
/// items stuck" that queue health reports, each entry naming the item so the operator
/// goes straight to its per-item trace rather than to a count to investigate.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct StuckReport {
    /// The stuck items, each linkable to its trace.
    pub items: Vec<StuckEntry>,
    /// Whether an \*arr's queue could not be read, so the list may be short — reported
    /// rather than read as "nothing stuck", the same honesty a trace keeps.
    pub incomplete: bool,
}

/// What a lifecycle command did, or would have done.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct LifecycleReport {
    /// The Compose subcommand that was run.
    pub action: String,
    /// The profiles that were activated.
    pub profiles: Vec<String>,
    /// Profiles the forms asked for that the configuration does not support.
    ///
    /// Reported rather than dropped quietly: an operator seeing fewer services
    /// than they expected needs to be told which, and why, before they go
    /// looking for a fault that is not there.
    pub dropped: Vec<String>,
    /// The exact command, so what happened is never a matter of trust.
    pub command: Vec<String>,
    /// Whether this was a rehearsal.
    pub rehearsed: bool,
    /// The exit status, absent for a rehearsal or a signalled process.
    pub status: Option<i32>,
    /// What each service ended up doing, where the action waited to find out.
    ///
    /// Empty for actions that do not wait. Stopping is finished when Compose
    /// says it is, and surveying afterwards would only report the absence it
    /// was asked to produce.
    pub services: Vec<crate::docker::Service>,
    /// What those services amount to, as one word.
    pub condition: Option<crate::docker::Condition>,
    /// Stack files the operator has edited, left as they set them rather than
    /// overwritten with lemonfiber's own. Empty in the ordinary case; a named entry
    /// warns that an upgrade would change a file they changed, and shows the diff.
    pub stack_edits: Vec<StackEdit>,
}

/// A stack file the operator edited, preserved rather than overwritten, with the
/// change an upgrade would make shown against it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct StackEdit {
    /// The file's path within the stack directory.
    pub path: String,
    /// The lines that differ between the operator's file and what lemonfiber would
    /// write — theirs marked `-`, lemonfiber's `+`, the matching head and tail left
    /// out. Empty where the two differ only in ways `lines` does not see.
    pub diff: String,
}

/// What a full reset did, or — until it is confirmed — would do: the operator edits it
/// reverts back to lemonfiber's own state, and whether it was carried out or only shown.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct ResetReport {
    /// The operator's edits that were reverted — or, unconfirmed, that a reset would
    /// revert — each with the diff of what is lost against what lemonfiber restores.
    pub reverted: Vec<StackEdit>,
    /// The service connections whose drifted value was reverted to lemonfiber's — or,
    /// unconfirmed, would be — each named as it reads in a seed report.
    pub reverted_connections: Vec<String>,
    /// Whether the reset was carried out, or only previewed pending confirmation.
    pub confirmed: bool,
}

/// What a diagnostic run found, and what it amounts to.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct DoctorReport {
    /// What the findings amount to, as one word.
    pub overall: crate::doctor::Overall,
    /// Each finding, in the order the checks produced them.
    pub findings: Vec<crate::doctor::Finding>,
}

/// What a watch saw, once the data root it was guarding was lost.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct SupervisionReport {
    /// The forms that were being watched, and are now stopped.
    pub forms: Vec<String>,
    /// Why the watch ended: the data root vanished, or a different volume took
    /// its place.
    pub reason: String,
    /// Whether stopping the services succeeded.
    pub stopped: bool,
}

/// What each service is doing, and what that adds up to.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct StatusReport {
    /// The forms asked about; empty means the whole stack was.
    pub forms: Vec<String>,
    /// What the services amount to, as one word.
    pub condition: crate::docker::Condition,
    /// Each service, worst first.
    pub services: Vec<crate::docker::Service>,
}

#[cfg(test)]
mod tests {
    use super::{Envelope, VersionReport, API_VERSION};

    /// These are plain data, so serialising cannot fail; an empty string on the
    /// impossible branch keeps the helper free of a line no test can cover.
    fn json<T: serde::Serialize>(envelope: &Envelope<T>) -> String {
        envelope.to_json().unwrap_or_default()
    }

    #[test]
    fn every_payload_carries_the_contract_version() {
        let envelope = Envelope::new("version", 7_u32);
        assert_eq!(envelope.api_version, API_VERSION);
        assert_eq!(
            json(&envelope),
            r#"{"api_version":1,"kind":"version","data":7}"#
        );
    }

    #[test]
    fn a_version_report_serialises_field_for_field() {
        let report = VersionReport {
            binary: "0.1.0".to_owned(),
            supported_schema: vec![1],
            stack: "0.1.0".to_owned(),
            compose: Some("Docker Compose version v2.32.1".to_owned()),
        };
        assert_eq!(
            json(&Envelope::new("version", report)),
            concat!(
                r#"{"api_version":1,"kind":"version","data":{"binary":"0.1.0","#,
                r#""supported_schema":[1],"stack":"0.1.0","#,
                r#""compose":"Docker Compose version v2.32.1"}}"#
            )
        );
    }

    #[test]
    fn an_unreachable_engine_is_absent_rather_than_guessed_at() {
        let report = VersionReport {
            binary: "0.1.0".to_owned(),
            supported_schema: vec![1],
            stack: "0.1.0".to_owned(),
            compose: None,
        };
        assert!(json(&Envelope::new("version", report)).contains(r#""compose":null"#));
    }
}
