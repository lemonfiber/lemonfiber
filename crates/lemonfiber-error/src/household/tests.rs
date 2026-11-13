use super::{spoken, unhoused, Obstacle, Words, FAULT, FAULT_REMEDY};
use crate::codes::{every_declared, play, serve, wish, Household};
use crate::{Code, Problem, Remedy, State, Stepped};

/// Every code there is has words a member may be told, naming no service, size, address,
/// version or state of the stack, and so does what a fault of the stack is said as.
///
/// Every code rather than only those some route is known to answer today: each is
/// declared with a household entry, so none can reach a member's session without one.
#[test]
fn every_code_a_member_could_be_answered_with_is_in_the_households_words() {
    for declared in every_declared() {
        let words = Words::of(declared.code());
        for said in [words.said, words.remedy] {
            assert_eq!(
                unhoused(said),
                None,
                "{} tells a member something not in the household's words: {said}",
                declared.code()
            );
            assert!(
                !said.trim().is_empty(),
                "{} tells a member nothing",
                declared.code()
            );
        }
    }
}

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
        panic!("WISH-5 carries no household words of its own");
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

/// What a member may not be shown is found, whichever kind of thing it is.
#[test]
fn what_a_member_may_not_be_shown_is_found() {
    for (text, found) in [
        ("Jellyfin did not answer", "jellyfin"),
        ("The media server did not answer", "media server"),
        ("It needs 4 GB more", "4"),
        ("It needs 12GB more", "12gb"),
        ("Fetching at 3 MB/s", "3"),
        ("Open http://house.lan", "http"),
        ("Reached at 192.168.1.20", "192.168.1.20"),
        ("Now on v1.4.2", "v1.4.2"),
        ("The stack is stopped", "stack"),
        ("It is still downloading", "downloading"),
        ("No indexer answered", "indexer"),
    ] {
        assert_eq!(unhoused(text).as_deref(), Some(found), "{text}");
    }
}

/// The household's own words pass, the house included, and a word is matched whole.
#[test]
fn the_households_own_words_pass() {
    for text in [
        "This isn't working right now, and whoever runs the house has been told.",
        "It is already in the house. Play it from your shelf.",
        "Ask again once your allowance frees up.",
        "Your library isn't answering right now.",
        "The idea is sound, and serving dinner can wait.",
        "Episode 3 of season 2.",
    ] {
        assert_eq!(unhoused(text), None, "{text}");
    }
}
