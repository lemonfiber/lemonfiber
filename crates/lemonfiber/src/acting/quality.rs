//! How good this library is to be, and the two ways a choice is put into force.
//!
//! Three writes behind one key, beside the reading they are all about. `quality` is
//! already one of the questions [`super::question`] opens — the preset in force,
//! what each one means and what it costs — and these three are the whole of what can
//! be done about what that reading reports. They are on a key of their own rather
//! than on the list of errands, for a reason that is not tidiness: the agreement does
//! not mean the same thing there.
//!
//! **An errand's unconfirmed run is a rehearsal. A quality choice's is the choice.**
//! Every errand that carries an agreement answers, unconfirmed, with what it would do
//! and changes nothing — which is what makes that run the account its question sits
//! under. `quality-set` is not built that way. Unconfirmed it *records* the choice,
//! and holds it only where this host would have to transcode the result in software,
//! which is the one cost its agreement is for. A list whose rule is "unconfirmed says
//! what it would do" cannot take an action for which that is false without the rule
//! quietly becoming untrue for the ones it was written for — and the place it would
//! become untrue is in front of somebody about to throw work away.
//!
//! **So what goes in front of each question is what that change actually has to
//! say**, and the three do not have the same thing to say:
//!
//! - Choosing is made off the four presets, each with what it means and roughly what
//!   an hour of it costs. The account is the list the choice comes off, because the
//!   run that would otherwise have stated it is the run that records it.
//! - Re-asserting has nothing to say first. It carries no agreement, and the core's
//!   report-only half of it is behind `--dry-run`, which is a property of a run
//!   rather than of a request — so no surface's action can ask for one. A preamble
//!   invented here for symmetry would be this screen claiming a rehearsal happened.
//! - Upgrading says what it would cost before anything is fetched: each media type,
//!   the bar in force for it, and roughly what an hour of that takes. That run
//!   triggers nothing, which is the errands' own pattern arriving where it fits.
//!
//! **The agreement is carried once the account has been read, and never before.**
//! Which of the three carries one at all is asked of
//! [`lemonfiber_api::actions::TAKES_AGREEMENT`] rather than written down again here:
//! a second list would come to disagree with the first, and it would disagree in
//! front of somebody about to spend a week of their connection.
//!
//! **A cost this host would pay is read before it is agreed to.** A choice this
//! machine could only transcode in software comes back held rather than recorded,
//! and the caution the core answers with is the account the second question sits
//! under. It arrives there rather than before the first question because whether
//! there is a cost at all depends on the media server and the platform, which are
//! the core's to know and not this screen's to guess.

mod chosen;

use lemonfiber_api::actions::{named, TAKES_AGREEMENT};
use lemonfiber_core::app::{Command, Outcome};
use lemonfiber_core::model::Disposition;
use lemonfiber_core::recyclarr::Kind;

use super::chooser::{Chooser, Listed};
use super::reading::{moved, Reading};
use super::{Press, Stage, Wanted};

pub(crate) use chosen::{Chosen, Grade, Scope};

/// The key that opens the three.
///
/// Not the letter the word begins with: `q` closes the screen, and a key that quit
/// for eleven slices and then changed its mind would be worse than an arbitrary one.
pub(crate) const KEY: char = 'c';

/// The word the footer puts beside that key.
pub(crate) const HINT: &str = "quality";

/// What is put in front of the question.
enum Before {
    /// The four presets, each with what it means and what it costs. The choice is
    /// made off the account rather than after it, because the run that would
    /// otherwise state the consequence is the run that records the choice.
    Presets,
    /// Nothing. The action carries no agreement and has no half that only reports,
    /// so there is nothing to put there that would be true.
    Nothing,
    /// The action's own unconfirmed run, which states what it would cost and
    /// triggers nothing.
    Cost,
}

/// One change to the quality this stack aims for.
pub(crate) struct Change {
    /// What it is called on the list, and on the box while it runs.
    pub(crate) name: &'static str,
    /// What it does, in one line.
    pub(crate) about: &'static str,
    /// The name every surface calls this action by.
    pub(crate) action: &'static str,
    /// How the question before it begins, what it was chosen completing it.
    pub(crate) asks: &'static str,
    /// What is put in front of that question.
    before: Before,
}

