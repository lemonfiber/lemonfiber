use super::{spoken, Obstacle, Words, FAULT, FAULT_REMEDY};
use crate::codes::{play, serve, wish, Household};
use crate::{Code, Problem, Remedy, State, Stepped};

/// A fault of the stack reads as not working right now, with whoever runs the house told.
#[test]
fn a_fault_of_the_stack_says_it_is_not_working_and_the_house_has_been_told() {
    assert!(FAULT.contains("isn't working right now"));
    assert!(FAULT.contains("whoever runs the house has been told"));
    let words = Words::of(serve::UNRENDERABLE);
    assert_eq!(
        (words.said, words.remedy, words.fault),
        (FAULT, FAULT_REMEDY, true)
    );
}

/// A code with words of its own is told in them, and is not a fault.
#[test]
fn a_code_with_words_of_its_own_is_told_in_them() {
    let declared = wish::ALREADY_HERE.declaration();
    let Some(Household::Said { said, remedy }) = declared.map(crate::codes::Declared::household)
    else {
        unreachable!("WISH-5 carries no household words of its own");
    };
    let words = Words::of(wish::ALREADY_HERE);
    assert_eq!(
        (words.said, words.remedy, words.fault),
        (said, remedy, false)
    );
}

/// A code nothing declares is said as a fault, never as nothing.
#[test]
fn a_code_nothing_declares_is_said_as_a_fault() {
    assert_eq!(Words::of(Code::new("NOWHERE-1")).said, FAULT);
}

/// A member's problem carries the code and the household's words, and none of the
/// operator's: no detail, no cause and no steps.
#[test]
fn a_members_problem_keeps_the_code_and_drops_everything_written_for_the_operator() {
    let operator = Problem::new(
        play::SERVER_SILENT,
        "The media server did not answer",
        "Jellyfin at 172.18.0.4:8096 refused the connection.",
        Remedy::new("Check the media server with lemonfiber status"),
    )
    .or_try(Remedy::new("Restart Jellyfin"))
    .in_state(State::Guided)
    .with_detail("connection refused")
    .caused_by(Problem::new(
        serve::UNRENDERABLE,
        "inner",
        "inner",
        Remedy::new("inner"),
    ))
    .with_steps(vec![Stepped {
        recipe: "seed".to_owned(),
        step: "one".to_owned(),
        came: crate::Came::Answered,
        landed: false,
    }]);
    let member = spoken(&operator);
    let words = Words::of(play::SERVER_SILENT);
    assert_eq!(member.code, operator.code);
    assert_eq!(member.severity, operator.severity);
    assert_eq!(member.state, operator.state);
    assert_eq!(member.summary, words.said);
    assert_eq!(member.meaning, words.said);
    assert_eq!(member.remedies, vec![Remedy::new(words.remedy)]);
    assert_eq!(member.detail, None);
    assert_eq!(member.cause, None);
    assert!(member.steps.is_empty());
}

/// An obstacle is told to a member in its code's household words, and to everybody else
/// in its own with the remedy its code is declared with.
#[test]
fn an_obstacle_is_worded_by_its_code_for_whoever_reads_it() {
    let obstacle = Obstacle::new(play::SERVER_SILENT, "The media server did not answer");
    assert_eq!(obstacle.said, "The media server did not answer");
    assert_eq!(
        Some(obstacle.remedy.as_str()),
        play::SERVER_SILENT
            .declaration()
            .map(crate::codes::Declared::remedy)
    );
    let words = Words::of(play::SERVER_SILENT);
    assert_eq!(
        obstacle.spoken(),
        Obstacle {
            code: play::SERVER_SILENT,
            said: words.said.to_owned(),
            remedy: words.remedy.to_owned(),
        }
    );
}
