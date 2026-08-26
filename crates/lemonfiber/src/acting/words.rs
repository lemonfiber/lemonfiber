//! What an action puts on the screen, as lines.
//!
//! Lines rather than widgets, for the reason the panels beside them are: what the
//! screen *says* is the part worth proving, and where the box goes is not. Every
//! one of these is a pure function over where the action stands, so the words are
//! read back without a terminal anywhere near them.
//!
//! Nothing here asks for a colour. Severity is not what this screen carries — a
//! question, a list and a report are none of them warnings — so what marks the
//! secondary line is dimming, which is an attribute rather than a colour and
//! survives a terminal that has been told to use none.
//!
//! What each of them says is here; the three arrangements a box is ever drawn in —
//! a list, a line to type on, an answer to move through — are in [`shapes`], because
//! those belong to no one flow and two flows drawing a list differently is how one
//! screen becomes two.

mod shapes;

use lemonfiber_core::text::plain;
use ratatui::text::Line;

use super::errand::{self, Errand};
use super::lasting::{self, Begun, Lasting};
use super::offer::{Offer, Taken, OFFERED};
use super::quality::{self, Change};
use super::question::{self, Question};
use super::reading::Reading;
use super::{surface, Stage};
use lemonfiber_core::plural::s;
use shapes::{agreed, choosing, dimmed, elsewhere, named, read, shortened, typing, AGREEING};

/// The keys the screen answers whatever else is open.
const ALWAYS: &str = "q quit   r refresh   ? words";

/// What the box holding a report calls itself.
const CAME: &str = " what it came to ";

/// What the box holding the questions calls itself.
const ASK: &str = " ask ";

/// What the box holding the rest of the errands calls itself.
const MORE: &str = " more ";

/// What the box holding the two that keep going calls itself.
const KEEPS_GOING: &str = " keeps going ";

/// What the box asking about the web surface calls itself.
const WEB: &str = " web interface ";

/// What the box holding the three quality changes calls itself.
const QUALITY: &str = " quality ";

/// What the box says while the core is working out what a change would cost.
const COSTING: &str = "working out what this would cost";

/// What the box says while a walk is running and has not said anything yet.
///
/// Said rather than shown as an empty box: a walk's first step arrives once the
/// indexers have been asked, which is seconds, and a box with nothing in it reads as
/// a walk that never started.
const WALKING: &str = "asking the services";

/// How a walk that is running is left, and how what it has said is moved through.
const WATCHING: &str = "up and down move   q closes the screen and waits for it";

/// How a guard is ended, and what leaving it does instead.
const GUARDING: &str = "esc lets it go   q closes the screen and leaves it guarding";

/// What the box says while the core is working out what an errand would do.
const WEIGHING: &str = "working out what this would do";

/// What the box says while a question is with the stack.
const ANSWERING: &str = "waiting for this stack to answer";

/// What is said beside a running action, and what leaving does about it.
///
/// There is no daemon behind this screen. The process drawing it is the one that
/// claimed the stack and issued the command, so leaving is not a tab being closed
/// on a server that carries on: the screen goes at once, and the run stays until
/// the action it started has finished and given the stack back.
const WAITING: &str = "still running   q closes the screen and waits for it";

/// A box over the screen: what it is called, and what it says.
pub(crate) struct Pane {
    /// What the box is called, on its own border.
    pub(crate) title: String,
    /// What it says.
    pub(crate) lines: Vec<Line<'static>>,
}

