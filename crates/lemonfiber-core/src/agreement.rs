//! What an operator read, in a form the answer to it can name it by.
//!
//! A surface with a terminal in front of it holds the question open in the process
//! that asks it: the offer is printed, the operator answers, and the run that acts
//! is the run that looked. A surface reached over a network has nothing of the
//! kind. What was read arrives in one request and the answer to it in another, and
//! whatever the answer was read against may have moved on in between.
//!
//! So what was read names itself, and the answer carries that name back. The run
//! that acts builds the name again from a fresh look and compares: an answer whose
//! name is not the one that stands now was given for something else, and is refused
//! rather than spent. Everything an operator reads before deciding goes into it, so
//! anything that would make them read it differently makes it a different name.
//!
//! Not a secret and not a signature. It says *which* offer, not *who* agreed —
//! whether a caller may ask at all is decided above, once, for every request. And
//! it is a race and replay guard rather than a permission: anybody who can send the
//! second request could have made the change themselves.
//!
//! **An answer refused for naming what has since moved is one refusal, wherever it
//! is raised.** It is the one a client answers by reading again and offering what it
//! reads now, rather than by reporting a failure, so it has to be told from every
//! other by its code alone. [`MOVED`] is every code it carries, and the web API
//! publishes the list among the refusals it answers with, each at the status its code
//! is declared with, so a code added here reaches a client as a regenerated diff.

use crate::error::codes::{
    gone, life, migrate, plugin, rate, repair, restore, space, update, wire,
};
use crate::error::Code;

/// Every code an answer is refused with for naming an offer or a listing that has
/// since moved.
///
/// Each is declared beside its family rather than here, because the family is where
/// an operator searching for the code reads what else that command refuses. Letting a
/// download go raises the disk account's, because its offer is one line of that
/// account.
pub const MOVED: [Code; 10] = [
    repair::STALE,
    restore::MOVED_ON,
    migrate::OFFER_MOVED,
    space::ANOTHER_OFFER,
    gone::ANOTHER_READING,
    wire::WIRING_MOVED,
    plugin::PLUGIN_OFFER_MOVED,
    life::RESTART_MOVED,
    update::UPDATE_MOVED,
    rate::PAUSING_MOVED,
];

/// A checksum over every word an operator read before agreeing.
///
/// The words in the order they were read, so a list re-ordered is a different
/// reading of it. CRC32 rather than a cryptographic digest: what this has to
/// notice is a change nobody meant, and eight characters is short enough to travel
/// in a request body and be read back in a log.
#[must_use]
pub fn over(words: &[&str]) -> String {
    let mut hasher = crc32fast::Hasher::new();
    for word in words {
        // Ended, so that two words cannot run together into a third: without this,
        // one field losing its last character to the next would read the same as
        // the pair that was there before.
        hasher.update(word.as_bytes());
        hasher.update(&[0]);
    }
    format!("{:08x}", hasher.finalize())
}

/// How many bytes of a SHA-256 digest a sealed part keeps.
///
/// Half of it: a hundred and twenty-eight bits is far past anything a forger could
/// search for a second reading that names alike, and thirty-two characters still
/// travel in a request body and read back in a log.
const SEALED: usize = 16;

/// A digest over words somebody other than the operator wrote, so that a reading
/// forged to name alike is as hard to make as one that is the same.
///
/// [`over`] notices a change nobody meant, and that is enough where what was read is
/// this machine's own state. It is not enough where it is a stranger's file: a
/// checksum can be steered, so a file rewritten with that in mind would answer to the
/// offer the original was read under. The words are ended as [`over`] ends them.
#[must_use]
pub fn sealed(words: &[&str]) -> String {
    let mut context = ring::digest::Context::new(&ring::digest::SHA256);
    for word in words {
        context.update(word.as_bytes());
        context.update(&[0]);
    }
    let digest = context.finish();
    crate::secret::render(digest.as_ref().get(..SEALED).unwrap_or_default())
}

/// An offer from parts already named, each by [`over`] or [`sealed`], in the order
/// given.
#[must_use]
pub fn joined(names: &[String]) -> String {
    names.join(&BETWEEN.to_string())
}

/// What joins the parts of an offer that names each part it was built from.
const BETWEEN: char = '-';

/// An offer named part by part, so a refusal can say which part moved.
///
/// Each part is the words of one thing an operator read — what a change would write,
/// what it would leave without — named over [`over`] and joined in the order given.
/// One name over everything says only *something changed*, and an operator told that
/// has to read the whole offer again to find what; this says what.
#[must_use]
pub fn parted(parts: &[&[&str]]) -> String {
    joined(
        &parts
            .iter()
            .map(|words| over(words))
            .collect::<Vec<String>>(),
    )
}

/// Which parts of an offer differ between the one answered and the one standing,
/// named by `names`, in the order the parts were built.
///
/// An answer with a different number of parts, or none at all, differs in every
/// part: it was not built from this offer, and naming only some parts would claim a
/// comparison that was never made.
#[must_use]
pub fn differs<'a>(answered: &str, standing: &str, names: &[&'a str]) -> Vec<&'a str> {
    let given: Vec<&str> = answered.split(BETWEEN).collect();
    let stands: Vec<&str> = standing.split(BETWEEN).collect();
    if given.len() != stands.len() || given.len() != names.len() {
        return names.to_vec();
    }
    names
        .iter()
        .zip(given.iter().zip(&stands))
        .filter(|(_, (was, is))| was != is)
        .map(|(name, _)| *name)
        .collect()
}

#[cfg(test)]
mod tests;
