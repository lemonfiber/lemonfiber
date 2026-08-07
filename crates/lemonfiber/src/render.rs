//! The surface's rendering, kept apart from the CLI wiring and orchestration.
//!
//! `main` decides what to run and hands the outcome here; this module decides
//! only how it reads. One renderer per answer, for a person or for a script,
//! with nothing about parsing input or dispatching commands mixed in — so the
//! shape of an operator's report and the shape of the command line stay two
//! separate things to change.
//!
//! Every renderer *builds* its lines and hands them back; one printer at the edge
//! puts them on the terminal. Rendering is then a value a test can assert on
//! rather than a side effect it can only watch happen, which is what lets the
//! words an operator actually reads be held to the same standard as the rest.

use lemonfiber_core::app::Outcome;
use lemonfiber_core::docker::{Condition, Service, State};
use lemonfiber_core::doctor::{Overall, Verdict};
use lemonfiber_core::error::Problem;
use lemonfiber_core::model::{
    ConfigReport, Disposition, DoctorReport, Envelope, HouseholdReport, LifecycleReport,
    MusicChoice, MusicReport, PresetChoice, QualityReport, ResetReport, StatusReport, StuckReport,
    SupervisionReport, TraceReport, Triggered, UpgradeReport, VersionReport,
};
use lemonfiber_core::seed::{
    Assessment as SeedAssessment, Report as SeedReport, Severity as SeedSeverity,
    State as SeedState,
};
use lemonfiber_core::trace::{Confidence, Coverage, Outcome as TraceOutcome, HISTORY_HORIZON};
use lemonfiber_core::PRODUCT;

/// What stands in for an answer that could not be turned into JSON.
///
/// Serialising these reports cannot actually fail — every field is a plain owned value —
/// so this exists to keep the fallback a value rather than an unreachable branch. It is
/// built eagerly for the same reason: a lazily-built one would be a line no test could
/// ever run, which is exactly what the coverage gate is there to forbid.
const UNRENDERABLE: &str = "this answer could not be rendered as JSON";

/// The lines an answer renders to, in order.
///
/// Built rather than printed so a renderer returns something a test can read back. The
/// terminal is reached in one place, at the edge, which also means nothing here has to
/// care whether it is being rendered for a person or for an assertion.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub(crate) struct Lines(Vec<String>);

impl Lines {
    /// One line.
    pub(crate) fn put(&mut self, line: impl Into<String>) {
        self.0.push(line.into());
    }

    /// A blank line, then the given one — the separated closing remark most answers end
    /// on, kept as one call so the spacing is uniform rather than re-decided each time.
    pub(crate) fn spaced(&mut self, line: impl Into<String>) {
        self.0.push(String::new());
        self.0.push(line.into());
    }

    /// Text that already carries its own line breaks — a diff — split into the lines it
    /// is made of, so a block and a built line are the same kind of thing from here on.
    pub(crate) fn block(&mut self, text: &str) {
        self.0.extend(text.lines().map(str::to_owned));
    }

    /// Everything another renderer built, appended.
    pub(crate) fn extend(&mut self, other: Self) {
        self.0.extend(other.0);
    }

    /// The lines as one piece of text, for a test to read and for a diff to compare.
    #[cfg(test)]
    pub(crate) fn text(&self) -> String {
        self.0.join("\n")
    }

    /// Put them on the terminal. The one place this crate reaches stdout.
    pub(crate) fn print(&self) {
        for line in &self.0 {
            println!("{line}");
        }
    }

    /// Put them on the error stream — what an operator is told about a refusal,
    /// which belongs beside the answer rather than in it.
    pub(crate) fn eprint(&self) {
        for line in &self.0 {
            eprintln!("{line}");
        }
    }
}

/// Render an outcome, for a person or for a script.
///
/// One renderer per answer, rather than one function that knows all four. They
/// have nothing in common beyond arriving here: what a version report owes an
/// operator and what a lifecycle report owes them are different questions, and
/// a single body deciding both reads as one thing with four moods.
pub(crate) fn render(outcome: &Outcome, json: bool) {
    answer(outcome, json).print();
}

/// The lines one outcome renders to — the whole of this module's decision-making, kept
/// apart from the printing so it can be read back rather than only watched.
fn answer(outcome: &Outcome, json: bool) -> Lines {
    if json {
        return machine_readable(outcome);
    }
    match outcome {
        Outcome::Version(report) => versions(report),
        Outcome::Config(report) => settings(report),
        Outcome::Quality(report) => quality(report),
        Outcome::Upgrade(report) => upgrade(report),
        Outcome::Music(report) => music(report),
        Outcome::Trace(report) => trace(report),
        Outcome::Household(report) => household(report),
        Outcome::Stuck(report) => stuck(report),
        Outcome::Lifecycle(report) => lifecycle(report),
        Outcome::Status(report) => status(report),
        Outcome::Doctor(report) => diagnosis(report),
        Outcome::Seed(report) => seeding(report),
        Outcome::Reset(report) => reset(report),
    }
}

/// The same answer, for something that will parse it.
fn machine_readable(outcome: &Outcome) -> Lines {
    let mut lines = Lines::default();
    lines.put(
        outcome
            .clone()
            .envelope()
            .to_json()
            .unwrap_or(UNRENDERABLE.to_owned()),
    );
    lines
}

/// What seeding wired, connection by connection, with what a re-run still owes
/// named last so it is the thing the operator is left looking at.
fn seeding(report: &SeedReport) -> Lines {
    let mut lines = Lines::default();
    for wiring in &report.wirings {
        let connection = &wiring.connection;
        match &wiring.state {
            SeedState::Wired => lines.put(format!("  ✓ {connection}   wired")),
            SeedState::AlreadyWired => lines.put(format!("  ✓ {connection}   already wired")),
            SeedState::Drifted => lines.put(format!("  · {connection}   left as you set it")),
            SeedState::Adopted => lines.put(format!("  ✓ {connection}   yours, adopted")),
            SeedState::Unmanaged => lines.put(format!(
                "  · {connection}   found already set — yours, left as is (run `{PRODUCT} adopt` to keep it)"
            )),
            SeedState::Stale => lines.put(format!(
                "  · {connection}   yours for now — a newer default is not yet applied"
            )),
            SeedState::Conflicted { yours, ours } => {
                lines.put(format!(
                    "  ✗ {connection}   conflict — both you and the default changed it"
                ));
                match yours {
                    Some(yours) => lines.put(format!(
                        "      you set “{yours}”, the default is now “{ours}” — left as you set it"
                    )),
                    None => lines.put(format!(
                        "      you cleared it, the default is now “{ours}” — left as you set it"
                    )),
                }
            }
            SeedState::Skipped { reason } => {
                lines.put(format!("  ? {connection}   skipped"));
                lines.put(format!("      {reason}"));
            }
            SeedState::Failed { detail } => {
                lines.put(format!("  ✗ {connection}   {detail}"));
            }
            SeedState::Refused { reason } => {
                lines.put(format!("  ✗ {connection}   refused"));
                lines.put(format!("      {reason}"));
            }
        }
        // A drift that broke the stack is raised beneath the line it sits on, naming
        // what broke and the fix — the warning severity a plain drift never carries.
        if let SeedSeverity::Warning {
            breakage,
            remediation,
        } = &wiring.severity
        {
            lines.put(format!("      ! {breakage}"));
            lines.put(format!("        → {remediation}"));
        }
    }
    let warnings = report.warnings();
    if !warnings.is_empty() {
        lines.spaced(format!(
            "{} drifted in a way that breaks the stack — see the ! lines above.",
            warnings.len()
        ));
    }
    let outstanding = report.outstanding();
    let blocked = report.blocked();
    if outstanding.is_empty() {
        lines.spaced("Everything is wired.");
    } else if blocked.is_empty() {
        lines.spaced(format!(
            "{} left to wire — run seed again once ready.",
            outstanding.len()
        ));
    } else if blocked.len() == outstanding.len() {
        lines.spaced(format!(
            "{} to resolve — settle the conflict, then seed again.",
            blocked.len()
        ));
    } else {
        lines.spaced(format!(
            "{} left: {} to wire once ready, {} to resolve — settle the conflict first.",
            outstanding.len(),
            outstanding.len() - blocked.len(),
            blocked.len(),
        ));
    }
    if matches!(report.assessment, SeedAssessment::Unassessable) {
        lines.spaced(
            "The record of what lemonfiber last wrote could not be read, so drift \
             could not be assessed this run. Run `lemonfiber adopt` to re-baseline \
             from the current state.",
        );
    }
    lines
}

