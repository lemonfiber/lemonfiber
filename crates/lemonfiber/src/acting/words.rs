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
mod tests;
