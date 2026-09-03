//! Which errands there are, in the order they are read.
//!
//! The catalogue, apart from the flow that sends one. [`super`] decides what a press
//! over a list does, what an errand has to be given first and what its yes carries;
//! this is the list itself — and a reader checking what this screen offers against the
//! other surfaces reads it without the stage machinery in the way.
//!
//! Each row names an action every surface calls by the same name, and nothing here
//! assembles a command: what an errand comes to is [`lemonfiber_api::actions::named`]'s
//! answer, so a row naming something no other surface offers is red rather than a
//! request only this screen can make.

use super::{Errand, Going, Needs};

/// The errand the list opens on.
///
/// Held apart from the rest for the reason the selected question is: a list built
/// from a slice that might have been empty carries a case for there being no
/// errands, which is not a state this screen can be in.
static OPENS_ON: Errand = Errand {
    name: "wiring",
    about: "connect each service to the others, leaving what you changed alone",
    action: "seed",
    asks: "Wire the services to each other",
    needs: Needs::Nothing,
    accepts: None,
    going: Going::Once,
};

/// The errands after it, read from the ones that keep work towards the one that
/// throws it away — which is also the order nobody lands on the destructive one by
/// pressing enter at the list.
static AFTER: &[Errand] = &[
    Errand {
        name: "your edits kept",
        about: "take every value you changed as lemonfiber's own, so a seed leaves it",
        action: "adopt",
        asks: "Keep every value you changed",
        needs: Needs::Nothing,
        accepts: None,
        going: Going::Once,
    },
    Errand {
        name: "an invitation",
        about: "make somebody in the house an account they claim by setting a password",
        action: "invite",
        asks: "Invite",
        needs: Needs::Invitation("Who it is for, as they will sign in"),
        accepts: None,
        going: Going::Once,
    },
    Errand {
        name: "a password somebody can set again",
        about: "put their account back to having none, so they choose the next one",
        action: "reissue",
        asks: "Let a new password be set for",
        needs: Needs::Named("Whose account, as they appear in the household"),
        accepts: None,
        // Once rather than agreed: nothing is destroyed and nothing is listed first.
        // What ends is a password nobody here knows, and the account is claimable the
        // moment this returns.
        going: Going::Once,
    },
    Errand {
        name: "somebody taken out of the household",
        about: "revoke their account on the media server and on the request service",
        action: "remove",
        asks: "Remove from the household",
        needs: Needs::Named("Who to remove, as they appear in the household"),
        accepts: None,
        // Agreed rather than once: the run before it says what goes — their watch
        // history, which cannot be kept, and every request they made, which stops
        // existing — so what is agreed to is what was read.
        going: Going::Agreed,
    },
    Errand {
        name: "a backup",
        about: "capture a configuration to an archive kept on this machine",
        action: "backup",
        asks: "Capture the configuration of",
        needs: Needs::Service,
        accepts: None,
        going: Going::Once,
    },
    Errand {
        name: "a support bundle",
        about: "what somebody helping would ask for, with every credential replaced",
        action: "support",
        asks: "Write the bundle",
        needs: Needs::Bundling("How many lines of each service's log to take"),
        accepts: None,
        going: Going::Written,
    },
    Errand {
        name: "the last repair put back",
        about: "reverse what the last repair changed, leaving the wiring under it alone",
        action: "undo",
        asks: "Put back what the last repair changed",
        needs: Needs::Nothing,
        accepts: None,
        going: Going::Once,
    },
    Errand {
        name: "a backup put back",
        about: "restore one this machine took, over the configuration here now",
        action: "restore",
        asks: "Restore from",
        needs: Needs::Archive("Which backup, by the name it was written under"),
        accepts: Some("re-pointing the data root to this machine's"),
        going: Going::Agreed,
    },
    Errand {
        name: "your edits thrown away",
        about: "put lemonfiber's own state back over every value you changed",
        action: "reset",
        asks: "Throw away every edit above",
        needs: Needs::Nothing,
        accepts: None,
        going: Going::Agreed,
    },
    Errand {
        name: "everything lemonfiber keeps removed",
        about: "take every file lemonfiber wrote off this machine; your library is not one",
        action: "forget",
        asks: "Remove everything listed above",
        needs: Needs::Nothing,
        accepts: None,
        going: Going::Agreed,
    },
];

/// The errands, the one the list opens on apart from the rest.
pub(crate) fn all() -> (&'static Errand, Vec<&'static Errand>) {
    (&OPENS_ON, AFTER.iter().collect())
}

/// Every errand, in the order they are read.
#[cfg(test)]
pub(crate) fn every() -> impl Iterator<Item = &'static Errand> {
    std::iter::once(&OPENS_ON).chain(AFTER)
}
