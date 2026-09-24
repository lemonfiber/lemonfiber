//! How far up an account may watch, in the one set of words every surface says it in.
//!
//! **The media server keeps this as a number, and the number is an age.** It holds back
//! everything it rates above that number. Driven against `jellyfin/jellyfin:10.10.3`:
//! its own rating tables, read at `GET /Localization/ParentalRatings`, put every
//! certificate against the age it is for — `TV-Y7` at 7, `12A` at 12, `PG-13` at 13,
//! `15` at 15, `18` at 18. So nought is not "nothing at all": it is everything the
//! youngest person in a house could watch, and it is said in words here for that
//! reason.
//!
//! **Certificates are country-specific.** The same tables read under a different
//! country give different names for the same numbers: `U` at nought and `PG` at eight
//! for the United Kingdom, `G` at nought and `PG` at ten for the United States. The age
//! is the same in both.
//!
//! **What the surfaces offer is a set of steps, not every value the server accepts.**
//! The server takes any number. A limit that is none of the steps is still said as the
//! age it is: an account may hold one set in the media server's own screens, or by
//! whoever ran this stack before.
//!
//! One place for the words, because they are said twice: when the limit is chosen, and
//! when it is read back off the account. Two copies would eventually disagree, and the
//! place they would disagree is a household list saying something other than what the
//! operator picked.

/// One age limit the surfaces offer, and who it suits.
pub struct Step {
    /// The age the media server keeps and holds things back above.
    pub age: u32,
    /// Who it suits, in one line — what choosing it comes to, beside the words for it.
    pub suits: &'static str,
}

/// The steps offered, lowest first.
///
/// Read in this order everywhere they are offered: a ladder from the youngest audience
/// upwards, which is how somebody deciding for a particular person in the house scans
/// it. No limit at all is not on this list, because it is the absence of a limit rather
/// than a step among them — a surface that offers it puts it where an answer that
/// changes nothing belongs, which on a command line is leaving the flag out.
const OFFERED: &[Step] = &[
    Step {
        age: 0,
        suits: "nothing the media server rates for an older audience",
    },
    Step {
        age: 7,
        suits: "about right for a young child",
    },
    Step {
        age: 12,
        suits: "about right for an older child",
    },
    Step {
        age: 15,
        suits: "about right for a teenager",
    },
    Step {
        age: 18,
        suits: "holds back only what is meant for adults",
    },
];

/// The steps offered, in the order they are read.
#[must_use]
pub fn steps() -> &'static [Step] {
    OFFERED
}

/// What a limit here is, and what it is not — said wherever one is set or reported.
///
/// **Overstating this is worse than an accurate modest claim**, because a parent may
/// rely on it. What the media server does is decide what it offers an account; it is
/// not a boundary anybody has to get past, and somebody with the run of the home
/// network and a browser has other ways at the same files. Said in one place because
/// two surfaces wording a promise differently is two surfaces making different
/// promises, and this is the promise it matters least to get wrong.
pub(crate) const A_FILTER_NOT_A_LOCK: &str = "These limits are a content filter, not a \
    security boundary: they decide what the media server offers an account, and \
    somebody with the run of the home network has other ways at the same files.";

/// How an age limit reads, given the number the media server keeps it as.
///
/// Nought is said in words rather than as a figure, because "nothing above about 0" is
/// a sentence about a number and this is a sentence about a household. Every other age
/// is said as the age, whether or not it is one of the steps offered.
#[must_use]
pub fn reading(age: Option<u32>) -> String {
    match age {
        None => "anything".to_owned(),
        Some(0) => "only what suits everyone".to_owned(),
        Some(age) => format!("nothing above about {age}"),
    }
}

#[cfg(test)]
mod tests;
