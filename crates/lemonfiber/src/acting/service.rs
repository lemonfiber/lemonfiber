//! Naming the services inside what an action was already given.
//!
//! Four of this screen's requests take a service and had no way to be given one.
//! Three of them are lifecycle actions on keys of their own — starting, stopping and
//! restarting some of a form's services rather than the whole form — and the fourth
//! is the capture behind the other list, which takes the one service whose
//! configuration goes into the archive. The command line spells all four
//! `--service`; the list this screen offered was the stack's own forms, and the
//! services inside one were a gather it did not have.
//!
//! **It has the gather.** The panel this box is drawn over lists every service and
//! what it is doing, refreshed every second, taken from the manifest rather than
//! from what happens to be running — so a service that has never started is on it.
//! The names are therefore in hand at the moment the key is pressed, which is the
//! rule the narrowing next door already follows: pick where the list is already
//! there, and type only where fetching it would cost the very request being narrowed
//! away from. Here it would cost more than that. A read asked between a keypress and
//! the frame after it is awaited in the loop with nothing drawn, and this one reaches
//! the container engine — so a line to type a name on would have been the only other
//! option, and a typed service name is a name nothing checked before the work ran.
//!
//! **One list, whatever it is filling.** `up` and `down` reach a different command
//! when services are named — `Command::Start` beside `Command::Up`, the way Compose
//! spells the pair — while a restart and a capture carry the services as an argument
//! to the command they already reach. None of that is visible here, because nothing
//! here assembles a command: every row goes through
//! [`lemonfiber_api::actions::named`] and comes back as whatever that action reaches
//! given what it now has. The fork is the table's, and a screen that had to know
//! about it would be a screen keeping a second copy of the table.
//!
//! What does differ is one name against a list of them, and that is the table's
//! answer too — a capture's scope is one scope, so its rows carry no box to mark and
//! enter takes the row under the cursor.
//!
//! **Naming no service is going on with what was already named.** Every one of the
//! four reads an empty list as the whole of what it was given, so the row that says
//! so is offered on all four — unlike the list of forms, where three actions refuse
//! an empty one and the row is dropped for them. An action taking that row goes on
//! with the question it would have been put without this list at all, so the forms it
//! names are named by their own names rather than by a row saying every service.

use lemonfiber_api::actions::{Arguments, TAKES_SERVICE};
use lemonfiber_core::dashboard::Panel;
use lemonfiber_core::docker::Service;

use super::chooser::Chooser;
use super::errand::{self, Errand, Given};
use super::offer::{choices, or_refused, over, Choice, Fills, Offer, Over, Taken};
use super::{Press, Stage, Wanted};

/// What the row naming no service is called on an action's list, and what taking it
/// comes to.
///
/// Its own wording rather than the errand's, because the row means a different whole
/// under each. An action arrives having already been given its forms, so what it
/// declines to narrow is those; the errand arrives having been given nothing, so what
/// it declines to narrow is the stack.
const ALL_OF_THEM: (&str, &str) = (
    "all of them",
    "every service in what was named, rather than some of them",
);

/// The same row on the capture's list, which has narrowed nothing yet.
const WHOLE_STACK: (&str, &str) = (
    "the whole stack",
    "every service's configuration, rather than one service's",
);

/// What a list of services is being named for, and what naming them leads to.
///
/// The two flows that reach this list, held as what each needs to go on with rather
/// than as a flag: an action arrives having already been given its forms, and the
/// capture arrives having been given nothing.
pub(super) enum Inside {
    /// One of the actions on a key of its own, and what its list of forms came to.
    Action {
        /// The action being named services for.
        offer: &'static Offer,
        /// What its list of forms came to, which is what goes ahead where no service
        /// is named.
        taken: Taken,
    },
    /// The errand that captures one service's configuration.
    Errand(&'static Errand),
}

impl Inside {
    /// The action every surface calls this by.
    const fn action(&self) -> &'static str {
        match *self {
            Self::Action { offer, .. } => offer.action,
            Self::Errand(errand) => errand.action,
        }
    }

    /// What was named before this list, which the services are named beside.
    fn before(&self) -> Arguments {
        match *self {
            Self::Action { ref taken, .. } => beside(taken),
            Self::Errand(_) => Arguments::default(),
        }
    }

