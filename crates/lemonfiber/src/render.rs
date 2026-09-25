//! The surface's rendering, kept apart from the CLI wiring and orchestration.
//!
//! `main` decides what to run and hands the outcome here; this module decides only how it
//! reads. One renderer per answer, for a person or for a script, with nothing about
//! parsing input or dispatching commands mixed in — so the shape of an operator's report
//! and the shape of the command line stay two separate things to change.
//!
//! Every renderer *builds* its lines and hands them back; one printer at the edge puts
//! them on the terminal. Rendering is then a value a test can assert on rather than a
//! side effect it can only watch happen, which is what lets the words an operator
//! actually reads be held to the same standard as the rest.

#[cfg(test)]
pub(crate) mod fixtures;

mod archive;
mod bandwidth;
mod catalogue;
mod changelog;
mod clients;
mod credentials;
mod doctor;
pub(crate) mod door;
pub(crate) mod downloads;
pub(crate) mod glossary;
mod held;
mod history;
pub(crate) mod host;
mod hosting;
mod invitation;
mod migration;
mod outbound;
pub(crate) mod plugin;
mod provenance;
mod qr;
mod quality;
mod reconfigure;
mod removal;
pub(crate) mod repair;
mod seed;
mod self_update;
mod space;
pub(crate) mod stack;
mod stored;
mod trace;
mod uninstall;
mod update;
pub(crate) mod walkthrough;
mod wiring;

use lemonfiber_core::app::Outcome;
use lemonfiber_core::model::{
    AlertReport, ConfigReport, FormsReport, SettingReport, VersionReport, WizardReport,
};
use lemonfiber_core::origin::Origin;
use lemonfiber_core::reconfigure::{Review, Stance};
use lemonfiber_core::wizard::Phase;
use lemonfiber_core::PRODUCT;

/// What stands in for an answer that could not be turned into JSON.
///
/// Serialising these reports cannot actually fail — every field is a plain owned value —
/// so this exists to keep the fallback a value rather than an unreachable branch. It is
/// built eagerly for the same reason: a lazily-built one would be a line no test could
/// ever run, which is exactly what the coverage gate is there to forbid.
pub(crate) const UNRENDERABLE: &str = "this answer could not be rendered as JSON";

/// The lines an answer renders to, in order.
///
/// Built rather than printed so a renderer returns something a test can read back. The
/// terminal is reached in one place, at the edge, which also means nothing here has to
/// care whether it is being rendered for a person or for an assertion.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub(crate) struct Lines {
    /// The lines themselves, in order.
    said: Vec<String>,
    /// Whether these are for something that will parse them rather than read them.
    ///
    /// Carried by the lines rather than passed to whoever prints them, because the
    /// renderer that built them is the only one that knows, and every `print` call
    /// site would otherwise have to be told and could be told wrong.
    parsed: bool,
}

impl Lines {
    /// Lines for something that will parse them, which go out exactly as produced.
    ///
    /// Neither made plain nor folded. Both of those are decisions about what a
    /// person's terminal can show, and there is no person here — and both of them
    /// damage this: the fold writes a curly quote as `"`, which inside a JSON string
    /// is not a character but the end of it, so a release name containing one would
    /// arrive as something that will not parse at all.
    ///
    /// What made it safe was said to be the serialising, on the grounds that JSON escapes
    /// every control character. It escapes the ones below a space. The C1 controls, the
    /// line separators, the bidirectional overrides and the zero-widths are carried raw,
    /// and each of those is an instruction to the terminal these lines are commonly
    /// printed to — so [`say::emitted`](crate::say) writes them out on the way through,
    /// which changes what a terminal reads and not what a parser does.
    pub(crate) fn for_a_parser() -> Self {
        Self {
            said: Vec::new(),
            parsed: true,
        }
    }

