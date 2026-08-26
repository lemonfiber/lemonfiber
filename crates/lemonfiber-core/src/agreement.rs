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

#[cfg(test)]
mod tests {
    use super::over;

    /// The same words read twice name the same thing, and eight characters of it.
    #[test]
    fn the_same_words_name_the_same_reading() {
        let name = over(&["move the client", "onto the forwarded port"]);

        assert_eq!(name, over(&["move the client", "onto the forwarded port"]));
        assert_eq!(name.len(), 8, "{name}");
    }

    /// A word changed is a different reading, which is the whole of what this is
    /// for: consent given for one cannot be spent on another.
    #[test]
    fn a_word_changed_names_something_else() {
        assert_ne!(over(&["was /srv/media"]), over(&["was /mnt/media"]));
    }

    /// Two words do not run together into a third. Without the ending between them
    /// a boundary that moved would read as the same reading, which is precisely the
    /// substitution this exists to notice.
    #[test]
    fn a_boundary_that_moved_names_something_else() {
        assert_ne!(over(&["/srv", "media"]), over(&["/srvmedia", ""]));
    }

    /// Nothing read is still a name, because an offer of nothing is still an offer
    /// somebody may agree to nothing of.
    #[test]
    fn nothing_read_still_names_itself() {
        assert_eq!(over(&[]).len(), 8);
    }
}
