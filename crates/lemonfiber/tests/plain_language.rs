//! The words this product ships, made executable.
//!
//! One rule per test, each over the same corpus: every string literal in the
//! production half of every source file. That corpus is coarser than "what an
//! operator reads" — it holds a few labels and format fragments too — and it is
//! the only place a message can be caught before it is written rather than after
//! it has been read.
//!
//! Rendered output is deliberately not the corpus. A rendered screen carries
//! another service's name, a release title and whatever a daemon said back, so a
//! guard reading it would be scanning words this product did not write.
//!
//! The other way round holds too, and less tidily: a handful of literals here are
//! words this product *reads* rather than writes — the vocabulary a provider's
//! complaint is matched against. They are held to the same rules as the messages,
//! which costs nothing while no service says anything these rules refuse.

mod source_tree;

use source_tree::{production, sources};

/// The text inside double quotes on one line, which is where an operator's words are.
///
/// Prose only: a word this product must not write is very often a perfectly good
/// identifier — `nntp` names a module, a type and a Compose profile here — so a
/// guard reading whole lines would report the code rather than the words.
fn quoted(line: &str) -> Vec<&str> {
    let mut found = Vec::new();
    let mut rest = line;
    while let Some(open) = rest.find('"') {
        let Some(after) = rest.get(open + 1..) else {
            break;
        };
        let Some(close) = after.find('"') else { break };
        let Some(said) = after.get(..close) else {
            break;
        };
        found.push(said);
        let Some(next) = after.get(close + 1..) else {
            break;
        };
        rest = next;
    }
    found
}

/// Every piece of prose this product ships, with where it was found.
fn shipped_prose() -> Vec<(String, usize, String)> {
    let mut prose = Vec::new();
    for (path, text) in sources() {
        let where_it_lives = path.to_string_lossy().replace('\\', "/");
        // The glossary is where the other names are written down, so it is the one
        // file allowed to write them.
        if !where_it_lives.contains("/src/") || where_it_lives.ends_with("glossary.rs") {
            continue;
        }
        for (number, said) in written_in(production(&text)) {
            prose.push((where_it_lives.clone(), number, said));
        }
    }
    // Every rule below reports what it found, so a reader that stopped finding
    // anything would pass all four at once and look like a clean codebase.
    assert!(
        prose.len() > 2_000,
        "the sweep found {} pieces of prose, which means it is reading the wrong half \
         of the tree",
        prose.len()
    );
    prose
}

/// The literals in one file, each with the line it starts on.
///
/// A message longer than a line is written here as a literal continued with a
/// trailing `\`, and it is read back as the one message it is: the guards below ask
/// about sentences, and half a sentence answers a different question. A hundred
/// messages are written that way, among them the longest explanations this product
/// gives, and every one of them was invisible until it was read like this.
///
/// Only that form is followed. A quote left open by anything else — a character
/// literal holding one, a raw string — would swallow the code beneath it and hand
/// every guard a file of identifiers to read as prose.
fn written_in(text: &str) -> Vec<(usize, String)> {
    let mut found = Vec::new();
    let mut carried: Option<(usize, String)> = None;

    for (at, line) in text.lines().enumerate() {
        if let Some((number, mut said)) = carried.take() {
            if carry_on(line, &mut said) {
                carried = Some((number, said));
            } else {
                found.push((number, said));
            }
            continue;
        }
        let trimmed = line.trim_start();
        if trimmed.starts_with("//") || trimmed.starts_with("/*") {
            continue;
        }
        for said in quoted(line) {
            found.push((at + 1, said.to_owned()));
        }
        if let Some(opened) = left_open(line) {
            carried = Some((at + 1, opened.to_owned()));
        }
    }
    if let Some((number, said)) = carried {
        found.push((number, said));
    }
    found
}

/// Adds this line to a literal already open, and says whether it is still open.
///
/// The leading whitespace goes, because a continuation eats it: what the operator
/// reads is the two halves joined by whatever space the first one ended with.
fn carry_on(line: &str, said: &mut String) -> bool {
    let rest = line.trim_start();
    if let Some(closes) = unescaped_quotes(rest).first() {
        said.push_str(rest.get(..*closes).unwrap_or_default());
        return false;
    }
    said.push_str(joined(rest).unwrap_or(rest));
    joined(rest).is_some()
}

