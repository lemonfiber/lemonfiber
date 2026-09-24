//! What the dashboard offers to do, what each offer can be given, and what becomes
//! of one.
//!
//! Five actions, each held by the name every surface calls it by. The name is what
//! the web surface's own table turns into one of the core's commands, and this
//! reaches that table rather than carrying a second one — so an action offered on
//! this screen reaches the command a browser reaches, and a terminal action that
//! could do something no other surface can do is not a state this can hold.
//!
//! What an action may be given is asked of that table too. Three of the five refuse
//! an empty list of forms, and which three is not written down here: every subject
//! is offered to the translation and only the ones that come to a command are put
//! in front of the operator. Building that list is [`choices`] rather than a method,
//! because the guard behind [`super::lasting`]'s own key is given one of the stack's
//! forms in exactly the same way and refuses the whole stack for exactly the same
//! reason.
//!
//! **The list names several.** The command line takes a list of forms and a browser
//! sends one whole; this list took one, and that was the last thing four of these
//! actions and the guard were short of. A row is marked with [`MARKS`] and the marked
//! rows are what enter takes — and where none is marked, enter takes the row under
//! the cursor, which is what this list has always done. An operator who never presses
//! the new key sees the screen they saw before it existed.
//!
//! **Naming nothing is not a state this reaches.** The cursor is always on a row, so
//! an empty list of forms reaches the core only from the row that says `everything` —
//! and that row is offered only where the translation carries one. The two actions
//! that read an empty list as the whole stack and the three that refuse it therefore
//! stay exactly as far apart on this screen as they are in the table both are asked
//! of, rather than being blurred into one keypress that means two things.

use lemonfiber_api::actions::{named, Arguments};
use lemonfiber_core::app::Command;
use lemonfiber_core::model::FormsReport;
use lemonfiber_core::plural::s;

use super::chooser::{Chooser, Listed};
use super::reading::Reading;
use super::{Asked, Press, Stage, Wanted};

/// The character that marks the row under the cursor, and takes the mark off again.
///
/// Space, which is what marks a row on every list anybody has met that takes several.
/// It is free on this screen: every other key here is a letter, and a list has never
/// had anything to do with one.
pub(super) const MARKS: char = ' ';

/// What the whole stack is called where it is one of the choices.
const WHOLE: &str = "everything";

/// What choosing it comes to, in the line under the name.
const EVERY_FORM: &str = "the whole stack, rather than one form of it";

/// One action the dashboard offers, and how it is spoken about.
pub(crate) struct Offer {
    /// The key that reaches it.
    pub(crate) key: char,
    /// The name every surface calls this action by.
    pub(crate) action: &'static str,
    /// The word the footer puts beside the key.
    pub(crate) hint: &'static str,
    /// How the question before it begins, the subject completing it.
    pub(crate) asks: &'static str,
}

/// The lifecycle actions this screen offers, in the order the footer reads them.
///
/// The keys avoid the three the screen already answers. A restart is on `t` because
/// `r` gathers afresh, and an operator who has just been told a service is unhealthy
/// is likelier to want the screen to be right than to want it restarted.
pub(crate) const OFFERED: &[Offer] = &[
    Offer {
        key: 'u',
        action: "up",
        hint: "start",
        asks: "Start",
    },
    Offer {
        key: 'd',
        action: "down",
        hint: "stop",
        asks: "Stop",
    },
    Offer {
        key: 's',
        action: "switch",
        hint: "switch",
        asks: "Switch to",
    },
    Offer {
        key: 't',
        action: "restart",
        hint: "restart",
        asks: "Restart",
    },
    Offer {
        key: 'p',
        action: "pull",
        hint: "fetch",
        asks: "Fetch newer images for",
    },
];

/// The action a key reaches, or nothing for a key that reaches none.
pub(crate) fn for_key(key: char) -> Option<&'static Offer> {
    OFFERED.iter().find(|offer| offer.key == key)
}

