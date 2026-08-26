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
const MARKS: char = ' ';

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

/// Something an action can be given, and what giving it comes to.
pub(crate) struct Choice {
    /// What it is called, in the stack's own words where it is a form.
    pub(crate) name: String,
    /// What it is for, in one line.
    pub(crate) about: String,
    /// The forms naming it names, which is empty where it is the whole stack.
    ///
    /// Kept beside the command rather than read back out of it: several of these are
    /// taken together by joining what each names, and a list assembled by taking a
    /// command apart again would be this screen deciding what a command means.
    pub(crate) forms: Vec<String>,
    /// Whether it is one of the several this action is about to be given.
    pub(crate) marked: bool,
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
        Some(self.marked)
    }
}

/// What an action is about to be taken on: the row under the cursor, or every row
/// marked.
pub(crate) struct Taken {
    /// The rows it covers, in the order the list offered them. Never empty.
    pub(crate) covers: Vec<Choice>,
    /// The command acting on them comes to.
    pub(crate) command: Command,
}

impl Taken {
    /// What it is called: the one name, or how many forms were named together.
    ///
    /// A count where there are several, because this is what the footer says while
    /// the work runs and what the line on the way out says — one row, on a screen
    /// whose width belongs to the panels behind it. The names themselves are said
    /// where there is room for them, which is the question asked before it runs.
    pub(crate) fn name(&self) -> String {
        match self.covers.as_slice() {
            [only] => only.name.clone(),
            several => format!("{} form{}", several.len(), s(several.len())),
        }
    }
}

