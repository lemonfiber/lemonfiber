use super::{a_word, LENGTH};

#[test]
fn a_word_is_lowercase_letters_of_one_length() {
    let word = a_word();
    assert_eq!(word.len(), LENGTH);
    assert!(word.bytes().all(|letter| letter.is_ascii_lowercase()));
}

#[test]
fn no_two_words_are_the_same() {
    assert_ne!(a_word(), a_word());
}
