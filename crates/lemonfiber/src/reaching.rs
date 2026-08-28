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
        request: "forget",
        through: "forget",
    },
    Reach {
        request: "reset",
        through: "reset",
    },
    Reach {
        request: "walkthrough",
        through: "walkthrough",
    },
    Reach {
        request: "watch",
        through: "watch",
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
        request: "doctor",
        through: "/api/checks",
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
        request: "front-door",
        through: "/api/front-door",
    },
    Reach {
        request: "trace",
        through: "/api/trace",
    },
    Reach {
        request: "stuck",
        through: "/api/stuck",
    },
    Reach {
        request: "outbound",
        through: "/api/outbound",
    },
    Reach {
        request: "stored",
        through: "/api/stored",
    },
    Reach {
        request: "clients",
        through: "/api/clients",
    },
];

/// The requests the dashboard's panels show without being asked.
///
/// Written down rather than derived, and it is the one claim on this page that is: a
/// panel is a rendering rather than a named request, so there is no list of them to
/// hold this against. What it still buys is the parity table — a row claiming this
/// screen shows what is running has to say so here too, and a panel removed leaves a
/// name here that a reader can see is unaccompanied.
///
/// `stuck` was here and is a question now. The panel that carried it reads the queue
/// health gather, which counts what has stopped and names the cause where several
/// items share one; the read the command line means by `stuck` names each item by the
/// title a trace is asked for. They are two renderings of one worry rather than one
/// rendering, and it was the second that the screen had no way to reach — so the
/// panel stays and the request is reached where it can be followed.
///
/// `doctor` went the same way, for the same reason and a sharper one. Two panels read
/// facts the diagnosis reads too — how much room is left on the disk and whether
/// imports link, where traffic leaves from and whether the client's is inside the
/// tunnel — but a fact is not a verdict, and neither panel carries the one thing a
/// diagnosis is for: what to do about it. A pass, a warning and a check that could
/// not be established all render as the same number in a panel. So the panels stay,
/// and the diagnosis is reached as a question, where its verdicts and its remedies
/// are read in the words the command line gives for the same run.
///
/// `front-door` arrived the other way round and settles in the same place. It was a
/// question first and is a panel as well now, because the operator who needs the
/// address is not the one who thought to ask for it — they have just been asked what
/// to open by somebody in the next room. The panel carries the address and the phrase
/// beside it; the question carries what else is on the network and why none of it is
/// the door. So it stays on [`ASKS`], where the fuller answer is, rather than being
/// named twice for one screen.
pub const SHOWS: &[&str] = &["ps"];

/// The requests the dashboard reaches a second way, having already reached them as a
/// question.
///
/// Two requests, seven actions. `quality` is on [`ASKS`] as a read — the preset in
/// force, what each one means and what it costs — and three of these are the writes
/// offered beside that reading. `doctor` is on [`ASKS`] too, and the other four are
/// the whole of what can be done about what that reading reports: the same diagnosis
/// widened to the checks that disturb a running system, the repairs it found put
/// right, one of its warnings answered, and the last repair put back. A read that
/// disturbed something would not be a read, and none of the other three is a read at
/// all, so all four are asked for at the door this screen asks for changes.
///
/// The last of them is offered from a different place on the screen — it is on the
/// list of errands rather than beside the other two, because it reads nothing before
/// it acts — and it is published here all the same, because what this list is keyed
/// by is the request, not the key that reaches it. `doctor --undo` is the `doctor`
/// row's, as `--fix` and `--accept` are.
///
/// None of the seven is an entry in [`ACTS`], because both requests are already
/// reached: [`reached`] is what the parity table's terminal column is held against in
/// both directions, and a request named there twice would leave a reader of one row
/// with two claims to reconcile against it.
///
/// What they are published for is the other direction. Every action the screen
/// offers is held to a name here by `acting/`'s own tests, so a write added to that
/// screen with no row accounting for it fails rather than going unnoticed — which is
/// the whole of what this list buys and the only thing it is read for.
pub const ALSO: &[Reach] = &[
    Reach {
        request: "quality",
        through: "quality-set",
    },
    Reach {
        request: "quality",
        through: "quality-reapply",
    },
    Reach {
        request: "quality",
        through: "quality-upgrade",
    },
    Reach {
        request: "doctor",
        through: "diagnose",
    },
    Reach {
        request: "doctor",
        through: "repair",
    },
    Reach {
        request: "doctor",
        through: "accept",
    },
    Reach {
        request: "doctor",
        through: "undo",
    },
];

/// The requests the dashboard reaches by giving the terminal back.
///
/// One, and it carries no second name because there is no table to name it in: no
/// other surface has an action for starting a surface, so there is nothing on the
/// web this could go through. What it goes through is the key itself, and the
/// screen's own test holds this to the request that key reaches.
pub const OPENS: &[&str] = &["ui"];

/// Every request this screen reaches, however it reaches it.
#[must_use]
pub fn reached() -> Vec<&'static str> {
    ACTS.iter()
        .chain(ASKS)
        .map(|reach| reach.request)
        .chain(SHOWS.iter().copied())
        .chain(OPENS.iter().copied())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::{reached, ACTS, ALSO, ASKS, OPENS, SHOWS};

    /// Every request is reached one way, or a reader of the table has two rows'
    /// worth of claim to reconcile against one row.
    #[test]
    fn no_request_is_reached_twice() {
        let every = reached();

        for request in &every {
            let same = every.iter().filter(|other| *other == request).count();
            assert_eq!(same, 1, "{request} is reached more than one way");
        }
        assert_eq!(
            every.len(),
            ACTS.len() + ASKS.len() + SHOWS.len() + OPENS.len()
        );
    }

    /// A read is named by its path and an action by a bare word, which is what tells
    /// the two vocabularies apart wherever this list is read.
    #[test]
    fn a_question_is_named_by_a_path_and_an_action_by_a_word() {
        assert!(ASKS.iter().all(|reach| reach.through.starts_with("/api/")));
        assert!(ACTS.iter().all(|reach| !reach.through.contains('/')));
        assert!(ALSO.iter().all(|reach| !reach.through.contains('/')));
    }

    /// A second way is a second way to something already reached.
    ///
    /// An entry here naming a request no other list holds would be a write nothing in
    /// the parity table accounts for — the failure the list beside it exists to catch,
    /// arriving through the list added to catch it.
    #[test]
    fn a_second_way_reaches_something_this_screen_already_reaches() {
        let every = reached();

        for reach in ALSO {
            assert!(
                every.contains(&reach.request),
                "{} is reached no other way",
                reach.request
            );
        }
        assert!(!ALSO.is_empty());
    }
}
