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

mod footing;
mod shapes;
mod titles;

pub(super) use footing::{footer, staying_for};

use ratatui::text::Line;

use super::chooser::Listed;
use super::errand::Errand;
use super::inviting;
use super::lasting::{self, Begun, Lasting};
use super::mending::{Agreed, Mending, Warning};
use super::offer::{Offer, Taken};
use super::quality::Change;
use super::reading::Reading;
use super::surface::{self, Open};
use super::Stage;
use crate::ui::Asked;
use lemonfiber_core::plural::s;
use shapes::{
    agreed, choosing, dimmed, elsewhere, named, read, setting, shortened, typing, AGREEING,
};
use titles::{
    asked, changing, keeping, narrowing, righting, sending, titled, ASK, CAME, KEEPS_GOING, MORE,
    PUT_RIGHT, QUALITY, WEB,
};

/// What the box says while the core is working out what a change would cost.
const COSTING: &str = "working out what this would cost";

/// How a walk that is running is left, and how what it has said is moved through.
///
/// The same words the foot of the screen says while one runs, because it is the same
/// walk: the box shows what it has said and the row underneath says what leaving does,
/// and two sentences about one run would eventually disagree.
const WATCHING: &str = "up and down move   q closes the screen and waits for it";

/// What the box says while a walk is running and has not said anything yet.
///
/// Said rather than shown as an empty box: a walk's first step arrives once the
/// indexers have been asked, which is seconds, and a box with nothing in it reads as
/// a walk that never started.
const WALKING: &str = "asking the services";

/// What the box says while the core is working out what an errand would do.
const WEIGHING: &str = "working out what this would do";

/// What the box says while a question is with the stack.
const ANSWERING: &str = "waiting for this stack to answer";

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
    let (title, lines) =
        off_a_list(stage, rows, across).or_else(|| on_a_key(stage, rows, across))?;
    Some(Pane { title, lines })
}

/// What the box one of the keyed flows opened says, or nothing where the box belongs
/// to a flow behind a list — and nothing, too, for the stages that draw no box.
///
/// Two functions over one enum, the way the routing beside them is two: forty stages
/// arrive here and a reader looking for one of them should not have to walk the other
/// thirty-nine. Nothing from either is the answer the caller wants, so this one also
/// carries the stages there is no box for at all.
fn on_a_key(stage: &Stage, rows: usize, across: usize) -> Option<(String, Vec<Line<'static>>)> {
    Some(match stage {
        Stage::Choosing { offer, chooser } => (titled(offer), choosing(chooser, rows, across)),
        // Titled by what the services are being named for, which is the action or the
        // errand the operator opened — a second title for the second list would read
        // as a second thing being started.
        Stage::Inside { inside, chooser } => (narrowing(inside), choosing(chooser, rows, across)),
        Stage::Confirming { offer, taken } => {
            (titled(offer), confirming(offer, taken, rows, across))
        }
        Stage::Came(reading) => (CAME.to_owned(), read(reading, rows, across)),
        Stage::Wondering(chooser) => (ASK.to_owned(), choosing(chooser, rows, across)),
        Stage::Typing {
            question,
            said,
            typed,
        } => (
            ASK.to_owned(),
            typing(said, question.asking(said.len()), typed, across),
        ),
        Stage::Waiting { question, .. } | Stage::Following(question) => {
            (asked(question), vec![dimmed(ANSWERING, across)])
        }
        // A diagnosis is the one answer with something offered under it, and the
        // account that offer sits under is the answer itself: the checks that
        // disturb are exactly the ones this reading reports as unverified, each of
        // them saying to run that one. So the box goes on moving through the report
        // and the question is the line beneath it, which is where a consequence
        // larger than a sentence is read on this screen.
        Stage::Answered {
            question,
            widening: Some(widening),
            reading,
        } => (
            asked(question),
            agreed(
                widening.widened().asks,
                widening.widened().about,
                Some(reading),
                rows,
                across,
            ),
        ),
        Stage::Answered {
            question, reading, ..
        } => (asked(question), read(reading, rows, across)),
        // Titled by the question rather than by the taking, because what is on
        // the screen is that question's answer — a list of what there is, which
        // taking one of asks the next question about it.
        Stage::Narrowing { question, chooser } => {
            (asked(question), choosing(chooser, rows, across))
        }
        // Everything else draws no box here. A running action has none, and neither
        // has an errand under way: the screen behind them is the report, the panels
        // go on gathering every second while the work runs, and a box over that would
        // take away the one thing worth watching. The rest have already been drawn by
        // the one above.
        _ => return None,
    })
}