/// Which of an action's own arguments a list of choices fills.
///
/// Named rather than assembled. What is taken off a list here goes into the field
/// [`Arguments`] holds for it and the translation decides what command that comes
/// to — so the fork `up` and `down` take when services are named is the table's to
/// take rather than this screen's, and a list here can only fill an argument a
/// browser could send.
///
/// Two of them fill a list and one fills a single name, which is the whole of what
/// tells a list that marks rows from a list that takes the row under the cursor.
/// The command line spells all three `--service` or a bare argument; which of them
/// an action takes is [`lemonfiber_api::actions`]'s answer and not one written down
/// twice.
#[derive(Clone, Copy)]
pub(crate) enum Fills {
    /// The forms to act on.
    Forms,
    /// The services to act on, leaving the rest of the form alone.
    Services,
    /// The one service to act on instead of the whole stack.
    Service,
}

impl Fills {
    /// The arguments as they now stand: what was named before, with this argument
    /// filled by the names taken.
    ///
    /// Beside what was named before rather than instead of it, because a service is
    /// named *inside* what a form list already chose — and an argument that dropped
    /// the forms would start named services in a stack nobody had narrowed.
    fn given(self, names: Vec<String>, before: &Arguments) -> Arguments {
        let mut given = before.clone();
        match self {
            Self::Forms => given.forms = names,
            Self::Services => given.services = names,
            Self::Service => given.service = names.into_iter().next(),
        }
        given
    }

    /// What one row of such a list is, where several have to be counted.
    const fn each(self) -> &'static str {
        match self {
            Self::Forms => "form",
            Self::Services | Self::Service => "service",
        }
    }

    /// Whether a list filling this argument may have several rows marked.
    ///
    /// A single name cannot: an archive's scope is one scope, so a row with a box
    /// beside it would be offering something the command has nowhere to put.
    const fn several(self) -> bool {
        !matches!(self, Self::Service)
    }
}

/// Something an action can be given, and what giving it comes to.
pub(crate) struct Choice {
    /// What it is called, in the stack's own words where it is a form or a service.
    pub(crate) name: String,
    /// What it is for, in one line.
    pub(crate) about: String,
    /// What naming it names, which is empty where it is the whole of them.
    ///
    /// Kept beside the command rather than read back out of it: several of these are
    /// taken together by joining what each names, and a list assembled by taking a
    /// command apart again would be this screen deciding what a command means.
    pub(crate) names: Vec<String>,
    /// Whether it is one of the several this action is about to be given, or nothing
    /// where the argument it fills takes one name.
    pub(crate) marked: Option<bool>,
    /// The command acting on it alone comes to.
    pub(crate) command: Command,
}

impl Listed for Choice {
    fn name(&self) -> &str {
        &self.name
    }

    fn about(&self) -> &str {
        &self.about
    }

    fn marked(&self) -> Option<bool> {
        self.marked
    }
}

/// What an action is about to be taken on: the row under the cursor, or every row
/// marked.
pub(crate) struct Taken {
    /// The rows it covers, in the order the list offered them. Never empty.
    pub(crate) covers: Vec<Choice>,
    /// The command acting on them comes to.
    pub(crate) command: Command,
    /// What one of those rows is, for the sentence that has to count them.
    pub(crate) each: &'static str,
}

impl Taken {
    /// What it is called: the one name, or how many of them were named together.
    ///
    /// A count where there are several, because this is what the footer says while
    /// the work runs and what the line on the way out says — one row, on a screen
    /// whose width belongs to the panels behind it. The names themselves are said
    /// where there is room for them, which is the question asked before it runs.
    pub(crate) fn name(&self) -> String {
        match self.covers.as_slice() {
            [only] => only.name.clone(),
            several => format!("{} {}{}", several.len(), self.each, s(several.len())),
        }
    }

    /// What it names, joined, which is what a list chosen inside it is named beside.
    pub(crate) fn named(&self) -> Vec<String> {
        self.covers
            .iter()
            .flat_map(|choice| choice.names.clone())
            .collect()
    }
}

impl Offer {
    /// What this action can be given, or the refusal where it can be given nothing.
    pub(crate) fn given(&self, report: &FormsReport) -> Result<(Choice, Vec<Choice>), String> {
        guarding(self.action, report)
    }
}

/// The stack's own forms, as one action's list of them.
///
/// Takes an action rather than belonging to the five on keys, because the guard
/// behind the key that opens what keeps going is given one of the stack's forms in
/// exactly the same way and refuses the whole stack for exactly the same reason.
pub(super) fn guarding(
    action: &str,
    report: &FormsReport,
) -> Result<(Choice, Vec<Choice>), String> {
    choices(
        action,
        Fills::Forms,
        &Arguments::default(),
        subjects(report),
    )
}

