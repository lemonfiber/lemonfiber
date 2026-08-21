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

mod doctor;
mod quality;
pub(crate) mod repair;
mod seed;
mod stack;
pub(crate) mod support;
mod trace;
pub(crate) mod walkthrough;

use lemonfiber_core::app::Outcome;
use lemonfiber_core::model::{ConfigReport, FormsReport, SupervisionReport, VersionReport};
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
pub(crate) struct Lines(Vec<String>);

impl Lines {
    /// One line.
    ///
    /// Made plain on the way in, because most of what is shown here came from
    /// somewhere else — a release name from an indexer, a failure message from a
    /// \*arr — and a terminal reads a control character in the middle of one as an
    /// instruction. One place rather than at each caller: a line that skipped it
    /// would be the one carrying the name somebody chose.
    pub(crate) fn put(&mut self, line: impl Into<String>) {
        self.0.push(lemonfiber_core::text::plain(&line.into()));
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
        self.0.push(String::new());
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
        Outcome::Forms(report) => forms(report),
        Outcome::Preview(plan) => stack::preview(plan),
        Outcome::Config(report) => settings(report),
        Outcome::Quality(report) => quality::quality(report),
        Outcome::Upgrade(report) => quality::upgrade(report),
        Outcome::Music(report) => quality::music(report),
        Outcome::Trace(report) => trace::trace(report),
        Outcome::Household(report) => trace::household(report),
        Outcome::Stuck(report) => trace::stuck(report),
        Outcome::Lifecycle(report) => stack::lifecycle(report),
        Outcome::Status(report) => stack::status(report),
        Outcome::Doctor(report) => doctor::diagnosis(report),
        Outcome::Seed(report) => seed::seeding(report),
        Outcome::Reset(report) => stack::reset(report),
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

#[cfg(test)]
mod tests {
    use super::fixtures::{
        a_lifecycle, a_plan, a_trace, a_version, a_watch, music_pick, preset, seed_report,
        some_forms,
    };
    use super::{answer, forms, machine_readable, render, settings, versions, watched, Lines};
    use lemonfiber_core::app::Outcome;
    use lemonfiber_core::docker::Condition;
    use lemonfiber_core::doctor::Overall;
    use lemonfiber_core::model::{
        ConfigReport, Disposition, DoctorReport, FormsReport, HouseholdReport, MusicReport,
        QualityReport, ResetReport, SettingReport, StatusReport, StuckReport, UpgradeReport,
        VersionReport,
    };

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
}