    /// One line.
    ///
    /// Made plain on the way in, because most of what is shown here came from
    /// somewhere else — a release name from an indexer, a failure message from a
    /// \*arr — and a terminal reads a control character in the middle of one as an
    /// instruction. One place rather than at each caller: a line that skipped it
    /// would be the one carrying the name somebody chose.
    pub(crate) fn put(&mut self, line: impl Into<String>) {
        let line = line.into();
        if self.parsed {
            self.said.push(line);
            return;
        }
        self.said.push(lemonfiber_core::text::plain(&line));
    }

    /// A remedy: what to do, and where to look when that helps.
    ///
    /// One shape wherever a remedy is shown, so the diagnosis, a repair's escalation and
    /// anything after them cannot drift on how an action and its detail sit together.
    pub(crate) fn remedy(&mut self, remedy: &lemonfiber_core::error::Remedy, indent: &str) {
        self.put(format!("{indent}→ {}", remedy.action));
        if let Some(detail) = &remedy.detail {
            self.put(format!("{indent}  {detail}"));
        }
    }

    /// A blank line, then the given one — the separated closing remark most answers end
    /// on, kept as one call so the spacing is uniform rather than re-decided each time.
    pub(crate) fn spaced(&mut self, line: impl Into<String>) {
        self.said.push(String::new());
        self.put(line);
    }

    /// Text that already carries its own line breaks — a diff — split into the lines it
    /// is made of, so a block and a built line are the same kind of thing from here on.
    pub(crate) fn block(&mut self, text: &str) {
        // Its own line breaks are what makes it a block, so those are kept and the
        // lines they separate are made plain the same way any other line is.
        for line in text.lines() {
            self.put(line);
        }
    }

    /// Everything another renderer built, appended.
    pub(crate) fn extend(&mut self, other: Self) {
        self.said.extend(other.said);
    }

    /// The lines as one piece of text, for a test to read, for a diff to compare,
    /// and for the footnote block to find its own report's words in.
    pub(crate) fn text(&self) -> String {
        self.said.join("\n")
    }

    /// Put them on the terminal. The one place this crate reaches stdout.
    pub(crate) fn print(&self) {
        for line in &self.said {
            if self.parsed {
                crate::say::emitted(line);
            } else {
                crate::say::said(line);
            }
        }
    }