/// The change the list opens on.
///
/// Held apart from the rest for the reason the selected errand and the selected
/// question are: a list built from a slice that might have been empty carries a case
/// for there being nothing to choose, which is not a state this screen can be in.
static OPENS_ON: Change = Change {
    name: "how good it should be",
    about: "choose what future downloads aim for; nothing already downloaded changes",
    action: "quality-set",
    asks: "Aim for",
    before: Before::Presets,
};

/// The two after it, read from the one that changes what happens next towards the
/// one that spends the connection — which is also the order nobody starts a
/// week-long download by pressing enter at a list they have only just opened.
static AFTER: &[Change] = &[
    Change {
        name: "the choice put back over your edits",
        about: "let the chosen preset overwrite the quality file you edited by hand",
        action: "quality-reapply",
        asks: "Overwrite your own edits with the chosen preset",
        before: Before::Nothing,
    },
    Change {
        name: "what is already here, upgraded",
        about: "fetch the library again at the quality chosen, which costs bandwidth and days",
        action: "quality-upgrade",
        asks: "Fetch everything again at the quality chosen",
        before: Before::Cost,
    },
];

impl Listed for Change {
    fn name(&self) -> &str {
        self.name
    }

    fn about(&self) -> &str {
        self.about
    }
}

impl Change {
    /// What this change sends, given what it was chosen and whether the account in
    /// front of the question has been read.
    ///
    /// The agreement goes on once there is an account and never before it. Whether
    /// this action carries one at all is the web's own table's answer rather than a
    /// second one kept here, so an action that stopped taking an agreement stops
    /// being sent one on this screen too.
    fn sent(&self, chosen: &Chosen, read: bool) -> Result<Command, String> {
        let mut given = chosen.asked();
        given.confirm = read && TAKES_AGREEMENT.contains(&self.action);
        named(self.action, given).map_err(|no| no.said())
    }

    /// What a choice can be made about, or the refusal where it can be made about
    /// nothing.
    ///
    /// The whole library, then each kind of media the quality model configures, then
    /// music. Every one of them goes through the translation carrying every bar in
    /// turn, and a scope no bar at all reaches a command for is not offered — which is
    /// the rule the five actions on their own keys build their subjects by, and the
    /// reason no list of media types is written down on this screen.
    fn scopes(&self) -> Result<(Scope, Vec<Scope>), String> {
        let offered = std::iter::once(Scope::everything())
            .chain(Kind::ALL.into_iter().map(Scope::kind))
            .chain(std::iter::once(Scope::music()));
        let mut scopes = Vec::new();
        let mut refused = String::new();
        for scope in offered {
            match self.grades(&Chosen::media(&scope)) {
                Ok(_) => scopes.push(scope),
                Err(said) => refused = said,
            }
        }
        let mut offered = scopes.into_iter();
        match offered.next() {
            Some(first) => Ok((first, offered.collect())),
            None => Err(refused),
        }
    }

    /// The bars this change can be given for that media, or the refusal where it can
    /// be given none.
    ///
    /// Every bar goes through the translation and only what comes to a command is
    /// offered. That is what makes music three audio formats where everything else is
    /// four resolution presets: the same list is put to the same table, and the table
    /// keeps what the action it reaches can carry. The first comes back apart from the
    /// rest, so what is handed on is a list something is already selected in.
    fn grades(&self, about: &Chosen) -> Result<(Grade, Vec<Grade>), String> {
        let mut grades = Vec::new();
        let mut refused = String::new();
        for grade in Grade::every() {
            match self.sent(&about.graded(grade.name), false) {
                Ok(_) => grades.push(grade),
                Err(said) => refused = said,
            }
        }
        let mut offered = grades.into_iter();
        match offered.next() {
            Some(first) => Ok((first, offered.collect())),
            None => Err(refused),
        }
    }
}

/// The three, the one the list opens on apart from the rest.
pub(super) fn all() -> (&'static Change, Vec<&'static Change>) {
    (&OPENS_ON, AFTER.iter().collect())
}

/// Every one of them, in the order they are read.
#[cfg(test)]
pub(super) fn every() -> impl Iterator<Item = &'static Change> {
    std::iter::once(&OPENS_ON).chain(AFTER)
}

