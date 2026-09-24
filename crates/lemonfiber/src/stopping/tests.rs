use super::{answered, asking, Asking, Choice, ASK_TO_WAIT};

/// An operator who typed `--wait` has decided; asking again would be asking a
/// question they have already answered.
#[test]
fn waiting_was_asked_for_so_nobody_is_asked() {
    assert_eq!(
        asking(true, false, true),
        Asking::Settled(Choice::Wait),
        "--wait decides it"
    );
    assert_eq!(
        asking(true, true, true),
        Asking::Settled(Choice::Wait),
        "--wait beats --yes, being the more specific of the two"
    );
}

/// A script has said everything it is going to say, and a prompt it cannot see
/// would hang it.
#[test]
fn nobody_who_cannot_answer_is_asked() {
    assert_eq!(asking(false, true, true), Asking::Settled(Choice::Stop));
    assert_eq!(asking(false, false, false), Asking::Settled(Choice::Stop));
    assert_eq!(asking(false, true, false), Asking::Settled(Choice::Stop));
}

#[test]
fn an_operator_at_a_terminal_is_asked() {
    assert_eq!(asking(false, false, true), Asking::Ask);
}

/// The default lands on what was typed. A prompt that waits unless told not to
/// would turn a stray keypress into a stack still up an hour later.
#[test]
fn only_an_explicit_yes_waits() {
    for said in ["y", "Y", "yes", "YES", " yes "] {
        assert_eq!(answered(said), Choice::Wait, "{said:?}");
    }
    for said in ["", "n", "no", "\n", "later", "yep", "sure"] {
        assert_eq!(answered(said), Choice::Stop, "{said:?}");
    }
}

/// The question says which way the default falls, because the answer that
/// interrupts a download should never be the one given by accident.
#[test]
fn the_question_shows_which_way_saying_nothing_goes() {
    assert!(ASK_TO_WAIT.contains("[y/N]"), "{ASK_TO_WAIT}");
}