/// What a line leaves inside an open literal, where it leaves one.
fn left_open(line: &str) -> Option<&str> {
    let quotes = unescaped_quotes(line);
    if quotes.len().is_multiple_of(2) {
        return None;
    }
    let last = quotes.last()?;
    joined(line.get(last + 1..)?)
}

/// This line without the mark that runs it on to the next, where it carries one.
fn joined(line: &str) -> Option<&str> {
    line.trim_end().strip_suffix('\\')
}

/// Where the quotes on this line are, leaving the escaped ones out.
fn unescaped_quotes(line: &str) -> Vec<usize> {
    let mut found = Vec::new();
    let mut escaped = false;
    for (at, letter) in line.char_indices() {
        match letter {
            _ if escaped => escaped = false,
            '\\' => escaped = true,
            '"' => found.push(at),
            _ => {}
        }
    }
    found
}

/// One concept, one word — the other names are recorded, never adopted.
///
/// Each service in this stack names the same thing differently, and the glossary
/// records those names so an operator moving between their screens can follow one
/// concept across them. That record is not a licence to use either word. A product
/// that says `indexer` on one screen and `search provider` on the next has told the
/// reader there are two things to understand, which is the confusion the record
/// exists to end — and it costs most exactly where it is least noticed, because
/// whoever wrote the second screen knew they meant the same thing.
#[test]
fn no_other_services_word_is_written_as_if_it_were_ours() {
    let mut borrowed_words: Vec<String> = Vec::new();
    for (path, number, said) in shipped_prose() {
        for (theirs, ours) in lemonfiber_core::glossary::borrowed(&said) {
            borrowed_words.push(format!("{path}:{number}: `{theirs}` — say `{ours}`"));
        }
    }
    assert!(
        borrowed_words.is_empty(),
        "another service's word, written as though it were this product's own: \
         {borrowed_words:?}"
    );
}

/// Short capitals an operator needs no help with, and why each is allowed.
///
/// Declaring one ordinary is a judgement. Writing it here is what makes the
/// judgement reviewable rather than invisible, and there are four kinds.
///
/// **Met everywhere**, and not this ecosystem's own: somebody running a media stack
/// has already met a URL.
///
/// **The operating system's**, each of which already appears in a sentence stating
/// its consequence — "FAT filesystems cannot create hardlinks" leaves a separate
/// entry nothing to add.
///
/// **Units, dates and formats**, which are read rather than understood.
///
/// **Ordinary English in capitals**, which is emphasis and not an abbreviation at
/// all: `NOT` in "the client's traffic is NOT going through the tunnel" is the same
/// word it would be in lower case.
///
/// The three the hours brought are one of each of the first three kinds. `UTC` is met
/// everywhere and every sentence it stands in says what it costs — that clocks naming
/// no zone keep these hours by it, and quiet hours would begin at the wrong time of
/// night. `TZ` is the operating system's own name for the setting, shown where it is
/// typed rather than described. `HH` is a format beside the `MM` already here.
const ORDINARY: &[&str] = &[
    "API", "URL", "TLS", "JSON", "DNS", "IP", "UI", "HTTP", "OSI", "FAT", "SMB", "CIFS", "NFS",
    "WSL2", "UID", "GID", "NAT", "PMP", "P2P", "GB", "MB", "CD", "TV", "MP3", "AAC", "FLAC",
    "ALAC", "YYYY", "MM", "DD", "NOT", "CPU", "UTC", "TZ", "HH",
];

