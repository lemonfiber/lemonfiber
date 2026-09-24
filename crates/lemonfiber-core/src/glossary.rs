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

use crate::error::{Amiss, Problem, Remedy, Severity};

/// A word this product uses, and what somebody meeting it needs to know.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, schemars::JsonSchema)]
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
    /// The other forms this product itself writes the word in, where a state or a
    /// stage is named by one — `grabbed` for `grab`, `seeding` for `seed`.
    ///
    /// Apart from [`Self::also_called`], which is another service's word and one this
    /// product must never write as its own. These are this product's own words, and a
    /// surface explaining a word it was sent looks for the word it was sent here, so
    /// that nothing on the far side has to guess which term an inflection belongs to.
    pub forms: &'static [&'static str],
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
            forms: &[],
        }
    }

    /// With more, for somebody who asks. Never needed in order to act.
    const fn explained(mut self, deep: &'static str) -> Self {
        self.deep = Some(deep);
        self
    }

    /// With the other forms this product writes the word in.
    const fn forms(mut self, forms: &'static [&'static str]) -> Self {
        self.forms = forms;
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
    )
    .forms(&["hardlinked"]),
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
    )
    .forms(&["seeding"]),
    Term::new(
        "grab",
        "To send a release to the download client. It is the moment something \
                stops being a search result and starts being a download.",
    )
    .also(&["snatch"])
    .forms(&["grabbed", "grabbing"]),
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
    Term::new(
        "PEM",
        "How a key is written down as text rather than as bytes. A file in this \
                form opens with a line of dashes saying what it holds, which is how \
                you can tell at a glance that you have the right one.",
    )
    .explained(
        "lemonfiber reads one where you name a key to check a plugin's images against. \
             It wants the public half — the file whose first line says PUBLIC KEY — and \
             refuses anything else by name rather than reading the end of it as a key and \
             reporting that nothing matched.",
    ),
];

/// Every word this product explains, for somebody who asked what there is to ask
/// about.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, schemars::JsonSchema)]
pub struct Vocabulary {
    /// The words, in the order somebody meets them.
    pub words: Vec<Term>,
}

/// What this product says a word means, where it explains it.
///
/// Matched without regard to case, because a word at the start of a sentence is the
/// same word — and on the other forms this product writes it in, because `grabbed`
/// is asked about as often as `grab`.
#[must_use]
pub fn explain(word: &str) -> Option<&'static Term> {
    let word = word.trim();
    TERMS.iter().find(|term| {
        term.word.eq_ignore_ascii_case(word)
            || term
                .forms
                .iter()
                .any(|form| form.eq_ignore_ascii_case(word))
    })
}

/// Every word there is to ask about, whole.
///
/// The table rather than a page of it: two dozen entries is a list somebody reads
/// to the end, and a surface paging through it would need a cursor for an answer
/// that fits on one screen. A surface asks for this rather than carrying its own
/// copy, which is what keeps every surface explaining a word the same way.
#[must_use]
pub fn vocabulary() -> Vocabulary {
    Vocabulary {
        words: TERMS.to_vec(),
    }
}

/// A word this product does not explain, as a refusal that says what it does.
///
/// Through the error model rather than a bare line, so it carries a code and a way
/// forward like every other refusal — and the way forward is the list itself, which
/// is short enough to be the answer rather than a pointer at one.
///
/// It lies in the naming: the word is the whole of what was asked for, and there is
/// no entry for it. A surface that reported this as its own failure would be telling
/// a caller to try again at something that will never work.
#[must_use]
pub fn unrecognised(word: &str) -> Problem {
    let words: Vec<&str> = TERMS.iter().map(|term| term.word).collect();
    Problem::new(
        crate::error::codes::word::UNRECOGNISED,
        Severity::Error,
        format!("`{word}` is not one of the words this product explains"),
        "What is explained here is this ecosystem's own vocabulary — the words that \
         are load-bearing and cannot be guessed. Having no entry is not the same as \
         meaning nothing, and nothing is wrong with your stack.",
        Remedy::new("Ask about one of the words its reports use"),
    )
    .lies_in(Amiss::Naming)
    .with_detail(format!("It explains these — {}.", words.join(", ")))
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
mod tests;
