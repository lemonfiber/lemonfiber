//! Printing one stored credential, because it is the operator's and they asked.
//!
//! Refusing outright would be this product deciding it knows better than the person
//! whose secrets these are, which it does not: an operator moving a service to
//! another machine, or signing in to qBittorrent's web UI by hand, has an ordinary
//! reason to need a value lemonfiber minted on their behalf and never showed them.
//!
//! What is owed instead is that it never happens by accident. Asking prints the
//! warning and not the value; asking again, having read it, prints the value. So a
//! credential can only reach a terminal through a request that says what it is
//! for — never as a side effect of listing what is held, and never from a surface
//! that merely renders whatever it is given.

use crate::app::targets::recorded_secret;
use crate::app::Ctx;
use crate::credential::{Held, Revealed, REVEALED, SHOULDER};

/// One credential, printed or explained.
///
/// Unconfirmed, the warning is the whole answer. Confirmed, the value comes with the
/// warning still attached, because the thing worth saying is as true afterwards as it
/// was before — the value is in the scrollback either way once it has been printed.
pub(crate) fn reveal(ctx: &Ctx, held: &Held, confirmed: bool) -> Revealed {
    if !confirmed {
        return Revealed {
            name: held.name.clone(),
            value: None,
            warning: SHOULDER.to_owned(),
        };
    }
    if let Some(plugin) = held.plugins() {
        return Revealed {
            name: held.name.clone(),
            value: None,
            warning: held.unheld(plugin),
        };
    }
    match recorded_secret(ctx, &held.setting) {
        Some(value) => Revealed {
            name: held.name.clone(),
            value: Some(value),
            warning: REVEALED.to_owned(),
        },
        None => Revealed {
            name: held.name.clone(),
            value: None,
            warning: format!(
                "There is nothing recorded under {} for lemonfiber to show. Where a service \
                 minted its own key, lemonfiber holds a copy only once the stack has been \
                 seeded; the service's own settings hold it either way.",
                held.setting
            ),
        },
    }
}

/// What is said where nothing this stack holds answers to the name that was given.
///
/// A reveal rather than a rotation, because that is what was asked for. Reported as a
/// rotation it would put a replacement nobody asked for into the answer, render as one
/// on the terminal, and earn the exit code a failed replacement earns.
pub(crate) fn nothing_by_that_name(credential: &str, known: &[String]) -> Revealed {
    Revealed {
        name: credential.to_owned(),
        value: None,
        warning: format!(
            "Nothing here is called `{credential}`. What is: {}.",
            known.join(", ")
        ),
    }
}

#[cfg(test)]
mod tests;