/// Over the list: move, take one, or leave it.
pub(super) fn deciding(
    stage: &mut Stage,
    mut chooser: Chooser<&'static Change>,
    press: &Press,
) -> Wanted {
    match *press {
        Press::Abandon => return Wanted::Nothing,
        Press::Accept => return taken(stage, chooser.taken()),
        Press::Back => chooser.back(),
        Press::Forward => chooser.forward(),
        Press::Typed(_) | Press::Rubout => (),
    }
    *stage = Stage::Deciding(chooser);
    Wanted::Nothing
}

/// Take the one selected: open the media a choice is about, ask what it would cost,
/// or put the question where the change has nothing to say first.
fn taken(stage: &mut Stage, change: &'static Change) -> Wanted {
    match change.before {
        Before::Presets => match change.scopes() {
            Ok((first, rest)) => {
                *stage = Stage::Scoping {
                    change,
                    chooser: Chooser::over(first, rest),
                };
                Wanted::Nothing
            }
            Err(said) => came(stage, said),
        },
        Before::Nothing => {
            *stage = Stage::Settling {
                change,
                chosen: Chosen::nothing(),
                account: None,
            };
            Wanted::Nothing
        }
        Before::Cost => match change.sent(&Chosen::nothing(), false) {
            Ok(command) => {
                *stage = Stage::Costing { change };
                Wanted::Carry(command)
            }
            Err(said) => came(stage, said),
        },
    }
}

/// Over the media a choice can be about: move, take one, or leave it.
///
/// Taking one opens the bars that media can be given, which is the same list put to
/// the same table with a different media named — so music comes back as three audio
/// formats and everything else as four resolution presets, without this screen
/// knowing which is which.
pub(super) fn scoping(
    stage: &mut Stage,
    change: &'static Change,
    mut chooser: Chooser<Scope>,
    press: &Press,
) -> Wanted {
    match *press {
        Press::Abandon => return Wanted::Nothing,
        Press::Accept => {
            let chosen = Chosen::media(&chooser.taken());
            return match change.grades(&chosen) {
                Ok((first, rest)) => {
                    *stage = Stage::Grading {
                        change,
                        chosen,
                        chooser: Chooser::over(first, rest),
                    };
                    Wanted::Nothing
                }
                Err(said) => came(stage, said),
            };
        }
        Press::Back => chooser.back(),
        Press::Forward => chooser.forward(),
        Press::Typed(_) | Press::Rubout => (),
    }
    *stage = Stage::Scoping { change, chooser };
    Wanted::Nothing
}

/// Over the bars: move, take one, or leave it.
pub(super) fn grading(
    stage: &mut Stage,
    change: &'static Change,
    chosen: Chosen,
    mut chooser: Chooser<Grade>,
    press: &Press,
) -> Wanted {
    match *press {
        Press::Abandon => return Wanted::Nothing,
        Press::Accept => {
            *stage = Stage::Settling {
                change,
                chosen: chosen.graded(chooser.taken().name),
                account: None,
            };
            return Wanted::Nothing;
        }
        Press::Back => chooser.back(),
        Press::Forward => chooser.forward(),
        Press::Typed(_) | Press::Rubout => (),
    }
    *stage = Stage::Grading {
        change,
        chosen,
        chooser,
    };
    Wanted::Nothing
}

/// While what it would cost is with the core: back out, or wait for it.
pub(super) fn costing(stage: &mut Stage, change: &'static Change, press: &Press) -> Wanted {
    if matches!(*press, Press::Abandon) {
        return Wanted::Nothing;
    }
    *stage = Stage::Costing { change };
    Wanted::Nothing
}

/// What the core said it would cost, held for the operator to read and answer.
pub(super) fn costed(change: &'static Change, would: Vec<String>) -> Stage {
    Stage::Settling {
        change,
        chosen: Chosen::nothing(),
        account: Some(Reading::of(would)),
    }
}

/// At the question: move through the account, agree to it, or leave it.
///
/// Only an explicit yes goes ahead, the way the teardown's own question is read and
/// the way every errand is offered. Everything else that is not a move puts the box
/// away and changes nothing.
pub(super) fn settling(
    stage: &mut Stage,
    change: &'static Change,
    chosen: Chosen,
    mut account: Option<Reading>,
    press: &Press,
) -> Wanted {
    if let Some(reading) = account.as_mut() {
        if moved(reading, press) {
            *stage = Stage::Settling {
                change,
                chosen,
                account,
            };
            return Wanted::Nothing;
        }
    }
    if !matches!(*press, Press::Typed('y' | 'Y')) {
        return Wanted::Nothing;
    }
    match change.sent(&chosen, account.is_some()) {
        Ok(command) => {
            *stage = Stage::Applying { change, chosen };
            Wanted::Carry(command)
        }
        Err(said) => came(stage, said),
    }
}