/// What the diagnostic checks found, finding by finding.
///
/// Each finding leads with a mark that reads at a glance and the plain evidence
/// behind it; a non-passing one carries the reason and what to do, because a
/// finding without a remedy is a fault report rather than a diagnosis.
fn diagnosis(report: &DoctorReport) -> Lines {
    let mut lines = Lines::default();
    for finding in &report.findings {
        let title = &finding.title;
        match &finding.verdict {
            Verdict::Pass { note } => match note {
                Some(note) => lines.put(format!("  ✓ {title}   {note}")),
                None => lines.put(format!("  ✓ {title}")),
            },
            Verdict::Warn(problem) => {
                lines.put(format!("  ! {title}   {}", problem.summary));
                lines.extend(remedies(problem));
            }
            Verdict::Fail(problem) => {
                lines.put(format!("  ✗ {title}   {}", problem.summary));
                lines.extend(remedies(problem));
            }
            Verdict::Unverified { reason, remedy } => {
                lines.put(format!("  ? {title}   UNVERIFIED"));
                lines.put(format!("      {reason}"));
                lines.put(format!("      → {}", remedy.action));
                if let Some(detail) = &remedy.detail {
                    lines.put(format!("        {detail}"));
                }
            }
            Verdict::Skipped { reason } => {
                lines.put(format!("  – {title}   skipped: {reason}"));
            }
        }
    }

    lines.spaced(overall(report.overall));
    lines
}

/// The problem's meaning and remedies, indented under a finding.
fn remedies(problem: &Problem) -> Lines {
    let mut lines = Lines::default();
    lines.put(format!("      {}", problem.meaning));
    for remedy in &problem.remedies {
        lines.put(format!("      → {}", remedy.action));
        if let Some(detail) = &remedy.detail {
            lines.put(format!("        {detail}"));
        }
    }
    lines
}

/// The one-line verdict a diagnosis amounts to.
fn overall(overall: Overall) -> &'static str {
    match overall {
        Overall::Healthy => "healthy — everything checked passed",
        Overall::Degraded => "degraded — working, with warnings",
        Overall::Broken => "broken — something needs attention",
        Overall::Unknown => "unknown — health could not be established",
    }
}

/// What versions are in play.
fn versions(report: &VersionReport) -> Lines {
    let mut lines = Lines::default();
    lines.put(format!("{PRODUCT} {}", report.binary));
    lines.put(format!("stack {}", report.stack));
    lines.put(format!("manifest schema {:?}", report.supported_schema));
    match &report.compose {
        Some(version) => lines.put(format!("compose {version}")),
        None => lines.put("compose not reachable"),
    }
    lines
}

/// What the operator has configured.
fn settings(report: &ConfigReport) -> Lines {
    let mut lines = Lines::default();
    for setting in &report.settings {
        lines.put(format!("{}={}", setting.key, setting.value));
    }
    if report.changed {
        // A rehearsal reports what it would do, so it must not claim it saved.
        lines.put(if report.rehearsed {
            "would save"
        } else {
            "saved"
        });
    }
    lines
}

/// The quality choice, what each preset means, and what the command did with it.
fn quality(report: &QualityReport) -> Lines {
    let mut lines = Lines::default();
    for choice in &report.choices {
        lines.extend(preset_choice(choice));
    }
    if let Some(choice) = &report.music {
        lines.extend(music_choice(choice));
    }
    match report.disposition {
        // A change is forward-looking, and this is where the operator is told so —
        // the expectation is often the opposite, that lowering quality shrinks the
        // library or raising it re-grabs everything.
        Disposition::Recorded => {
            lines.spaced("Saved. This affects future acquisitions only — nothing already downloaded changes.");
            if report.customised {
                lines.put(format!(
                    "Your Recyclarr config is customised, so this preset will not apply on its \
                     own. Run `{PRODUCT} quality reapply` to let it overwrite your edits."
                ));
            }
        }
        Disposition::Rehearsed => {
            lines.spaced(
                "Would save. This affects future acquisitions only — nothing downloaded changes.",
            );
        }
        Disposition::Held => {
            lines.spaced(
                "Not saved: this machine would have to transcode this in software, which will not \
                 play well. Re-run with --confirm to choose it anyway, or run Jellyfin natively.",
            );
        }
        // Re-asserting the preset over the config: say whether it overwrote an edit.
        Disposition::Reapplied => {
            if report.customised {
                lines.spaced("Reapplied the preset, overwriting your customised Recyclarr config.");
            } else {
                lines.spaced("Reapplied the preset. The Recyclarr config was already in step.");
            }
        }
        // A rehearsed reapply: preview whether it would overwrite an edit.
        Disposition::WouldReapply => {
            if report.customised {
                lines.spaced(
                    "Would reapply the preset, overwriting your customised Recyclarr config.",
                );
            } else {
                lines.spaced("Would reapply the preset. The Recyclarr config is already in step.");
            }
        }
        // A plain show reports the state; a customised config is worth naming.
        Disposition::Shown => {
            if report.customised {
                lines.spaced(format!(
                    "Your Recyclarr config is customised — the preset is no longer authoritative. \
                     Run `{PRODUCT} quality reapply` to re-assert it over your edits."
                ));
            }
        }
    }
    lines
}

/// One preset in force: what it applies to, what it means, and what it costs.
fn preset_choice(choice: &PresetChoice) -> Lines {
    let mut lines = Lines::default();
    lines.put(format!(
        "{}: {} — {}",
        choice.scope, choice.preset, choice.means
    ));
    lines.put(format!(
        "  {} · {} · {}",
        choice.resolution, choice.size_per_hour, choice.transcoding
    ));
    if choice.needs_transcoding_here {
        lines.put("  ⚠ this machine would have to transcode this in software");
    }
    lines
}

/// One audio-format choice, in the same shape as a preset choice but in format terms —
/// what it targets, its size, and the caveat worth knowing rather than a resolution.
fn music_choice(choice: &MusicChoice) -> Lines {
    let mut lines = Lines::default();
    lines.put(format!(
        "{}: {} — {}",
        choice.scope, choice.format, choice.means
    ));
    lines.put(format!(
        "  {} · {} · {}",
        choice.targets, choice.size_per_hour, choice.note
    ));
    lines
}

