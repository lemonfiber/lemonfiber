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