/// What one action can be given, or the refusal where it can be given nothing.
///
/// Every subject goes through the translation, and only what comes to a command is
/// offered. A stack that declares no forms leaves the actions that insist on one
/// with nothing to offer, and the words the operator gets then are the words the
/// web surface gives for the same request. A guard is refused the whole stack the
/// same way and for the same reason, which is why this takes an action rather than
/// belonging to the five that sit on keys.
///
/// The first choice comes back apart from the rest, so that what is handed on is a
/// list something is already selected in. A list and a selection carried separately
/// can disagree, and the place they would disagree is under the operator's finger.
pub(super) fn choices(
    action: &str,
    fills: Fills,
    before: &Arguments,
    offered: Vec<(Vec<String>, String, String)>,
) -> Result<(Choice, Vec<Choice>), String> {
    let mut choices = Vec::new();
    let mut refused = String::new();
    for (names, name, about) in offered {
        match named(action, fills.given(names.clone(), before)) {
            Ok(command) => choices.push(Choice {
                name,
                about,
                names,
                marked: fills.several().then_some(false),
                command,
            }),
            Err(no) => refused = no.said(),
        }
    }
    let mut offered = choices.into_iter();
    match offered.next() {
        Some(first) => Ok((first, offered.collect())),
        None => Err(refused),
    }
}

/// Begin the action a key reaches, by asking what there is to act on.
///
/// The list is asked for rather than remembered from a previous run: a stack's
/// declarations are a file on disk that an operator may have just edited, and a list
/// gathered once would offer a form that is no longer there.
pub(super) fn begin(asked: &mut Option<Asked>, key: char) -> Wanted {
    let Some(offer) = for_key(key) else {
        return Wanted::Nothing;
    };
    *asked = Some(Asked::Action(offer));
    Wanted::Ask(Command::Forms)
}

/// Mark the row under the cursor, or take the mark off it.
///
/// The whole stack is *instead of* naming forms rather than one more of them, so
/// marking it takes the marks off the forms and marking a form takes the mark off it.
/// Nothing else would be honest: what the two together would send is what the whole
/// stack alone would send, and a list showing both marked would be naming something
/// the action was not about to be given. The marks move where the operator can watch
/// them move, which is the whole of how that rule is taught.
fn marking(chooser: &mut Chooser<Choice>) {
    let whole = chooser
        .listed()
        .any(|(here, choice)| here && choice.names.is_empty());
    for (here, choice) in chooser.each() {
        if here {
            choice.marked = choice.marked.map(|was| !was);
        } else if whole || choice.names.is_empty() {
            choice.marked = choice.marked.map(|_| false);
        }
    }
}

/// What the list comes to when it is taken: every row marked, or the row under the
/// cursor where none is.
///
/// Several go through the translation exactly as one does, over the forms the marked
/// rows name joined together — so a list the command will not carry is refused in the
/// words a browser is refused with rather than in a sentence written here, and the
/// screen still cannot ask for something no other surface can ask for.
fn taking(
    action: &str,
    fills: Fills,
    before: &Arguments,
    chooser: Chooser<Choice>,
) -> Result<Taken, String> {
    let each = fills.each();
    if !chooser
        .listed()
        .any(|(_, choice)| choice.marked == Some(true))
    {
        let chosen = chooser.taken();
        return Ok(Taken {
            command: chosen.command.clone(),
            covers: vec![chosen],
            each,
        });
    }
    let covers: Vec<Choice> = chooser
        .all()
        .into_iter()
        .filter(|choice| choice.marked == Some(true))
        .collect();
    let names = covers
        .iter()
        .flat_map(|choice| choice.names.clone())
        .collect();
    named(action, fills.given(names, before))
        .map(|command| Taken {
            covers,
            command,
            each,
        })
        .map_err(|no| no.said())
}

