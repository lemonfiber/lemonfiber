use super::chooser::Chooser;
use super::offer::tests::{a_listing, nothing_declared};
use super::{
    errand, lasting, meaning, mending, quality, question, surface, Acting, Line, Press, Wanted,
};
use crate::render::fixtures::{a_lifecycle, a_plan};
use lemonfiber_core::walkthrough::Line as Step;
// Two consents now, deliberately parallel: a restore's and a repair's. The
// repair one is bare and the restore one is said with its module, which is the
// way `named.rs` tells the same pair apart — a file holding both cannot leave a
// reader to guess which `Consent::Given` a line means.
use lemonfiber_core::app::bundle::Wanted as Bundled;
use lemonfiber_core::app::repair::{Consent, Report as RepairReport};
use lemonfiber_core::app::restore::{self, Kept, Preview, Restoration};
use lemonfiber_core::app::support::{Bundle, Destination};
use lemonfiber_core::app::{backup, Allowance, Command, Outcome, QualityAction, Waiting};
use lemonfiber_core::audio::Format;
use lemonfiber_core::backup::{Manifest, Relocation, Scope, SCHEMA};
use lemonfiber_core::bundle::{Contents, Filenames};
use lemonfiber_core::doctor::{Category, Finding, Narrowing, Overall, Verdict};
use lemonfiber_core::error::{Code, Problem, Remedy, Severity};
use lemonfiber_core::model::{
    Disposition, DoctorReport, FormsReport, PresetChoice, QualityReport, ResetReport, StackEdit,
    StuckEntry, StuckReport, SupervisionReport, TraceReport, UpgradeMedia, UpgradeReport,
    VersionReport,
};
use lemonfiber_core::quality::Preset;
use lemonfiber_core::repair::{agreement, Repair};
use lemonfiber_core::trace::Stage as TraceStage;
use lemonfiber_core::walkthrough::Step as WalkStep;
use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use std::path::PathBuf;

/// The screen, having got as far as the list for one action.
fn choosing(key: char, report: FormsReport) -> (Acting, Wanted) {
    let mut acting = Acting::opened();
    let wanted = acting.pressed(&Press::Typed(key));
    acting.told(Ok(Outcome::Forms(report)));
    (acting, wanted)
}

/// The same, with the services the panels are showing in hand.
///
/// The gather lands first, because that is the order the loop puts them in: the
/// panels are refreshed every second and a key is pressed against whatever they
/// were last showing.
fn holding(key: char) -> Acting {
    let mut acting = Acting::opened();
    acting.gathered(&super::service::tests::two_services());
    acting.pressed(&Press::Typed(key));
    acting.told(Ok(Outcome::Forms(a_listing())));
    acting
}

/// The screen, moved onto the row of this name.
///
/// The row the cursor is on rather than the row containing the name, because a
/// list that takes several draws a box between the cursor and the name — and a
/// walk that matched anywhere would stop on the first row that merely mentioned
/// it.
fn onto(acting: &mut Acting, name: &str) {
    for _ in 0..MOST {
        if showing(acting)
            .lines()
            .any(|line| line.starts_with("> ") && line.contains(name))
        {
            break;
        }
        acting.pressed(&Press::Forward);
    }
}

/// Rows the pane is drawn into here.
///
/// Taller than any list this screen offers, because these tests are about what a
/// list *says* and not about what it does when it will not fit. A screen too short
/// for its list counts what it left out rather than showing part of it, and that
/// is proved where it belongs, in `words`. Left at the height of the longest list,
/// every test that walks to an entry near the end would start failing the day
/// somebody adds one — about the wrong thing.
const TALL: usize = 40;

/// Everything the pane says, as one piece of text.
fn showing(acting: &Acting) -> String {
    acting.pane(TALL, 100).map_or_else(String::new, |pane| {
        let mut said = vec![pane.title.clone()];
        said.extend(pane.lines.iter().map(text));
        said.join("\n")
    })
}

/// One line as text, its spans joined the way the screen shows them.
fn text(line: &Line<'_>) -> String {
    line.spans
        .iter()
        .map(|span| span.content.as_ref())
        .collect::<Vec<&str>>()
        .concat()
}

/// The footer, as text.
fn footing(acting: &Acting) -> String {
    text(&acting.footer(200))
}