/// While the change is with the core: leaving is the only thing left to ask.
pub(super) fn applying(
    stage: &mut Stage,
    change: &'static Change,
    chosen: Chosen,
    press: &Press,
) -> Wanted {
    *stage = Stage::Applying { change, chosen };
    if super::leaving(press) {
        return Wanted::Leave;
    }
    Wanted::Nothing
}

/// What a change came to: the report, or the caution with the question under it.
///
/// A choice this host could only transcode in software is held rather than recorded,
/// and being held is the one cost the agreement on that action is for. So the caution
/// the core answered with becomes the account, and the question goes under it — the
/// same reading a reset is offered under, arriving here rather than before the first
/// question because whether there is a cost at all is the core's to know.
pub(super) fn applied(
    change: &'static Change,
    chosen: Chosen,
    outcome: &Outcome,
    said: Vec<String>,
) -> Stage {
    if held(outcome) {
        return Stage::Settling {
            change,
            chosen,
            account: Some(Reading::of(said)),
        };
    }
    Stage::Came(Reading::of(said))
}

/// Whether the choice was held rather than recorded.
fn held(outcome: &Outcome) -> bool {
    match outcome {
        Outcome::Quality(report) => matches!(report.disposition, Disposition::Held),
        _ => false,
    }
}

/// A translation that came to no command, said in the words the other surface gives.
fn came(stage: &mut Stage, said: String) -> Wanted {
    *stage = Stage::Came(Reading::of(vec![said]));
    Wanted::Nothing
}

#[cfg(test)]
pub(crate) mod tests {
    use super::{all, every, Before, Change, Chooser, Chosen, Press, Scope, Stage, Wanted, KEY};
    use crate::acting::offer::OFFERED as KEYED;
    use lemonfiber_api::actions::{OFFERED as WEB, TAKES_AGREEMENT};
    use lemonfiber_core::app::{Command, QualityAction};
    use lemonfiber_core::quality::Preset;

    /// A change naming an action no surface offers, on the path that asks for the
    /// presets — for the arm that reports a translation which came to nothing, and
    /// for the same arm after the question, which every change reaches.
    ///
    /// Nothing on the list is one, and the guard beside the list is what holds that.
    /// These are the arms that would carry a name that stopped being offered.
    pub(crate) static UNCHOOSABLE: Change = Change {
        name: "a change nothing answers",
        about: "for the refusal a translation that reaches no command produces",
        action: "not an action any surface offers",
        asks: "Do the impossible",
        before: Before::Presets,
    };

    /// The same, on the path that asks what it would cost first.
    pub(crate) static UNCOSTABLE: Change = Change {
        name: "a cost nothing answers",
        about: "for the same refusal, reached before the question rather than after it",
        action: "not an action any surface offers",
        asks: "Cost the impossible",
        before: Before::Cost,
    };