/// Every acronym an operator is shown is explained, or declared ordinary.
///
/// A domain term used without an explanation is a defect, and the hard part is that
/// most jargon cannot be told from ordinary writing by a machine. An acronym can be,
/// and it is jargon at its sharpest: somebody who does not know `NZB` cannot infer it
/// from the letters, cannot look it up under a word they never saw spelled out, and
/// has nothing to go on but the sentence around it.
///
/// So this refuses the whole class rather than a list of known offenders. A new
/// acronym cannot reach an operator without somebody deciding which it is —
/// explained in the glossary, or written into [`ORDINARY`] with a reason. Neither
/// costs much. Not deciding is what costs.
///
/// **Only inside sentences.** Three things wear capitals without being acronyms —
/// an environment variable (`LEMONFIBER_USENET`), the placeholder in a help line
/// (`SERVICE`), and a name with a capital run inside it (`SABnzbd`) — and none of
/// them is prose. Rather than name them, this looks only at literals shaped like
/// something written to be read: several words, at least one of them an ordinary
/// lower-case one. The cost is real and worth stating: an acronym shown entirely on
/// its own, as a bare label on a screen, is not checked here.
#[test]
fn every_acronym_an_operator_reads_is_explained_or_declared_ordinary() {
    let mut unexplained: Vec<String> = Vec::new();
    for (path, number, said) in shipped_prose() {
        for short in unexplained_acronyms(&said) {
            unexplained.push(format!("{path}:{number}: `{short}`"));
        }
    }
    assert!(
        unexplained.is_empty(),
        "an operator is shown these and given nothing to make sense of them — explain \
         each in the glossary, or add it to ORDINARY with a reason: {unexplained:?}"
    );
}

/// The acronyms in one literal that are neither explained nor declared ordinary.
///
/// Its own function rather than three loops inside the test, so that what the guard
/// asks of one piece of text reads as one thing and the test reads as the sweep.
fn unexplained_acronyms(said: &str) -> Vec<String> {
    let prose = outside_escapes(&outside_braces(said));
    if !reads_as_a_sentence(&prose) {
        return Vec::new();
    }
    outside_shouting(&prose)
        .split_whitespace()
        // An error code is looked up rather than read, and a path, a variable name or
        // a setting given a value is not addressed to anybody.
        .filter(|word| !is_a_code(word) && !word.contains('_') && !word.contains('/'))
        .filter(|word| !word.contains('='))
        .flat_map(capitals)
        .filter(|short| !ORDINARY.contains(&short.as_str()))
        .filter(|short| lemonfiber_core::glossary::explain(short).is_none())
        .collect()
}

/// Capitalised words in a row that make a stretch of them shouting rather than
/// abbreviations.
const SHOUTING: usize = 3;

/// The text with every stretch of shouting removed.
///
/// Three capitalised words in a row are a sentence said loudly, not three
/// abbreviations — the support bundle shouts nine of them over a page holding a
/// credential, and every one is an ordinary English word. One or two in a row are
/// left to be checked, which is where the acronyms are.
fn outside_shouting(prose: &str) -> String {
    let mut kept: Vec<&str> = Vec::new();
    let mut run: Vec<&str> = Vec::new();
    for word in prose.split_whitespace() {
        if shouted(word) {
            run.push(word);
            continue;
        }
        if run.len() < SHOUTING {
            kept.append(&mut run);
        }
        run.clear();
        kept.push(word);
    }
    if run.len() < SHOUTING {
        kept.append(&mut run);
    }
    kept.join(" ")
}

/// Whether this word is written in capitals throughout, having letters at all.
fn shouted(word: &str) -> bool {
    let mut letters = word
        .chars()
        .filter(|letter| letter.is_alphabetic())
        .peekable();
    letters.peek().is_some() && letters.all(char::is_uppercase)
}

/// The text with the escapes that break a line read as the breaks they are.
///
/// `\n` between two words is source text, not letters, and leaving it there makes
/// `\nSHOWN` a word carrying a lower-case letter — enough to hide it from any rule
/// about how a word is capitalised, and enough to weld two words into one.
fn outside_escapes(said: &str) -> String {
    said.replace("\\n", " ").replace("\\t", " ")
}

/// The text with every `{…}` removed, a placeholder being code inside a string.
fn outside_braces(said: &str) -> String {
    let mut prose = String::new();
    let mut depth = 0_usize;
    for character in said.chars() {
        match character {
            '{' => depth += 1,
            '}' => depth = depth.saturating_sub(1),
            _ if depth == 0 => prose.push(character),
            _ => {}
        }
    }
    prose
}

/// Whether this was written to be read: several words, one of them an ordinary one.
fn reads_as_a_sentence(prose: &str) -> bool {
    let words: Vec<&str> = prose.split_whitespace().collect();
    words.len() >= 4
        && words
            .iter()
            .any(|word| word.len() > 2 && word.chars().all(|letter| letter.is_ascii_lowercase()))
}

