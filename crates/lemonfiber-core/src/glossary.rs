//! The words this product uses, and what they mean to somebody meeting them.
//!
//! This ecosystem's vocabulary is a wall. *Indexer, NZB, hardlink, retention,
//! killswitch, ratio* — all load-bearing, none guessable, and the documentation
//! defines each by reference to the others, so understanding requires already
//! understanding. Somebody who cannot infer has to leave and look it up, and some
//! of them do not come back.
//!
//! So the words are explained where they are used. Three rules shape what is
//! written here, and all three are easy to get wrong:
//!
//! **Say what it is for, not what it is.** "A searchable index of Usenet articles"
//! is a definition, and answers a question nobody asked. "Search engines that find
//! what you are looking for — you need at least one, and most cost a small yearly
//! fee" answers *why should I care*, which is the actual question.
//!
//! **Accurate and longer beats simple and wrong.** A simplification that leaves
//! somebody with a false picture costs more than the words it saved, because they
//! will act on the picture. Where a concept has no honest short form, explain it by
//! what it causes rather than reaching for an analogy that does not hold.
//!
//! **The real word stays.** Plain language sits beside the term, never instead of
//! it: an operator who never learns the word `indexer` cannot search for help about
//! indexers. The explanation is a way in, not a replacement.
//!
//! Written here, beside the behaviour, so the two version together — an explanation
//! in a wiki drifts from the thing it describes and nobody notices until it is
//! wrong.

use serde::Serialize;

/// A word this product uses, and what somebody meeting it needs to know.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct Term {
    /// The word as it appears in the interface.
    pub word: &'static str,
    /// One sentence: what it is for and what it costs or gains.
    ///
    /// Enough to act on. Somebody who reads only this should not be stuck.
    pub short: &'static str,
    /// More, for somebody who asks — never needed in order to act.
    pub deep: Option<&'static str>,
    /// What other services in this stack call the same thing.
    ///
    /// Sonarr and `SABnzbd` do not agree on words, and an operator moving between
    /// their screens should not have to work out that two of them are one.
    pub also_called: &'static [&'static str],
}

impl Term {
    /// A word and the sentence somebody needs, which is all most of them have.
    ///
    /// Built rather than written out as a struct each time. Twenty-three entries
    /// repeating `deep: None, also_called: &[],` is the same six lines over and
    /// over, and the words — which are the whole point of the table — end up
    /// buried in the shape that carries them.
    const fn new(word: &'static str, short: &'static str) -> Self {
        Self {
            word,
            short,
            deep: None,
            also_called: &[],
        }
    }

    /// With more, for somebody who asks. Never needed in order to act.
    const fn explained(mut self, deep: &'static str) -> Self {
        self.deep = Some(deep);
        self
    }

    /// With what other services in this stack call the same thing.
    const fn also(mut self, also_called: &'static [&'static str]) -> Self {
        self.also_called = also_called;
        self
    }
}