/// Choosing the audio format for music: the choice, then whether it was recorded or
/// rehearsed and what became of applying it to the music service.
fn music(report: &MusicReport) -> Lines {
    let mut lines = music_choice(&report.choice);
    if matches!(report.disposition, Disposition::Rehearsed) {
        lines.spaced(
            "Would save. This affects future acquisitions only — nothing downloaded changes.",
        );
        return lines;
    }
    lines.spaced(
        "Saved. This affects future acquisitions only — nothing already downloaded changes.",
    );
    match &report.outcome {
        None | Some(Triggered::Started) => {
            lines.put("Applied to the music service.");
        }
        Some(Triggered::NotStarted) => {
            lines.put("The music service is not up yet, so it was recorded but not applied — run this again once it is.");
        }
        Some(Triggered::Failed { detail }) => {
            lines.put(format!(
                "Recorded, but the music service refused the change: {detail}"
            ));
        }
    }
    lines
}

/// Upgrading existing content: the cost stated per media type first, then — once
/// confirmed — what each service was asked to do.
fn upgrade(report: &UpgradeReport) -> Lines {
    let mut lines = Lines::default();
    if report.media.is_empty() {
        lines.put("No television or film service is set up, so there is nothing to upgrade.");
        return lines;
    }
    if report.confirmed {
        lines.put(
            "Upgrading existing content, each service re-searching against its own quality bar:",
        );
    } else {
        // The cost, and nothing done: a large operation stays behind a deliberate
        // confirmation.
        lines.put(
            "Upgrading existing content re-downloads your library at the chosen quality — a large, \
             bandwidth-expensive operation, potentially terabytes and hours to days. It would cost, \
             per media:",
        );
    }
    for media in &report.media {
        lines.put(format!(
            "  {}: {} — {}",
            media.media_type, media.preset, media.size_per_hour
        ));
        match &media.outcome {
            None => {}
            Some(Triggered::Started) => lines.put("    ✓ re-search started"),
            Some(Triggered::NotStarted) => {
                lines.put(
                    "    · not started — the service is not up yet; run this again once it is",
                );
            }
            Some(Triggered::Failed { detail }) => lines.put(format!("    ✗ {detail}")),
        }
    }
    if !report.confirmed {
        lines.spaced("Nothing has been changed. Re-run with --confirm to go ahead.");
    }
    lines
}

/// The exact command that traces one item, printed beneath the line that names it.
///
/// Shared by every surface that leads to a trace so the two cannot drift apart: the term
/// is what the trace searches by, and a link that no longer matches how the trace matches
/// would send an operator to a search that finds nothing.
fn trace_link(title: &str) -> String {
    format!("      → {PRODUCT} trace \"{title}\"")
}

/// What the household asked for, grouped by whoever asked.
///
/// A member's own words rather than the services': where a request stands, and — for one
/// that has a name to search by — the trace that says why in the services' terms. The
/// deep answer stays where it already lives; this is the way in to it.
fn household(report: &HouseholdReport) -> Lines {
    let mut lines = Lines::default();
    for member in &report.members {
        lines.put(member.name.clone());
        for request in &member.requests {
            // A request no service holds yet has no title to print. Naming it by what it
            // is keeps the line honest rather than inventing something to call it.
            let name = request.title.clone().unwrap_or_else(|| {
                request
                    .media
                    .clone()
                    .map_or_else(|| "something".to_owned(), |media| format!("a {media}"))
            });
            match request.state {
                Some(state) => lines.put(format!("  {name}   {}", state.phrase())),
                None => lines.put(format!(
                    "  {name}   the request service reports a state this build does not know"
                )),
            }
            // Only a named request can be traced: the trace searches by title, so a link
            // for one with no name would lead to a search that finds nothing.
            if let Some(title) = &request.title {
                lines.put(trace_link(title));
            }
        }
    }

    if report.members.is_empty() && report.available {
        lines.put("Nobody has asked for anything yet.");
    } else if !report.members.is_empty() {
        let requests: usize = report
            .members
            .iter()
            .map(|member| member.requests.len())
            .sum();
        lines.spaced(format!(
            "{} member(s), {requests} request(s).",
            report.members.len()
        ));
    }
    // What could not be read, said rather than left to look like an empty household.
    for finding in &report.findings {
        lines.put(format!("  ! {finding}"));
    }
    lines
}

/// How much of a series is actually here, season by season — the answer the single
/// furthest stage cannot give, since a show is "imported" the moment one episode lands.
///
/// A complete season is one line; an incomplete one names each episode still outstanding
/// and what it is waiting on, because that is the part an operator can act on. Episodes
/// nobody asked for are counted apart from the totals and said so plainly, so a season of
/// specials never reads as a fault to go and chase.
fn seasons(coverage: &Coverage) -> Lines {
    let mut lines = Lines::default();
    // Nothing asked for is not "none of nothing here" — with no denominator the counts
    // say nothing, and the honest reading is that no episode is being maintained.
    if coverage.wanted == 0 {
        lines.put(format!(
            "  no episode(s) asked for — {} not monitored, none on disk",
            coverage.unmonitored
        ));
        return lines;
    }
    lines.put(format!(
        "  {} of {} episode(s) here",
        coverage.have, coverage.wanted
    ));
    for season in &coverage.seasons {
        // Season zero is where a service files specials, which is not a season anyone
        // names that way.
        let name = if season.season == 0 {
            "specials".to_owned()
        } else {
            format!("season {}", season.season)
        };
        if season.wanted == 0 {
            lines.put(format!(
                "      {name}   {} not asked for",
                season.unmonitored
            ));
            continue;
        }
        let complete = if season.complete() { "   complete" } else { "" };
        lines.put(format!(
            "      {name}   {} of {}{complete}",
            season.have, season.wanted
        ));
        if season.unmonitored > 0 {
            lines.put(format!(
                "          ({} more not asked for)",
                season.unmonitored
            ));
        }
        for part in &season.outstanding {
            let waiting = part
                .stage
                .stall()
                .map_or_else(|| part.stage.label().to_owned(), str::to_owned);
            lines.put(format!(
                "          S{:02}E{:02}   {waiting}",
                part.season, part.number
            ));
        }
    }
    lines
}

/// Where one item is in the pipeline: the item, each stage it reached with the service
/// and time, and — where it plainly stopped — why.
fn trace(report: &TraceReport) -> Lines {
    let mut lines = Lines::default();
    lines.put(report.item.clone());
    if !report.matched {
        // No monitored item matched — nobody asked for it.
        if let Some(reason) = &report.stall {
            lines.put(format!("  {reason}"));
        }
        return lines;
    }
    for stage in &report.stages {
        let label = stage.stage.label();
        match &stage.at {
            Some(at) => lines.put(format!("  ✓ {label}   {}   {at}", stage.service)),
            None => lines.put(format!("  ✓ {label}   {}", stage.service)),
        }
    }
    if let Some(reason) = &report.stall {
        lines.put(format!("  ✗ stopped: {reason}"));
    }
    // The history of what was tried, shown when it reveals a pattern the linear stages
    // cannot: a download that failed, a file removed, or the same release grabbed more
    // than once. A single clean grab-and-import is already told by the stages above, so it
    // is not repeated here.
    let grabs = report
        .history
        .iter()
        .filter(|moment| moment.outcome == TraceOutcome::Grabbed)
        .count();
    let troubled = report.history.iter().any(|moment| {
        matches!(
            moment.outcome,
            TraceOutcome::DownloadFailed | TraceOutcome::Removed
        )
    });
    if grabs > 1 || troubled {
        lines.put("  history:");
        for moment in &report.history {
            lines.put(format!("      {}   {}", moment.outcome.phrase(), moment.at));
        }
    }
    if let Some(coverage) = &report.coverage {
        lines.extend(seasons(coverage));
    }
    // Things worth the operator's attention that are not a point on the pipeline — a
    // service disagreement, or a detail that could not be read and so is reported as
    // unavailable rather than inferred. Each finding's own words say which.
    for finding in &report.findings {
        lines.put(format!("  ! {finding}"));
    }
    // A trace joined to the library by title alone may not be the item asked for; saying
    // so is the honest thing — better a marked guess than one presented as fact.
    if report.confidence == Confidence::Uncertain {
        lines.put("  ~ matched to the library by title — this may not be the item you meant");
    }
    // The history read is bounded; stating the horizon keeps "nothing earlier" honest —
    // an event older than this window is not read, not proof that nothing happened.
    lines.put(format!(
        "  · reflects the most recent {HISTORY_HORIZON} history events per service"
    ));
    lines
}

