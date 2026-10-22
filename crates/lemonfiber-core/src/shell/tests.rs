use super::{posix, powershell};

/// A quote inside the word closes nothing, so what follows it stays part of the word.
#[test]
fn a_quote_inside_the_word_stays_in_the_word() {
    assert_eq!(posix("it's; rm -rf ~"), r"'it'\''s; rm -rf ~'");
    assert_eq!(powershell("it's; Remove-Item ~"), "'it''s; Remove-Item ~'");
}

/// A word with a space or a command separator in it is one word.
#[test]
fn a_space_or_a_separator_is_one_word() {
    assert_eq!(posix("/srv/my media/$(id)"), "'/srv/my media/$(id)'");
    assert_eq!(powershell("C:\\my media\\$env:x"), "'C:\\my media\\$env:x'");
}
