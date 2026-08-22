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

/// A word this product uses, and what somebody meeting it needs to know.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
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

/// Every word the interface explains.
///
/// Ordered as somebody meets them rather than alphabetically: what a thing is for
/// comes before what it is measured in.
pub const TERMS: &[Term] = &[
    Term {
        word: "indexer",
        short: "Search engines that find what you are looking for. You need at least one, \
                and most cost a small yearly fee.",
        deep: Some(
            "An indexer keeps track of what has been posted and where. lemonfiber asks \
             yours whenever something is wanted; without one, nothing can be found to \
             download, however much else is configured. Prowlarr lists Usenet indexers \
             and torrent sites together under the one word, which is why one screen \
             holds both. A torrent site is often called a tracker after the part of it \
             that introduces peers to each other, and on a private one that same part \
             is what watches your ratio — so the two words overlap without meaning the \
             same thing.",
        ),
        also_called: &["search provider"],
    },
    Term {
        word: "hardlink",
        short: "Lets one file appear in two places while taking up the space once — so \
                importing is instant and costs no extra disk.",
        deep: Some(
            "Both names point at the same data. Deleting one leaves the other working. \
             This is why the download folder and the library should sit on one volume: \
             across two, the file has to be copied instead, which takes time and twice \
             the room.",
        ),
        also_called: &[],
    },
    Term {
        word: "retention",
        short: "How far back your Usenet provider keeps things. Longer retention means \
                older releases can still be downloaded.",
        deep: Some(
            "Measured in days, and it is the age of the post rather than of the film or \
             episode. A provider with short retention is fine for new things and will \
             quietly fail to find old ones.",
        ),
        also_called: &[],
    },
    Term {
        word: "usenet",
        short: "One of the two ways this stack downloads. You pay a provider, downloads \
                are fast and private, and nothing is expected of you afterwards.",
        deep: None,
        also_called: &["nntp"],
    },
    Term {
        word: "torrent",
        short: "The other way this stack downloads. Free, and you share back what you \
                take — which is why it goes through the VPN.",
        deep: None,
        also_called: &[],
    },
    Term {
        word: "NZB",
        short: "What your indexer hands the download client so it can fetch the pieces \
                of a Usenet download. You rarely handle one yourself, and nothing \
                expects you to.",
        deep: None,
        also_called: &[],
    },
    Term {
        word: "VPN",
        short: "A tunnel your torrent traffic leaves through, so your own connection is \
                not the one seen doing it.",
        deep: Some(
            "lemonfiber checks that the torrent client's traffic genuinely leaves through \
             the tunnel rather than trusting that it was configured to. A tunnel that is \
             up but not carrying the traffic is the failure that looks like success.",
        ),
        also_called: &[],
    },
    Term {
        word: "killswitch",
        short: "Stops the torrent client reaching the internet at all if the VPN drops, \
                rather than letting it carry on unprotected.",
        deep: None,
        also_called: &[],
    },
    Term {
        word: "ratio",
        short: "How much you have shared back compared with what you took. Some trackers \
                expect a minimum before they let you keep downloading.",
        deep: None,
        also_called: &[],
    },
    Term {
        word: "seed",
        short: "To keep sharing a finished torrent so others can take it. Stopping too \
                early is what a ratio requirement is about.",
        deep: None,
        also_called: &[],
    },
    Term {
        word: "grab",
        short: "To send a release to the download client. It is the moment something \
                stops being a search result and starts being a download.",
        deep: None,
        also_called: &["snatch"],
    },
    Term {
        word: "root folder",
        short: "Where a service files what it has finished with — the library it manages, \
                rather than the folder downloads land in.",
        deep: None,
        also_called: &["library folder"],
    },
    Term {
        word: "quality profile",
        short: "The rules deciding which version of something is good enough to grab, and \
                which is worth replacing later.",
        deep: Some(
            "Resolution is only part of it. A profile also weighs the source, the encoder \
             and the audio, which is why two files of the same resolution are not equally \
             welcome. The services' own screens file these under Profiles, which is not \
             quite the same word: this stack also has Compose profiles, and they decide \
             which services run rather than which releases are wanted.",
        ),
        also_called: &[],
    },
    Term {
        word: "custom format",
        short: "A rule that nudges a profile for or against particular releases — a \
                preferred group, or a thing you never want.",
        deep: None,
        also_called: &[],
    },
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
    let words: Vec<String> = text.split_whitespace().map(word).collect();

    TERMS
        .iter()
        .filter(|term| uses(&words, term.word))
        .collect()
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
    let words: Vec<String> = text.split_whitespace().map(word).collect();

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
        .any(|run| run == wanted.as_slice())
}

#[cfg(test)]
mod tests {
    use super::{borrowed, explain, mentioned, TERMS};

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

        assert!(
            words("Seed.of.Chucky.2004.1080p stalled").is_empty(),
            "a film is not an instruction to seed"
        );
        assert!(words("calibre-web-automated is unhealthy").is_empty());
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

    /// Worth reading the first time, noise every time after.
    #[test]
    fn a_term_used_over_and_over_is_explained_once() {
        let found = mentioned("indexer, indexer, and again indexer");

        assert_eq!(found.len(), 1);
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