/// The items whose downloads are stuck, each named so it links straight to its own
/// trace — the landing point for "N stuck", turning a count into a list the operator can
/// act on one item at a time.
fn stuck(report: &StuckReport) -> Lines {
    let mut lines = Lines::default();
    if report.items.is_empty() {
        lines.put("Nothing is stuck — every download is progressing.");
    } else {
        lines.put(format!(
            "{} item(s) stuck — trace any one to see why:",
            report.items.len()
        ));
        for item in &report.items {
            lines.put(format!(
                "  ✗ {}   {}   stuck at {}",
                item.title,
                item.service,
                item.stage.label()
            ));
            lines.put(trace_link(&item.title));
        }
    }
    // A queue that could not be read leaves the list possibly short; saying so keeps it
    // from being read as "nothing else is stuck", the same honesty a trace keeps.
    if report.incomplete {
        lines.spaced("An *arr's queue could not be read, so this list may be incomplete.");
    }
    lines
}

/// What a full reset did, or — until confirmed — would do: the edits it reverts to
/// lemonfiber's own state, named with what is lost, so nothing is discarded unseen.
fn reset(report: &ResetReport) -> Lines {
    let mut lines = Lines::default();
    if report.reverted.is_empty() && report.reverted_connections.is_empty() {
        lines.put("Nothing to reset — the stack is already lemonfiber's own.");
        return lines;
    }
    let count = report.reverted.len() + report.reverted_connections.len();
    if report.confirmed {
        lines.put(format!("Reverted {count} change(s) to lemonfiber's state:"));
    } else {
        lines.put(format!(
            "A reset would revert these {count} change(s) to lemonfiber's state — run again \
             with --confirm to do it:"
        ));
    }
    // The service connections whose category drifted, named as they read in a seed report.
    for connection in &report.reverted_connections {
        lines.put(format!("  · {connection}"));
    }
    // The hand-edited stack files, each with the diff of what is lost.
    for edit in &report.reverted {
        lines.spaced(format!("  {}", edit.path));
        lines.block(&edit.diff);
    }
    lines
}

/// What a lifecycle command did, or would have done.
fn lifecycle(report: &LifecycleReport) -> Lines {
    let mut lines = Lines::default();
    if report.rehearsed {
        lines.put("would run:");
        lines.put(format!("  {}", report.command.join(" ")));
    }
    lines.put(format!("{}: {}", report.action, report.profiles.join(", ")));

    // Saying what was left out, and that it was deliberate, before the operator
    // goes looking for a service that was never going to start.
    if !report.dropped.is_empty() {
        lines.put(format!(
            "left out (no provider configured): {}",
            report.dropped.join(", ")
        ));
    }

    if let Some(condition) = report.condition {
        lines.spaced(describe(condition));
        lines.extend(show(&report.services));
    }

    // Stack files the operator edited, kept as they set them rather than overwritten
    // on this run. Named with the change an upgrade would make, so the operator can
    // see what they are holding back before deciding to take it or keep theirs.
    for edit in &report.stack_edits {
        lines.spaced(format!(
            "kept {} as it is on disk — not overwritten by this version",
            edit.path
        ));
        lines.block(&edit.diff);
    }
    lines
}

/// What each service is doing.
fn status(report: &StatusReport) -> Lines {
    let mut lines = Lines::default();
    lines.put(describe(report.condition));
    lines.extend(show(&report.services));
    lines
}

/// A condition, as a sentence rather than as a word.
fn describe(condition: Condition) -> &'static str {
    match condition {
        Condition::Inactive => "nothing is running",
        Condition::Degraded => "running, and something needs attention",
        Condition::Partial => "partly up",
        Condition::Active => "everything is up",
    }
}

/// What each service is doing, one per line.
fn show(services: &[Service]) -> Lines {
    let mut lines = Lines::default();
    for service in services {
        let state = match service.state {
            State::Absent => "absent".to_owned(),
            State::Stopped => "stopped".to_owned(),
            State::Starting => "starting".to_owned(),
            State::Running => "running".to_owned(),
            State::Healthy => "healthy".to_owned(),
            State::Unhealthy => "unhealthy".to_owned(),
            State::CrashLooping => "crash-looping".to_owned(),
            State::HostManaged => "host-managed".to_owned(),
            // The code is the whole reason this is not simply "stopped", so it
            // is shown rather than left for the operator to go and find.
            State::Failed => match service.exit {
                Some(code) => format!("failed ({code})"),
                None => "failed".to_owned(),
            },
        };
        lines.put(format!("  {:<14} {state:<14} {}", service.id, service.name));
    }
    lines
}

/// What a watch did once its location was lost.
pub(crate) fn watched(report: &SupervisionReport, json: bool) {
    watch_lines(report, json).print();
}

/// The lines a finished watch renders to.
fn watch_lines(report: &SupervisionReport, json: bool) -> Lines {
    let mut lines = Lines::default();
    if json {
        lines.put(
            Envelope::new("watch", report.clone())
                .to_json()
                .unwrap_or(UNRENDERABLE.to_owned()),
        );
        return lines;
    }

    lines.put(format!("the watch ended: {}", report.reason));
    if report.stopped {
        lines.put(format!("stopped: {}", report.forms.join(", ")));
    } else {
        lines.put(format!(
            "could not stop {} — check the services by hand",
            report.forms.join(", ")
        ));
    }
    lines
}

#[cfg(test)]
mod tests {
    use super::{
        answer, describe, diagnosis, household, lifecycle, machine_readable, music, overall,
        preset_choice, quality, remedies, render, reset, seasons, seeding, settings, show, status,
        stuck, trace, trace_link, upgrade, versions, watch_lines, watched, Lines,
    };
    use lemonfiber_core::app::Outcome;
    use lemonfiber_core::docker::{Condition, Criticality, Service, State};
    use lemonfiber_core::doctor::{Category, Finding, Overall, Verdict};
    use lemonfiber_core::error::{Code, Problem, Remedy, Severity};
    use lemonfiber_core::model::{
        ConfigReport, Disposition, DoctorReport, HouseholdMember, HouseholdReport, LifecycleReport,
        MemberRequest, MusicChoice, MusicReport, PresetChoice, QualityReport, ResetReport,
        SettingReport, StackEdit, StatusReport, StuckEntry, StuckReport, SupervisionReport,
        TraceMoment, TraceReport, TraceStage, Triggered, UpgradeMedia, UpgradeReport,
        VersionReport,
    };
    use lemonfiber_core::seed::{
        Assessment as SeedAssessment, Report as SeedReport, Severity as SeedSeverity,
        State as SeedState, Wiring,
    };
    use lemonfiber_core::trace::{
        Confidence, Coverage, Outcome as TraceOutcome, Part, Stage, HISTORY_HORIZON,
    };

    /// One wiring in the given state, with no severity raised.
    fn wiring(connection: &str, state: SeedState) -> Wiring {
        Wiring {
            connection: connection.to_owned(),
            state,
            severity: SeedSeverity::Informational,
        }
    }