    /// Put them on the error stream — what an operator is told about a refusal,
    /// which belongs beside the answer rather than in it.
    pub(crate) fn eprint(&self) {
        for line in &self.said {
            if self.parsed {
                crate::say::refused(line);
            } else {
                crate::say::complained(line);
            }
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
    let mut lines = shaped(outcome);
    // An explanation is not a report that used a word; it is the word. A footnote
    // under it would explain the same word a second time and point at the command
    // that had just been run.
    if matches!(outcome, Outcome::Word(_) | Outcome::Glossary(_)) {
        return lines;
    }
    // Built from the finished report rather than by each renderer, because what a
    // report explains is a property of what it ended up saying — a renderer that
    // had to remember to do this would be a renderer that could forget.
    //
    // After the `json` return, never before it: a footnote is prose for a person,
    // and appending it to a machine-readable answer would corrupt the one thing
    // that answer exists to be.
    let notes = glossary::footnotes(&lines.text(), glossary::wanted(), glossary::known());
    lines.extend(notes);
    lines
}

/// The lines one outcome renders to, before anything is said about its words.
///
/// Reached by the dashboard as well as by the printer, so a question asked at a
/// screen is answered in the words the same request typed at a prompt is. The
/// footnotes are not part of it: a report ends and can put its words underneath
/// itself, and a screen that never ends explains them on a keypress instead.
pub(crate) fn shaped(outcome: &Outcome) -> Lines {
    match outcome {
        Outcome::Version(report) => versions(report),
        Outcome::Forms(report) => forms(report),
        Outcome::Preview(plan) => stack::preview(plan),
        Outcome::Config(report) => settings(report),
        Outcome::Alerts(report) => alerts(report),
        Outcome::History(report) => history::history(report),
        Outcome::Migration(report) => migration::migration(report),
        Outcome::Adoption(report) => migration::adoption(report),
        Outcome::Beside(report) => migration::beside(report),
        Outcome::Replacement(report) => migration::replacement(report),
        Outcome::Import(report) => migration::carried(report),
        Outcome::Quality(report) => quality::quality(report),
        Outcome::Upgrade(report) => quality::upgrade(report),
        Outcome::Music(report) => quality::music(report),
        Outcome::Trace(report) => trace::trace(report),
        Outcome::Household(report) => trace::household(report),
        Outcome::Held(report) => held::held(report),
        Outcome::Hosting(report) => hosting::hosting(report),
        Outcome::FrontDoor(report) => door::front_door(report),
        Outcome::Stuck(report) => trace::stuck(report),
        Outcome::Word(term) => glossary::explanation(term),
        Outcome::Glossary(listed) => glossary::vocabulary(listed),
        Outcome::Clients(all) => clients::guidance(all),
        Outcome::Invitation(report) => invitation::invitation(report),
        Outcome::Removal(report) => removal::removal(report),
        Outcome::Outbound(report) => outbound::leaving(report),
        Outcome::Plugins(report) => plugin::installs(report),
        Outcome::Provenance(report) => provenance::comes_from(report),
        Outcome::Catalogue(report) => catalogue::holds(report),
        Outcome::Wiring(report) => wiring::wired(report),
        Outcome::Substitution(report) => wiring::substituted(report),
        Outcome::Credentials(inventory) => credentials::listing(inventory),
        Outcome::Stored(report) => stored::kept(report),
        Outcome::SelfUpdate(report) => self_update::standing(report),
        Outcome::Space(report) => space::reckoning(report),
        Outcome::StopSeeding(offer) => space::letting(offer),
        Outcome::Bandwidth(report) => bandwidth::sharing(report),
        Outcome::Lifecycle(report) => stack::lifecycle(report),
        Outcome::Status(report) => stack::status(report),
        Outcome::Doctor(report) => doctor::diagnosis(report),
        Outcome::Repair(report) => repair::mended(report),
        Outcome::Undo(report) => repair::reversed(report),
        Outcome::Seed(report) => seed::seeding(report),
        Outcome::Reset(report) => stack::reset(report),
        Outcome::Uninstall(report) => uninstall::removal(report),
        Outcome::Wizard(report) => standing(report),
        Outcome::Update(report) => update::update(report),
        Outcome::Backup(report) => archive::backup(report),
        Outcome::Bundle(report) => archive::bundle(report),
        Outcome::Archives(listing) => archive::kept(listing),
        Outcome::Restore(report) => archive::restoration(report),
        Outcome::Watch(report) => stack::watch(report),
        Outcome::Walkthrough(report) => walkthrough::ending(report),
    }
}

/// Where setup stands, for somebody who asked rather than answered.
///
/// The plan is shown only once every question has an answer, because until then
/// it is a partial list of settings that reads as the whole of what will be
/// written. Its values arrive withheld where they are credentials, and are put
/// through unexamined here — deciding again which of them to hide would be a
/// second rule to keep in step with the one that already did it.
fn standing(report: &WizardReport) -> Lines {
    let mut lines = Lines::default();
    if !report.offered {
        lines.put(format!("{PRODUCT} is already set up on this machine."));
        return lines;
    }
    if report.phase == Phase::Applying {
        lines.put("An earlier apply stopped part-way and has not been recovered.".to_owned());
        for change in &report.written {
            lines.put(format!("  · it had written: {change}"));
        }
    }
    lines.put(format!("Setup is at: {}", report.at.label()));
    for step in &report.unanswered {
        lines.put(format!("  · still to answer: {}", step.label()));
    }
    if report.ready_for_review {
        lines.put("Every question is answered.".to_owned());
        lines.put(String::new());
        lines.put("What applying will write:".to_owned());
        for setting in &report.plan {
            lines.put(format!("  {}={}", setting.key, setting.value));
        }
    }
    lines
}

/// The same answer, for something that will parse it.
fn machine_readable(outcome: &Outcome) -> Lines {
    let mut lines = Lines::for_a_parser();
    lines.put(
        outcome
            .clone()
            .envelope()
            .to_json()
            .unwrap_or(UNRENDERABLE.to_owned()),
    );
    lines
}

/// What versions are in play, and what the one this build is brought.
///
/// The numbers first and in four lines, because somebody checking which version
/// they are on wants that answer and not a paragraph before it. What changed goes
/// underneath, where it is the second half of the same question rather than a
/// separate command somebody has to know exists.
fn versions(report: &VersionReport) -> Lines {
    let mut lines = Lines::default();
    lines.put(format!("{PRODUCT} {}", report.binary));
    lines.put(format!("stack {}", report.stack));
    lines.put(format!("manifest schema {:?}", report.supported_schema));
    match &report.compose {
        Some(version) => lines.put(format!("compose {version}")),
        None => lines.put("compose not reachable"),
    }
    lines.extend(changelog::notes(&report.changelog, &report.binary));
    lines
}

/// The forms this stack declares, in its own words.
///
/// The id first, because that is what gets typed, and the description after it, because
/// that is what decides which one to type. A form that cannot be combined says so here
/// rather than only when a combination is refused: somebody choosing between two forms is
/// exactly who needs to know it is a choice.
fn forms(report: &FormsReport) -> Lines {
    let mut lines = Lines::default();
    // Said rather than shown as nothing. A stack is free to declare no forms, and a
    // command that answered that with a blank screen would read as a broken command
    // rather than as an answer.
    if report.forms.is_empty() {
        lines.put("This stack declares no forms.");
        return lines;
    }
    for form in &report.forms {
        lines.put(format!("{} — {}", form.id, form.description));
        if !form.composable {
            lines.put("    on its own; it cannot be combined with another form");
        }
    }
    lines
}

/// What the operator is told about, what that means, and anything set apart from it.
fn alerts(report: &AlertReport) -> Lines {
    let mut lines = Lines::default();
    lines.put(format!("telling you about: {}", report.preset));
    lines.put(report.means.clone());
    for exception in &report.exceptions {
        // Named apart from the preset, so the operator can see why one kind does not
        // follow the answer they just read.
        lines.put(format!(
            "  {} — {}",
            exception.kind,
            if exception.wanted {
                "always told"
            } else {
                "never told"
            }
        ));
    }
    if report.changed {
        lines.put(String::new());
        // A rehearsal reports what it would do, so it must not claim it saved.
        lines.put(if report.rehearsed {
            "would save"
        } else {
            "saved"
        });
    }
    lines
}

/// What the operator has configured, and what a change to it would do.
fn settings(report: &ConfigReport) -> Lines {
    let mut lines = Lines::default();
    for setting in &report.settings {
        lines.put(format!(
            "{}={}  — {}",
            setting.key,
            setting.value,
            came_from(&setting.origin)
        ));
    }
    if let Some(review) = &report.review {
        let change = &review.change;
        lines.put(format!(
            "{}: {} → {}",
            change.key,
            change.from.as_deref().unwrap_or(UNSET),
            change.to
        ));
        for line in verdict(review, report.rehearsed) {
            lines.put(line);
        }
        // What the change comes to on this machine, under the verdict rather than
        // over it: the operator reads whether it landed first and why second.
        lines.extend(reconfigure::found(&review.findings));
    }
    // Said at the moment the choice is being made, and only then — the checks
    // deliberately do not raise it again on every run afterwards.
    if let Some(consequence) = &report.consequence {
        lines.put(String::new());
        lines.put(consequence.clone());
    }
    // Last, so the values and any change to one read as one block. These are the
    // footnotes to the listing above rather than part of it, and a sentence sitting
    // between a setting and the change being made to it would read as being about
    // the change.
    lines.extend(unsettled(&report.settings));
    lines
}

/// How one setting's origin reads on the line beside it.
fn came_from(origin: &Origin) -> String {
    match origin {
        Origin::Bundled => "lemonfiber's own".to_owned(),
        Origin::Operator => "yours".to_owned(),
        Origin::Plugin { named } => format!("set by plugin {named}"),
        Origin::Unknown { .. } => "origin unknown".to_owned(),
        Origin::Overridden { named, replaced } => format!(
            "set by plugin {named}, replacing {} ({})",
            match (&replaced.value, replaced.withheld) {
                (_, true) => "a withheld value",
                (Some(value), false) => value.as_str(),
                (None, false) => "nothing",
            },
            came_from(&replaced.from)
        ),
        Origin::Orphaned { named } => {
            format!("left by plugin {named}, which is no longer installed")
        }
    }
}

/// Why any of them could not be attributed, each reason given once.
///
/// Under the listing rather than on every line. The reasons are sentences and the
/// settings are many, and one repeated per row would bury the values somebody came
/// to read — but leaving the word *unknown* on the page with nothing behind it is
/// how it comes to read as a fault rather than as an honest answer.
fn unsettled(settings: &[SettingReport]) -> Lines {
    let mut lines = Lines::default();
    let mut said: Vec<&str> = Vec::new();
    for setting in settings {
        // The value a plugin replaced has an origin of its own, and an unknown one is
        // explained here as a setting's own is.
        let why = setting.origin.why().or(match &setting.origin {
            Origin::Overridden { replaced, .. } => replaced.from.why(),
            _ => None,
        });
        if let Some(why) = why {
            if !said.contains(&why) {
                said.push(why);
            }
        }
    }
    if said.is_empty() {
        return lines;
    }
    lines.spaced("Where a setting's origin is unknown:");
    for why in said {
        lines.put(format!("  {why}"));
    }
    lines
}

/// What a setting that has never been written reads as on the left of a difference.
///
/// Named rather than blank, because a blank left-hand side reads as a setting whose
/// value is the empty string — which is a different thing, and one the operator would
/// act on differently.
const UNSET: &str = "(not set)";

/// What became of a proposed change, said in the words that follow from it.
///
/// A refusal carries its own reason, so the standing line says only that nothing
/// moved; the sentence underneath is the service's or the file format's, not this
/// renderer's paraphrase of one.
fn verdict(review: &Review, rehearsed: bool) -> Vec<String> {
    match review.stance {
        Stance::Applied => vec!["saved".to_owned()],
        // A rehearsal reports what it would do, so it must not claim it saved.
        Stance::Pending if rehearsed => vec!["would save".to_owned()],
        Stance::Pending => vec![
            "not saved: this change has consequences".to_owned(),
            "run it again with --confirm to apply it".to_owned(),
        ],
        Stance::Unchanged => vec!["already set to that — nothing saved".to_owned()],
        Stance::Blocked => {
            let mut said = vec!["not saved — nothing was changed".to_owned()];
            said.extend(review.refusal.clone());
            said
        }
    }
}

/// One log line, as it should reach a terminal.
///
/// A log line is the least trustworthy text this product shows: it is written by
/// somebody else's container, verbatim, and a container that emits `\x1b[2J` clears
/// the operator's screen. Everything else rendered here goes through [`Lines::put`]
/// and is made plain on the way; a stream has no report to build, so it would
/// otherwise be the one line that skipped it.
///
/// The service is made plain before it is padded rather than after, so a container
/// whose name carries control characters cannot push the column out of true — the
/// width has to be counted on what will actually be drawn.
pub(crate) fn logged(service: &str, line: &str) -> String {
    format!(
        "{:<12} {}",
        lemonfiber_core::text::plain(service),
        lemonfiber_core::text::plain(line)
    )
}

#[cfg(test)]
mod tests;