    /// What follows the list, once something has been taken off it.
    fn onwards(self, stage: &mut Stage, inside: Taken) -> Wanted {
        match self {
            Self::Action { offer, taken } => {
                *stage = Stage::Confirming {
                    offer,
                    taken: went_on(taken, inside),
                };
                Wanted::Nothing
            }
            Self::Errand(errand) => errand::begun(stage, errand, Given::picked(&inside)),
        }
    }
}

/// The forms an action was already given, as the argument a service is named beside.
fn beside(taken: &Taken) -> Arguments {
    Arguments {
        forms: taken.named(),
        ..Arguments::default()
    }
}

/// Which of the arguments this action's services go in.
///
/// Asked of the table that publishes both rather than decided again here. The command
/// line spells them alike and what tells them apart is that an archive records one
/// scope, which is that table's fact about the command rather than this screen's
/// about its list.
fn fills(action: &str) -> Fills {
    if TAKES_SERVICE.contains(&action) {
        Fills::Service
    } else {
        Fills::Services
    }
}

/// What an action goes on to be asked about: the services where any were named, and
/// what it was already given where none were.
///
/// The row naming no service comes to the same command the list before it came to, so
/// what is left to decide is which words the question is put in. Naming nothing
/// narrows nothing, so it is put in the words it would have been put in without this
/// list at all — the forms by their own names, rather than a row saying every service.
fn went_on(before: Taken, inside: Taken) -> Taken {
    if inside.covers.iter().all(|choice| choice.names.is_empty()) {
        before
    } else {
        inside
    }
}

/// The services the screen has in hand, as the rows a list is built from.
///
/// Nothing at all where the panel could not be filled: a stack the engine cannot be
/// asked about has no services to narrow to.
pub(super) fn gathered(panel: &Panel<Vec<Service>>) -> Vec<(String, String, String)> {
    let Panel::Ready(services) = panel else {
        return Vec::new();
    };
    services
        .iter()
        .map(|service| {
            (
                service.id.clone(),
                service.name.clone(),
                crate::render::stack::doing(service),
            )
        })
        .collect()
}

/// The list of services this action could be given, or the question itself where it
/// could be given none.
pub(super) fn or_the_question(
    offer: &'static Offer,
    taken: Taken,
    gathered: &[(String, String, String)],
) -> Stage {
    match offered(offer.action, ALL_OF_THEM, &beside(&taken), gathered) {
        Some(chooser) => Stage::Inside {
            inside: Inside::Action { offer, taken },
            chooser,
        },
        None => Stage::Confirming { offer, taken },
    }
}

/// What a capture is given where there was no service to choose between.
pub(super) fn nothing_to_choose() -> Given {
    Given::whole(WHOLE_STACK.0)
}

/// The list of services this errand could be given, or nothing where there are none.
pub(super) fn for_the_errand(
    errand: &'static Errand,
    gathered: &[(String, String, String)],
) -> Option<Stage> {
    offered(errand.action, WHOLE_STACK, &Arguments::default(), gathered).map(|chooser| {
        Stage::Inside {
            inside: Inside::Errand(errand),
            chooser,
        }
    })
}

/// The list one of these could be given, or nothing where it could be given none.
///
/// Every service goes through the translation and only what comes to a command is
/// offered, which is [`super::offer::choices`]'s rule rather than a second one. What
/// survives is then held to one thing: a list has to offer more than the row that
/// names nothing, or it is not a choice.
///
/// That one rule answers both ways this list can come to nothing. An action with
/// nowhere to put a service has every service row refused, and a screen that could
/// not reach the container engine has no service row to refuse — and in each case the
/// flow goes on as though this list did not exist. Which actions those are is the
/// translation's answer rather than a list of names kept here.
fn offered(
    action: &str,
    whole: (&str, &str),
    before: &Arguments,
    gathered: &[(String, String, String)],
) -> Option<Chooser<Choice>> {
    let mut subjects = vec![(Vec::new(), whole.0.to_owned(), whole.1.to_owned())];
    subjects.extend(
        gathered
            .iter()
            .map(|(id, name, doing)| (vec![id.clone()], name.clone(), doing.clone())),
    );
    let (first, rest) = choices(action, fills(action), before, subjects).ok()?;
    (!rest.is_empty()).then(|| Chooser::over(first, rest))
}