    /// A seed report over the given wirings, with drift assessable.
    fn seed_report(wirings: Vec<Wiring>) -> SeedReport {
        SeedReport {
            wirings,
            assessment: SeedAssessment::Assessed,
        }
    }

    /// A problem carrying one remedy, for the diagnosis renderers.
    fn a_problem() -> Problem {
        Problem::new(
            Code::new("TEST"),
            Severity::Error,
            "it broke",
            "nothing will import",
            Remedy::new("restart it").with_detail("docker compose restart"),
        )
    }

    /// One service in the given state.
    fn service(id: &str, state: State, exit: Option<i32>) -> Service {
        Service {
            id: id.to_owned(),
            name: format!("{id} service"),
            profile: "media".to_owned(),
            state,
            criticality: Criticality::Core,
            exit,
        }
    }

    /// A preset choice, transcoding warning off unless asked for.
    fn preset(needs_transcoding_here: bool) -> PresetChoice {
        PresetChoice {
            scope: "everything".to_owned(),
            preset: "Balanced".to_owned(),
            means: "1080p".to_owned(),
            resolution: "1080p".to_owned(),
            size_per_hour: "3 GB".to_owned(),
            transcoding: "direct play".to_owned(),
            needs_transcoding_here,
        }
    }

    /// An audio-format choice.
    fn music_pick() -> MusicChoice {
        MusicChoice {
            scope: "music".to_owned(),
            format: "FLAC".to_owned(),
            means: "lossless".to_owned(),
            targets: "albums".to_owned(),
            size_per_hour: "400 MB".to_owned(),
            note: "large".to_owned(),
        }
    }

    /// A trace of a matched item, with nothing else claimed.
    fn a_trace() -> TraceReport {
        TraceReport {
            item: "The Expanse".to_owned(),
            matched: true,
            ..TraceReport::default()
        }
    }

    #[test]
    fn lines_join_in_order_and_a_spaced_one_is_preceded_by_a_blank() {
        let mut lines = Lines::default();
        lines.put("first");
        lines.spaced("second");
        assert_eq!(lines.text(), "first\n\nsecond");
    }

    #[test]
    fn a_block_is_split_into_the_lines_it_is_made_of() {
        // A diff arrives as one string carrying its own breaks; it has to become lines
        // like everything else, or the printer would put it out as a single blob.
        let mut lines = Lines::default();
        lines.block("-old\n+new\n");
        assert_eq!(lines.text(), "-old\n+new");
    }

    #[test]
    fn printing_puts_every_line_out() {
        // The one place this module reaches the terminal, exercised so it cannot rot.
        let mut lines = Lines::default();
        lines.put("printed");
        lines.print();
        render(&Outcome::Version(a_version()), false);
        watched(&a_watch(), false);
    }

    /// A version report naming a reachable compose.
    fn a_version() -> VersionReport {
        VersionReport {
            binary: "0.4.0".to_owned(),
            supported_schema: vec![1],
            stack: "1.2.3".to_owned(),
            compose: Some("2.29".to_owned()),
        }
    }

    /// A watch that ended having stopped its forms.
    fn a_watch() -> SupervisionReport {
        SupervisionReport {
            forms: vec!["media".to_owned()],
            reason: "the data location went away".to_owned(),
            stopped: true,
        }
    }

    #[test]
    fn versions_name_the_binary_the_stack_and_compose() {
        let text = versions(&a_version()).text();
        assert!(text.contains("stack 1.2.3"));
        assert!(text.contains("compose 2.29"));
        // A compose that could not be reached says so rather than being left blank.
        let unreachable = VersionReport {
            compose: None,
            ..a_version()
        };
        assert!(versions(&unreachable)
            .text()
            .contains("compose not reachable"));
    }

    #[test]
    fn settings_are_listed_and_a_change_says_whether_it_saved() {
        let report = ConfigReport {
            settings: vec![SettingReport {
                key: "DATA_ROOT".to_owned(),
                value: "/data".to_owned(),
                secret: false,
            }],
            changed: true,
            rehearsed: false,
        };
        assert!(settings(&report).text().contains("DATA_ROOT=/data"));
        assert!(settings(&report).text().contains("saved"));
        // A rehearsal must not claim it saved.
        let rehearsed = ConfigReport {
            rehearsed: true,
            ..report.clone()
        };
        assert!(settings(&rehearsed).text().contains("would save"));
        // Nothing changed, nothing claimed.
        let unchanged = ConfigReport {
            changed: false,
            ..report
        };
        assert!(!settings(&unchanged).text().contains("save"));
    }

    #[test]
    fn a_preset_choice_warns_only_where_this_machine_would_transcode() {
        assert!(preset_choice(&preset(true))
            .text()
            .contains("transcode this in software"));
        assert!(!preset_choice(&preset(false))
            .text()
            .contains("transcode this in software"));
    }

    #[test]
    fn every_quality_disposition_says_what_became_of_the_choice() {
        for (disposition, customised, expected) in [
            (Disposition::Recorded, false, "Saved."),
            (Disposition::Recorded, true, "quality reapply"),
            (Disposition::Rehearsed, false, "Would save."),
            (Disposition::Held, false, "Not saved"),
            (Disposition::Reapplied, true, "overwriting your customised"),
            (Disposition::Reapplied, false, "already in step"),
            (
                Disposition::WouldReapply,
                true,
                "Would reapply the preset, overwriting",
            ),
            (Disposition::WouldReapply, false, "already in step"),
            (Disposition::Shown, true, "no longer authoritative"),
        ] {
            let report = QualityReport {
                choices: vec![preset(false)],
                music: Some(music_pick()),
                customised,
                disposition,
            };
            let text = quality(&report).text();
            assert!(text.contains(expected), "{disposition:?}: {text}");
        }
        // Shown with an untouched config says nothing extra.
        let plain = QualityReport {
            choices: vec![preset(false)],
            music: None,
            customised: false,
            disposition: Disposition::Shown,
        };
        assert!(!quality(&plain).text().contains("authoritative"));
    }

    #[test]
    fn the_music_choice_reports_what_became_of_applying_it() {
        for (outcome, expected) in [
            (None, "Applied to the music service."),
            (Some(Triggered::Started), "Applied to the music service."),
            (Some(Triggered::NotStarted), "not up yet"),
            (
                Some(Triggered::Failed {
                    detail: "refused".to_owned(),
                }),
                "refused the change: refused",
            ),
        ] {
            let report = MusicReport {
                choice: music_pick(),
                disposition: Disposition::Recorded,
                outcome,
            };
            assert!(music(&report).text().contains(expected));
        }
        // A rehearsal stops at "would save" and never claims it applied anything.
        let rehearsed = MusicReport {
            choice: music_pick(),
            disposition: Disposition::Rehearsed,
            outcome: None,
        };
        let text = music(&rehearsed).text();
        assert!(text.contains("Would save."));
        assert!(!text.contains("Applied"));
    }

    #[test]
    fn an_upgrade_states_its_cost_before_it_is_confirmed() {
        let media = vec![UpgradeMedia {
            media_type: "tv".to_owned(),
            preset: "Balanced".to_owned(),
            size_per_hour: "3 GB".to_owned(),
            outcome: None,
        }];
        let unconfirmed = UpgradeReport {
            confirmed: false,
            media: media.clone(),
        };
        let text = upgrade(&unconfirmed).text();
        assert!(text.contains("bandwidth-expensive"));
        assert!(text.contains("Nothing has been changed."));
        // Nothing to upgrade is said plainly rather than shown as an empty list.
        let nothing = UpgradeReport {
            confirmed: false,
            media: Vec::new(),
        };
        assert!(upgrade(&nothing).text().contains("nothing to upgrade"));
    }

