//! What the dashboard offers to do, and what each offer can be given.
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
//! in front of the operator.

use lemonfiber_api::actions::{named, Arguments};
use lemonfiber_core::app::Command;
use lemonfiber_core::model::FormsReport;

use super::chooser::Listed;

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
    /// The command acting on it comes to.
    pub(crate) command: Command,
}

impl Listed for Choice {
    fn name(&self) -> &str {
        &self.name
    }

    fn about(&self) -> &str {
        &self.about
    }
}

impl Offer {
    /// What this action can be given, or the refusal where it can be given nothing.
    ///
    /// Every subject goes through the translation, and only what comes to a command
    /// is offered. A stack that declares no forms leaves the three actions that
    /// insist on one with nothing to offer, and the words the operator gets then are
    /// the words the web surface gives for the same request.
    ///
    /// The first choice comes back apart from the rest, so that what is handed on is
    /// a list something is already selected in. A list and a selection carried
    /// separately can disagree, and the place they would disagree is under the
    /// operator's finger.
    pub(crate) fn given(&self, report: &FormsReport) -> Result<(Choice, Vec<Choice>), String> {
        let mut choices = Vec::new();
        let mut refused = String::new();
        for (forms, name, about) in subjects(report) {
            let asked = Arguments {
                forms,
                ..Arguments::default()
            };
            match named(self.action, asked) {
                Ok(command) => choices.push(Choice {
                    name,
                    about,
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
    use super::{for_key, subjects, Choice, OFFERED, WHOLE};
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
