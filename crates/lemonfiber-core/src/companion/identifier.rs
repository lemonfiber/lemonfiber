//! The name a phone knows this stack by, whatever else about it changes.
//!
//! A phone that pairs again has to land on the machine it already holds rather than
//! beside it, and every other thing pairing material carries is something re-pairing
//! exists to change: the address moves when a router hands out another, and the
//! certificate is replaced when somebody asks. So the stack carries a name of its own,
//! minted once and kept, which neither of those touches.
//!
//! **It is made of nothing.** Sixteen bytes the operating system chose, and nothing
//! derived from the address, the certificate, a credential, the household or anybody in
//! it — so it says nothing about where this machine is or whose it is, and learning it
//! tells somebody only that two pieces of material came from one stack.

use std::path::Path;

use lemonfiber_ports::random::Random;

/// Where the identifier is kept, beside the certificate.
const FILE: &str = "stack";

/// How many bytes it is made of.
const WIDTH: usize = 16;

/// Why there is no identifier to hand a phone.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum Unnamed {
    /// What is written down is not one. It is not minted again over it: a stack that
    /// changed its name is one every phone holding it takes for a different machine.
    #[error("{FILE} does not hold an identifier this stack made")]
    Unreadable,
    /// This machine would not supply the bytes one is made of.
    #[error("this machine would not supply the unpredictable bytes an identifier is made of")]
    Unminted,
    /// It could not be written down.
    #[error("{FILE} could not be written: {0}")]
    Unwritten(String),
}

/// The identifier kept in `directory`, minted and written down first where there is none.
///
/// # Errors
///
/// [`Unnamed`] where what is kept is not an identifier, or where a new one could not be
/// minted or written down.
pub fn kept_or_minted(directory: &Path, random: &dyn Random) -> Result<String, Unnamed> {
    let path = directory.join(FILE);
    if let Ok(written) = std::fs::read_to_string(&path) {
        let kept = written.trim();
        return if is_one(kept) {
            Ok(kept.to_owned())
        } else {
            Err(Unnamed::Unreadable)
        };
    }
    let minted = random
        .bytes(WIDTH)
        .filter(|bytes| bytes.len() == WIDTH)
        .ok_or(Unnamed::Unminted)?;
    let minted = crate::secret::render(&minted);
    crate::config::store::write(&path, &format!("{minted}\n"))
        .map_err(|why| Unnamed::Unwritten(why.to_string()))?;
    Ok(minted)
}

/// Whether a word is one this stack would have minted.
fn is_one(word: &str) -> bool {
    word.len() == WIDTH * 2
        && word
            .chars()
            .all(|letter| letter.is_ascii_digit() || ('a'..='f').contains(&letter))
}

#[cfg(test)]
mod tests;
