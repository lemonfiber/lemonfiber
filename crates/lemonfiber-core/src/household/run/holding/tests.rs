use super::{NOT_WRITTEN_DOWN, STILL_ASKING, STILL_HELD};

/// Each sentence says which way round it went wrong, and what to do next.
///
/// The two directions are different things for an operator to act on — a household
/// still able to ask for what cannot be fetched, and a household unable to ask at
/// all — and a message that could be either leaves them reading permissions to find
/// out which.
#[test]
fn each_sentence_says_which_way_round_it_went_wrong_and_what_to_do() {
    assert!(STILL_ASKING.contains("would not stop the household asking"));
    assert!(STILL_ASKING.contains("read the household again"));
    assert!(STILL_HELD.contains("give the household back"));
    assert!(STILL_HELD.contains("read the household again"));
    assert!(NOT_WRITTEN_DOWN.contains("older answer"));
    assert!(NOT_WRITTEN_DOWN.contains("check in the request service"));
}