/// The box over the screen, or nothing where an action has none open.
///
/// A running action has none, and neither has an errand under way. The screen behind
/// them is the report — the panels go on gathering every second while the work runs,
/// so what the action is doing to the services shows where the services are listed,
/// and a box over that would take away the one thing worth watching.
pub(super) fn pane(stage: &Stage, rows: usize, across: usize) -> Option<Pane> {
    // What each arm decides is the two things a box is: what it is called, and what
    // it says. The box itself is put together once underneath, so an arm cannot come
    // to disagree with the others about what a box is made of.
    let (title, lines) = match stage {
        Stage::Idle
        | Stage::Running { .. }
        | Stage::Doing { .. }
        | Stage::Applying { .. }
        | Stage::Keeping { said: None, .. } => return None,
        Stage::Choosing { offer, chooser } => (titled(offer), choosing(chooser, rows, across)),
        Stage::Confirming { offer, taken } => {
            (titled(offer), confirming(offer, taken, rows, across))
        }
        Stage::Came(reading) => (CAME.to_owned(), read(reading, rows, across)),
        Stage::Wondering(chooser) => (ASK.to_owned(), choosing(chooser, rows, across)),
        Stage::Typing { asks, typed, .. } => (ASK.to_owned(), typing(asks, typed, across)),
        Stage::Waiting(question) => (asked(question), vec![dimmed(ANSWERING, across)]),
        Stage::Answered { question, reading } => (asked(question), read(reading, rows, across)),
        Stage::Sending(chooser) => (MORE.to_owned(), choosing(chooser, rows, across)),
        Stage::Naming {
            errand,
            asks,
            typed,
        } => (sending(errand), typing(asks, typed, across)),
        Stage::Weighing { errand, .. } => (sending(errand), vec![dimmed(WEIGHING, across)]),
        Stage::Agreeing {
            errand,
            typed,
            would,
        } => (
            sending(errand),
            agreeing(errand, typed, would.as_ref(), rows, across),
        ),
        Stage::Starting(chooser) => (KEEPS_GOING.to_owned(), choosing(chooser, rows, across)),
        Stage::Wording {
            lasting,
            asks,
            typed,
        } => (keeping(lasting), typing(asks, typed, across)),
        Stage::Picking { lasting, chooser } => (keeping(lasting), choosing(chooser, rows, across)),
        Stage::Beginning { lasting, begun } => {
            (keeping(lasting), beginning(lasting, begun, rows, across))
        }
        // A walk's steps are its whole report and nothing behind this box is showing
        // them, so the box stays. A guard says nothing until it ends and what it is
        // guarding is the panels underneath, which is why it is up there with the
        // running things that have no box of their own.
        Stage::Keeping {
            lasting,
            said: Some(reading),
            ..
        } => (keeping(lasting), walking(reading, rows, across)),
        Stage::Deciding(chooser) => (QUALITY.to_owned(), choosing(chooser, rows, across)),
        Stage::Grading { change, chooser } => (changing(change), choosing(chooser, rows, across)),
        Stage::Costing { change } => (changing(change), vec![dimmed(COSTING, across)]),
        Stage::Settling {
            change,
            chosen,
            account,
        } => (
            changing(change),
            settling(change, *chosen, account.as_ref(), rows, across),
        ),
        Stage::Handing => (WEB.to_owned(), handing(across)),
    };
    Some(Pane { title, lines })
}

/// The one line at the foot of the screen: the keys, or what is running.
///
/// The keys give way to the running action rather than sitting beside it. There is
/// one row, an operator who has just started something is owed what it is more than
/// they are owed a list they have just used, and the keys come back the moment it
/// is over.
pub(super) fn footer(stage: &Stage, across: usize) -> Line<'static> {
    let said = match stage {
        Stage::Running { offer, taken } => {
            format!("{} {}   {WAITING}", offer.hint, taken.name())
        }
        Stage::Doing { errand, .. } => format!("{}   {WAITING}", errand.name),
        Stage::Applying { change, .. } => format!("{}   {WAITING}", change.name),
        // The one with no ending of its own says how it is ended; the one that ends
        // by itself says what leaving does, as every other running thing here does.
        Stage::Keeping {
            lasting,
            named,
            ends,
            ..
        } => format!(
            "{}   {}",
            doing(lasting, named),
            if *ends { GUARDING } else { WATCHING }
        ),
        _ => keys(),
    };
    dimmed(&said, across)
}

/// What a run leaving this screen now would stay for, or nothing where it may go.
///
/// Only what changes something. A read is with the core the same way and claims
/// nothing, so a screen left with one outstanding has nothing to stay for — which is
/// why this asks about [`Stage::Running`] and not about [`Stage::Waiting`]. A guard
/// that has already been let go has nothing to stay for either: it is no longer this
/// stage by then.
///
/// Said on the ordinary terminal once the screen is given back, where there is room
/// for the whole of it and no width to fit — so this is the one line here that goes
/// through [`plain`] alone rather than through [`shortened`].
pub(super) fn staying_for(stage: &Stage) -> Option<String> {
    let said = match stage {
        Stage::Running { offer, taken } => waited(&format!("{} {}", offer.hint, taken.name())),
        Stage::Doing { errand, .. } => waited(errand.name),
        Stage::Applying { change, .. } => waited(change.name),
        // The one that never ends by itself is the one this cannot say "to finish"
        // about. What it will go on doing, and the one thing that ends it once the
        // screen is gone, are said instead — a run held open on a promise nobody
        // explained is exactly what an operator reads as a hang.
        Stage::Keeping {
            lasting,
            named,
            ends: true,
            ..
        } => format!(
            "{} is still running — nothing more will happen until the data location is \
             lost, and Ctrl-C ends it",
            doing(lasting, named)
        ),
        // And this one does end, in minutes, but it does not reach the container
        // engine and claims nothing — so the reason to wait for it is that there is
        // nothing else to carry it, not that it holds the stack.
        Stage::Keeping { lasting, named, .. } => format!(
            "waiting for {} to finish — nothing else can carry it once this run has gone",
            doing(lasting, named)
        ),
        _ => return None,
    };
    Some(plain(&said))
}

