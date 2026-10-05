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
//! other by its code alone. [`MOVED`] is every code it carries, and [`moved`] is how
//! each is raised: the web API publishes the list among the refusals it answers
//! with, and answers each at the status [`MOVED_AMISS`] gives, so a code added here
//! reaches a client as a regenerated diff and a refusal raised without [`moved`]
//! would be answered as a failure of the machine.

use crate::error::codes::{gone, migrate, repair, restore, space, wire};
use crate::error::{Amiss, Code, Problem};

/// Every code an answer is refused with for naming an offer or a listing that has
/// since moved.
///
/// Each is declared beside its family rather than here, because the family is where
/// an operator searching for the code reads what else that command refuses. Letting a
/// download go raises the disk account's, because its offer is one line of that
/// account.
pub const MOVED: [Code; 6] = [
    repair::STALE,
    restore::MOVED_ON,
    migrate::OFFER_MOVED,
    space::ANOTHER_OFFER,
    gone::ANOTHER_READING,
    wire::WIRING_MOVED,
];

/// Where the fault lies in an answer that named what has since moved.
///
/// In how it asked: nothing is broken, and the same request with the name that
/// stands now is answered. Not a failure of the machine, which is what a client
/// gives up on, and not other work holding the stack, which a client waits out.
pub const MOVED_AMISS: Amiss = Amiss::Asking;

/// The refusal of an answer that named what has since moved, as every command
/// raising one raises it.
#[must_use]
pub fn moved(refusal: Problem) -> Problem {
    refusal.lies_in(MOVED_AMISS)
}

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
    parts
        .iter()
        .map(|words| over(words))
        .collect::<Vec<String>>()
        .join(&BETWEEN.to_string())
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