/// A version report, which is both an answer this screen asks for and an answer
/// of a shape an action never has.
fn a_version() -> VersionReport {
    VersionReport {
        binary: "0.8.0".to_owned(),
        supported_schema: vec![1],
        stack: "1.2.3".to_owned(),
        compose: None,
        changelog: lemonfiber_core::changelog::Notes::unread(),
    }
}

/// The screen, with the list of questions open at the one named.
///
/// Moved to by name off the list an operator moves over, rather than reached by
/// index: a question added above one of these would silently renumber every test
/// below, and each of them would go on passing about the wrong question.
fn at(name: &str) -> Acting {
    let mut acting = Acting::opened();
    acting.pressed(&Press::Typed(question::KEY));
    let wanted = format!("> {name}");
    for _ in 0..MOST {
        if showing(&acting).contains(&wanted) {
            break;
        }
        acting.pressed(&Press::Forward);
    }
    acting
}

/// More presses than there are questions, so a name nothing matches stops rather
/// than moving for ever.
const MOST: usize = 30;

/// The screen, having taken the question named.
fn asking(name: &str) -> Acting {
    let mut acting = at(name);
    acting.pressed(&Press::Accept);
    acting
}

/// The screen, having got as far as the line where what to follow is typed.
fn asking_where() -> Acting {
    asking("where one thing is")
}

/// A trace of the item a test follows, which is what the second read answers
/// with — a different shape from the listing that led to it.
fn a_trace(item: &str) -> TraceReport {
    TraceReport {
        item: item.to_owned(),
        matched: true,
        ..TraceReport::default()
    }
}

/// A diagnosis whose one check could not be established, which is what an
/// ordinary run reports about both of the checks that disturb — each of them
/// saying to run that one.
fn a_diagnosis() -> DoctorReport {
    DoctorReport {
        overall: Overall::Unknown,
        findings: vec![Finding::in_category(
            Category::Vpn,
            "vpn.killswitch",
            "Traffic stops when the tunnel does",
            Verdict::Unverified {
                reason: "the check that takes the tunnel away has not been asked for".to_owned(),
                remedy: Remedy::new("Run the disruptive check when transfers can be interrupted"),
            },
        )],
    }
}

/// A failure to render, for the paths that report one.
fn a_failure() -> Problem {
    Problem::new(
        Code::new("TEST-1"),
        Severity::Error,
        "the container engine could not be reached",
        "Nothing can be started or stopped until it answers.",
        Remedy::new("Start the container engine"),
    )
}

/// One keypress as this screen reads it.
fn read(code: KeyCode, modifiers: KeyModifiers) -> Option<Press> {
    meaning(KeyEvent::new(code, modifiers))
}

/// The screen, having taken one of the two things that can be done about a
/// diagnosis off the list the `put right` key opens.
fn putting(action: &str) -> (Acting, Wanted) {
    let mut acting = Acting::opened();
    acting.pressed(&Press::Typed(mending::KEY));
    let named = mending::tests::doing(action).name;
    for _ in 0..MOST {
        if showing(&acting).contains(&format!("> {named}")) {
            break;
        }
        acting.pressed(&Press::Forward);
    }
    let wanted = acting.pressed(&Press::Accept);
    (acting, wanted)
}

/// The screen, having taken one errand off the list the `more` key opens.
fn sending(action: &str) -> (Acting, Wanted) {
    let mut acting = Acting::opened();
    acting.pressed(&Press::Typed(errand::KEY));
    onto(&mut acting, &errand::tests::listed(action));
    let wanted = acting.pressed(&Press::Accept);
    (acting, wanted)
}

/// The screen, having taken one quality change off the list its key opens.
fn changing(action: &str) -> (Acting, Wanted) {
    let mut acting = Acting::opened();
    acting.pressed(&Press::Typed(quality::KEY));
    onto(&mut acting, &quality::tests::listed(action));
    let wanted = acting.pressed(&Press::Accept);
    (acting, wanted)
}

/// The screen, having taken one of the two that keep going off its own list.
fn starting(action: &str) -> Acting {
    let mut acting = Acting::opened();
    acting.pressed(&Press::Typed(lasting::KEY));
    let named = lasting::tests::listed(action);
    while !showing(&acting).contains(&format!("> {named}")) {
        acting.pressed(&Press::Forward);
    }
    acting.pressed(&Press::Accept);
    acting
}

mod asking_more;
mod choosing;
mod errands;
mod lasting;
mod quality;
mod questions;
mod repairing;
mod services;
mod widening;