    /// The change one action is on, for a test that wants a particular one.
    fn changing(action: &str) -> Option<&'static Change> {
        every().find(|change| change.action == action)
    }

    /// What one change is called on the list, for the screen's own tests, which move
    /// to it by the name an operator would read rather than by number.
    pub(crate) fn listed(action: &str) -> String {
        every()
            .filter(|change| change.action == action)
            .map(|change| change.name)
            .collect()
    }

    /// The presets one change offers, taken from the list the screen really builds
    /// rather than from [`Preset::ALL`] directly — what is asserted is what an
    /// operator would be shown.
    fn presets_of(action: &str) -> Vec<(&'static str, String)> {
        every()
            .filter(|change| change.action == action)
            .filter_map(|change| change.grades(&Chosen::nothing()).ok())
            .flat_map(|(first, rest)| std::iter::once(first).chain(rest))
            .map(|grade| (grade.name, grade.about))
            .collect()
    }

    /// What one change sends, once it has been chosen this and read that.
    fn sends(
        action: &str,
        chosen: Option<&'static str>,
        read: bool,
    ) -> Option<Result<Command, String>> {
        changing(action).map(|change| change.sent(&aiming(chosen), read))
    }

    /// A choice about the whole library, at the bar named or at none.
    fn aiming(chosen: Option<&'static str>) -> Chosen {
        let nothing = Chosen::nothing();
        chosen.map_or(nothing, |grade| Chosen::everywhere().graded(grade))
    }

    /// The whole point of naming the action rather than assembling a command here:
    /// what this screen sends has to be something another surface already offers, or
    /// the requirement it is being built for is defeated by the thing built for it.
    #[test]
    fn every_change_this_screen_makes_is_one_the_other_surfaces_offer() {
        let missing: Vec<&str> = every()
            .map(|change| change.action)
            .filter(|action| !WEB.contains(action))
            .collect();

        assert!(missing.is_empty(), "{missing:?}");
    }

    /// A change is named once, or the second is unreachable on a list that shows both
    /// and nobody would know which they took.
    #[test]
    fn no_two_changes_go_by_the_same_name() {
        for change in every() {
            let same = every().filter(|other| other.name == change.name).count();
            assert_eq!(same, 1, "more than one change is called {}", change.name);
        }
        assert!(every().all(|change| !change.about.is_empty()));
        assert!(every().all(|change| !change.asks.is_empty()));
    }

    /// The key this list opens on is not one the screen already answers, or the
    /// thing it already did stops happening and nothing says so.
    #[test]
    fn the_key_that_opens_them_is_not_one_the_screen_already_answers() {
        for taken in [
            'q',
            'r',
            '?',
            'y',
            crate::acting::question::KEY,
            crate::acting::errand::KEY,
            crate::acting::lasting::KEY,
            crate::acting::surface::KEY,
        ] {
            assert_ne!(KEY, taken, "{taken:?} was already spoken for");
        }
        assert!(KEYED.iter().all(|offer| offer.key != KEY));
    }

    /// Which of the three carries the operator's agreement is asked of the table that
    /// says so, rather than being decided again here — and it goes on once there is
    /// an account to have read and never before it, which is the whole of what the
    /// question in front of these is for.
    #[test]
    fn the_agreement_goes_on_once_the_account_has_been_read_and_never_before() {
        for change in every() {
            let takes = TAKES_AGREEMENT.contains(&change.action);
            let chosen = aiming(matches!(change.before, Before::Presets).then_some("balanced"));

            assert_eq!(
                carries(&change.sent(&chosen, true)),
                Some(takes),
                "{} having read the account",
                change.name
            );
            assert_eq!(
                carries(&change.sent(&chosen, false)),
                Some(false),
                "{} before there is one",
                change.name
            );
        }
        assert_eq!(
            carries(&UNCHOOSABLE.sent(&Chosen::nothing(), true)),
            None,
            "a refusal says nothing about an agreement either way"
        );
    }

    /// Whether a command carries the agreement, over the two shapes that hold one.
    fn carries(sent: &Result<Command, String>) -> Option<bool> {
        match sent {
            Ok(
                Command::Quality(QualityAction::Set { confirm, .. })
                | Command::QualityUpgrade { confirm },
            ) => Some(*confirm),
            Ok(_) => Some(false),
            Err(_) => None,
        }
    }

    /// Choosing is not a rehearsal. Unconfirmed it records the choice, which is why
    /// no run of it is put in front of its own question: the account is the list the
    /// choice is made off instead.
    #[test]
    fn choosing_a_preset_records_it_and_the_agreement_is_for_the_cost_this_host_would_pay() {
        assert_eq!(
            sends("quality-set", Some("maximum"), false),
            Some(Ok(Command::Quality(QualityAction::Set {
                preset: Preset::Maximum,
                media_type: None,
                confirm: false,
            })))
        );
        assert_eq!(
            sends("quality-set", Some("maximum"), true),
            Some(Ok(Command::Quality(QualityAction::Set {
                preset: Preset::Maximum,
                media_type: None,
                confirm: true,
            })))
        );
    }

    /// Re-asserting carries no agreement and has no half that only reports, so
    /// nothing is put in front of its question rather than something invented for
    /// symmetry. The translation refuses an agreement it has nowhere to put, which is
    /// what would catch this screen sending one anyway.
    #[test]
    fn re_asserting_the_choice_carries_no_agreement_and_says_nothing_first() {
        assert_eq!(
            sends("quality-reapply", None, true),
            Some(Ok(Command::Quality(QualityAction::Reapply)))
        );
    }

    /// What an upgrade would cost is asked for before one is started, and asking
    /// costs nothing: the run that says is the run that fetches nothing.
    #[test]
    fn what_an_upgrade_would_cost_is_asked_for_before_anything_is_fetched() {
        assert_eq!(
            sends("quality-upgrade", None, false),
            Some(Ok(Command::QualityUpgrade { confirm: false }))
        );
        assert_eq!(
            sends("quality-upgrade", None, true),
            Some(Ok(Command::QualityUpgrade { confirm: true }))
        );
    }

    /// Every preset the core offers is on the list, in the core's own words, with
    /// what an hour of it costs beside what it means. A screen naming three of four
    /// would be an operator choosing from a shorter menu than a browser's.
    #[test]
    fn every_preset_is_offered_with_what_it_means_and_what_it_costs() {
        let offered = presets_of("quality-set");

        let named: Vec<&str> = offered.iter().map(|(name, _)| *name).collect();
        assert_eq!(
            named,
            Preset::ALL
                .into_iter()
                .map(Preset::label)
                .collect::<Vec<&str>>()
        );
        assert!(offered.iter().all(|(_, about)| about.contains("per hour")));
        assert!(offered.iter().all(|(name, about)| {
            Preset::from_label(name)
                .is_some_and(|preset| about.starts_with(preset.means().trim_end_matches('.')))
        }));
    }

    /// Only the change that records a choice is offered one. The other two take the
    /// choice already on record, and the translation refuses a preset it has nowhere
    /// to put — so a list of presets in front of either of them is a state this
    /// screen cannot reach, and it is that table saying so rather than a rule here.
    #[test]
    fn the_two_that_take_the_choice_on_record_are_offered_no_preset() {
        assert!(presets_of("quality-reapply").is_empty());
        assert!(presets_of("quality-upgrade").is_empty());
    }

    /// What goes in front of each question is written on the change, because it
    /// cannot be derived: the table that says which actions carry an agreement says
    /// nothing about which of them have a half that only reports, and on these three
    /// the two come apart.
    #[test]
    fn each_change_carries_what_goes_in_front_of_its_own_question() {
        let before: Vec<&str> = every()
            .map(|change| match change.before {
                Before::Presets => "the presets",
                Before::Nothing => "nothing",
                Before::Cost => "what it would cost",
            })
            .collect();

        assert_eq!(before, vec!["the presets", "nothing", "what it would cost"]);
    }

    /// A change whose action no surface translates is refused in the sentence the
    /// other surface gives for the same request, rather than in one written here.
    #[test]
    fn a_change_nothing_answers_is_refused_in_the_words_the_other_surface_gives() {
        let said: String = UNCHOOSABLE
            .grades(&Chosen::nothing())
            .err()
            .into_iter()
            .collect();

        assert!(said.contains("not an action any surface offers"), "{said}");
        assert!(UNCOSTABLE.sent(&Chosen::nothing(), false).is_err());
    }

    /// A media no bar can be chosen for says so, in the words the other surface gives
    /// for the same request, rather than opening an empty list of bars.
    ///
    /// Not a state the list can be left in — a media no bar reaches a command for is
    /// never offered — so it is driven here directly. It is the arm that would carry a
    /// media type the translation had stopped accepting, and a screen that fell through
    /// it would put a choice in front of somebody that they could not make.
    #[test]
    fn a_media_no_bar_can_be_chosen_for_says_so_rather_than_opening_an_empty_list() {
        let mut stage = Stage::Idle;

        let wanted = super::scoping(
            &mut stage,
            &UNCHOOSABLE,
            Chooser::over(Scope::everything(), Vec::new()),
            &Press::Accept,
        );

        assert_eq!(wanted, Wanted::Nothing);
        assert!(matches!(stage, Stage::Came(_)));
    }

    /// The list opens on the first change and holds every one of them.
    #[test]
    fn the_list_opens_on_the_first_change_and_holds_them_all() {
        let (first, rest) = all();

        assert_eq!(first.action, "quality-set");
        assert_eq!(rest.len() + 1, every().count());
        assert_eq!(listed("quality-upgrade"), "what is already here, upgraded");
    }
}