/// What the box one of the flows behind a list opened says, or nothing where the box
/// belongs to a flow that began at a key — which is [`on_a_key`]'s to draw.
fn off_a_list(stage: &Stage, rows: usize, across: usize) -> Option<(String, Vec<Line<'static>>)> {
    Some(match stage {
        Stage::Sending(chooser) => (MORE.to_owned(), choosing(chooser, rows, across)),
        Stage::Naming {
            errand,
            asks,
            typed,
        } => (sending(errand), typing(&[], asks, typed, across)),
        Stage::Bundling { errand, chooser } => (sending(errand), choosing(chooser, rows, across)),
        // The name stays on the screen, dimmed, for the reason a trace's title does:
        // "which libraries" is a question about somebody, and a box that had taken
        // the name away would be asking who.
        Stage::Allowing {
            errand,
            name,
            typed,
        } => (
            sending(errand),
            typing(
                std::slice::from_ref(name),
                inviting::ASKS_LIBRARIES,
                typed,
                across,
            ),
        ),
        Stage::Limiting { errand, chooser } => (sending(errand), choosing(chooser, rows, across)),
        Stage::Unrated { errand, chooser } => (sending(errand), choosing(chooser, rows, across)),
        Stage::Weighing { errand, .. } => (sending(errand), vec![dimmed(WEIGHING, across)]),
        Stage::Agreeing {
            errand,
            given,
            would,
        } => (
            sending(errand),
            agreeing(errand, given.said(), would.as_ref(), rows, across),
        ),
        Stage::Starting(chooser) => (KEEPS_GOING.to_owned(), choosing(chooser, rows, across)),
        Stage::Wording {
            lasting,
            asks,
            typed,
        } => (keeping(lasting), typing(&[], asks, typed, across)),
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
        Stage::Scoping { change, chooser } => (changing(change), choosing(chooser, rows, across)),
        Stage::Grading {
            change, chooser, ..
        } => (changing(change), choosing(chooser, rows, across)),
        Stage::Costing { change } => (changing(change), vec![dimmed(COSTING, across)]),
        Stage::Settling {
            change,
            chosen,
            account,
        } => (
            changing(change),
            settling(change, chosen.said(), account.as_ref(), rows, across),
        ),
        Stage::Righting(chooser) => (PUT_RIGHT.to_owned(), choosing(chooser, rows, across)),
        Stage::Looking(mending) => (righting(mending), vec![dimmed(mending.waiting, across)]),
        Stage::Marking { mending, offering } => (
            righting(mending),
            choosing(offering.offered(), rows, across),
        ),
        // The one question on this screen whose account is not a rehearsal of what is
        // about to happen: the unconfirmed run *is* the offer, so what is above the
        // question is the very words the repairs were offered in.
        Stage::Consenting { mending, agreed } => {
            (righting(mending), agreed_to(mending, agreed, rows, across))
        }
        Stage::Warned {
            mending, chooser, ..
        } => (righting(mending), choosing(chooser, rows, across)),
        Stage::Answering { mending, warning } => {
            (righting(mending), answered(mending, warning, rows, across))
        }
        Stage::Handing { asked, open } => (WEB.to_owned(), handing(asked, open, rows, across)),
        _ => return None,
    })
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

/// The question over the repairs agreed to, under what each of them would do.
///
/// The count where there are several and the check itself where there is one. "The 1
/// above" is a sentence nobody writes, and a question naming the one thing it is
/// about is the clearer of the two anyway.
fn agreed_to(
    mending: &Mending,
    consented: &Agreed,
    rows: usize,
    across: usize,
) -> Vec<Line<'static>> {
    let asks = match consented.checks.as_slice() {
        [one] => format!("{} {one}", mending.asks),
        many => format!("{} the {} above", mending.asks, many.len()),
    };
    agreed(&asks, mending.costs, Some(&consented.account), rows, across)
}

