use super::{borrowed, explain, mentioned, unrecognised, vocabulary, Term, TERMS};

/// Every word the published contract sends that is a form of a word explained
/// here — a stage, a standing, an outcome — is one of that word's forms, so a
/// surface looking the sent word up finds it without keeping a mapping of its own.
///
/// Read off the committed contract rather than a list kept here, so a new stage
/// named `grabbing` or a standing named `seeded` goes red on the day it is added.
/// A form is a single word that begins with the term and carries more letters; a
/// plain plural is already the same word, and a joined name like
/// `indexers-failed` is the name of a state rather than a form of a word.
#[test]
fn every_form_of_an_explained_word_the_contract_sends_is_found() {
    let contract: serde_json::Value =
        serde_json::from_str(include_str!("../../../contract/web-api.contract.json"))
            .unwrap_or_default();
    let mut sent = std::collections::BTreeSet::new();
    let mut open = vec![&contract];
    while let Some(value) = open.pop() {
        match value {
            serde_json::Value::Object(fields) => {
                if let Some(serde_json::Value::String(one)) = fields.get("const") {
                    sent.insert(one.clone());
                }
                let listed = fields.get("enum").and_then(serde_json::Value::as_array);
                sent.extend(
                    listed
                        .into_iter()
                        .flatten()
                        .filter_map(serde_json::Value::as_str)
                        .map(str::to_owned),
                );
                open.extend(fields.values());
            }
            serde_json::Value::Array(items) => open.extend(items),
            _ => {}
        }
    }
    assert!(sent.contains("grabbed"), "the contract was read");

    let forms: Vec<(&String, &str)> = sent
        .iter()
        .filter(|one| one.chars().all(|letter| letter.is_ascii_lowercase()))
        .flat_map(|one| {
            TERMS
                .iter()
                .filter(move |term| {
                    let word = term.word.to_ascii_lowercase();
                    one.len() > word.len() + 1 && one.starts_with(&word)
                })
                .map(move |term| (one, term.word))
        })
        .collect();
    assert!(
        !forms.is_empty(),
        "the contract sends forms of explained words"
    );
    let unexplained: Vec<&(&String, &str)> = forms
        .iter()
        .filter(|(one, word)| explain(one).is_none_or(|found| found.word != *word))
        .collect();
    assert!(unexplained.is_empty(), "{unexplained:?}");
}

#[test]
fn a_form_this_product_writes_finds_its_word_and_another_services_word_does_not() {
    assert_eq!(explain("grabbed").map(|term| term.word), Some("grab"));
    assert_eq!(explain("Seeding").map(|term| term.word), Some("seed"));
    assert_eq!(
        explain("hardlinked").map(|term| term.word),
        Some("hardlink")
    );
    assert!(
        explain("snatch").is_none(),
        "another service's word is not a form"
    );
}

/// Every surface asks for the words rather than carrying its own copy, so the
/// list handed out is the table itself.
#[test]
fn the_vocabulary_is_every_word_in_the_table() {
    let listed = vocabulary();

    assert_eq!(listed.words.len(), TERMS.len());
    assert_eq!(listed.words.first().map(|term| term.word), Some("indexer"));
}

/// A refusal an operator cannot act on is worse than none, and here what to do
/// about it is short enough to simply be said.
#[test]
fn a_word_it_does_not_explain_is_refused_with_the_ones_it_does() {
    let problem = unrecognised("indexr");

    let summary = &problem.summary;
    assert!(summary.contains("indexr"), "{summary}");
    let detail = problem.detail.clone().unwrap_or_default();
    assert!(detail.contains("indexer"), "{detail}");
    assert!(detail.contains("hardlink"), "{detail}");
}

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
        .also(&["other"])
        .forms(&["worded"]);
    assert_eq!(full.deep, Some("longer."));
    assert_eq!(full.also_called, ["other"]);
    assert_eq!(full.forms, ["worded"]);
    assert!(plain.forms.is_empty());
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