/// What is said about a run left with something that claimed the stack to finish.
fn waited(doing: &str) -> String {
    format!("waiting for {doing} to finish — leaving it now would leave the stack claimed")
}

/// Every key this screen answers, in the order they are worth reading.
///
/// Built from the offers rather than written out, so an action added to the table
/// is an action the operator is told about.
fn keys() -> String {
    let mut said = vec![
        ALWAYS.to_owned(),
        format!("{} {}", question::KEY, question::HINT),
        format!("{} {}", errand::KEY, errand::HINT),
        format!("{} {}", lasting::KEY, lasting::HINT),
        format!("{} {}", quality::KEY, quality::HINT),
        format!("{} {}", surface::KEY, surface::HINT),
    ];
    said.extend(
        OFFERED
            .iter()
            .map(|offer| format!("{} {}", offer.key, offer.hint)),
    );
    said.join("   ")
}

/// What the box holding one action is called.
fn titled(offer: &Offer) -> String {
    format!(" {} ", offer.hint)
}

/// What the box holding one question's answer is called.
fn asked(question: &Question) -> String {
    format!(" {} ", question.name)
}

/// What the box holding one errand is called.
fn sending(errand: &Errand) -> String {
    format!(" {} ", errand.name)
}

/// The question before an errand, under what it would do where the errand could say.
fn agreeing(
    errand: &Errand,
    typed: &str,
    would: Option<&Reading>,
    rows: usize,
    across: usize,
) -> Vec<Line<'static>> {
    agreed(
        &format!("{} {typed}", errand.asks),
        errand.about,
        would,
        rows,
        across,
    )
}

/// What the box holding one quality change is called.
fn changing(change: &Change) -> String {
    format!(" {} ", change.name)
}

/// The question before a quality change, under the account where there is one.
///
/// The preset completes the question where one was chosen, the way a form completes
/// an action's. Where none was, the question is whole on its own: neither of the
/// other two is given anything, and a sentence left hanging for a subject that does
/// not exist would be asking about nothing.
fn settling(
    change: &Change,
    chosen: Option<&str>,
    account: Option<&Reading>,
    rows: usize,
    across: usize,
) -> Vec<Line<'static>> {
    let asks = match chosen {
        Some(chosen) => format!("{} {chosen}", change.asks),
        None => change.asks.to_owned(),
    };
    agreed(&asks, change.about, account, rows, across)
}

/// What the box holding one of the two that keep going is called.
fn keeping(lasting: &Lasting) -> String {
    format!(" {} ", lasting.name)
}

/// What one of them is called while it runs, with what it was given.
///
/// The name alone where nothing was named, which is what a walk asked for nothing in
/// particular is: there is no subject to say, and inventing one would be this screen
/// naming something the operator did not.
fn doing(lasting: &Lasting, named: &str) -> String {
    format!("{} {named}", lasting.name).trim_end().to_owned()
}

/// The question before one of them, and what it was given.
fn beginning(lasting: &Lasting, begun: &Begun, rows: usize, across: usize) -> Vec<Line<'static>> {
    let asked = match begun {
        Begun::Chosen(taken) => format!("{} {}", lasting.asks, taken.name()),
        // A walk asked for nothing in particular is a request of its own rather than
        // a half-finished one, so it is put as one — "walk through?" asks nothing at
        // all, and an operator answering it would not know what they had agreed to.
        Begun::Looked(typed) if typed.trim().is_empty() => lasting::ANYTHING.to_owned(),
        Begun::Looked(typed) => format!("{} {typed}", lasting.asks),
    };
    let mut lines = vec![Line::raw(shortened(&format!("{asked}?"), across))];
    if let Begun::Chosen(taken) = begun {
        // Four rows are kept back for the question, the line under it, the blank and
        // the hint, so the forms being named never grow over the thing being agreed
        // to.
        lines.extend(covering(taken, rows.saturating_sub(4), across));
    }
    lines.push(dimmed(lasting.about, across));
    lines.push(Line::raw(""));
    lines.push(dimmed(AGREEING, across));
    lines
}