/// Whether this is an error code — a run of capitals, a dash, and a number.
fn is_a_code(word: &str) -> bool {
    let trimmed = word.trim_matches(|letter: char| !letter.is_ascii_alphanumeric());
    let Some((letters, number)) = trimmed.split_once('-') else {
        return false;
    };
    !letters.is_empty()
        && letters.chars().all(|letter| letter.is_ascii_uppercase())
        && !number.is_empty()
        && number.chars().all(|letter| letter.is_ascii_digit())
}

/// Every run of two or more capitals in a word, with names left out.
///
/// A run that runs straight into a lower-case letter is part of a word rather than
/// an abbreviation of one: `SABnzbd` and `QBittorrent` are names, not acronyms.
fn capitals(word: &str) -> Vec<String> {
    let mut found = Vec::new();
    let mut run = String::new();
    for letter in word.chars() {
        if letter.is_ascii_uppercase() || (!run.is_empty() && letter.is_ascii_digit()) {
            run.push(letter);
            continue;
        }
        if run.len() > 1 && !letter.is_ascii_lowercase() {
            found.push(run.clone());
        }
        run.clear();
    }
    if run.len() > 1 {
        found.push(run);
    }
    found
}

/// Turns of phrase that do not survive being read by somebody who learned English
/// second, or translated.
///
/// Not an exhaustive list of idiom — no such list exists — but the ones that reach
/// for a sport, a war or a piece of folk wisdom, which are the ones that fail hardest
/// because they are opaque rather than merely unusual. Somebody who does not know
/// the reference cannot infer it from the words.
const IDIOMS: &[&str] = &[
    "out of the box",
    "under the hood",
    "at the end of the day",
    "on the fly",
    "rule of thumb",
    "silver bullet",
    "cut corners",
    "off the shelf",
    "ballpark",
    "touch base",
    "low-hanging fruit",
    "sanity check",
    "bite the bullet",
    "in the weeds",
    "piece of cake",
    "elephant in the room",
    "home run",
    "curveball",
    "slam dunk",
    "level playing field",
    "spanner in the works",
    "boil the ocean",
    "bells and whistles",
    "chicken and egg",
    "smoke and mirrors",
    "tip of the iceberg",
    "red herring",
    "the last straw",
    "first base",
    "back to square one",
];

/// Nothing an operator reads leans on an idiom.
///
/// This product is read by people who did not learn English first, and it will be
/// translated. An idiom is the one kind of plain-looking sentence that cannot be
/// worked out from its words: somebody who does not know that a ballpark is a place
/// where baseball is played has no way to reach "approximate" from it, and no
/// dictionary will take them there either. The plain word costs nothing and lands
/// everywhere.
#[test]
fn nothing_an_operator_reads_leans_on_an_idiom() {
    let mut figures: Vec<String> = Vec::new();
    for (path, number, said) in shipped_prose() {
        let plainly = said.to_lowercase();
        for idiom in IDIOMS {
            if plainly.contains(idiom) {
                figures.push(format!("{path}:{number}: `{idiom}`"));
            }
        }
    }
    assert!(
        figures.is_empty(),
        "these do not survive translation, and say nothing a plain word would not: \
         {figures:?}"
    );
}

// ── Never the operator's fault ────────────────────────────────

/// Words that name something done wrong.
///
/// A fault, not a person: this product says "the key is wrong" about a key and is
/// right to. These are read together with [`READER`] rather than on their own,
/// because what turns a fault into blame is who is standing next to it.
const FAULT: &[&str] = &[
    "wrong",
    "wrongly",
    "invalid",
    "incorrect",
    "incorrectly",
    "broke",
    "broken",
    "failed",
    "forgot",
    "forgotten",
    "mistake",
    "mistakes",
    "mistyped",
    "misspelled",
    "misconfigured",
    "misused",
    "error",
    "errors",
    "fault",
    "blame",
    "careless",
    "carelessly",
    "neglected",
    "botched",
    "messed",
    "sloppy",
    "supposed",
];