/// Every word the interface explains.
///
/// Ordered as somebody meets them rather than alphabetically: what a thing is for
/// comes before what it is measured in.
pub const TERMS: &[Term] = &[
    Term::new(
        "indexer",
        "Search engines that find what you are looking for. You need at least one, \
                and most cost a small yearly fee.",
    )
    .explained(
        "An indexer keeps track of what has been posted and where. lemonfiber asks \
             yours whenever something is wanted; without one, nothing can be found to \
             download, however much else is configured. Prowlarr lists Usenet indexers \
             and torrent sites together under the one word, which is why one screen \
             holds both. A torrent site is often called a tracker after the part of it \
             that introduces peers to each other, and on a private one that same part \
             is what watches your ratio — so the two words overlap without meaning the \
             same thing.",
    )
    .also(&["search provider"]),
    Term::new(
        "hardlink",
        "Lets one file appear in two places while taking up the space once — so \
                importing is instant and costs no extra disk.",
    )
    .explained(
        "Both names point at the same data. Deleting one leaves the other working. \
             This is why the download folder and the library should sit on one volume: \
             across two, the file has to be copied instead, which takes time and twice \
             the room.",
    ),
    Term::new(
        "retention",
        "How far back your Usenet provider keeps things. Longer retention means \
                older releases can still be downloaded.",
    )
    .explained(
        "Measured in days, and it is the age of the post rather than of the film or \
             episode. A provider with short retention is fine for new things and will \
             quietly fail to find old ones.",
    ),
    Term::new(
        "usenet",
        "One of the two ways this stack downloads. You pay a provider, downloads \
                are fast and private, and nothing is expected of you afterwards.",
    )
    .also(&["nntp"]),
    Term::new(
        "torrent",
        "The other way this stack downloads. Free, and you share back what you \
                take — which is why it goes through the VPN.",
    ),
    Term::new(
        "peer",
        "Somebody else sharing the same torrent. You take from them and they take \
                from you, which is why a torrent with nobody on it never finishes.",
    ),
    Term::new(
        "NZB",
        "What your indexer hands the download client so it can fetch the pieces \
                of a Usenet download. You rarely handle one yourself, and nothing \
                expects you to.",
    ),
    Term::new(
        "VPN",
        "A tunnel your torrent traffic leaves through, so your own connection is \
                not the one seen doing it.",
    )
    .explained(
        "lemonfiber checks that the torrent client's traffic genuinely leaves through \
             the tunnel rather than trusting that it was configured to. A tunnel that is \
             up but not carrying the traffic is the failure that looks like success.",
    ),
    Term::new(
        "killswitch",
        "Stops the torrent client reaching the internet at all if the VPN drops, \
                rather than letting it carry on unprotected.",
    ),
    Term::new(
        "backbone",
        "The network a Usenet provider actually stores its articles on. Two \
                providers sharing one hold the same things, so a second account there \
                finds nothing the first could not.",
    ),
    Term::new(
        "block account",
        "Usenet data bought as a fixed amount rather than a monthly allowance. \
                Useful as a second provider, since you spend it only on what the first \
                could not find.",
    ),
    Term::new(
        "port forwarding",
        "A way back in for other peers, opened by your VPN. Without it they \
                cannot start a connection to you, so torrents are slower and your ratio \
                suffers.",
    ),
    Term::new(
        "ratio",
        "How much you have shared back compared with what you took. Some trackers \
                expect a minimum before they let you keep downloading.",
    ),
    Term::new(
        "seed",
        "To keep sharing a finished torrent so others can take it. Stopping too \
                early is what a ratio requirement is about.",
    ),
    Term::new(
        "grab",
        "To send a release to the download client. It is the moment something \
                stops being a search result and starts being a download.",
    )
    .also(&["snatch"]),
    Term::new(
        "monitored",
        "Whether a service is still looking for something. Unmonitored means it \
                will not go and find it even when it is missing, which is the usual \
                reason nothing is happening.",
    ),
    Term::new(
        "stalled",
        "A download that has stopped making progress without failing outright. It \
                sits there until something moves it, which is why it is worth saying \
                rather than counting as running.",
    ),
    Term::new(
        "root folder",
        "Where a service files what it has finished with — the library it manages, \
                rather than the folder downloads land in.",
    )
    .also(&["library folder"]),
    Term::new(
        "quality profile",
        "The rules deciding which version of something is good enough to grab, and \
                which is worth replacing later.",
    )
    .explained(
        "Resolution is only part of it. A profile also weighs the source, the encoder \
             and the audio, which is why two files of the same resolution are not equally \
             welcome. The services' own screens file these under Profiles, which is not \
             quite the same word: this stack also has Compose profiles, and they decide \
             which services run rather than which releases are wanted.",
    ),
    Term::new(
        "transcode",
        "Rebuilding a video into a form the device asking for it can play. It \
                costs a great deal of processing, so a machine doing it often is one \
                that feels slow.",
    ),
    Term::new(
        "bitrate",
        "How much data each second of sound or video uses. Higher means better \
                quality and larger files, which is the whole of the trade.",
    ),
    Term::new(
        "HDR",
        "A wider range of brightness and colour than a screen normally shows. It \
                needs a display that can take it; on one that cannot, the picture can \
                look washed out.",
    ),
    Term::new(
        "custom format",
        "A rule that nudges a profile for or against particular releases — a \
                preferred group, or a thing you never want.",
    ),
];

/// What this product says a word means, where it explains it.
///
/// Matched without regard to case, because a word at the start of a sentence is the
/// same word.
#[must_use]
pub fn explain(word: &str) -> Option<&'static Term> {
    TERMS
        .iter()
        .find(|term| term.word.eq_ignore_ascii_case(word.trim()))
}