/// The question over one warning, which has no account above it.
///
/// There is no run that would report what accepting comes to — it records that a
/// choice was weighed and changes nothing else — so a preamble invented here for
/// symmetry would be this screen claiming a rehearsal happened.
fn answered(
    mending: &Mending,
    warning: &Warning,
    rows: usize,
    across: usize,
) -> Vec<Line<'static>> {
    agreed(
        &format!("{} {}", mending.asks, warning.check),
        mending.costs,
        None,
        rows,
        across,
    )
}

/// The question before a quality change, under the account where there is one.
///
/// The preset completes the question where one was chosen, the way a form completes
/// an action's. Where none was, the question is whole on its own: neither of the
/// other two is given anything, and a sentence left hanging for a subject that does
/// not exist would be asking about nothing.
fn settling(
    change: &Change,
    chosen: &str,
    account: Option<&Reading>,
    rows: usize,
    across: usize,
) -> Vec<Line<'static>> {
    agreed(
        &format!("{} {chosen}", change.asks),
        change.about,
        account,
        rows,
        across,
    )
}

/// What one of them is called while it runs, with what it was given.
///
/// The name alone where nothing was named, which is what a walk asked for nothing in
/// particular is: there is no subject to say, and inventing one would be this screen
/// naming something the operator did not.
pub(super) fn doing(lasting: &Lasting, named: &str) -> String {
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
/// The question before the terminal is handed to the web surface, over the three
/// choices it is about to be started with.
///
/// The values are read off what the surface will be given rather than held beside
/// it, so the rows an operator agrees to are the ones it starts with. A refusal sits
/// under them where the last word typed was not taken, because a choice that did not
/// land and a question that does not say so is an operator agreeing to something
/// other than what they typed.
fn handing(asked: &Asked, open: &Open, rows: usize, across: usize) -> Vec<Line<'static>> {
    match open {
        Open::Nothing { refused } => handed(asked, *refused, across),
        Open::Choosing(chooser) => choosing(chooser, rows, across),
        Open::Typing { asks, typed, .. } => setting(asks, typed, across),
    }
}

/// The question itself, over the three choices as they stand.
fn handed(asked: &Asked, refused: Option<&str>, across: usize) -> Vec<Line<'static>> {
    let mut lines = vec![
        Line::raw(shortened(&format!("{}?", surface::ASKS), across)),
        dimmed(surface::ABOUT, across),
        Line::raw(""),
    ];
    let (first, rest) = surface::choices(asked);
    lines.extend(
        std::iter::once(first)
            .chain(rest)
            .map(|given| named(given.name(), given.about(), across)),
    );
    if let Some(refused) = refused {
        lines.push(Line::raw(""));
        lines.push(Line::raw(shortened(refused, across)));
    }
    lines.push(Line::raw(""));
    lines.push(dimmed(SERVING, across));
    lines
}

/// How the question about the web surface is answered.
///
/// [`AGREEING`] with the one more thing this question offers, since the three
/// choices under it are no use to somebody who is not told they can be changed.
const SERVING: &str =
    "y goes ahead   enter changes how it is served   any other key changes nothing";

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
            &format!(
                "{left} more {}{} than this screen has room for",
                taken.each,
                s(left)
            ),
            across,
        ));
    }
    lines
}