/// What a press over a list of the stack's own forms came to.
///
/// Two lists are made of these — the five actions on keys of their own, and the guard
/// behind the key that opens what keeps going — and they move, mark and take
/// identically. One movement rather than a copy beside each, because the day the two
/// stopped being one is the day the screen behaved differently depending on which key
/// opened it. What each of them does with what was taken is its own, which is why this
/// answers with the outcome rather than setting a stage.
pub(super) enum Over {
    /// Still choosing, and the list as it now stands.
    Choosing(Chooser<Choice>),
    /// Taken, and what it comes to.
    Taken(Result<Taken, String>),
    /// Left, with nothing taken.
    Left,
}

/// What a taken list came to, with a refusal put where the operator is looking.
///
/// One place for both lists. A refusal that read differently under one key than under
/// another would be this screen having an opinion about a translation it does not
/// own — and it is the same box an action refused a subject already opens.
pub(super) fn or_refused(stage: &mut Stage, taken: Result<Taken, String>) -> Option<Taken> {
    match taken {
        Ok(taken) => Some(taken),
        Err(refused) => {
            *stage = Stage::Came(Reading::of(vec![refused]));
            None
        }
    }
}

/// Over a list of the stack's own forms: move, mark, take, or leave it.
pub(super) fn over(
    action: &str,
    fills: Fills,
    before: &Arguments,
    mut chooser: Chooser<Choice>,
    press: &Press,
) -> Over {
    match *press {
        Press::Abandon => return Over::Left,
        Press::Accept => return Over::Taken(taking(action, fills, before, chooser)),
        Press::Back => chooser.back(),
        Press::Forward => chooser.forward(),
        Press::Typed(MARKS) => marking(&mut chooser),
        Press::Typed(_) | Press::Rubout => (),
    }
    Over::Choosing(chooser)
}

/// Over the list: move, mark, take, or leave it.
///
/// What follows is the services inside what was named, where this action can be given
/// some and the screen has any in hand — and the question itself where it cannot, so
/// an action with no service to narrow to reaches the same question it always did.
pub(super) fn choosing(
    stage: &mut Stage,
    offer: &'static Offer,
    chooser: Chooser<Choice>,
    press: &Press,
    services: &[(String, String, String)],
) -> Wanted {
    match over(
        offer.action,
        Fills::Forms,
        &Arguments::default(),
        chooser,
        press,
    ) {
        Over::Left => (),
        Over::Choosing(chooser) => *stage = Stage::Choosing { offer, chooser },
        Over::Taken(taken) => {
            if let Some(taken) = or_refused(stage, taken) {
                *stage = super::service::or_the_question(offer, taken, services);
            }
        }
    }
    Wanted::Nothing
}

/// At the question: only an explicit yes goes ahead.
///
/// Everything else — a no, a stray return, a key that is neither — leaves the stack
/// as it is, which is the same way the teardown's own question is read. The answer
/// that changes something should never be the one given by accident.
pub(super) fn confirming(
    stage: &mut Stage,
    offer: &'static Offer,
    taken: Taken,
    press: &Press,
) -> Wanted {
    if !matches!(*press, Press::Typed('y' | 'Y')) {
        return Wanted::Nothing;
    }
    let command = taken.command.clone();
    *stage = Stage::Running { offer, taken };
    Wanted::Carry(command)
}

/// While the action is with the core: leaving is the only thing left to ask.
///
/// The stage is put back either way. Leaving does not stop the action — the run
/// waits for it once the screen is given back — so it is still where it was, and
/// what the screen says on the way out is what says so.
pub(super) fn running(
    stage: &mut Stage,
    offer: &'static Offer,
    taken: Taken,
    press: &Press,
) -> Wanted {
    *stage = Stage::Running { offer, taken };
    if super::leaving(press) {
        return Wanted::Leave;
    }
    Wanted::Nothing
}

/// Everything an action could be given, the whole stack first.
///
/// The forms as the stack declares them, in its own words: a listing that
/// paraphrased would be describing a different stack from the one being run.
fn subjects(report: &FormsReport) -> Vec<(Vec<String>, String, String)> {
    let mut all = vec![(Vec::new(), WHOLE.to_owned(), EVERY_FORM.to_owned())];
    all.extend(report.forms.iter().map(|form| {
        (
            vec![form.id.clone()],
            form.name.clone(),
            form.description.clone(),
        )
    }));
    all
}

#[cfg(test)]
pub(crate) mod tests;