/// What marks a token as somebody's name for something rather than a word.
///
/// Release names are built this way — `Seed.of.Chucky.2004.1080p` — and so are
/// service names: `calibre-web-automated`, `lf-sonarr`. No term here is.
const JOINED: [char; 3] = ['.', '_', '-'];

/// Every explained word this text uses, in the order somebody reads them.
///
/// Matched on whole words, so `seed` in a sentence is found and `seeded` inside
/// `unseeded` is not — an explanation attached to a word that is not there reads as
/// a non-sequitur, and the operator has to work out which word it was about.
///
/// Each is reported once however often it appears: an explanation is worth reading
/// the first time and is noise every time after.
#[must_use]
pub fn mentioned(text: &str) -> Vec<&'static Term> {
    let words = words(text);

    TERMS
        .iter()
        .filter(|term| uses(&words, term.word))
        .collect()
}

/// The words in a piece of text, with anybody's name for something passed over.
fn words(text: &str) -> Vec<String> {
    text.split_whitespace().map(word).collect()
}

/// The word inside a token, or nothing where the token is a name.
///
/// Text reaching a report is not all this product's own: a release name, a service
/// name and another service's failure message all arrive in it verbatim. `seed` in
/// `Seed.of.Chucky.2004.1080p` is somebody's film, and explaining torrent seeding
/// underneath it is the exact non-sequitur these explanations exist to avoid — so a
/// token still joined by dots, underscores or hyphens once its surrounding
/// punctuation is off is taken as a name and contributes nothing.
///
/// Nothing rather than no entry at all, because an empty word cannot equal any term
/// and so also breaks the run: a term of two words can never be found spanning a
/// name that was passed over between them.
fn word(token: &str) -> String {
    let trimmed = token.trim_matches(|character: char| !character.is_ascii_alphanumeric());
    if trimmed.contains(JOINED) {
        return String::new();
    }
    trimmed.to_ascii_lowercase()
}

/// Every word this text borrows from another service's vocabulary, each with the
/// word this product uses instead.
///
/// The other names are recorded so an operator moving between screens can follow one
/// concept across them — not so this product may use either. Writing both is how a
/// reader comes to believe there are two things, which is the confusion the record
/// exists to end.
#[must_use]
pub fn borrowed(text: &str) -> Vec<(&'static str, &'static str)> {
    let words = words(text);

    let mut found = Vec::new();
    for term in TERMS {
        for also in term.also_called {
            if uses(&words, also) {
                found.push((*also, term.word));
            }
        }
    }
    found
}

/// Whether these words include this term, which may itself be more than one word.
fn uses(words: &[String], term: &str) -> bool {
    let wanted: Vec<String> = term.split(' ').map(str::to_ascii_lowercase).collect();
    words
        .windows(wanted.len())
        .any(|run| run.iter().zip(&wanted).all(|(said, want)| same(said, want)))
}

/// Whether a word this text used is this term's word, in either number.
///
/// Counted as the same word, because a plural is not a different one and this
/// product writes the plural far more often than the singular: "there are no
/// indexers configured", "hardlinks are not usable across an SMB share", "this is
/// what the indexers had". Matching only the singular missed thirty sentences,
/// among them the ones a first run shows somebody who has never met the word.
///
/// Only the text's word may carry the extra letter, never the term's. Going the
/// other way would let a term match a word that merely began with it.
fn same(said: &str, wanted: &str) -> bool {
    said == wanted
        || said
            .strip_suffix('s')
            .is_some_and(|singular| singular == wanted)
}

#[cfg(test)]
mod tests {
    use super::{borrowed, explain, mentioned, Term, TERMS};

    #[test]
    fn a_word_is_explained_however_it_is_capitalised() {
        assert_eq!(explain("indexer").map(|term| term.word), Some("indexer"));
        assert_eq!(explain("Indexer").map(|term| term.word), Some("indexer"));
        assert_eq!(explain("  VPN  ").map(|term| term.word), Some("VPN"));
        assert!(explain("sonarr").is_none(), "a service is not a term");
    }

    /// An explanation attached to a word that is not there reads as a non-sequitur,
    /// and the operator has to work out which word it was supposed to be about.
    #[test]
    fn only_whole_words_count_as_using_a_term() {
        let words = |text| {
            mentioned(text)
                .into_iter()
                .map(|term| term.word)
                .collect::<Vec<_>>()
        };

        assert_eq!(words("no indexer answered"), ["indexer"]);
        assert_eq!(words("seed it back"), ["seed"]);
        assert!(
            words("unseeded and reindexed").is_empty(),
            "not inside other words"
        );
        assert!(words("nothing of note here").is_empty());
    }