/// What a walk has said so far, or that it has not said anything yet.
fn walking(reading: &Reading, rows: usize, across: usize) -> Vec<Line<'static>> {
    let (shown, above, below) = reading.window(rows.saturating_sub(2));
    let mut lines: Vec<Line<'static>> = if shown.is_empty() {
        vec![dimmed(WALKING, across)]
    } else {
        shown
            .into_iter()
            .map(|line| Line::raw(shortened(line, across)))
            .collect()
    };
    if let Some(place) = elsewhere(above, below) {
        lines.push(dimmed(&place, across));
    }
    lines.push(Line::raw(""));
    lines.push(dimmed(WATCHING, across));
    lines
}

/// The question before the terminal is handed to the web surface.
fn handing(across: usize) -> Vec<Line<'static>> {
    vec![
        Line::raw(shortened(&format!("{}?", surface::ASKS), across)),
        dimmed(surface::ABOUT, across),
        Line::raw(""),
        dimmed(AGREEING, across),
    ]
}

/// The question before an action, and what it is being asked about.
///
/// One name is answered with the one line under it the list already showed. Several
/// are answered with the names themselves: agreeing to a teardown of four forms is
/// agreeing to four names, and a box saying only "4 forms" would be asking somebody
/// to remember what they had marked a moment ago.
fn confirming(offer: &Offer, taken: &Taken, rows: usize, across: usize) -> Vec<Line<'static>> {
    let mut lines = vec![Line::raw(shortened(
        &format!("{} {}?", offer.asks, taken.name()),
        across,
    ))];
    match taken.covers.as_slice() {
        [only] => lines.push(dimmed(&only.about, across)),
        // Three rows are kept back for the question, the blank and the hint under it.
        _ => lines.extend(covering(taken, rows.saturating_sub(3), across)),
    }
    lines.push(Line::raw(""));
    lines.push(dimmed(AGREEING, across));
    lines
}

/// The forms a question covers, where it covers more than one.
///
/// Nothing at all for a single, whose name is already in the question above. What
/// will not fit is counted rather than dropped, because a list of four that showed
/// two would be asking for agreement to something other than what it displayed.
fn covering(taken: &Taken, room: usize, across: usize) -> Vec<Line<'static>> {
    let covers = &taken.covers;
    if covers.len() < 2 {
        return Vec::new();
    }
    let mut lines: Vec<Line<'static>> = covers
        .iter()
        .take(room)
        .map(|choice| named(&choice.name, &choice.about, across))
        .collect();
    let left = covers.len().saturating_sub(lines.len());
    if left > 0 {
        lines.push(dimmed(
            &format!("{left} more form{} than this screen has room for", s(left)),
            across,
        ));
    }
    lines
}

#[cfg(test)]
mod tests {
    use super::{footer, keys, pane, staying_for, Offer, Stage};
    use crate::acting::chooser::Chooser;
    use crate::acting::errand::{self, Errand};
    use crate::acting::offer::{Choice, Taken, OFFERED};
    use crate::acting::question::{Needed, Question, HINT, KEY};
    use crate::acting::reading::Reading;
    use lemonfiber_core::app::Command;
    use ratatui::text::Line;

    /// One action, held here rather than taken out of the table by number, since
    /// what these read is what is drawn and not which action it was.
    static A_START: Offer = Offer {
        key: 'u',
        action: "up",
        hint: "start",
        asks: "Start",
    };

    /// One question that takes a word, held here for the same reason.
    static A_TRACE: Question = Question {
        name: "where one thing is",
        about: "follow one show or film across the services",
        read: "/api/trace",
        needs: Needed::Term("What to follow"),
    };

    /// A reading over nine numbered lines.
    fn nine() -> Reading {
        Reading::of((0..9).map(|at| format!("line {at}")).collect())
    }

    /// A choice by name, for the tests that only read what is drawn.
    fn a_choice(name: &str, about: &str) -> Choice {
        Choice {
            name: name.to_owned(),
            about: about.to_owned(),
            forms: vec![name.to_owned()],
            marked: false,
            command: Command::Up {
                forms: vec![name.to_owned()],
            },
        }
    }

    /// One choice taken on its own, which is what a list with nothing marked on it
    /// comes to when enter is pressed over that row.
    fn a_taking(name: &str, about: &str) -> Taken {
        Taken {
            command: Command::Up {
                forms: vec![name.to_owned()],
            },
            covers: vec![a_choice(name, about)],
        }
    }

