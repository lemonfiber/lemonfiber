//! Which of the command line's requests the dashboard reaches, and how it reaches
//! each one.
//!
//! A projection of what the terminal offers, published where something outside the
//! binary can read it. The screen's own lists — the actions on its keys, the errands
//! behind one of them, and the questions behind another — live in `acting/`, which is
//! `mod acting;` in `main.rs` and therefore private to the binary. An integration test
//! cannot reach a private module, so the parity table's terminal column was the one
//! column nothing checked: twenty-six rows re-read by eye, every slice.
//!
//! So the list is here, in the library, and it is held at both ends. `acting/`'s own
//! tests hold every action and every question the screen offers to an entry here, and
//! every entry here to something the screen offers; the parity table's terminal column
//! is held to the same list by
//! [`surface_parity.rs`](../../../crates/lemonfiber/tests/surface_parity.rs). A row
//! claiming this screen reaches a request it does not, and an offer no row accounts
//! for, each fail — which is what the web column has had since the table was written.
//!
//! Two vocabularies meet here, which is why an entry carries both names. The table is
//! written from the command line outwards and names a *request*; the screen names the
//! *action* or the *read* the web offers, because that name is what it puts through
//! the web's own translation. `household` is asked for at `/api/requests`, and neither
//! name is derivable from the other.

/// One request the dashboard reaches, and the name it reaches it by.
pub struct Reach {
    /// The command-line request, as the parity table names it.
    pub request: &'static str,
    /// The web's own name for what the screen goes through to reach it.
    pub through: &'static str,
}

/// The requests the dashboard offers an action for, by the action's own name.
///
/// The five on keys of their own and the six behind the key that opens the rest of
/// them, in the order the screen reads them.
pub const ACTS: &[Reach] = &[
    Reach {
        request: "up",
        through: "up",
    },
    Reach {
        request: "down",
        through: "down",
    },
    Reach {
        request: "switch",
        through: "switch",
    },
    Reach {
        request: "restart",
        through: "restart",
    },
    Reach {
        request: "pull",
        through: "pull",
    },
    Reach {
        request: "seed",
        through: "seed",
    },
    Reach {
        request: "adopt",
        through: "adopt",
    },
    Reach {
        request: "backup",
        through: "backup",
    },
    Reach {
        request: "support",
        through: "support",
    },
    Reach {
        request: "restore",
        through: "restore",
    },
    Reach {
        request: "reset",
        through: "reset",
    },
];

/// The requests the dashboard answers as a question, by the path the web serves the
/// read at.
pub const ASKS: &[Reach] = &[
    Reach {
        request: "version",
        through: "/api/version",
    },
    Reach {
        request: "forms",
        through: "/api/forms",
    },
    Reach {
        request: "config",
        through: "/api/config",
    },
    Reach {
        request: "quality",
        through: "/api/quality",
    },
    Reach {
        request: "household",
        through: "/api/requests",
    },
    Reach {
        request: "trace",
        through: "/api/trace",
    },
];

/// The requests the dashboard's panels show without being asked.
///
/// Written down rather than derived, and it is the one claim on this page that is: a
/// panel is a rendering rather than a named request, so there is no list of them to
/// hold this against. What it still buys is the parity table — a row claiming this
/// screen shows what is running has to say so here too, and a panel removed leaves a
/// name here that a reader can see is unaccompanied.
pub const SHOWS: &[&str] = &["ps", "doctor", "stuck"];

/// Every request this screen reaches, however it reaches it.
#[must_use]
pub fn reached() -> Vec<&'static str> {
    ACTS.iter()
        .chain(ASKS)
        .map(|reach| reach.request)
        .chain(SHOWS.iter().copied())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::{reached, ACTS, ASKS, SHOWS};

    /// Every request is reached one way, or a reader of the table has two rows'
    /// worth of claim to reconcile against one row.
    #[test]
    fn no_request_is_reached_twice() {
        let every = reached();

        for request in &every {
            let same = every.iter().filter(|other| *other == request).count();
            assert_eq!(same, 1, "{request} is reached more than one way");
        }
        assert_eq!(every.len(), ACTS.len() + ASKS.len() + SHOWS.len());
    }

    /// A read is named by its path and an action by a bare word, which is what tells
    /// the two vocabularies apart wherever this list is read.
    #[test]
    fn a_question_is_named_by_a_path_and_an_action_by_a_word() {
        assert!(ASKS.iter().all(|reach| reach.through.starts_with("/api/")));
        assert!(ACTS.iter().all(|reach| !reach.through.contains('/')));
    }
}
