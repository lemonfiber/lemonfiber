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

        /// Every code this file declares.
        pub(super) const DECLARED: &[crate::Code] = &[$($($family::$name,)*)*];

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
    ack, bind, config, diag, docker, env, form, host, life, proc, read, rehearse, serve, setup,
    stack, telling, tui, update, watch, word,
};
pub use serving::{
    admit, backup, bundle, cred, gone, invite, kept, plugin, provider, qual, quota, rate, reissue,
    remove, repair, restore, seed, space, storage, undo, vpn, wire, wiring,
};

/// Every code there is, family by family, in number order.
#[must_use]
pub fn every() -> Vec<Code> {
    let mut every: Vec<Code> = operating::DECLARED
        .iter()
        .chain(serving::DECLARED)
        .copied()
        .collect();
    every.sort_by_key(|code| ordering(code.as_str()));
    every
}

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