#[cfg(test)]
mod tests {
    use super::{footer, pane, staying_for, surface, Asked, Offer, Open, Stage};
    use crate::acting::chooser::Chooser;
    use crate::acting::disturbing::{under, Widening, DIAGNOSIS};
    use crate::acting::errand::{self, Errand, Given};
    use crate::acting::mending::{self, Mending};
    use crate::acting::narrowing::Subject;
    use crate::acting::offer::{Choice, Taken};
    use crate::acting::question::{Narrows, Needed, Question, Wants};
    use crate::acting::reading::Reading;
    use crate::acting::Press;
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
        needs: Needed::Typed(&[Wants {
            asks: "What to follow",
            narrows: Narrows::Term,
        }]),
    };

    /// One question that is narrowed by taking an entry off its own listing.
    static A_STUCK: Question = Question {
        name: "what is stuck",
        about: "the downloads that have stopped, each one followable",
        read: "/api/stuck",
        needs: Needed::Picked {
            at: "/api/trace",
            narrows: Narrows::Term,
        },
    };

    /// The question that answers with a diagnosis, which is the one answer this
    /// screen offers anything under.
    static A_DIAGNOSIS: Question = Question {
        name: "how this stack is doing",
        about: "every check that disturbs nothing, and what each one found",
        read: "/api/checks",
        needs: Needed::Nothing,
    };

    /// The widened run offered under that answer.
    fn a_widening() -> Option<Widening> {
        under(&A_DIAGNOSIS, &[])
    }

    /// One of the two things that can be done about a diagnosis, by its action.
    fn a_mending(action: &str) -> &'static Mending {
        mending::tests::doing(action)
    }

    /// A reading over nine numbered lines.
    fn nine() -> Reading {
        Reading::of((0..9).map(|at| format!("line {at}")).collect())
    }

    /// A choice by name, for the tests that only read what is drawn.
    fn a_choice(name: &str, about: &str) -> Choice {
        Choice {
            name: name.to_owned(),
            about: about.to_owned(),
            names: vec![name.to_owned()],
            marked: Some(false),
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
            each: "form",
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
            each: "form",
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
    fn two_listed() -> Chooser<Subject> {
        Chooser::over(
            a_subject("Full stack", "everything, behind the tunnel"),
            vec![a_subject("Lean stack", "the download clients only")],
        )
    }

    /// One of the things a listing offered, the command behind it beside the point
    /// for a test that only reads what is drawn.
    fn a_subject(name: &str, about: &str) -> Subject {
        Subject {
            name: name.to_owned(),
            about: about.to_owned(),
            command: Command::Stuck,
        }
    }

    fn two() -> Chooser<Choice> {
        Chooser::over(
            a_choice("Full stack", "everything, behind the tunnel"),
            vec![a_choice("Lean stack", "the download clients only")],
        )
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
            choice.marked = Some(true);
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
            said: Vec::new(),
            typed: "The Exp".to_owned(),
        };

        let said = said(&stage, 20, 80);

        assert!(said.contains("What to follow"), "{said}");
        assert!(said.contains("> The Exp"), "{said}");
        assert!(said.contains("enter goes on"), "{said}");
    }

    /// A question with the core says so, under the name of the question asked, so
    /// an answer that is slow to arrive does not read as a screen that stopped.
    #[test]
    fn a_question_with_the_core_says_what_it_is_waiting_for() {
        let waiting = Stage::Waiting {
            question: &A_TRACE,
            said: vec!["The Expanse".to_owned()],
        };

        let said = said(&waiting, 20, 80);

        assert!(said.contains("where one thing is"), "{said}");
        assert!(said.contains("waiting for this stack to answer"), "{said}");
    }

    /// An answer is drawn under the name of the question it answers, so a box that
    /// is still open a minute later still says what it was asked.
    #[test]
    fn an_answer_is_drawn_under_the_question_it_answers() {
        let stage = Stage::Answered {
            question: &A_TRACE,
            widening: None,
            reading: nine(),
        };

        let said = said(&stage, 20, 80);

        assert!(said.contains("where one thing is"), "{said}");
        assert!(said.contains("line 0"), "{said}");
        assert!(said.contains("any other key closes"), "{said}");
        assert!(!said.contains("disturb"), "{said}");
    }

    /// A diagnosis is the one answer with a question under it: the report goes on
    /// being moved through, and the line beneath it offers to run the same checks
    /// again including the ones that disturb — which is what those very findings say
    /// to do. An account read after agreeing is not one anybody agreed to.
    #[test]
    fn a_diagnosis_is_read_with_the_widening_offered_under_it() {
        let stage = Stage::Answered {
            question: &A_DIAGNOSIS,
            widening: a_widening(),
            reading: nine(),
        };

        let said = said(&stage, 20, 100);

        assert!(said.contains("how this stack is doing"), "{said}");
        assert!(said.contains("line 0"), "{said}");
        assert!(said.contains("including the ones that disturb?"), "{said}");
        assert!(said.contains("takes the tunnel away"), "{said}");
        assert!(said.contains("y goes ahead"), "{said}");
    }

    /// While it runs, nothing is drawn over the panels — the VPN panel behind this
    /// box is the check being run — and the footer says what is running and what
    /// leaving would leave the stack in.
    ///
    /// Named off the run itself rather than off one written down here, because there
    /// are two of these: a screen naming the checks that disturb while the indexers
    /// are being searched would be telling an operator about the wrong wait.
    #[test]
    fn a_run_that_disturbs_covers_nothing_and_is_named_in_the_footer() {
        let running = Stage::Disturbing(&DIAGNOSIS);
        let said = said(&running, 20, 200);
        let footing = text(&footer(&running, 200));
        let staying = staying_for(&running);

        assert!(said.is_empty(), "{said}");
        assert!(footing.contains(DIAGNOSIS.name), "{footing}");
        assert!(footing.contains("still running"), "{footing}");
        assert_eq!(
            staying,
            Some(format!(
                "waiting for {} to finish — leaving it now would leave the stack claimed",
                DIAGNOSIS.name
            ))
        );
    }

    /// A listing offered to be taken one of is drawn under the name of the question
    /// that listed it, and read as a list rather than as an answer — so what is on
    /// the screen says both what was asked and that there is something to do with it.
    #[test]
    fn a_listing_to_take_one_of_is_drawn_under_the_question_that_listed_it() {
        let stage = Stage::Narrowing {
            question: &A_STUCK,
            chooser: two_listed(),
        };

        let said = said(&stage, 20, 80);

        assert!(said.contains("what is stuck"), "{said}");
        assert!(said.contains("> Full stack"), "{said}");
        assert!(said.contains("enter"), "{said}");
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

    /// The list of what to do about a diagnosis is drawn the way every other list on
    /// this screen is, so what an operator learned on one box carries to the next.
    #[test]
    fn what_to_do_about_a_diagnosis_is_listed_the_way_the_errands_are() {
        let (first, rest) = mending::all();

        let said = said(&Stage::Righting(Chooser::over(first, rest)), 20, 90);

        assert!(said.contains(" put right "), "{said}");
        assert!(said.contains("> what is wrong put right"), "{said}");
        assert!(
            said.contains("a warning you have already weighed"),
            "{said}"
        );
        assert!(said.contains("enter goes on"), "{said}");
    }

    /// While the offer is with the core the box says what it is waiting for, rather
    /// than going quiet under a screen that is still gathering.
    #[test]
    fn an_offer_being_worked_out_says_what_it_is_waiting_for() {
        let said = said(&Stage::Looking(a_mending("repair")), 20, 90);

        assert!(said.contains("what is wrong put right"), "{said}");
        assert!(
            said.contains("working out what could be put right"),
            "{said}"
        );
    }

    /// The offer is a list that takes several, so each repair is a row that can be
    /// marked on its own — which is the whole of what makes the yes a selection.
    #[test]
    fn the_repairs_offered_are_a_list_that_takes_several() {
        let said = said(&mending::tests::an_offer(), 20, 100);

        assert!(said.contains("what is wrong put right"), "{said}");
        assert!(said.contains("> [ ] vpn.port-forward-client"), "{said}");
        assert!(said.contains("[ ] config.wiring"), "{said}");
        assert!(said.contains("space marks"), "{said}");
    }

    /// The question sits under what the repairs marked would do, what else changes if
    /// they do, and whether they can be put back. An effect somebody reads after
    /// agreeing to it is not one they agreed to.
    #[test]
    fn the_question_over_a_repair_sits_under_what_it_would_do() {
        let said = said(&mending::tests::an_agreement(), 20, 100);

        assert!(said.contains("put vpn.port-forward-client back"), "{said}");
        assert!(said.contains("pauses briefly"), "{said}");
        assert!(said.contains("cannot be put back"), "{said}");
        assert!(
            said.contains("Put right vpn.port-forward-client?"),
            "{said}"
        );
        assert!(said.contains("y goes ahead"), "{said}");
    }

    /// The warnings are a list that takes one, because an accept answers one warning.
    #[test]
    fn the_warnings_are_a_list_that_takes_one() {
        let said = said(&mending::tests::warned_about(), 20, 100);

        assert!(
            said.contains("a warning you have already weighed"),
            "{said}"
        );
        assert!(said.contains("> vpn.unprotected"), "{said}");
        assert!(!said.contains("space marks"), "{said}");
    }

    /// The question over a warning names the check it answers and says what
    /// answering it comes to, with no account above it — there is no run that would
    /// report what accepting would do, and a preamble invented for symmetry would be
    /// this screen claiming a rehearsal happened.
    #[test]
    fn the_question_over_a_warning_names_the_check_it_answers() {
        let said = said(&mending::tests::an_answer(), 20, 100);

        assert!(said.contains("Accept vpn.unprotected?"), "{said}");
        assert!(said.contains("stops leading"), "{said}");
        assert!(said.contains("y goes ahead"), "{said}");
        assert!(!said.contains("up and down move"), "{said}");
    }

    /// While it runs, nothing is drawn over the panels — a repair reaches the
    /// services and the panel that says what each one is doing is behind this box —
    /// and the footer says what is running and what leaving would leave.
    #[test]
    fn a_run_that_puts_things_right_covers_nothing_and_is_named_in_the_footer() {
        let running = Stage::Putting(a_mending("repair"));

        let said = said(&running, 20, 200);
        let footing = text(&footer(&running, 200));
        let staying = staying_for(&running);

        assert!(said.is_empty(), "{said}");
        assert!(footing.contains("what is wrong put right"), "{footing}");
        assert!(footing.contains("still running"), "{footing}");
        assert_eq!(
            staying,
            Some(
                "waiting for what is wrong put right to finish — leaving it now would leave \
                 the stack claimed"
                    .to_owned()
            )
        );
    }

    /// The question over several names how many rather than one of them, because
    /// naming one of three would be naming the wrong thing about the other two.
    #[test]
    fn a_question_over_several_repairs_says_how_many_it_is_over() {
        let (_, first) = mending::tests::pressed(mending::tests::an_offer(), &Press::Typed(' '));
        let (_, moved_down) = mending::tests::pressed(first, &Press::Forward);
        let (_, both) = mending::tests::pressed(moved_down, &Press::Typed(' '));
        let (_, asked) = mending::tests::pressed(both, &Press::Accept);

        let said = said(&asked, 20, 100);

        assert!(said.contains("Put right the 2 above?"), "{said}");
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
                said: Vec::new(),
                typed: "something with a very long name indeed".to_owned(),
            },
            Stage::Waiting {
                question: &A_TRACE,
                said: vec!["The Expanse".to_owned()],
            },
            Stage::Answered {
                question: &A_TRACE,
                widening: None,
                reading: nine(),
            },
            Stage::Answered {
                question: &A_DIAGNOSIS,
                widening: a_widening(),
                reading: nine(),
            },
            Stage::Narrowing {
                question: &A_STUCK,
                chooser: two_listed(),
            },
            mending::tests::an_offer(),
            mending::tests::an_agreement(),
            mending::tests::warned_about(),
            mending::tests::an_answer(),
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
        assert!(said.contains("enter goes on"), "{said}");
    }

    /// While the core is working out what an errand would do, the box says so
    /// rather than going quiet under a screen that is still gathering.
    #[test]
    fn an_errand_being_weighed_says_what_it_is_waiting_for() {
        let stage = Stage::Weighing {
            errand: sent("reset"),
            given: Given::nothing(),
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
            given: Given::nothing(),
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
            given: Given::nothing(),
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
            given: Given::typed("lemonfiber-full-1.tar.gz".to_owned()),
            would: None,
        };

        let said = said(&stage, 20, 90);

        assert!(
            said.contains("Restore from lemonfiber-full-1.tar.gz?"),
            "{said}"
        );
    }

    /// A person's name completes the question the same way a file's does.
    ///
    /// Same line, different argument — which is the errand's business rather than
    /// the line's, and this is where that shows: the sentence names the person.
    #[test]
    fn the_name_of_somebody_invited_completes_its_question() {
        let stage = Stage::Agreeing {
            errand: sent("invite"),
            given: Given::named("ana".to_owned()),
            would: None,
        };

        let said = said(&stage, 20, 90);

        assert!(said.contains("Invite ana?"), "{said}");
    }

    /// A long report keeps the question on the screen: the box holds back the rows
    /// the question needs rather than filling them, so what is being agreed to is
    /// never the thing scrolled off.
    #[test]
    fn a_long_report_never_pushes_the_question_off_the_box() {
        let stage = Stage::Agreeing {
            errand: sent("reset"),
            given: Given::nothing(),
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
            given: Given::nothing(),
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
            given: Given::nothing(),
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
            given: Given::nothing(),
        };

        assert!(staying_for(&stage).is_none());
    }

    /// The question about the web surface names the three choices and what each of
    /// them is set to, because what an operator agrees to is what is on the screen.
    #[test]
    fn the_web_question_says_what_the_surface_will_be_given() {
        let stage = Stage::Handing {
            asked: Asked::unsaid(),
            open: Open::Nothing { refused: None },
        };
        let said = said(&stage, 20, 100);

        assert!(said.contains("Close this screen"), "{said}");
        assert!(said.contains("port"), "{said}");
        assert!(said.contains("whichever one is free"), "{said}");
        assert!(said.contains("browser"), "{said}");
        assert!(said.contains("app"), "{said}");
        assert!(said.contains("y goes ahead"), "{said}");
        assert!(said.contains("any other key changes nothing"), "{said}");
    }

    /// A value that was named is the value the row shows, or the row is showing a
    /// default the surface will not be started with.
    #[test]
    fn the_rows_show_what_was_named_rather_than_what_it_would_have_been() {
        let stage = Stage::Handing {
            asked: Asked {
                port: Some(7171),
                browser: false,
                assets: Some(std::path::PathBuf::from("/srv/app")),
                password: true,
                reach: crate::ui::reach::Reach::Network,
            },
            open: Open::Nothing { refused: None },
        };
        let said = said(&stage, 20, 100);

        assert!(said.contains("7171"), "{said}");
        assert!(said.contains("/srv/app"), "{said}");
        assert!(said.contains("none is opened"), "{said}");
    }

    /// A word that was not taken is said under the rows, because a choice that did
    /// not land and a question that does not say so is somebody agreeing to
    /// something other than what they typed.
    #[test]
    fn a_refused_word_is_said_under_the_question_it_goes_back_to() {
        let stage = Stage::Handing {
            asked: Asked::unsaid(),
            open: Open::Nothing {
                refused: Some(crate::ui::NOT_A_PORT),
            },
        };
        let said = said(&stage, 20, 100);

        assert!(said.contains(crate::ui::NOT_A_PORT), "{said}");
        assert!(said.contains("whichever one is free"), "{said}");
    }

    /// The three are listed the way every other list on this screen is, so nobody
    /// has to learn a second movement for them.
    #[test]
    fn the_three_choices_are_listed_the_way_the_others_are() {
        let (first, rest) = surface::choices(&Asked::unsaid());
        let stage = Stage::Handing {
            asked: Asked::unsaid(),
            open: Open::Choosing(Chooser::over(first, rest)),
        };
        let said = said(&stage, 20, 100);

        assert!(said.contains("> port"), "{said}");
        assert!(said.contains("up and down choose"), "{said}");
        assert!(said.contains("esc leaves it"), "{said}");
    }

    /// The line a value is typed on says what it wants and shows what has been typed,
    /// which is the same line every other typed answer on this screen is given.
    #[test]
    fn a_value_the_surface_takes_says_what_it_wants_and_what_was_typed() {
        let stage = Stage::Handing {
            asked: Asked::unsaid(),
            open: Open::Typing {
                fills: surface::Fills::Port,
                asks: "Which port to listen on",
                typed: "717".to_owned(),
            },
        };
        let said = said(&stage, 20, 100);

        assert!(said.contains("Which port to listen on"), "{said}");
        assert!(said.contains("> 717"), "{said}");
    }
}
