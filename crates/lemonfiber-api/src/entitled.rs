//! What one caller may ask for, decided on the command rather than on the route.
//!
//! The guard says who is knocking. This says what that person may have, and it is
//! the only place that decides — a surface drawing its own conclusion from the same
//! answer would be a second opinion able to disagree with this one, and the one that
//! disagreed would be the one nobody tested.
//!
//! **Asked of the command, not of the path.** A path is an alias for a command, and
//! a rule written against the alias is a rule that can drift from what the command
//! actually does: a route renamed, a second route reaching the same command, or a
//! command growing a new effect all leave a path list saying something that was true
//! when it was written. What runs is the command, so what is ruled on is the command.
//!
//! **A member's read is narrowed by construction rather than filtered afterwards.**
//! The command that leaves here names the member who asked, whatever the request
//! named — so there is no path on which the core is asked for somebody else's row,
//! let alone sends one. Filtering afterwards would read every row and rely on the
//! device to draw one of them, which is a promise about a screen where this is a
//! fact about the wire, and it is what the requirement about one member seeing
//! another's requests asks for.

use lemonfiber_core::app::{Command, Diagnosing, Whom};
use lemonfiber_core::keys::Scope;

use crate::admission::Caller;

/// Which door a command was asked for at.
///
/// Asked because a key's scope draws its line between the two: a `read` key reaches
/// every read and no action, so the same caller is answered differently at each.
/// Nobody else's answer turns on it — the operator has both doors, and a member has
/// what is theirs at either.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Door {
    /// A read, asked for at the path it is served under.
    Reading,
    /// An action, or a step of setup.
    Acting,
}

/// What is left of a command once who is asking has been taken into account.
///
/// A type of its own rather than an `Option<Command>`, because `Option` carries
/// combinators — `unwrap_or`, `unwrap_or_default` — that would turn a refusal back
/// into the command that was refused, in one word, while reading like tidying up.
/// There is no such word for this, so the only way past it is to write the arm.
#[derive(Debug, PartialEq, Eq)]
pub enum Permitted {
    /// Carry this out. It may not be the command that was asked for: a command
    /// narrowed to its caller arrives here as the narrowed one.
    This(Command),
    /// Nothing, and say so. The caller proved who they are and this is not theirs.
    Nothing,
    /// Nothing, because a key with this scope may not call it.
    NotForAKey(String),
}

impl Permitted {
    /// The command to carry out, or the refusal the caller is answered with.
    ///
    /// # Errors
    ///
    /// The refusal, where this is not a command to carry out: what is not theirs, or
    /// what a key with this scope may not call, named by that scope.
    pub fn granted(self) -> Result<Command, Box<axum::response::Response>> {
        match self {
            Self::This(command) => Ok(command),
            Self::Nothing => Err(Box::new(crate::refusal::Refusal::NotYours.answered())),
            Self::NotForAKey(scope) => Err(Box::new(crate::refusal::Refusal::NotForAKey.saying(
                format!(
                    "A key with the scope {scope} may not call this. A key calls only the \
                     actions the contract publishes as callable by a key, and a read key \
                     calls none."
                ),
            ))),
        }
    }
}

/// The command this caller actually gets at `door`, or nothing.
///
/// Every command reaches this, whether it was asked for as a read, at the actions
/// door or as a step of setup, and whether it is answered now or handed to a job —
/// so a control a member is not entitled to is refused by the core rather than by
/// the app having omitted it, which is what the requirement about a control a
/// member is not entitled to asks for. A hand-written
/// request gets no further here than a tapped button does.
#[must_use]
pub fn may(caller: &Caller, door: Door, command: Command) -> Permitted {
    match caller {
        // The whole surface, unchanged. Somebody at this machine's terminal and
        // somebody holding this machine's password are the two people this product
        // already answered everything for, and nothing here narrows that.
        Caller::Machine | Caller::Operator => Permitted::This(command),
        Caller::Member(id) => members(id, &command),
        Caller::Key(keyed) => match (&keyed.scope, door) {
            // Exactly what that member's own session admits, at either door.
            (Scope::Member { id, .. }, _) => members(id, &command),
            (Scope::Read | Scope::Act, Door::Reading) => Permitted::This(command),
            (Scope::Act, Door::Acting) if callable_by_a_key(&command) => Permitted::This(command),
            (scope @ (Scope::Read | Scope::Act), Door::Acting) => {
                Permitted::NotForAKey(scope.written())
            }
        },
    }
}

/// Whether a key may call this command, as the contract publishes.
///
/// **Everything not named here is refused to a key**, for the reason everything not
/// named is refused to a member: a command added later is not a key's until somebody
/// decides it is. The list is short on purpose — a key is the credential most likely
/// to be held somewhere the operator is not, so nothing here cannot be undone or widens
/// what the stack trusts.
#[must_use]
pub const fn callable_by_a_key(command: &Command) -> bool {
    matches!(
        command,
        Command::Restart { .. }
            | Command::Doctor(Diagnosing { accept: None, .. })
            | Command::Update(_)
            | Command::Downloads(_)
    )
}

/// The household read narrowed to one member: their own row and nobody else's.
///
/// What a member asking for the household is given, and what their own stream reads, so
/// the two cannot come to narrow it differently.
#[must_use]
pub(crate) fn their_household(id: &str) -> Command {
    Command::Household {
        member: Some(Whom::Named(id.to_owned())),
    }
}

/// One member's own shelf, `most` of it.
#[must_use]
pub(crate) fn their_shelf(id: &str, most: u32) -> Command {
    Command::Held {
        member: Whom::Named(id.to_owned()),
        most,
    }
}

/// What one member is playing, and nobody else.
#[must_use]
pub(crate) fn their_playing(id: &str) -> Command {
    Command::Playing {
        member: Some(id.to_owned()),
    }
}

/// What a household member may have of a command.
fn members(id: &str, command: &Command) -> Permitted {
    match command {
        // Theirs, narrowed to them. Whatever the request named is discarded
        // rather than compared, so there is no arm on which a mismatch could be
        // let through — and a page left open reloading with a stale name is
        // answered with their own row rather than signed out for holding it.
        // Asking for the household's defaults is discarded the same way: a member
        // is somebody, and what they are told is what they are told.
        Command::Household { .. } => Permitted::This(their_household(id)),
        // Theirs, and narrowed the same way. How much of the shelf to answer with
        // is the caller's to choose and is carried through; whose shelf it is
        // never was, so what the request named is discarded rather than checked.
        Command::Held { most, .. } => Permitted::This(their_shelf(id, *most)),
        // Theirs, narrowed the same way: what they are playing, and nobody else's.
        // Whatever the request named is discarded, so an operator's narrowing cannot
        // be borrowed to read another member's sessions.
        Command::Playing { .. } => Permitted::This(their_playing(id)),
        // **Everything not named above is refused**, and the catch-all is the
        // statement rather than an omission: a command added later is not a
        // member's until somebody decides it is and writes it down. Listing what
        // members may *not* do would make every new command theirs by default, and
        // the day that is wrong is the day nobody notices.
        _ => Permitted::Nothing,
    }
}

#[cfg(test)]
mod tests;
