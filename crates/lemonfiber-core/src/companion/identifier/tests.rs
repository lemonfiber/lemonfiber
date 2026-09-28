use lemonfiber_fixtures::scratch::Scratch;
use lemonfiber_fixtures::support::FixedRandom;

use super::{kept_or_minted, Unnamed, FILE};

/// Sixteen bytes, written as thirty-two lower-case hex digits and nothing else.
fn bytes() -> FixedRandom {
    FixedRandom(Some((0..16).collect()))
}

/// Minted once from the bytes the machine chose, and the same every time after —
/// whatever else those later runs are handed.
#[test]
fn an_identifier_is_minted_once_and_kept() {
    let at = Scratch::new("companion-identifier-kept");
    let minted = kept_or_minted(&at, &bytes());
    assert_eq!(minted, Ok("000102030405060708090a0b0c0d0e0f".to_owned()));
    let other = FixedRandom(Some(vec![0xff; 16]));
    assert_eq!(kept_or_minted(&at, &other), minted, "not minted again");
}

/// What is written down that is not an identifier is refused, not minted over.
#[test]
fn what_is_not_an_identifier_is_refused_rather_than_replaced() {
    let at = Scratch::new("companion-identifier-unreadable");
    for written in ["not one", "000102030405060708090A0B0C0D0E0F", "0001"] {
        let _ = std::fs::write(at.join(FILE), written);
        assert_eq!(
            kept_or_minted(&at, &bytes()),
            Err(Unnamed::Unreadable),
            "{written}"
        );
    }
}

/// No bytes, or too few, is no identifier — never a shorter or guessable one.
#[test]
fn an_identifier_is_not_made_of_less_than_the_machine_owes_it() {
    let at = Scratch::new("companion-identifier-unminted");
    assert_eq!(
        kept_or_minted(&at, &FixedRandom(None)),
        Err(Unnamed::Unminted)
    );
    assert_eq!(
        kept_or_minted(&at, &FixedRandom(Some(vec![1; 8]))),
        Err(Unnamed::Unminted)
    );
}

/// An identifier that cannot be written down is not handed out, because a phone would
/// hold a name this stack had forgotten by the next run.
#[test]
fn an_identifier_that_cannot_be_written_down_is_not_handed_out() {
    let at = Scratch::new("companion-identifier-unwritable");
    let blocked = at.join("blocked");
    let _ = std::fs::write(&blocked, "a file where the directory would go");
    assert!(matches!(
        kept_or_minted(&blocked, &bytes()),
        Err(Unnamed::Unwritten(_))
    ));
}
