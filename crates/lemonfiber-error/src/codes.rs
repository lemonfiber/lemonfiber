//! Every problem code lemonfiber can raise, declared in one place.
//!
//! A code is a family and a number. It is never renumbered and never recycled, so an
//! operator who searches for one finds the same answer a year later. The families are
//! modules here, and every crate names a code through them, so a code exists exactly
//! where these lists say it does and the reference in `reference/error-codes.md` is
//! written from [`every`].
//!
//! `WIRE` and `WIRING` are both the wiring domain's: the two prefixes were published
//! apart, and a published code keeps its spelling.
//!
//! What a run that ends on a code leaves with is declared beside it, with `=>`, where
//! it is not a general failure; [`leaves`] reads it.

use crate::Code;

/// What a run that ends on a problem leaves with, for a script that reads nothing else.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Leaves {
    /// A general failure.
    Failure,
    /// Something outside lemonfiber has to be fixed before it can act.
    Preflight,
    /// Something the operator wrote was refused.
    Validation,
    /// Started, and a service never became usable.
    NeverSettled,
}

/// Declares each family as a module of codes, with the list of them and what a run
/// ending on one leaves with.
macro_rules! codes {
    ($(
        $(#[doc = $family_doc:literal])*
        $family:ident {
            $($(#[doc = $doc:literal])* $name:ident = $id:literal $(=> $leaves:ident)?,)*
        }
    )*) => {
        $(
            $(#[doc = $family_doc])*
            pub mod $family {
                use crate::Code;
                $($(#[doc = $doc])* pub const $name: Code = Code::declared($id);)*
            }
        )*

        /// Every code this file declares, with the name and the line it is declared under.
        pub(super) const DECLARED: &[super::Declared] = &[$($(super::Declared {
            code: $family::$name,
            name: stringify!($name),
            said: concat!($($doc),*),
        },)*)*];

        /// What a run ending on `code` leaves with, where this file says.
        pub(super) fn leaving(code: crate::Code) -> Option<super::Leaves> {
            match code.as_str() {
                $($($($id => Some(super::Leaves::$leaves),)?)*)*
                _ => None,
            }
        }
    };
}

mod operating;
mod serving;

pub use operating::{
    ack, ask, bind, config, diag, docker, env, form, host, life, pair, proc, read, rehearse, serve,
    setup, stack, telling, tui, update, watch, word,
};
pub use serving::{
    admit, backup, bundle, cred, gone, handoff, invite, kept, plugin, provider, qual, quota, rate,
    reissue, remove, repair, restore, seed, space, storage, undo, vpn, wire, wiring,
};

/// One code as this file declares it: the code, the name it is declared under and
/// the line written above it.
///
/// Read by whatever has to publish a code rather than only raise it. The contract
/// lists the codes a refusal carries, and a client generating a value per code needs
/// a name to give each value and a sentence to document it with — the ones written
/// here, so that a published name cannot drift from the declared one.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Declared {
    code: Code,
    name: &'static str,
    said: &'static str,
}

impl Declared {
    /// The code itself.
    #[must_use]
    pub const fn code(self) -> Code {
        self.code
    }

    /// The name it is declared under, as a constant is spelled.
    #[must_use]
    pub const fn name(self) -> &'static str {
        self.name
    }

    /// The line written above it, as one sentence.
    #[must_use]
    pub fn description(self) -> &'static str {
        self.said.trim()
    }
}

/// How one code is declared, or nothing where no family declares it.
#[must_use]
pub fn declared(code: Code) -> Option<Declared> {
    operating::DECLARED
        .iter()
        .chain(serving::DECLARED)
        .find(|declared| declared.code == code)
        .copied()
}

/// Every code there is, family by family, in number order.
#[must_use]
pub fn every() -> Vec<Code> {
    let mut every: Vec<Code> = operating::DECLARED
        .iter()
        .chain(serving::DECLARED)
        .map(|declared| declared.code)
        .collect();
    every.sort_by_key(|code| ordering(code.as_str()));
    every
}

/// Numbers no family declares and none may declare again.
///
/// Each was published with a meaning and nothing raises it. An operator who searches for
/// one must never land on a different problem that was given its number.
pub const RETIRED: &[&str] = &["QUOTA-3"];

/// Where a code sorts: its family, then its number.
fn ordering(code: &str) -> (&str, u32) {
    let (family, number) = code.rsplit_once('-').unwrap_or((code, ""));
    (family, number.parse().unwrap_or_default())
}

/// What a run that ends on `code` leaves with.
#[must_use]
pub fn leaves(code: Code) -> Leaves {
    operating::leaving(code)
        .or_else(|| serving::leaving(code))
        .unwrap_or(Leaves::Failure)
}

#[cfg(test)]
mod tests;