/// The person reading, in the words a message would name them by.
const READER: &[&str] = &[
    "you", "your", "yours", "yourself", "you're", "you've", "you'd", "you'll",
];

/// How near a fault has to stand to the person before it is being said of them.
const NEARBY: usize = 4;

/// Blame that names nobody, and means the operator anyway.
///
/// Each needs its exact sequence rather than two words standing near each other:
/// "input" and "error" are ordinary on their own, and it is the pairing that
/// decides somebody typed the wrong thing.
const BLAMING: &[&str] = &[
    "user error",
    "operator error",
    "human error",
    "pilot error",
    "invalid input",
    "bad input",
    "you should have",
    "you should not have",
    "you shouldn't have",
    "you ought to have",
    "this is on you",
];

/// Nothing an operator reads puts the failure on them.
///
/// A message that blames costs twice: the operator stops reading for the remedy
/// and starts defending themselves, and the product has told them the wrong thing
/// anyway — a setting that was wrong to begin with, a service that changed its
/// answer, and a person who typed what was asked all arrive here as the same
/// failure, and only one of them is anybody's doing.
///
/// What this refuses is a fault named beside the person reading, not the second
/// person itself. Saying what the operator did is how this product defers to
/// them — "You have changed this since lemonfiber wrote it, so it is left alone"
/// is the whole point of that message. Blame is what happens when a fault word
/// joins it.
#[test]
fn nothing_an_operator_reads_puts_the_failure_on_them() {
    let mut blamed: Vec<String> = Vec::new();
    for (path, number, said) in shipped_prose() {
        for blame in blaming(&said) {
            blamed.push(format!("{path}:{number}: `{blame}`"));
        }
    }
    assert!(
        blamed.is_empty(),
        "these put the failure on the person reading — say what is wrong, not who \
         got it wrong: {blamed:?}"
    );
}

/// A fault is blame only where the person is the one being faulted.
///
/// The sweep passing over the corpus proves only that nothing in it matched. This
/// is the other half: the four sentences below are shipped today and must stay
/// legal, and the five under them must not, so a list quietly widened until it
/// catches everything — or narrowed until it catches nothing — is a failure here
/// rather than a guard that looks green.
#[test]
fn a_fault_is_blame_only_where_the_person_is_the_one_being_faulted() {
    for plainly in [
        // The key is at fault, and no one is.
        "The indexer answered and rejected the API key configured for it. The key is \
         wrong, expired, or for a different indexer",
        // The services went wrong, and their output is where to look.
        "Whatever went wrong is usually in their own output, which is below.",
        // What the operator did, said in order to leave it alone.
        "You have changed this since lemonfiber wrote it, so it is left alone",
        "conflict — both you and the default changed it",
    ] {
        assert!(blaming(plainly).is_empty(), "{plainly}");
    }

    for blamed in [
        "You forgot to set a password",
        "The key you entered is wrong",
        "Your settings are invalid",
        "you should have run setup first",
        "user error",
    ] {
        assert!(!blaming(blamed).is_empty(), "{blamed}");
    }
}

/// The blaming constructions in one literal.
fn blaming(said: &str) -> Vec<String> {
    let plainly = outside_escapes(&outside_braces(said)).to_lowercase();
    let mut found: Vec<String> = BLAMING
        .iter()
        .filter(|blame| plainly.contains(*blame))
        .map(|blame| (*blame).to_owned())
        .collect();
    found.extend(faults_beside_the_reader(&plainly));
    found
}

/// Every fault named within [`NEARBY`] words of the person reading.
fn faults_beside_the_reader(plainly: &str) -> Vec<String> {
    let words: Vec<&str> = plainly
        .split_whitespace()
        .map(|word| word.trim_matches(|letter: char| !letter.is_alphanumeric() && letter != '\''))
        .collect();

    let mut found = Vec::new();
    for (at, word) in words.iter().enumerate() {
        if !FAULT.contains(word) {
            continue;
        }
        let from = at.saturating_sub(NEARBY);
        let to = words.len().min(at + NEARBY + 1);
        let Some(nearby) = words.get(from..to) else {
            continue;
        };
        if let Some(reader) = nearby.iter().find(|near| READER.contains(near)) {
            found.push(format!("{reader} … {word}"));
        }
    }
    found
}