/// Over the services inside what was named: move, mark, take, or leave it.
pub(super) fn choosing(
    stage: &mut Stage,
    inside: Inside,
    chooser: Chooser<Choice>,
    press: &Press,
) -> Wanted {
    let action = inside.action();
    match over(action, fills(action), &inside.before(), chooser, press) {
        Over::Left => Wanted::Nothing,
        Over::Choosing(chooser) => {
            *stage = Stage::Inside { inside, chooser };
            Wanted::Nothing
        }
        Over::Taken(taken) => match or_refused(stage, taken) {
            Some(taken) => inside.onwards(stage, taken),
            None => Wanted::Nothing,
        },
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::{choosing, fills, for_the_errand, gathered, or_the_question, Fills};
    use crate::acting::errand::tests::sending;
    use crate::acting::offer::tests::{a_form_taken, offering};
    use crate::acting::{Press, Stage};
    use lemonfiber_api::actions::{named, Arguments, TAKES_SERVICE, TAKES_SERVICES};
    use lemonfiber_core::app::{Command, Waiting};
    use lemonfiber_core::dashboard::Panel;
    use lemonfiber_core::docker::{Criticality, Service, State};

    /// The four requests that take a service, by the name every surface calls them.
    const TAKING_ONE: [&str; 4] = ["up", "down", "restart", "backup"];

    /// One service as the gather holds it.
    fn a_service(id: &str, name: &str, state: State) -> Service {
        Service {
            id: id.to_owned(),
            name: name.to_owned(),
            profile: "tv".to_owned(),
            state,
            criticality: Criticality::Core,
            depends_on: Vec::new(),
            exit: None,
        }
    }

    /// A gather holding two services, one of them unwell.
    pub(crate) fn two_services() -> Panel<Vec<Service>> {
        Panel::Ready(vec![
            a_service("sonarr", "Sonarr", State::Healthy),
            a_service("radarr", "Radarr", State::CrashLooping),
        ])
    }

    /// The same, as the screen keeps it between gathers.
    pub(crate) fn in_hand() -> Vec<(String, String, String)> {
        gathered(&two_services())
    }

    /// The screen with one action's list of services open, over that gather.
    ///
    /// Opened the way the screen opens it, so an action offered no list at all
    /// arrives at whatever it would have arrived at instead — which is the thing two
    /// of these tests are about.
    fn opened(action: &str, having: &[(String, String, String)]) -> Stage {
        match offering(action).zip(a_form_taken(action)) {
            Some((offer, taken)) => or_the_question(offer, taken, having),
            None => sending(action)
                .and_then(|errand| for_the_errand(errand, having))
                .map_or(Stage::Idle, |stage| stage),
        }
    }

    /// Every row of the list that stage holds, or nothing where it holds no list.
    fn listed(stage: &Stage) -> Vec<(String, String, Vec<String>, Option<bool>)> {
        match *stage {
            Stage::Inside { ref chooser, .. } => chooser
                .listed()
                .map(|(_, choice)| {
                    (
                        choice.name.clone(),
                        choice.about.clone(),
                        choice.names.clone(),
                        choice.marked,
                    )
                })
                .collect(),
            _ => Vec::new(),
        }
    }

    /// The presses that walk the cursor from the top of the list onto this row.
    ///
    /// By name rather than by number, because a service added to the gather above
    /// one of these would silently renumber every test below it.
    fn onto(stage: &Stage, name: &str) -> Vec<Press> {
        let above = listed(stage).iter().take_while(|row| row.0 != name).count();
        (0..above).map(|_| Press::Forward).collect()
    }

    /// The stage a run of presses over that list leaves behind.
    fn after(mut stage: Stage, presses: &[Press]) -> Stage {
        for press in presses {
            stage = match stage {
                Stage::Inside { inside, chooser } => {
                    let mut next = Stage::Idle;
                    choosing(&mut next, inside, chooser, press);
                    next
                }
                other => other,
            };
        }
        stage
    }

    /// The command the question a stage holds is about, or nothing where it holds
    /// no question.
    fn carried(stage: &Stage) -> Option<Command> {
        match *stage {
            Stage::Confirming { ref taken, .. } => Some(taken.command.clone()),
            _ => None,
        }
    }

    /// What the question a stage holds calls what it is about.
    fn about(stage: &Stage) -> String {
        match *stage {
            Stage::Confirming { ref taken, .. } => taken.name(),
            Stage::Agreeing { ref given, .. } => given.said().to_owned(),
            _ => String::new(),
        }
    }

    /// The list with one row of it taken, as the stage that leaves.
    fn taking(action: &str, row: &str) -> Stage {
        let stage = opened(action, &in_hand());
        let mut presses = onto(&stage, row);
        presses.push(Press::Accept);
        after(stage, &presses)
    }

    /// What one of the four is given with no service named: the form the three
    /// lifecycle actions are given, and nothing at all for the capture, which takes
    /// no form.
    fn without_a_service(action: &str) -> Arguments {
        let mut given = Arguments::default();
        if action != "backup" {
            given.forms = vec!["full".to_owned()];
        }
        given
    }

    /// Naming no service means the whole of what was named, on every one of the four
    /// — asked of the table rather than assumed, because the list of forms beside
    /// this one is the opposite: three actions there refuse an empty list of forms,
    /// and two lists assumed to agree is exactly where that trap was found.
    #[test]
    fn naming_no_service_is_the_whole_of_what_was_named_on_all_four() {
        let refused: Vec<&str> = TAKING_ONE
            .into_iter()
            .filter(|action| named(action, without_a_service(action)).is_err())
            .collect();

        assert!(refused.is_empty(), "{refused:?}");
        assert_eq!(
            named("backup", Arguments::default()).ok(),
            Some(Command::Backup { service: None })
        );
    }

    /// So the row naming none is on all four lists — which the list of forms cannot
    /// say, because there it is dropped for the three that refuse it.
    #[test]
    fn the_row_naming_no_service_is_offered_on_every_one_of_the_four() {
        for action in TAKING_ONE {
            let rows: Vec<String> = listed(&opened(action, &in_hand()))
                .into_iter()
                .map(|(name, _, _, _)| name)
                .collect();

            assert_eq!(rows.len(), 3, "{action}: {rows:?}");
            assert!(
                rows.first().is_some_and(
                    |first| first.starts_with("all of") || first.starts_with("the whole")
                ),
                "{action}: {rows:?}"
            );
        }
    }

    /// An action with nowhere to put a service is offered no list at all, and which
    /// actions those are is the translation's answer rather than a list kept here.
    #[test]
    fn an_action_that_carries_no_service_is_offered_no_list() {
        for action in ["switch", "pull"] {
            assert!(
                listed(&opened(action, &in_hand())).is_empty(),
                "{action} was offered a list of services"
            );
        }
        assert_eq!(listed(&opened("up", &in_hand())).len(), 3);
    }

    /// The service is picked and never typed: the row shows the name the manifest
    /// gives an operator and what the service is doing, and what is handed on is the
    /// identifier — which is the half a typed name would get wrong.
    #[test]
    fn a_service_is_shown_by_its_name_and_handed_on_by_its_identifier() {
        let rows = listed(&opened("restart", &in_hand()));

        assert_eq!(
            rows.get(1).map(|(name, doing, names, _)| (
                name.as_str(),
                doing.as_str(),
                names.clone()
            )),
            Some(("Sonarr", "healthy", vec!["sonarr".to_owned()]))
        );
        assert_eq!(
            rows.get(2).map(|(_, doing, _, _)| doing.as_str()),
            Some("crash-looping")
        );
    }

    /// Starting named services and stopping them reach a different command from the
    /// whole-form one, and the fork is the table's: nothing here chooses between
    /// `Up` and `Start`, or between `Down` and `Halt`, and the restart beside them
    /// keeps the command it already reached.
    #[test]
    fn naming_a_service_reaches_whatever_command_that_action_forks_to() {
        assert_eq!(
            carried(&taking("up", "Sonarr")),
            Some(Command::Start {
                forms: vec!["full".to_owned()],
                services: vec!["sonarr".to_owned()],
            })
        );
        assert_eq!(
            carried(&taking("down", "Sonarr")),
            Some(Command::Halt {
                forms: vec!["full".to_owned()],
                services: vec!["sonarr".to_owned()],
            })
        );
        assert_eq!(
            carried(&taking("restart", "Sonarr")),
            Some(Command::Restart {
                forms: vec!["full".to_owned()],
                services: vec!["sonarr".to_owned()],
            })
        );
    }

    /// Several are named together, because `--service` takes a list. The marks are
    /// the ones the list of forms already draws, over the list beside it.
    #[test]
    fn several_services_are_named_together() {
        let stage = opened("up", &in_hand());
        let mut presses = onto(&stage, "Sonarr");
        presses.extend([
            Press::Typed(' '),
            Press::Forward,
            Press::Typed(' '),
            Press::Accept,
        ]);

        let stage = after(stage, &presses);

        assert_eq!(
            carried(&stage),
            Some(Command::Start {
                forms: vec!["full".to_owned()],
                services: vec!["sonarr".to_owned(), "radarr".to_owned()],
            })
        );
        assert_eq!(about(&stage), "2 services");
    }

    /// Naming no service goes on with what was already named, in the words that
    /// question would have been put in without this list at all — the form by its own
    /// name, rather than a row saying every service.
    #[test]
    fn naming_no_service_goes_on_with_what_was_already_named() {
        let stage = taking("down", "all of them");

        assert_eq!(about(&stage), "Full stack");
        assert_eq!(
            carried(&stage),
            Some(Command::Down {
                forms: vec!["full".to_owned()],
                wait: Waiting::Never,
            })
        );
    }

    /// A capture records one scope, so its list takes one: no row carries a box to
    /// mark. Which of the two arguments an action fills is the table's answer rather
    /// than a rule written here.
    #[test]
    fn the_capture_takes_one_service_and_the_others_take_a_list() {
        let capture: Vec<Option<bool>> = listed(&opened("backup", &in_hand()))
            .into_iter()
            .map(|(_, _, _, marked)| marked)
            .collect();
        let lifecycle: Vec<Option<bool>> = listed(&opened("up", &in_hand()))
            .into_iter()
            .map(|(_, _, _, marked)| marked)
            .collect();

        assert_eq!(capture, vec![None, None, None]);
        assert_eq!(lifecycle, vec![Some(false), Some(false), Some(false)]);
        assert!(TAKES_SERVICE.contains(&"backup") && TAKES_SERVICES.contains(&"up"));
        assert!(matches!(fills("backup"), Fills::Service));
        assert!(matches!(fills("up"), Fills::Services));
    }

    /// The capture is given the service off the list and says which one it is about,
    /// rather than a name nothing checked before the capture ran.
    #[test]
    fn the_capture_is_given_the_service_it_was_shown() {
        let whole = taking("backup", "the whole stack");

        assert_eq!(about(&taking("backup", "Radarr")), "Radarr");
        assert_eq!(about(&whole), "the whole stack");
        assert!(carried(&whole).is_none(), "a capture is not an action");
    }

    /// A gather that could not be filled has no service to narrow to, so no list is
    /// opened and the flow is the one this screen has always had.
    #[test]
    fn a_gather_that_could_not_be_filled_opens_no_list() {
        let nothing = gathered(&Panel::unavailable("the engine did not answer"));

        assert!(nothing.is_empty());
        assert!(matches!(opened("up", &nothing), Stage::Confirming { .. }));
        assert!(matches!(opened("backup", &nothing), Stage::Idle));
    }

    /// One service in hand is still a choice, because the row naming none is a real
    /// alternative to it — which is why the list is dropped for having nothing on it
    /// rather than for being short.
    #[test]
    fn one_service_in_hand_is_still_a_choice() {
        let one: Vec<(String, String, String)> = in_hand().into_iter().take(1).collect();

        let rows: Vec<String> = listed(&opened("up", &one))
            .into_iter()
            .map(|(name, _, _, _)| name)
            .collect();

        assert_eq!(rows, vec!["all of them".to_owned(), "Sonarr".to_owned()]);
    }
}