    #[test]
    fn a_term_of_several_words_is_found_as_one() {
        let words: Vec<&str> = mentioned("check the root folder and the quality profile")
            .into_iter()
            .map(|term| term.word)
            .collect();

        assert!(words.contains(&"root folder"), "{words:?}");
        assert!(words.contains(&"quality profile"), "{words:?}");
    }

    /// Not all the text in a report is this product's own words: a release name, a
    /// service name and another service's failure message all arrive in it verbatim.
    /// Explaining torrent seeding underneath a film called `Seed.of.Chucky` is the
    /// exact non-sequitur these explanations exist to avoid.
    #[test]
    fn a_word_inside_somebody_elses_name_is_not_a_term() {
        let words = |text| {
            mentioned(text)
                .into_iter()
                .map(|term| term.word)
                .collect::<Vec<_>>()
        };

        // Asserted as "the film contributed no `seed`" rather than "the sentence
        // found nothing at all". The words around a name are ordinary words until
        // one of them is explained — `stalled` was innocuous padding here until it
        // became a term — and a test that forbids the whole sentence from matching
        // is asserting something it was never about.
        let beside_a_name = words("Seed.of.Chucky.2004.1080p stalled");
        assert!(
            !beside_a_name.contains(&"seed"),
            "a film is not an instruction to seed: {beside_a_name:?}"
        );
        // The sharper case, and a real identifier this stack writes: split on its
        // hyphens it would read as the two-word term `quality profile`.
        let identifier = words("radarr-quality-profile-remux-web-1080p was applied");
        assert!(
            !identifier.contains(&"quality profile"),
            "a name is not the words inside it: {identifier:?}"
        );
        assert_eq!(
            words("The.Seed.2021 was not found by the indexer"),
            ["indexer"],
            "the product's own word still counts beside a name"
        );
    }

    /// A word does not stop being a word for ending a sentence or sitting in
    /// brackets, and a footnote that went missing there would look arbitrary.
    #[test]
    fn punctuation_around_a_word_does_not_hide_it() {
        let words = |text| {
            mentioned(text)
                .into_iter()
                .map(|term| term.word)
                .collect::<Vec<_>>()
        };

        assert_eq!(words("nothing answered the indexer."), ["indexer"]);
        assert_eq!(words("(indexer) refused"), ["indexer"]);
    }

    /// Recorded so an operator can follow one concept between screens — not so this
    /// product may use either word, which is how a reader comes to believe there are
    /// two things.
    #[test]
    fn a_word_borrowed_from_another_service_is_named_with_ours() {
        assert_eq!(
            borrowed("check the library folder"),
            [("library folder", "root folder")]
        );
        assert!(
            borrowed("check the root folder").is_empty(),
            "our own word is not borrowed from anyone"
        );
    }

    /// A plural is not a different word, and this product writes the plural far
    /// more often — including in the sentences a first run shows somebody who has
    /// never met the word.
    #[test]
    fn a_word_in_the_plural_is_still_the_word() {
        let words = |text| {
            mentioned(text)
                .into_iter()
                .map(|term| term.word)
                .collect::<Vec<_>>()
        };

        assert_eq!(words("there are no indexers configured"), ["indexer"]);
        assert_eq!(words("check the root folders"), ["root folder"]);
        assert_eq!(words("hardlinks are not usable here"), ["hardlink"]);
        assert!(
            words("unseeded and reindexed").is_empty(),
            "and a word inside another word is still not the word"
        );
    }

    /// Exercised at run time as well as in the table, because a `const fn` used
    /// only in a `const` item is evaluated by the compiler and leaves nothing for a
    /// coverage run to see.
    #[test]
    fn a_term_is_built_from_the_word_and_the_sentence() {
        let plain = Term::new("word", "what it is for.");
        assert_eq!((plain.word, plain.short), ("word", "what it is for."));
        assert!(plain.deep.is_none() && plain.also_called.is_empty());

        let full = Term::new("word", "short.")
            .explained("longer.")
            .also(&["other"]);
        assert_eq!(full.deep, Some("longer."));
        assert_eq!(full.also_called, ["other"]);
    }

