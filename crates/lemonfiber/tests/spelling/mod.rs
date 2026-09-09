//! Numbers as this workspace writes them, and as it reads them back.
//!
//! Several pages here state a count in words and a guard checks the count is the one
//! the code actually carries. Both halves needed a way to turn a number into the word
//! for it, and both grew a hand-kept table of the words they had needed so far.
//!
//! A table like that is a ratchet: it has to grow when a feature does, it lives in a
//! different file from the thing it counts, and — the part that cost real time — it
//! fails *quietly*. One of them fell off its end and wrote the count in digits, so a
//! page saying `forty-five` was told it did not say `45`. The other matched the longest
//! word it knew, so a surface of `twenty-seven` read as a surface of **seven**, and the
//! failure named the file that was right.
//!
//! So the words are built rather than listed. English writes everything below a hundred
//! from twenty words and nine, and building them means there is no list to outgrow and
//! nothing to keep in step.

// Every test binary that declares this module compiles all of it, and each of them
// wants one half: the pages want to write a number, the readers want to read one back.
#![allow(dead_code)]

/// The words English does not build from parts.
const ALONE: [&str; 20] = [
    "zero",
    "one",
    "two",
    "three",
    "four",
    "five",
    "six",
    "seven",
    "eight",
    "nine",
    "ten",
    "eleven",
    "twelve",
    "thirteen",
    "fourteen",
    "fifteen",
    "sixteen",
    "seventeen",
    "eighteen",
    "nineteen",
];

/// The tens everything from twenty upwards is built from.
///
/// The first two are empty because nothing is built on them: nought to nineteen are
/// their own words above, and an index into this is only ever reached for twenty and
/// beyond.
const TENS: [&str; 10] = [
    "", "", "twenty", "thirty", "forty", "fifty", "sixty", "seventy", "eighty", "ninety",
];

/// `number` in words, the way a page here writes one.
///
/// `None` past ninety-nine rather than the digits, which is the whole point: a page
/// that has grown past what this can say should fail saying so, not fail saying that
/// the page does not contain a numeral it was never going to contain.
pub(crate) fn spelled(number: usize) -> Option<String> {
    if let Some(word) = ALONE.get(number) {
        return Some((*word).to_owned());
    }
    let ten = TENS.get(number / 10)?;
    let unit = ALONE.get(number % 10)?;
    if number.is_multiple_of(10) {
        return Some((*ten).to_owned());
    }
    Some(format!("{ten}-{unit}"))
}

/// Every number this can spell, as the words a reader would meet in prose.
///
/// Hyphens are spaces here because the readers below sanitise punctuation to spaces
/// before they look — `twenty-seven` in a doc comment arrives as `twenty seven`, and a
/// word with a hyphen in it would be one no sanitised prose could ever hold.
fn every_word() -> Vec<(String, usize)> {
    (0..100)
        .filter_map(|number| spelled(number).map(|word| (word.replace('-', " "), number)))
        .collect()
}

/// The prose of a module's own `//!` block, with punctuation flattened to spaces.
///
/// Padded at both ends so the first and last word match the same way the rest do.
fn opening(text: &str) -> String {
    let said: String = text
        .lines()
        .take_while(|line| line.starts_with("//!") || line.is_empty())
        .collect::<Vec<&str>>()
        .join(" ")
        .to_lowercase();
    let mut doc = String::from(" ");
    doc.extend(said.chars().map(|letter| {
        if letter.is_ascii_alphabetic() {
            letter
        } else {
            ' '
        }
    }));
    doc.push(' ');
    doc
}

/// The number a module's opening states about `noun`, where it states one.
///
/// The longest match rather than the first, because a compound number contains a
/// shorter one: `twenty one reads` holds `one reads`, and taking the first would read
/// a surface of twenty-one as a surface of one. Every number below a hundred is a
/// candidate, so there is no compound this knows half of.
pub(crate) fn stated_about(text: &str, noun: &str) -> Option<usize> {
    let doc = opening(text);
    every_word()
        .into_iter()
        .filter(|(word, _)| {
            doc.contains(&format!(" {word} {noun} ")) || doc.contains(&format!(" {word} {noun}s "))
        })
        .max_by_key(|(word, _)| word.len())
        .map(|(_, count)| count)
}

#[cfg(test)]
mod tests {
    use super::{spelled, stated_about};

    #[test]
    fn it_writes_the_shapes_english_writes() {
        for (number, word) in [
            (0, "zero"),
            (7, "seven"),
            (13, "thirteen"),
            (20, "twenty"),
            (27, "twenty-seven"),
            (44, "forty-four"),
            (90, "ninety"),
            (99, "ninety-nine"),
        ] {
            assert_eq!(
                spelled(number).as_deref(),
                Some(word),
                "{number} is written {word}"
            );
        }
    }

    /// Past what English writes in one word here, it says so rather than guessing.
    ///
    /// The table this replaced fell off its end into digits, so a page that had grown
    /// was told it did not contain a numeral nobody had written.
    #[test]
    fn a_number_it_cannot_say_is_an_absence_rather_than_a_numeral() {
        assert_eq!(spelled(100), None, "it says nothing rather than `100`");
        assert_eq!(spelled(1_000), None);
    }

    /// The failure that cost a CI round: a compound read as the shorter number in it.
    #[test]
    fn a_compound_is_read_whole_rather_than_as_the_word_inside_it() {
        let doc = "//! The twenty-seven reads: one endpoint per question.\n\nuse std::fs;";

        assert_eq!(
            stated_about(doc, "read"),
            Some(27),
            "not seven, which is the word sitting inside it"
        );
    }

    #[test]
    fn a_module_stating_nothing_about_the_noun_states_nothing() {
        let doc = "//! The four ways a reading is cut.\n\nuse std::fs;";

        assert_eq!(
            stated_about(doc, "read"),
            None,
            "cut four ways is not a count of reads"
        );
    }

    /// Only the opening block is read, so prose further down is not a claim.
    #[test]
    fn a_number_below_the_opening_is_not_the_modules_own_count() {
        let doc = "//! What this serves.\n\nuse std::fs;\n// nine reads elsewhere\n";

        assert_eq!(stated_about(doc, "read"), None);
    }
}