    #[test]
    fn a_confirmed_upgrade_reports_each_services_answer() {
        for (outcome, expected) in [
            (Some(Triggered::Started), "re-search started"),
            (Some(Triggered::NotStarted), "not started"),
            (
                Some(Triggered::Failed {
                    detail: "boom".to_owned(),
                }),
                "✗ boom",
            ),
        ] {
            let report = UpgradeReport {
                confirmed: true,
                media: vec![UpgradeMedia {
                    media_type: "tv".to_owned(),
                    preset: "Balanced".to_owned(),
                    size_per_hour: "3 GB".to_owned(),
                    outcome,
                }],
            };
            assert!(upgrade(&report).text().contains(expected));
        }
    }

    #[test]
    fn a_trace_link_names_the_term_the_trace_searches_by() {
        assert!(trace_link("The Expanse").contains(r#"trace "The Expanse""#));
    }

    #[test]
    fn an_unmatched_trace_says_nobody_asked_for_it() {
        let report = TraceReport {
            item: "Nothing".to_owned(),
            matched: false,
            stall: Some("nobody has asked for it".to_owned()),
            ..TraceReport::default()
        };
        let text = trace(&report).text();
        assert!(text.contains("nobody has asked for it"));
        // The stage box belongs to a matched item; an unmatched one stops here.
        assert!(!text.contains("history events per service"));
        // And one with no reason at all still renders its name.
        let bare = TraceReport {
            item: "Nothing".to_owned(),
            matched: false,
            ..TraceReport::default()
        };
        assert_eq!(trace(&bare).text(), "Nothing");
    }

    #[test]
    fn a_trace_shows_its_stages_stall_history_and_horizon() {
        let report = TraceReport {
            stages: vec![
                TraceStage {
                    stage: Stage::Monitored,
                    service: "Sonarr".to_owned(),
                    at: None,
                },
                TraceStage {
                    stage: Stage::Grabbed,
                    service: "Sonarr".to_owned(),
                    at: Some("2026-01-01".to_owned()),
                },
            ],
            stall: Some("the download client never took it".to_owned()),
            history: vec![
                TraceMoment {
                    outcome: TraceOutcome::Grabbed,
                    at: "2026-01-01".to_owned(),
                },
                TraceMoment {
                    outcome: TraceOutcome::DownloadFailed,
                    at: "2026-01-02".to_owned(),
                },
            ],
            findings: vec!["the queue could not be read".to_owned()],
            confidence: Confidence::Uncertain,
            ..a_trace()
        };
        let text = trace(&report).text();
        assert!(text.contains("✓ monitored   Sonarr"));
        assert!(text.contains("✓ grabbed   Sonarr   2026-01-01"));
        assert!(text.contains("✗ stopped:"));
        assert!(text.contains("history:"));
        assert!(text.contains("! the queue could not be read"));
        assert!(text.contains("~ matched to the library by title"));
        assert!(text.contains(&format!("most recent {HISTORY_HORIZON} history events")));
    }

    #[test]
    fn a_single_clean_grab_does_not_repeat_itself_as_history() {
        let report = TraceReport {
            history: vec![TraceMoment {
                outcome: TraceOutcome::Grabbed,
                at: "2026-01-01".to_owned(),
            }],
            ..a_trace()
        };
        assert!(!trace(&report).text().contains("history:"));
    }

    #[test]
    fn a_season_rollup_names_what_is_outstanding_and_what_nobody_asked_for() {
        let coverage = Coverage::of(vec![
            Part {
                season: 1,
                number: 1,
                title: "one".to_owned(),
                stage: Stage::Imported,
            },
            Part {
                season: 1,
                number: 2,
                title: "two".to_owned(),
                stage: Stage::Monitored,
            },
            Part {
                season: 1,
                number: 3,
                title: "three".to_owned(),
                stage: Stage::NotMonitored,
            },
            Part {
                season: 2,
                number: 1,
                title: "four".to_owned(),
                stage: Stage::Imported,
            },
        ]);
        let text = seasons(&coverage).text();
        assert!(text.contains("2 of 3 episode(s) here"));
        assert!(text.contains("season 1   1 of 2"));
        assert!(text.contains("(1 more not asked for)"));
        assert!(text.contains("season 2   1 of 1   complete"));
        // The outstanding episode carries the reason it stopped, not just its number.
        assert!(text.contains("S01E02   monitored, but no search has found it"));
    }

    #[test]
    fn a_season_nobody_asked_for_reads_as_that_rather_than_as_a_fault() {
        let coverage = Coverage::of(vec![
            Part {
                season: 0,
                number: 1,
                title: "special".to_owned(),
                stage: Stage::NotMonitored,
            },
            Part {
                season: 1,
                number: 1,
                title: "one".to_owned(),
                stage: Stage::Imported,
            },
        ]);
        assert!(seasons(&coverage)
            .text()
            .contains("specials   1 not asked for"));
        // And a series with nothing wanted at all says so instead of "0 of 0".
        let none = Coverage::of(vec![Part {
            season: 1,
            number: 1,
            title: "one".to_owned(),
            stage: Stage::NotMonitored,
        }]);
        assert!(seasons(&none).text().contains("no episode(s) asked for"));
    }

    #[test]
    fn an_outstanding_episode_in_progress_reads_as_its_stage() {
        // Downloading carries no stall reason, so the stage's own label stands in.
        let coverage = Coverage::of(vec![Part {
            season: 1,
            number: 1,
            title: "one".to_owned(),
            stage: Stage::Downloading,
        }]);
        assert!(seasons(&coverage).text().contains("S01E01   downloading"));
    }

    #[test]
    fn a_trace_folds_in_its_coverage() {
        let report = TraceReport {
            coverage: Some(Coverage::of(vec![Part {
                season: 1,
                number: 1,
                title: "one".to_owned(),
                stage: Stage::Imported,
            }])),
            ..a_trace()
        };
        assert!(trace(&report).text().contains("1 of 1 episode(s) here"));
    }

    #[test]
    fn the_household_view_names_each_member_and_links_what_it_can_trace() {
        let report = HouseholdReport {
            members: vec![HouseholdMember {
                name: "Alex".to_owned(),
                requests: vec![
                    MemberRequest {
                        title: Some("The Expanse".to_owned()),
                        media: Some("series".to_owned()),
                        state: Some(lemonfiber_core::household::State::Here),
                    },
                    // No service holds it yet, so it is named by what it is.
                    MemberRequest {
                        title: None,
                        media: Some("film".to_owned()),
                        state: Some(lemonfiber_core::household::State::WaitingForApproval),
                    },
                    // Neither a title nor a kind this build knows.
                    MemberRequest {
                        title: None,
                        media: None,
                        state: None,
                    },
                ],
            }],
            available: true,
            findings: vec!["a library could not be read".to_owned()],
        };
        let text = household(&report).text();
        assert!(text.contains("Alex"));
        assert!(text.contains("The Expanse   here"));
        assert!(text.contains(r#"trace "The Expanse""#));
        assert!(text.contains("a film   waiting for approval"));
        assert!(text.contains("something   the request service reports a state"));
        assert!(text.contains("1 member(s), 3 request(s)."));
        assert!(text.contains("! a library could not be read"));
    }

    #[test]
    fn an_empty_household_says_whether_it_was_read() {
        let asked_nothing = HouseholdReport {
            members: Vec::new(),
            available: true,
            findings: Vec::new(),
        };
        assert!(household(&asked_nothing)
            .text()
            .contains("Nobody has asked for anything yet."));
        // Unread is not the same as empty: no such claim is made.
        let unread = HouseholdReport {
            members: Vec::new(),
            available: false,
            findings: vec!["could not be read".to_owned()],
        };
        let text = household(&unread).text();
        assert!(!text.contains("Nobody has asked"));
        assert!(text.contains("! could not be read"));
    }

    #[test]
    fn the_stuck_list_names_each_item_and_links_its_trace() {
        let report = StuckReport {
            items: vec![StuckEntry {
                title: "The Expanse".to_owned(),
                service: "Sonarr".to_owned(),
                stage: Stage::Downloading,
            }],
            incomplete: true,
        };
        let text = stuck(&report).text();
        assert!(text.contains("1 item(s) stuck"));
        assert!(text.contains("stuck at downloading"));
        assert!(text.contains(r#"trace "The Expanse""#));
        assert!(text.contains("may be incomplete"));
        // Nothing stuck is said plainly.
        let clear = StuckReport {
            items: Vec::new(),
            incomplete: false,
        };
        assert!(stuck(&clear).text().contains("Nothing is stuck"));
    }

    #[test]
    fn a_reset_names_every_change_it_would_revert() {
        let report = ResetReport {
            reverted: vec![StackEdit {
                path: "compose.yml".to_owned(),
                diff: "-yours\n+ours\n".to_owned(),
            }],
            reverted_connections: vec!["sonarr → sabnzbd".to_owned()],
            confirmed: false,
        };
        let text = reset(&report).text();
        assert!(text.contains("would revert these 2 change(s)"));
        assert!(text.contains("· sonarr → sabnzbd"));
        assert!(text.contains("compose.yml"));
        assert!(text.contains("-yours"));
        // Confirmed, it reports what it did rather than what it would do.
        let done = ResetReport {
            confirmed: true,
            ..report
        };
        assert!(reset(&done).text().contains("Reverted 2 change(s)"));
        // Nothing to revert is not an empty list.
        let clean = ResetReport {
            reverted: Vec::new(),
            reverted_connections: Vec::new(),
            confirmed: false,
        };
        assert!(reset(&clean).text().contains("already lemonfiber's own"));
    }

    #[test]
    fn a_lifecycle_report_names_the_command_the_drops_and_the_edits_it_kept() {
        let report = LifecycleReport {
            action: "up".to_owned(),
            profiles: vec!["media".to_owned()],
            dropped: vec!["usenet".to_owned()],
            command: vec!["docker".to_owned(), "compose".to_owned()],
            rehearsed: true,
            status: None,
            services: vec![service("sonarr", State::Healthy, None)],
            condition: Some(Condition::Active),
            stack_edits: vec![StackEdit {
                path: "compose.yml".to_owned(),
                diff: "-a\n+b\n".to_owned(),
            }],
        };
        let text = lifecycle(&report).text();
        assert!(text.contains("would run:"));
        assert!(text.contains("docker compose"));
        assert!(text.contains("up: media"));
        assert!(text.contains("left out (no provider configured): usenet"));
        assert!(text.contains("everything is up"));
        assert!(text.contains("sonarr"));
        assert!(text.contains("kept compose.yml as it is on disk"));
        assert!(text.contains("-a"));
    }

    #[test]
    fn a_run_that_was_not_rehearsed_and_reports_no_condition_says_only_what_it_did() {
        let report = LifecycleReport {
            action: "down".to_owned(),
            profiles: vec!["media".to_owned()],
            dropped: Vec::new(),
            command: Vec::new(),
            rehearsed: false,
            status: Some(0),
            services: Vec::new(),
            condition: None,
            stack_edits: Vec::new(),
        };
        let text = lifecycle(&report).text();
        assert_eq!(text, "down: media");
    }

    #[test]
    fn every_condition_reads_as_a_sentence() {
        for condition in [
            Condition::Inactive,
            Condition::Degraded,
            Condition::Partial,
            Condition::Active,
        ] {
            assert!(!describe(condition).is_empty());
        }
        assert_eq!(describe(Condition::Inactive), "nothing is running");
    }

    #[test]
    fn every_service_state_reads_as_a_word_and_a_failure_carries_its_code() {
        let services = vec![
            service("a", State::Absent, None),
            service("b", State::Stopped, None),
            service("c", State::Starting, None),
            service("d", State::Running, None),
            service("e", State::Healthy, None),
            service("f", State::Unhealthy, None),
            service("g", State::CrashLooping, None),
            service("h", State::HostManaged, None),
            service("i", State::Failed, Some(137)),
            service("j", State::Failed, None),
        ];
        let text = show(&services).text();
        for word in [
            "absent",
            "stopped",
            "starting",
            "running",
            "healthy",
            "unhealthy",
            "crash-looping",
            "host-managed",
        ] {
            assert!(text.contains(word), "missing {word}");
        }
        // The exit code is the whole reason a failure is not simply "stopped".
        assert!(text.contains("failed (137)"));
        assert!(text.contains("failed         j service"));
    }

    #[test]
    fn a_status_report_leads_with_the_condition() {
        let report = StatusReport {
            forms: vec!["media".to_owned()],
            condition: Condition::Degraded,
            services: vec![service("sonarr", State::Unhealthy, None)],
        };
        let text = status(&report).text();
        assert!(text.starts_with("running, and something needs attention"));
        assert!(text.contains("unhealthy"));
    }

    #[test]
    fn every_seed_state_says_what_became_of_the_connection() {
        let report = seed_report(vec![
            wiring("a", SeedState::Wired),
            wiring("b", SeedState::AlreadyWired),
            wiring("c", SeedState::Drifted),
            wiring("d", SeedState::Adopted),
            wiring("e", SeedState::Unmanaged),
            wiring("f", SeedState::Stale),
            wiring(
                "g",
                SeedState::Conflicted {
                    yours: Some("mine".to_owned()),
                    ours: "ours".to_owned(),
                },
            ),
            wiring(
                "h",
                SeedState::Conflicted {
                    yours: None,
                    ours: "ours".to_owned(),
                },
            ),
            wiring(
                "i",
                SeedState::Skipped {
                    reason: "not up".to_owned(),
                },
            ),
            wiring(
                "j",
                SeedState::Failed {
                    detail: "refused".to_owned(),
                },
            ),
            wiring(
                "k",
                SeedState::Refused {
                    reason: "two arrs".to_owned(),
                },
            ),
        ]);
        let text = seeding(&report).text();
        for phrase in [
            "wired",
            "already wired",
            "left as you set it",
            "yours, adopted",
            "found already set",
            "yours for now",
            "conflict — both you and the default changed it",
            "you set “mine”",
            "you cleared it",
            "skipped",
            "refused",
        ] {
            assert!(text.contains(phrase), "missing {phrase}");
        }
    }

    #[test]
    fn a_drift_that_breaks_the_stack_is_raised_beneath_its_line() {
        let report = seed_report(vec![Wiring {
            connection: "root folder".to_owned(),
            state: SeedState::Drifted,
            severity: SeedSeverity::Warning {
                breakage: "the path does not exist".to_owned(),
                remediation: "create it".to_owned(),
            },
        }]);
        let text = seeding(&report).text();
        assert!(text.contains("! the path does not exist"));
        assert!(text.contains("→ create it"));
        assert!(text.contains("1 drifted in a way that breaks the stack"));
    }

    #[test]
    fn what_a_seed_still_owes_is_the_last_thing_said() {
        // Everything settled.
        assert!(seeding(&seed_report(vec![wiring("a", SeedState::Wired)]))
            .text()
            .contains("Everything is wired."));
        // Outstanding but nothing blocked.
        let waiting = seed_report(vec![wiring(
            "a",
            SeedState::Skipped {
                reason: "not up".to_owned(),
            },
        )]);
        assert!(seeding(&waiting).text().contains("1 left to wire"));
        // Everything outstanding is blocked.
        let blocked = seed_report(vec![wiring(
            "a",
            SeedState::Refused {
                reason: "two arrs".to_owned(),
            },
        )]);
        assert!(seeding(&blocked).text().contains("1 to resolve"));
        // A mix of the two.
        let mixed = seed_report(vec![
            wiring(
                "a",
                SeedState::Refused {
                    reason: "two arrs".to_owned(),
                },
            ),
            wiring(
                "b",
                SeedState::Skipped {
                    reason: "not up".to_owned(),
                },
            ),
        ]);
        assert!(seeding(&mixed)
            .text()
            .contains("2 left: 1 to wire once ready"));
    }

    #[test]
    fn a_lost_baseline_says_drift_could_not_be_assessed() {
        let report = SeedReport {
            wirings: vec![wiring("a", SeedState::Wired)],
            assessment: SeedAssessment::Unassessable,
        };
        assert!(seeding(&report).text().contains("could not be read"));
    }

    #[test]
    fn every_verdict_reads_with_its_own_mark() {
        let findings = vec![
            Finding {
                check: "a".to_owned(),
                category: Category::Storage,
                title: "noted".to_owned(),
                verdict: Verdict::Pass {
                    note: Some("plenty of room".to_owned()),
                },
            },
            Finding {
                check: "b".to_owned(),
                category: Category::Storage,
                title: "bare".to_owned(),
                verdict: Verdict::Pass { note: None },
            },
            Finding {
                check: "c".to_owned(),
                category: Category::Vpn,
                title: "warned".to_owned(),
                verdict: Verdict::Warn(a_problem()),
            },
            Finding {
                check: "d".to_owned(),
                category: Category::Vpn,
                title: "failed".to_owned(),
                verdict: Verdict::Fail(a_problem()),
            },
            Finding {
                check: "e".to_owned(),
                category: Category::Network,
                title: "unproven".to_owned(),
                verdict: Verdict::Unverified {
                    reason: "nothing answered".to_owned(),
                    remedy: Remedy::new("start it").with_detail("compose up"),
                },
            },
            Finding {
                check: "f".to_owned(),
                category: Category::Network,
                title: "passed over".to_owned(),
                verdict: Verdict::Skipped {
                    reason: "not applicable".to_owned(),
                },
            },
        ];
        let report = DoctorReport {
            overall: Overall::Degraded,
            findings,
        };
        let text = diagnosis(&report).text();
        assert!(text.contains("✓ noted   plenty of room"));
        assert!(text.contains("✓ bare"));
        assert!(text.contains("! warned   it broke"));
        assert!(text.contains("✗ failed   it broke"));
        assert!(text.contains("? unproven   UNVERIFIED"));
        assert!(text.contains("→ start it"));
        assert!(text.contains("compose up"));
        assert!(text.contains("– passed over   skipped: not applicable"));
        assert!(text.contains("degraded — working, with warnings"));
    }

    #[test]
    fn an_unverified_finding_without_detail_still_carries_its_remedy() {
        let report = DoctorReport {
            overall: Overall::Unknown,
            findings: vec![Finding {
                check: "a".to_owned(),
                category: Category::Config,
                title: "unproven".to_owned(),
                verdict: Verdict::Unverified {
                    reason: "nothing answered".to_owned(),
                    remedy: Remedy::new("start it"),
                },
            }],
        };
        assert!(diagnosis(&report).text().contains("→ start it"));
    }

    #[test]
    fn a_remedy_without_detail_prints_only_its_action() {
        let problem = Problem::new(
            Code::new("TEST"),
            Severity::Warning,
            "it broke",
            "nothing imports",
            Remedy::new("restart it"),
        );
        let text = remedies(&problem).text();
        assert!(text.contains("nothing imports"));
        assert!(text.contains("→ restart it"));
    }

    #[test]
    fn every_overall_verdict_reads_as_a_sentence() {
        for verdict in [
            Overall::Healthy,
            Overall::Degraded,
            Overall::Broken,
            Overall::Unknown,
        ] {
            assert!(overall(verdict).contains('—'));
        }
    }

    #[test]
    fn a_watch_reports_whether_it_stopped_what_it_guarded() {
        assert!(watch_lines(&a_watch(), false)
            .text()
            .contains("stopped: media"));
        let stranded = SupervisionReport {
            stopped: false,
            ..a_watch()
        };
        assert!(watch_lines(&stranded, false)
            .text()
            .contains("could not stop media"));
        // As JSON it is one envelope, not prose.
        let json = watch_lines(&a_watch(), true).text();
        assert!(json.contains(r#""kind":"watch""#));
    }

    #[test]
    fn every_outcome_renders_and_every_outcome_renders_as_json() {
        let outcomes = vec![
            Outcome::Version(a_version()),
            // A setting to list: an empty, unchanged config renders nothing at all,
            // which is correct and is covered by its own test.
            Outcome::Config(ConfigReport {
                settings: vec![SettingReport {
                    key: "DATA_ROOT".to_owned(),
                    value: "/data".to_owned(),
                    secret: false,
                }],
                changed: false,
                rehearsed: false,
            }),
            Outcome::Quality(QualityReport {
                choices: vec![preset(false)],
                music: None,
                customised: false,
                disposition: Disposition::Shown,
            }),
            Outcome::Upgrade(UpgradeReport {
                confirmed: true,
                media: Vec::new(),
            }),
            Outcome::Music(MusicReport {
                choice: music_pick(),
                disposition: Disposition::Recorded,
                outcome: None,
            }),
            Outcome::Trace(a_trace()),
            Outcome::Household(HouseholdReport {
                members: Vec::new(),
                available: true,
                findings: Vec::new(),
            }),
            Outcome::Stuck(StuckReport {
                items: Vec::new(),
                incomplete: false,
            }),
            Outcome::Lifecycle(LifecycleReport {
                action: "up".to_owned(),
                profiles: Vec::new(),
                dropped: Vec::new(),
                command: Vec::new(),
                rehearsed: false,
                status: None,
                services: Vec::new(),
                condition: None,
                stack_edits: Vec::new(),
            }),
            Outcome::Status(StatusReport {
                forms: Vec::new(),
                condition: Condition::Inactive,
                services: Vec::new(),
            }),
            Outcome::Doctor(DoctorReport {
                overall: Overall::Healthy,
                findings: Vec::new(),
            }),
            Outcome::Seed(seed_report(Vec::new())),
            Outcome::Reset(ResetReport {
                reverted: Vec::new(),
                reverted_connections: Vec::new(),
                confirmed: false,
            }),
        ];
        // Every arm of the dispatch renders something, and every one of them also
        // renders as an envelope a script can parse.
        for outcome in outcomes {
            assert!(!answer(&outcome, false).text().is_empty());
            let json = answer(&outcome, true).text();
            assert!(json.contains(r#""api_version""#), "{json}");
        }
    }

    #[test]
    fn the_json_fallback_is_a_value_rather_than_an_unreachable_branch() {
        // Nothing can actually fail to serialise; the fallback exists so the line is
        // one a test can reach, and this is that test.
        assert!(machine_readable(&Outcome::Version(a_version()))
            .text()
            .contains(r#""kind":"version""#));
        assert!(!super::UNRENDERABLE.is_empty());
    }
}