    /// Worth reading the first time, noise every time after.
    #[test]
    fn a_term_used_over_and_over_is_explained_once() {
        let found = mentioned("indexer, indexer, and again indexer");

        assert_eq!(found.len(), 1);
    }

    /// A simplification that leaves somebody with a false picture costs more than
    /// the words it saved, because they will act on the picture. Truth is not
    /// testable and this does not pretend to test it — but two shapes cause most of
    /// them, and both can be refused.
    ///
    /// **An analogy that does not hold.** This module's own rule is to explain by
    /// what a thing causes rather than by what it resembles, because an analogy
    /// invites the reader to carry across every other property of the thing it was
    /// compared to — and they will not stop at the one that was meant.
    ///
    /// **A word that makes a real cost sound like none.** *Simply* and *just* are
    /// how a consequence gets talked past, and the rule beside them is that accurate
    /// and longer beats simple and wrong.
    #[test]
    fn no_explanation_reaches_for_an_analogy_or_talks_a_cost_away() {
        for term in TERMS {
            let word = term.word;
            for said in [Some(term.short), term.deep].into_iter().flatten() {
                let plainly = said.to_ascii_lowercase();
                for reaching in ["like a ", "like an ", "think of it", "imagine", "as if"] {
                    assert!(
                        !plainly.contains(reaching),
                        "{word} explains by resemblance rather than by consequence: {said}"
                    );
                }
                for away in ["simply", "just ", "merely", "nothing more than"] {
                    assert!(
                        !plainly.contains(away),
                        "{word} makes a real cost sound like none: {said}"
                    );
                }
            }
        }
    }

    /// The rule these were written to: say what it is for, not what it is. A
    /// definition answers a question nobody asked.
    #[test]
    fn no_explanation_begins_by_defining_the_word() {
        for term in TERMS {
            let opens = term.short.to_ascii_lowercase();
            let defining = format!("{} is ", term.word.to_ascii_lowercase());
            let (word, short) = (term.word, term.short);
            assert!(
                !opens.starts_with(&defining) && !opens.starts_with(&format!("a {defining}")),
                "{word} reads as a definition: {short}"
            );
        }
    }

    /// Enough to act on, and nothing that reads as a fragment.
    #[test]
    fn every_explanation_is_a_sentence_somebody_can_act_on() {
        for term in TERMS {
            let (word, short) = (term.word, term.short);
            assert!(short.ends_with('.'), "{word} does not end: {short}");
            assert!(
                short.split_whitespace().count() >= 8,
                "{word} is too short to say why it matters: {short}"
            );
            if let Some(deep) = term.deep {
                assert!(
                    deep.ends_with('.'),
                    "the longer form of {word} does not end"
                );
            }
        }
    }

    /// A report explains a word once, and that rests on the table holding it once.
    /// Two entries for one word would be explained twice, and `explain` would answer
    /// with whichever came first — so the rule the footnote block states would be
    /// broken by the data rather than by the code that reads it.
    #[test]
    fn no_word_is_in_the_table_twice() {
        let mut said: Vec<&str> = TERMS.iter().map(|term| term.word).collect();
        said.sort_unstable();
        let mut once = said.clone();
        once.dedup();

        assert_eq!(said, once, "a word is in the table more than once");
        assert!(
            TERMS.iter().all(|term| !term.word.is_empty()),
            "and none of them is nothing"
        );
    }

    /// Sonarr and `SABnzbd` do not agree on words, and an operator moving between
    /// their screens should not have to work out that two of them are one.
    #[test]
    fn the_words_other_services_use_are_recorded() {
        let also = explain("grab")
            .map(|term| term.also_called)
            .unwrap_or_default();

        assert!(also.contains(&"snatch"), "{also:?}");
    }

    /// One concept, one word — so no term may be listed as another's synonym while
    /// also being a term of its own, which would be two names for one thing.
    #[test]
    fn no_word_is_both_a_term_and_another_terms_synonym() {
        for term in TERMS {
            for also in term.also_called {
                let word = term.word;
                // Asked of the Option directly rather than through a closure over
                // it: a closure here only runs in the case being forbidden, so it
                // is a branch no passing run can reach.
                assert!(
                    explain(also).is_none(),
                    "{also:?} is both a term and a synonym of {word}"
                );
            }
        }
    }
}