impl Offer {
    /// What this action can be given, or the refusal where it can be given nothing.
    pub(crate) fn given(&self, report: &FormsReport) -> Result<(Choice, Vec<Choice>), String> {
        choices(self.action, report)
    }
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
pub(super) fn choices(action: &str, report: &FormsReport) -> Result<(Choice, Vec<Choice>), String> {
    let mut choices = Vec::new();
    let mut refused = String::new();
    for (forms, name, about) in subjects(report) {
        let asked = Arguments {
            forms: forms.clone(),
            ..Arguments::default()
        };
        match named(action, asked) {
            Ok(command) => choices.push(Choice {
                name,
                about,
                forms,
                marked: false,
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
        .any(|(here, choice)| here && choice.forms.is_empty());
    for (here, choice) in chooser.each() {
        if here {
            choice.marked = !choice.marked;
        } else if whole || choice.forms.is_empty() {
            choice.marked = false;
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
fn taking(action: &str, chooser: Chooser<Choice>) -> Result<Taken, String> {
    if !chooser.listed().any(|(_, choice)| choice.marked) {
        let chosen = chooser.taken();
        return Ok(Taken {
            command: chosen.command.clone(),
            covers: vec![chosen],
        });
    }
    let covers: Vec<Choice> = chooser
        .all()
        .into_iter()
        .filter(|choice| choice.marked)
        .collect();
    let asked = Arguments {
        forms: covers
            .iter()
            .flat_map(|choice| choice.forms.clone())
            .collect(),
        ..Arguments::default()
    };
    named(action, asked)
        .map(|command| Taken { covers, command })
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
pub(super) fn over(action: &str, mut chooser: Chooser<Choice>, press: &Press) -> Over {
    match *press {
        Press::Abandon => return Over::Left,
        Press::Accept => return Over::Taken(taking(action, chooser)),
        Press::Back => chooser.back(),
        Press::Forward => chooser.forward(),
        Press::Typed(MARKS) => marking(&mut chooser),
        Press::Typed(_) | Press::Rubout => (),
    }
    Over::Choosing(chooser)
}

/// Over the list: move, mark, take, or leave it.
pub(super) fn choosing(
    stage: &mut Stage,
    offer: &'static Offer,
    chooser: Chooser<Choice>,
    press: &Press,
) -> Wanted {
    match over(offer.action, chooser, press) {
        Over::Left => (),
        Over::Choosing(chooser) => *stage = Stage::Choosing { offer, chooser },
        Over::Taken(taken) => {
            if let Some(taken) = or_refused(stage, taken) {
                *stage = Stage::Confirming { offer, taken };
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
pub(crate) mod tests {
    use super::{
        for_key, marking, or_refused, over, subjects, taking, Choice, Chooser, Command, Over,
        Press, Stage, OFFERED, WHOLE,
    };
    use lemonfiber_api::actions::{named, Arguments, OFFERED as WEB};
    use lemonfiber_core::model::{FormReport, FormsReport};

    /// A stack declaring two forms, as a listing shows them.
    pub(crate) fn a_listing() -> FormsReport {
        FormsReport {
            forms: vec![
                FormReport {
                    id: "full".to_owned(),
                    name: "Full stack".to_owned(),
                    description: "everything, behind the tunnel".to_owned(),
                    composable: false,
                },
                FormReport {
                    id: "lean".to_owned(),
                    name: "Lean stack".to_owned(),
                    description: "the download clients only".to_owned(),
                    composable: true,
                },
            ],
        }
    }

    /// A stack that declares no forms at all.
    pub(crate) fn nothing_declared() -> FormsReport {
        FormsReport { forms: Vec::new() }
    }

    /// One form, as a row of the list holds it.
    fn a_form(id: &str) -> Choice {
        Choice {
            name: format!("{id} stack"),
            about: format!("what {id} is for"),
            forms: vec![id.to_owned()],
            marked: false,
            command: Command::Pull {
                forms: vec![id.to_owned()],
            },
        }
    }

    /// The whole stack, which is the one row of the list that names no form.
    fn the_whole_stack() -> Choice {
        Choice {
            name: WHOLE.to_owned(),
            about: super::EVERY_FORM.to_owned(),
            forms: Vec::new(),
            marked: false,
            command: Command::Up { forms: Vec::new() },
        }
    }

    /// The list an action that can mean everything is given: the whole stack, then
    /// two forms.
    fn a_list() -> Chooser<Choice> {
        Chooser::over(the_whole_stack(), vec![a_form("full"), a_form("lean")])
    }

    /// Which rows are marked, read off the list the screen is given rather than off
    /// a field, so what is asserted is what an operator would see marked.
    fn marked(chooser: &Chooser<Choice>) -> Vec<String> {
        chooser
            .listed()
            .filter(|(_, choice)| choice.marked)
            .map(|(_, choice)| choice.name.clone())
            .collect()
    }

    /// The list with the cursor moved to the row of this name.
    ///
    /// Back to the top first and then down, because the cursor may be below the row
    /// being looked for and a walk that only goes one way would stop on whatever it
    /// ended on — which is a mark put on the wrong row, silently.
    fn at(mut chooser: Chooser<Choice>, name: &str) -> Chooser<Choice> {
        for _ in 0..3 {
            chooser.back();
        }
        for _ in 0..3 {
            if chooser
                .listed()
                .any(|(here, choice)| here && choice.name == name)
            {
                break;
            }
            chooser.forward();
        }
        chooser
    }

    /// Space marks the row under the cursor, and space again takes the mark off.
    /// Both from the same key, because a mark nobody can undo is a mark somebody has
    /// to abandon the whole list to escape.
    #[test]
    fn the_row_under_the_cursor_is_marked_and_unmarked_by_the_same_key() {
        let mut chooser = at(a_list(), "full stack");

        marking(&mut chooser);
        assert_eq!(marked(&chooser), vec!["full stack".to_owned()]);

        marking(&mut chooser);
        assert!(marked(&chooser).is_empty(), "{:?}", marked(&chooser));
    }

    /// The whole stack is *instead of* naming forms rather than one more of them.
    /// What the two together would send is what the whole stack alone would send, so
    /// a list showing both marked would be naming something it was not about to send.
    #[test]
    fn the_whole_stack_and_a_form_are_never_marked_together() {
        let mut chooser = at(a_list(), "full stack");
        marking(&mut chooser);
        let mut chooser = at(chooser, WHOLE);

        marking(&mut chooser);
        assert_eq!(marked(&chooser), vec![WHOLE.to_owned()]);

        let mut chooser = at(chooser, "lean stack");
        marking(&mut chooser);
        assert_eq!(marked(&chooser), vec!["lean stack".to_owned()]);
    }

    /// Several marked forms go to the translation as one list, and come back as the
    /// one command the command line produces for the same request — rather than as
    /// several commands this screen would have to decide the order of.
    #[test]
    fn several_marked_forms_reach_one_command_over_all_of_them() {
        let mut chooser = at(a_list(), "full stack");
        marking(&mut chooser);
        let mut chooser = at(chooser, "lean stack");
        marking(&mut chooser);

        let taken = taking("pull", chooser).ok();

        assert_eq!(
            taken.as_ref().map(|taken| taken.command.clone()),
            named(
                "pull",
                Arguments {
                    forms: vec!["full".to_owned(), "lean".to_owned()],
                    ..Arguments::default()
                }
            )
            .ok()
        );
        assert_eq!(taken.map(|taken| taken.name()), Some("2 forms".to_owned()));
    }

    /// Nothing marked is the list this screen has always had: enter takes the row
    /// under the cursor. An operator who never presses the new key gets the screen
    /// they had before it existed.
    #[test]
    fn nothing_marked_takes_the_row_under_the_cursor() {
        let taken = taking("pull", at(a_list(), "lean stack")).ok();

        assert_eq!(
            taken.as_ref().map(|taken| taken.command.clone()),
            Some(Command::Pull {
                forms: vec!["lean".to_owned()]
            })
        );
        assert_eq!(
            taken.map(|taken| taken.name()),
            Some("lean stack".to_owned())
        );
    }

    /// What the screen would say about a stage, as one piece of text.
    fn said(stage: &Stage) -> String {
        crate::acting::words::pane(stage, 20, 100).map_or_else(String::new, |pane| {
            pane.lines
                .iter()
                .flat_map(|line| line.spans.iter())
                .map(|span| span.content.as_ref())
                .collect::<Vec<&str>>()
                .join(" ")
        })
    }

    /// A list the translation will not carry is refused in the words the web surface
    /// gives, and those words are put where the operator is looking rather than
    /// swallowed on the way to a command.
    ///
    /// Nothing this screen offers can reach it: every action here and the guard next
    /// door are names the web's table knows, which the test at the top of this list
    /// and its twin beside the other one both hold. So it is driven through a name
    /// that is not — and it is one path for both lists, because a refusal that read
    /// differently under one key than under another would be this screen having an
    /// opinion about a translation it does not own.
    #[test]
    fn a_list_the_translation_refuses_is_put_in_front_of_the_operator() {
        let mut chooser = at(a_list(), "full stack");
        marking(&mut chooser);
        let mut stage = Stage::Idle;

        let taken = or_refused(&mut stage, taking("nonsense", chooser));

        assert!(taken.is_none());
        assert!(said(&stage).contains("nonsense"), "{}", said(&stage));
    }

    /// The list a press left behind, where it left one.
    fn still_choosing(pressed: Over) -> Option<Chooser<Choice>> {
        match pressed {
            Over::Choosing(chooser) => Some(chooser),
            Over::Taken(_) | Over::Left => None,
        }
    }

    /// Space reaches the marking, enter reaches the taking, and escape leaves the
    /// list with nothing taken — the three things a press over this list can be.
    #[test]
    fn a_press_over_the_list_marks_takes_or_leaves_it() {
        let marked = still_choosing(over("pull", at(a_list(), "full stack"), &Press::Typed(' ')))
            .map(|chooser| marked(&chooser))
            .unwrap_or_default();
        assert_eq!(marked, vec!["full stack".to_owned()]);

        let taken = matches!(
            over("pull", at(a_list(), "full stack"), &Press::Accept),
            Over::Taken(Ok(_))
        );
        assert!(taken);

        assert!(still_choosing(over("pull", a_list(), &Press::Abandon)).is_none());
    }

    /// What one action can be given over this listing.
    fn choices_for(action: &str, report: &FormsReport) -> Vec<Choice> {
        OFFERED
            .iter()
            .filter(|offer| offer.action == action)
            .filter_map(|offer| offer.given(report).ok())
            .flat_map(|(first, rest)| std::iter::once(first).chain(rest))
            .collect()
    }

    /// Why one action can be given nothing over this listing.
    fn refusal_for(action: &str, report: &FormsReport) -> String {
        OFFERED
            .iter()
            .filter(|offer| offer.action == action)
            .filter_map(|offer| offer.given(report).err())
            .collect::<Vec<String>>()
            .join(" ")
    }

    /// The whole point of naming the action rather than assembling a command here:
    /// what this screen offers has to be something another surface already offers,
    /// or the requirement it is being built for is defeated by the thing built for
    /// it.
    #[test]
    fn every_action_this_screen_offers_is_one_the_other_surfaces_offer() {
        let missing: Vec<&str> = OFFERED
            .iter()
            .map(|offer| offer.action)
            .filter(|action| !WEB.contains(action))
            .collect();

        assert!(missing.is_empty(), "{missing:?}");
    }

    /// One key per action, or the second is unreachable and nobody would know.
    #[test]
    fn no_two_actions_answer_to_the_same_key() {
        for offer in OFFERED {
            let same = OFFERED
                .iter()
                .filter(|other| other.key == offer.key)
                .count();
            assert_eq!(same, 1, "more than one action is on {:?}", offer.key);
        }
    }

    /// The four keys the screen already answers stay answered by it.
    #[test]
    fn no_action_takes_a_key_the_screen_already_uses() {
        for taken in [
            'q',
            'r',
            '?',
            crate::acting::question::KEY,
            crate::acting::errand::KEY,
        ] {
            assert!(for_key(taken).is_none(), "{taken:?} was already spoken for");
        }
    }

    #[test]
    fn a_key_no_action_is_on_reaches_none() {
        assert!(for_key('z').is_none());
        assert!(for_key('u').is_some());
    }

    /// Naming nothing means the whole stack for the two whose command can carry an
    /// empty list, and this screen learns which two by asking rather than by
    /// keeping a second list that could come to disagree with the first.
    #[test]
    fn the_whole_stack_is_offered_only_where_the_command_can_carry_it() {
        let listing = a_listing();

        let offering_it: Vec<&str> = OFFERED
            .iter()
            .filter(|offer| {
                choices_for(offer.action, &listing)
                    .iter()
                    .any(|choice| choice.name == WHOLE)
            })
            .map(|offer| offer.action)
            .collect();

        assert_eq!(offering_it, vec!["up", "down"]);
    }

    /// Every form the stack declares is a choice, named as the stack names it.
    #[test]
    fn each_form_the_stack_declares_is_offered_in_the_stacks_own_words() {
        let choices = choices_for("restart", &a_listing());

        let named: Vec<&str> = choices.iter().map(|choice| choice.name.as_str()).collect();
        assert_eq!(named, vec!["Full stack", "Lean stack"]);
        let about: Vec<&str> = choices.iter().map(|choice| choice.about.as_str()).collect();
        assert!(about.contains(&"the download clients only"), "{about:?}");
    }

    /// The command a choice comes to is the command the other surfaces produce for
    /// the same request, rather than one this screen assembled.
    #[test]
    fn a_choice_comes_to_the_command_every_surface_produces() {
        let asked = Arguments {
            forms: vec!["full".to_owned()],
            ..Arguments::default()
        };
        let wanted = named("restart", asked).ok();

        let reached = choices_for("restart", &a_listing())
            .into_iter()
            .next()
            .map(|choice| choice.command);

        assert!(wanted.is_some(), "the web surface translates this one");
        assert_eq!(reached, wanted);
    }

    /// A stack with no forms leaves an action that insists on one with nothing to
    /// offer, and the words the operator gets are the words the web surface gives
    /// for the same request rather than a sentence this screen wrote.
    #[test]
    fn an_action_that_insists_on_a_form_says_so_where_the_stack_declares_none() {
        let said = refusal_for("switch", &nothing_declared());

        assert!(said.contains("switch"), "{said}");
        assert!(said.contains("forms"), "{said}");
        assert!(choices_for("switch", &nothing_declared()).is_empty());
    }

    /// The two that can mean the whole stack still have something to offer there.
    #[test]
    fn an_action_that_can_mean_everything_offers_it_even_with_no_forms_declared() {
        let choices = choices_for("down", &nothing_declared());

        assert_eq!(choices.len(), 1);
        assert!(choices.iter().any(|choice| choice.name == WHOLE));
        assert!(refusal_for("down", &nothing_declared()).is_empty());
    }

    /// The whole stack leads, because it is the one choice that is not a form and
    /// reading it after the forms would read as another of them.
    #[test]
    fn the_whole_stack_is_the_first_subject_offered() {
        let subjects = subjects(&a_listing());

        let first = subjects
            .first()
            .map(|(forms, name, _)| (forms.len(), name.clone()));
        assert_eq!(first, Some((0, WHOLE.to_owned())));
        assert_eq!(subjects.len(), 3);
    }
}