    /// Several taken together, which is what marking them and pressing enter comes
    /// to.
    fn several() -> Taken {
        Taken {
            command: Command::Up {
                forms: vec!["full".to_owned(), "lean".to_owned()],
            },
            covers: vec![
                a_choice("Full stack", "everything, behind the tunnel"),
                a_choice("Lean stack", "the download clients only"),
            ],
        }
    }

    /// One line as text, its spans joined the way the screen shows them.
    fn text(line: &Line<'_>) -> String {
        line.spans
            .iter()
            .map(|span| span.content.as_ref())
            .collect::<Vec<&str>>()
            .concat()
    }

    /// Everything a pane says, as one piece of text.
    fn said(stage: &Stage, rows: usize, across: usize) -> String {
        pane(stage, rows, across).map_or_else(String::new, |drawn| {
            let mut all = vec![drawn.title.clone()];
            all.extend(drawn.lines.iter().map(text));
            all.join("\n")
        })
    }

    /// The errand one action is on, taken from the list the screen really offers
    /// rather than built here — what these read is the words an operator gets.
    fn sent(action: &str) -> &'static Errand {
        let (first, rest) = errand::all();
        std::iter::once(first)
            .chain(rest)
            .find(|errand| errand.action == action)
            .unwrap_or(first)
    }

    /// A chooser over two, for the list tests.
    fn two() -> Chooser<Choice> {
        Chooser::over(
            a_choice("Full stack", "everything, behind the tunnel"),
            vec![a_choice("Lean stack", "the download clients only")],
        )
    }

    /// Every action is on the footer, or the operator has no way to learn a key
    /// exists — a screen whose only account of what it can do is its source.
    #[test]
    fn the_footer_names_every_key_this_screen_answers() {
        let said = keys();

        assert!(said.contains("q quit"), "{said}");
        assert!(said.contains(&format!("{KEY} {HINT}")), "{said}");
        assert!(
            said.contains(&format!("{} {}", errand::KEY, errand::HINT)),
            "{said}"
        );
        for offer in OFFERED {
            assert!(
                said.contains(offer.hint),
                "{} is missing: {said}",
                offer.hint
            );
            assert!(said.contains(offer.key), "{} is missing: {said}", offer.key);
        }
    }

    /// While something is running the footer says so, and says what leaving does —
    /// which is the one thing an operator watching a teardown needs to know. What it
    /// must not say is that the work outlives the screen: the process drawing it is
    /// the one carrying the work out.
    #[test]
    fn a_running_action_says_what_leaving_the_screen_does_about_it() {
        let stage = Stage::Running {
            offer: &A_START,
            taken: a_taking("Full stack", "everything"),
        };

        let said = text(&footer(&stage, 120));

        assert!(said.contains("Full stack"), "{said}");
        assert!(said.contains("still running"), "{said}");
        assert!(said.contains("closes the screen and waits"), "{said}");
        assert!(!said.contains("the work goes on"), "{said}");
    }

    /// Leaving mid-action says which action is being waited on and why, because an
    /// operator who pressed q and got a wait instead of a shell is owed both.
    #[test]
    fn leaving_mid_action_says_what_is_being_waited_on_and_why() {
        let stage = Stage::Running {
            offer: &A_START,
            taken: a_taking("Full stack", "everything"),
        };

        let said = staying_for(&stage).unwrap_or_default();

        assert!(said.contains("start Full stack"), "{said}");
        assert!(said.contains("leave the stack claimed"), "{said}");
        assert!(
            staying_for(&Stage::Idle).is_none(),
            "nothing runs, nothing said"
        );
    }

    /// A form's own name reaches an ordinary terminal here rather than a drawn row,
    /// and a control character is an instruction to both.
    #[test]
    fn a_name_from_somewhere_else_is_made_safe_before_it_is_said() {
        let stage = Stage::Running {
            offer: &A_START,
            taken: a_taking("Full\u{1b}[2Jstack", "everything"),
        };

        let said = staying_for(&stage).unwrap_or_default();

        assert!(!said.contains('\u{1b}'), "{said:?}");
        assert!(said.contains("Full[2Jstack"), "{said}");
    }

    /// A running action draws no box, because the panels behind it are the report.
    #[test]
    fn a_running_action_leaves_the_screen_behind_it_visible() {
        let stage = Stage::Running {
            offer: &A_START,
            taken: a_taking("Full stack", "everything"),
        };

        assert!(pane(&stage, 20, 80).is_none());
        assert!(pane(&Stage::Idle, 20, 80).is_none());
        assert!(text(&footer(&Stage::Idle, 120)).contains("r refresh"));
    }

    /// The list says what each choice is for, marks the one selected, shows every
    /// row as one that could be marked, and says how to take it.
    #[test]
    fn the_list_marks_what_is_selected_and_says_how_to_take_it() {
        let stage = Stage::Choosing {
            offer: &A_START,
            chooser: two(),
        };

        let said = said(&stage, 20, 80);

        assert!(said.contains("> [ ] Full stack"), "{said}");
        assert!(said.contains("  [ ] Lean stack"), "{said}");
        assert!(said.contains("the download clients only"), "{said}");
        assert!(said.contains("space marks"), "{said}");
    }

    /// The line under the list says what enter would do, and changes when that
    /// changes — which is the whole of how the one ambiguity on a list that takes
    /// several is resolved. A rule an operator has to remember is a rule they will
    /// get wrong once, on the teardown.
    #[test]
    fn the_line_under_a_list_says_which_thing_enter_would_take() {
        let mut chooser = two();
        let nothing = said(
            &Stage::Choosing {
                offer: &A_START,
                chooser: two(),
            },
            20,
            80,
        );

        for (_, choice) in chooser.each() {
            choice.marked = true;
        }
        let both = said(
            &Stage::Choosing {
                offer: &A_START,
                chooser,
            },
            20,
            80,
        );

        assert!(nothing.contains("enter takes this one"), "{nothing}");
        assert!(!nothing.contains("marked"), "{nothing}");
        assert!(both.contains("enter takes the 2 marked"), "{both}");
        assert!(both.contains("> [x] Full stack"), "{both}");
        assert!(both.contains("  [x] Lean stack"), "{both}");
    }

    /// A list that takes one draws no box beside its rows and offers no key for
    /// marking, because there is nothing on it that could be taken with another.
    #[test]
    fn a_list_that_takes_one_offers_nothing_to_mark() {
        let said = said(
            &Stage::Wondering(Chooser::over(&A_TRACE, Vec::new())),
            20,
            80,
        );

        assert!(said.contains("> where one thing is"), "{said}");
        assert!(!said.contains("[ ]"), "{said}");
        assert!(!said.contains("space marks"), "{said}");
        assert!(said.contains("enter goes on"), "{said}");
    }

    /// Several named together are named in the question, one row each. A box saying
    /// only how many there were would be asking somebody to remember what they
    /// marked a moment ago, which on a teardown is the wrong thing to be unsure of.
    #[test]
    fn the_question_over_several_names_every_one_of_them() {
        let stage = Stage::Confirming {
            offer: &A_START,
            taken: several(),
        };

        let said = said(&stage, 20, 80);

        assert!(said.contains("Start 2 forms?"), "{said}");
        assert!(said.contains("Full stack"), "{said}");
        assert!(said.contains("Lean stack"), "{said}");
        assert!(said.contains("the download clients only"), "{said}");
        assert!(said.contains("any other key changes nothing"), "{said}");
    }

    /// More forms than the box has room for are counted rather than dropped: a
    /// question that displayed two of four would be asking for agreement to
    /// something other than what it showed.
    #[test]
    fn a_question_over_more_forms_than_fit_counts_the_rest() {
        let stage = Stage::Confirming {
            offer: &A_START,
            taken: several(),
        };

        let said = said(&stage, 4, 80);

        assert!(said.contains("Start 2 forms?"), "{said}");
        assert!(
            said.contains("1 more form than this screen has room for"),
            "{said}"
        );
    }

    /// A list longer than the screen counts what it left out rather than showing
    /// one of two as though there were one.
    #[test]
    fn a_list_too_long_for_the_screen_counts_what_it_left_out() {
        let stage = Stage::Choosing {
            offer: &A_START,
            chooser: two(),
        };

        let short = said(&stage, 3, 80);
        let whole = said(&stage, 20, 80);

        assert!(short.contains("1 more choice"), "{short}");
        assert!(!whole.contains("more choice"), "{whole}");
    }

    /// The question names the action and what it is about, and says which way
    /// saying nothing falls.
    #[test]
    fn the_question_names_what_is_about_to_happen_and_to_what() {
        let stage = Stage::Confirming {
            offer: &A_START,
            taken: a_taking("Full stack", "everything, behind the tunnel"),
        };

        let said = said(&stage, 20, 80);

        assert!(said.contains("Start Full stack?"), "{said}");
        assert!(said.contains("everything, behind the tunnel"), "{said}");
        assert!(said.contains("any other key changes nothing"), "{said}");
    }

    #[test]
    fn the_questions_are_listed_the_way_the_choices_are() {
        let stage = Stage::Wondering(Chooser::over(&A_TRACE, Vec::new()));

        let said = said(&stage, 20, 80);

        assert!(said.contains("> where one thing is"), "{said}");
        assert!(said.contains("follow one show or film"), "{said}");
        assert!(said.contains("enter goes on"), "{said}");
    }

    /// A question that takes a word says what it wants, shows what has been typed,
    /// and says which key asks it.
    #[test]
    fn a_question_taking_a_word_says_what_it_wants_and_what_was_typed() {
        let stage = Stage::Typing {
            question: &A_TRACE,
            asks: "What to follow",
            typed: "The Exp".to_owned(),
        };

        let said = said(&stage, 20, 80);

        assert!(said.contains("What to follow"), "{said}");
        assert!(said.contains("> The Exp"), "{said}");
        assert!(said.contains("enter asks"), "{said}");
    }

    /// A question with the core says so, under the name of the question asked, so
    /// an answer that is slow to arrive does not read as a screen that stopped.
    #[test]
    fn a_question_with_the_core_says_what_it_is_waiting_for() {
        let said = said(&Stage::Waiting(&A_TRACE), 20, 80);

        assert!(said.contains("where one thing is"), "{said}");
        assert!(said.contains("waiting for this stack to answer"), "{said}");
    }

    /// An answer is drawn under the name of the question it answers, so a box that
    /// is still open a minute later still says what it was asked.
    #[test]
    fn an_answer_is_drawn_under_the_question_it_answers() {
        let stage = Stage::Answered {
            question: &A_TRACE,
            reading: nine(),
        };

        let said = said(&stage, 20, 80);

        assert!(said.contains("where one thing is"), "{said}");
        assert!(said.contains("line 0"), "{said}");
    }

    /// A control character in another service's account of a failure is an
    /// instruction to a terminal, and the screen must not carry one.
    #[test]
    fn text_from_somewhere_else_is_made_safe_before_it_is_drawn() {
        let stage = Stage::Confirming {
            offer: &A_START,
            taken: a_taking("Full\u{1b}[2Jstack", "quietly clearing the screen"),
        };

        let said = said(&stage, 20, 120);

        assert!(!said.contains('\u{1b}'), "{said:?}");
        assert!(said.contains("Full[2Jstack"), "{said}");
    }

    /// A narrow screen shortens rather than running past its edge, whichever of the
    /// boxes is open on it.
    #[test]
    fn no_row_runs_past_the_edge_of_a_narrow_screen() {
        let opened = [
            Stage::Choosing {
                offer: &A_START,
                chooser: two(),
            },
            Stage::Confirming {
                offer: &A_START,
                taken: a_taking("Full stack", "everything, behind the tunnel"),
            },
            Stage::Confirming {
                offer: &A_START,
                taken: several(),
            },
            Stage::Wondering(Chooser::over(&A_TRACE, Vec::new())),
            Stage::Typing {
                question: &A_TRACE,
                asks: "What to follow",
                typed: "something with a very long name indeed".to_owned(),
            },
            Stage::Waiting(&A_TRACE),
            Stage::Answered {
                question: &A_TRACE,
                reading: nine(),
            },
            Stage::Came(nine()),
        ];

        for stage in &opened {
            for across in [4usize, 12, 24, 40] {
                for row in said(stage, 20, across).lines().skip(1) {
                    assert!(row.chars().count() <= across, "at {across}: {row:?}");
                }
            }
        }
    }

    /// The rest of the errands are listed the way every other list is, so what an
    /// operator learned on one box carries to the next.
    #[test]
    fn the_errands_are_listed_the_way_the_choices_are() {
        let (first, rest) = errand::all();

        let said = said(&Stage::Sending(Chooser::over(first, rest)), 20, 90);

        assert!(said.contains(" more "), "{said}");
        assert!(said.contains("> wiring"), "{said}");
        assert!(said.contains("  your edits thrown away"), "{said}");
        assert!(said.contains("enter goes on"), "{said}");
    }

    /// An errand that has to be given a name says what it wants, and what has been
    /// typed of it — the same line a question that takes a word gets.
    #[test]
    fn an_errand_taking_a_name_says_what_it_wants_and_what_was_typed() {
        let stage = Stage::Naming {
            errand: sent("restore"),
            asks: "Which backup, by the name it was written under",
            typed: "lemonfiber-full".to_owned(),
        };

        let said = said(&stage, 20, 90);

        assert!(said.contains("Which backup"), "{said}");
        assert!(said.contains("> lemonfiber-full"), "{said}");
        assert!(said.contains("enter asks"), "{said}");
    }

    /// While the core is working out what an errand would do, the box says so
    /// rather than going quiet under a screen that is still gathering.
    #[test]
    fn an_errand_being_weighed_says_what_it_is_waiting_for() {
        let stage = Stage::Weighing {
            errand: sent("reset"),
            typed: String::new(),
        };

        let said = said(&stage, 20, 90);

        assert!(said.contains("your edits thrown away"), "{said}");
        assert!(said.contains("working out what this would do"), "{said}");
    }

    /// What it would do is above the question and never below it. An effect
    /// somebody reads after agreeing is not one they agreed to.
    #[test]
    fn what_an_errand_would_do_is_said_above_the_question_and_not_under_it() {
        let stage = Stage::Agreeing {
            errand: sent("reset"),
            typed: String::new(),
            would: Some(nine()),
        };

        let said = said(&stage, 20, 90);

        let before = said
            .split("Throw away every edit above?")
            .next()
            .unwrap_or_default();
        assert!(before.contains("line 0"), "{said}");
        assert!(said.contains("put lemonfiber's own state back"), "{said}");
        assert!(said.contains("y goes ahead"), "{said}");
        assert!(said.contains("up and down move"), "{said}");
    }

    /// An errand with nothing to say first is one question and no report, and the
    /// hint under it does not offer a movement there is nothing to move through.
    #[test]
    fn an_errand_with_nothing_to_show_first_is_the_question_alone() {
        let stage = Stage::Agreeing {
            errand: sent("seed"),
            typed: String::new(),
            would: None,
        };

        let said = said(&stage, 20, 90);

        assert!(said.contains("Wire the services to each other?"), "{said}");
        assert!(said.contains("y goes ahead"), "{said}");
        assert!(!said.contains("up and down move"), "{said}");
    }

    /// The name an errand was given completes the question, so what is about to be
    /// overwritten is named in the sentence agreeing to it.
    #[test]
    fn the_name_an_errand_was_given_completes_its_question() {
        let stage = Stage::Agreeing {
            errand: sent("restore"),
            typed: "lemonfiber-full-1.tar.gz".to_owned(),
            would: None,
        };

        let said = said(&stage, 20, 90);

        assert!(
            said.contains("Restore from lemonfiber-full-1.tar.gz?"),
            "{said}"
        );
    }

    /// A long report keeps the question on the screen: the box holds back the rows
    /// the question needs rather than filling them, so what is being agreed to is
    /// never the thing scrolled off.
    #[test]
    fn a_long_report_never_pushes_the_question_off_the_box() {
        let stage = Stage::Agreeing {
            errand: sent("reset"),
            typed: String::new(),
            would: Some(nine()),
        };

        let said = said(&stage, 6, 90);

        assert!(said.contains("Throw away every edit above?"), "{said}");
        assert!(said.contains("more lines below"), "{said}");
    }

    /// An errand under way leaves the screen behind it visible and says what is
    /// running on the one line the footer has.
    #[test]
    fn an_errand_under_way_says_so_on_the_footer_and_covers_nothing() {
        let stage = Stage::Doing {
            errand: sent("backup"),
            typed: String::new(),
        };

        assert!(pane(&stage, 20, 80).is_none());
        let footing = text(&footer(&stage, 200));
        assert!(footing.contains("a backup"), "{footing}");
        assert!(footing.contains("still running"), "{footing}");
    }

    /// Leaving mid-errand says what is being waited on, in the same sentence an
    /// action gets — there is one process here and it is the one holding the claim.
    #[test]
    fn leaving_mid_errand_says_what_is_being_waited_on() {
        let stage = Stage::Doing {
            errand: sent("backup"),
            typed: String::new(),
        };

        let said = staying_for(&stage).unwrap_or_default();

        assert!(said.contains("waiting for a backup to finish"), "{said}");
        assert!(said.contains("leave the stack claimed"), "{said}");
    }

    /// A read outstanding claims nothing, so a screen left with one waits for
    /// nothing — including the run that only says what an errand would do.
    #[test]
    fn nothing_is_waited_for_where_an_errand_has_only_been_weighed() {
        let stage = Stage::Weighing {
            errand: sent("reset"),
            typed: String::new(),
        };

        assert!(staying_for(&stage).is_none());
    }
}
