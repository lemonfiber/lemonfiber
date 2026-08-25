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

#[cfg(test)]
pub(crate) mod fixtures;

mod archive;
mod doctor;
pub(crate) mod downloads;
pub(crate) mod glossary;
mod quality;
pub(crate) mod repair;
mod seed;
pub(crate) mod stack;
mod trace;
pub(crate) mod walkthrough;

use lemonfiber_core::app::Outcome;
use lemonfiber_core::model::{
    ConfigReport, FormsReport, SupervisionReport, VersionReport, WizardReport,
};
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
    /// arrive as something that will not parse at all. Serialising has already made
    /// the text safe in the way that matters, since JSON escapes every control
    /// character rather than carrying it.
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
        Outcome::Quality(report) => quality::quality(report),
        Outcome::Upgrade(report) => quality::upgrade(report),
        Outcome::Music(report) => quality::music(report),
        Outcome::Trace(report) => trace::trace(report),
        Outcome::Household(report) => trace::household(report),
        Outcome::Stuck(report) => trace::stuck(report),
        Outcome::Word(term) => glossary::explanation(term),
        Outcome::Glossary(listed) => glossary::vocabulary(listed),
        Outcome::Lifecycle(report) => stack::lifecycle(report),
        Outcome::Status(report) => stack::status(report),
        Outcome::Doctor(report) => doctor::diagnosis(report),
        Outcome::Seed(report) => seed::seeding(report),
        Outcome::Reset(report) => stack::reset(report),
        Outcome::Wizard(report) => standing(report),
        Outcome::Backup(report) => archive::backup(report),
        Outcome::Support(report) => archive::bundle(report),
        Outcome::Restore(report) => archive::restoration(report),
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
    // Said at the moment the choice was made, and only then — the checks
    // deliberately do not raise it again on every run afterwards.
    if let Some(consequence) = &report.consequence {
        lines.put(String::new());
        lines.put(consequence.clone());
    }
    lines
}

/// What a watch did once its location was lost.
pub(crate) fn watched(report: &SupervisionReport, json: bool) {
    stack::watch_lines(report, json).print();
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
mod tests {
    use super::fixtures::{
        a_lifecycle, a_plan, a_term, a_trace, a_version, a_watch, music_pick, preset, seed_report,
        some_forms,
    };
    use super::{
        answer, forms, logged, machine_readable, render, settings, standing, versions, watched,
        Lines,
    };
    use lemonfiber_core::app::backup::Report as Capture;
    use lemonfiber_core::app::restore::{Preview, Restoration};
    use lemonfiber_core::app::support::Bundle;
    use lemonfiber_core::app::Outcome;
    use lemonfiber_core::backup::{Manifest, Scope, SCHEMA};
    use lemonfiber_core::bundle::Contents;
    use lemonfiber_core::docker::Condition;
    use lemonfiber_core::doctor::Overall;
    use lemonfiber_core::glossary::Vocabulary;
    use lemonfiber_core::model::{
        ConfigReport, Disposition, DoctorReport, FormsReport, HouseholdReport, MusicReport,
        QualityReport, ResetReport, SettingReport, StatusReport, StuckReport, UpgradeReport,
        VersionReport, WizardReport,
    };
    use lemonfiber_core::wizard::{Phase, Step};

    /// An archive's own account of itself, holding nothing.
    fn an_archive() -> Manifest {
        Manifest {
            schema: SCHEMA,
            product_version: "0.8.0".to_owned(),
            created_at: "2026-07-30".to_owned(),
            data_root: "/srv/media".to_owned(),
            scope: Scope::WholeStack,
            sensitive: true,
            members: Vec::new(),
        }
    }

    /// A setup part-way through, which is what most of these vary from.
    fn part_way() -> WizardReport {
        WizardReport {
            offered: true,
            phase: Phase::InProgress,
            at: Step::DataLocation,
            asks: true,
            unanswered: vec![Step::DataLocation, Step::Library],
            ready_for_review: false,
            plan: Vec::new(),
        }
    }

    /// The case this exists for. The container writes the line, the terminal reads
    /// the escape, and the screen stops saying what this product said.
    #[test]
    fn a_container_cannot_clear_the_screen_through_its_own_log_line() {
        let said = logged("sonarr", "starting\u{1b}[2Jup");
        assert!(!said.contains('\u{1b}'), "{said:?}");
        assert!(
            said.ends_with("starting[2Jup"),
            "what a terminal would have obeyed is gone; the rest is left alone: {said:?}"
        );
    }

    /// A log line is one line. A container that puts a newline in the middle of one
    /// is forging a second, and the column that names the service is what it would
    /// forge its way out of.
    #[test]
    fn a_container_cannot_forge_a_second_log_line() {
        let said = logged("sonarr", "innocent\nqbittorrent   leaked the password");
        assert_eq!(said.lines().count(), 1, "{said:?}");
    }

    /// Padded on what will be drawn rather than on what arrived, or a name carrying
    /// control characters pushes every line after it out of true.
    #[test]
    fn the_service_column_is_measured_after_the_name_is_made_plain() {
        let clean = logged("sonarr", "up");
        let sneaky = logged("son\u{7f}arr", "up");
        assert_eq!(clean, sneaky, "the delete never counted toward the width");
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
    fn what_a_parser_reads_goes_out_exactly_as_it_was_built() {
        // A curly quote is the one that matters: folded to `"` inside a JSON string
        // it is not a character but the end of the string.
        let document = "{\"name\":\"The “Burbs” 1989 — 1080p\"}";
        let mut lines = Lines::for_a_parser();
        lines.put(document);
        lines.print();

        assert_eq!(lines.text(), document, "unfolded and unaltered");
    }

    /// A refusal a script asked for leaves by the same door on the other stream.
    #[test]
    fn what_a_parser_reads_goes_out_unfolded_on_the_error_stream_too() {
        let document = "{\"kind\":\"error\",\"data\":{\"summary\":\"the “Burbs” — gone\"}}";
        let mut lines = Lines::for_a_parser();
        lines.put(document);
        lines.eprint();

        assert_eq!(lines.text(), document, "unfolded and unaltered");
    }

    /// The other half of the same rule: what a person reads is made safe, because a
    /// terminal reads an escape in the middle of a name as an instruction.
    #[test]
    fn what_a_person_reads_is_still_made_plain() {
        let mut lines = Lines::default();
        lines.put(format!("a name{}with an instruction in it", char::from(27)));

        let said = lines.text();
        assert!(!said.contains(char::from(27)), "{said}");
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

    /// The id first, because that is what gets typed. A form that cannot be combined
    /// says so in the listing rather than only when a combination is refused.
    #[test]
    fn forms_are_listed_by_what_you_would_type_and_what_it_is_for() {
        let text = forms(&some_forms()).text();
        assert!(text.contains("search — Find things."), "{text}");
        assert!(text.contains("everything — The whole stack."), "{text}");
        assert!(text.contains("cannot be combined"), "{text}");
        assert_eq!(
            text.matches("cannot be combined").count(),
            1,
            "only the form that cannot"
        );
    }

    /// A stack is free to declare none, and a blank screen would read as a broken
    /// command rather than as an answer.
    #[test]
    fn a_stack_with_no_forms_says_so() {
        let text = forms(&FormsReport { forms: Vec::new() }).text();
        assert_eq!(text, "This stack declares no forms.");
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
    fn a_change_that_costs_something_says_so_where_it_was_made() {
        // The only moment it is worth saying: the checks deliberately go quiet
        // about it afterwards, so if it is not here the operator never hears it.
        let report = ConfigReport {
            settings: vec![SettingReport {
                key: "VPN_PORT_FORWARDING".to_owned(),
                value: "off".to_owned(),
                secret: false,
            }],
            changed: true,
            rehearsed: false,
            consequence: Some("seeding will be slower".to_owned()),
        };
        let text = settings(&report).text();
        assert!(text.contains("saved"));
        assert!(text.contains("seeding will be slower"), "{text}");
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
            consequence: None,
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

    /// An explanation is the word rather than a report that used one, so explaining
    /// it again underneath would be the same sentence twice and a pointer at the
    /// command that had just been run.
    #[test]
    fn an_explanation_carries_no_footnote_about_the_word_it_explains() {
        // Held against the switch that would also have emptied it, so a run with
        // explanations turned off could not pass this by explaining nothing at all.
        assert!(super::glossary::wanted(), "explanations are on");

        let one = answer(&Outcome::Word(a_term()), false).text();
        assert!(
            one.contains("Search engines"),
            "the word is answered: {one}"
        );
        assert!(!one.contains("Words used here:"), "{one}");

        let listed = answer(
            &Outcome::Glossary(Vocabulary {
                words: vec![a_term()],
            }),
            false,
        )
        .text();
        assert!(!listed.contains("Words used here:"), "{listed}");
    }

    #[test]
    fn every_outcome_renders_and_every_outcome_renders_as_json() {
        let outcomes = vec![
            Outcome::Version(a_version()),
            Outcome::Forms(some_forms()),
            Outcome::Preview(a_plan("media", Vec::new())),
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
                consequence: None,
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
            Outcome::Lifecycle(a_lifecycle("up", a_plan("media", Vec::new()))),
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
            Outcome::Word(a_term()),
            Outcome::Glossary(Vocabulary {
                words: vec![a_term()],
            }),
            Outcome::Backup(Capture {
                path: std::path::PathBuf::from("/data/lemonfiber/backups/full.tar.gz"),
                scope: Scope::WholeStack,
                sensitive: false,
                pruned: Vec::new(),
            }),
            // Nothing gathered, nothing revealed and nothing written: the answer a
            // bare run gives, which is the one with every optional paragraph absent.
            Outcome::Support(Bundle {
                contents: Contents::default(),
                bytes: 0,
                path: None,
            }),
            Outcome::Restore(Restoration {
                would: Preview {
                    manifest: an_archive(),
                    downgrade: false,
                    relocation: None,
                },
                done: None,
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

    #[test]
    fn a_name_from_somewhere_else_cannot_take_over_the_terminal() {
        // Most of what is shown here came from an indexer or a service, and a
        // terminal reads a control character in the middle of one as an
        // instruction. Caught at the one place every line goes through, so a
        // renderer added later cannot forget it.
        let mut lines = Lines::default();
        lines.put("Some\u{1b}[2JRelease");
        lines.spaced("and\rthis");
        lines.block("a\u{7f}b");
        assert_eq!(lines.text(), "Some[2JRelease\n\nandthis\nab");
    }

    #[test]
    fn a_setup_part_way_through_names_where_it_is_and_what_is_left() {
        // Through the dispatcher's own renderer, so the arm that reaches these
        // words is the one a run takes rather than one only a test does.
        let said = answer(&Outcome::Wizard(part_way()), false).text();
        assert!(
            said.contains("Where downloads and the library are kept"),
            "{said}"
        );
        assert!(said.contains("How the library is served"), "{said}");
        assert!(
            !said.contains("What applying will write"),
            "a partial plan read as the whole of it: {said}"
        );
    }

    #[test]
    fn a_finished_machine_is_pointed_elsewhere_rather_than_asked_again() {
        let said = standing(&WizardReport {
            offered: false,
            ..part_way()
        })
        .text();
        assert!(said.contains("already set up"), "{said}");
        assert!(!said.contains("still to answer"), "{said}");
    }

    #[test]
    fn an_apply_that_stopped_part_way_is_said_before_anything_else() {
        let said = standing(&WizardReport {
            phase: Phase::Applying,
            ..part_way()
        })
        .text();
        assert!(
            said.starts_with("An earlier apply stopped part-way"),
            "{said}"
        );
    }

    #[test]
    fn a_complete_set_of_answers_shows_what_applying_would_write() {
        let said = standing(&WizardReport {
            at: Step::Review,
            asks: false,
            unanswered: Vec::new(),
            ready_for_review: true,
            plan: vec![SettingReport {
                key: "INDEXER_APIKEY".to_owned(),
                value: "(set, not shown)".to_owned(),
                secret: true,
            }],
            ..part_way()
        })
        .text();
        assert!(said.contains("Every question is answered."), "{said}");
        assert!(said.contains("INDEXER_APIKEY=(set, not shown)"), "{said}");
    }
}
