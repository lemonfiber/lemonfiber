use std::path::PathBuf;

use crate::config::store;
use crate::error::codes::setup::{
    NEEDS_SERVICE, NOT_MOVED_BACK, NOT_OPENED, NOT_PUT_BACK, NOT_REMOVED, NOT_REWOUND,
    NOT_WITHDRAWN, STILL_HOLDING, UNKEPT,
};
use crate::error::{Diagnose as _, Problem, Remedy};

/// An I/O failure that stops a reversal: the environment file or a directory that
/// would not budge. A service-made change is not one of these — it does not stop
/// the reversal, so it is reported apart, in [`needs_service`].
pub(super) enum Fault {
    /// The environment file could not be rewritten.
    Store(store::Failure),
    /// A region lemonfiber wrote into one of the stack's files could not be taken out.
    NotWithdrawn {
        /// The file the region is in.
        path: PathBuf,
        /// The operating system's own words.
        reason: String,
    },
    /// A file lemonfiber wrote over could not be written back to what it held.
    NotRewound {
        /// The file left as lemonfiber wrote it.
        path: PathBuf,
        /// The operating system's own words.
        reason: String,
    },
    /// A service's state could not be moved back.
    NotMovedBack {
        /// Where it is still.
        path: PathBuf,
        /// Why, in the operating system's words or lemonfiber's.
        reason: String,
    },
    /// A directory could not be removed.
    NotRemoved {
        /// The directory left in place.
        path: PathBuf,
        /// The operating system's own words.
        reason: String,
    },
}

impl Fault {
    /// The problem to report, in the words that fit what stopped the reversal.
    pub(super) fn problem(&self) -> Problem {
        match self {
            Self::Store(failure) => failure.problem(),
            Self::NotRemoved { path, reason } => Problem::new(
                NOT_REMOVED,
                "A directory from the interrupted setup could not be removed",
                "The rest of the setup was reversed; this one directory is still there. It holds nothing.",
                Remedy::new("Remove it by hand, or leave it where it is"),
            )
            .with_detail(format!("{}: {reason}", path.display())),
            Self::NotMovedBack { path, reason } => Problem::new(
                NOT_MOVED_BACK,
                "A service's state lemonfiber moved could not be moved back",
                "Everything before it was put back; this is still where lemonfiber moved it.",
                Remedy::new(
                    "Run it again once both places are directories of the project's own, \
                     reached through no link, that lemonfiber can write",
                ),
            )
            .with_detail(format!("{}: {reason}", path.display())),
            Self::NotRewound { path, reason } => Problem::new(
                NOT_REWOUND,
                "A file lemonfiber wrote over could not be written back to what it held",
                "Everything before it was put back; this file still holds what lemonfiber wrote.",
                Remedy::new("Run it again once the file can be written"),
            )
            .with_detail(format!("{}: {reason}", path.display())),
            Self::NotWithdrawn { path, reason } => Problem::new(
                NOT_WITHDRAWN,
                "A region lemonfiber wrote into one of the stack's files could not be taken out",
                "Everything before it was put back; this region is still in the file, between \
                 the markers that name whose it is.",
                Remedy::new("Delete the region by hand, markers included, or run it again"),
            )
            .with_detail(format!("{}: {reason}", path.display())),
        }
    }
}

/// The problem naming the changes a reversal reached the end with still undone,
/// because only the service that made each can undo it.
pub(super) fn needs_service(resources: &[String]) -> Problem {
    Problem::new(
        NEEDS_SERVICE,
        "Some changes cannot be undone without the service that made them",
        "This reversal restores settings and removes directories; a resource a service was told to create is undone through that service, not here. Everything else was reversed.",
        Remedy::new("Reverse them from the service once it is reachable"),
    )
    .with_detail(resources.join(", "))
}

/// The problem naming the settings a reversal left exactly as they are, because
/// they hold neither what the change wrote nor what putting it back would write.
///
/// Every one of them is named. "Something was left alone" tells an operator that a
/// reversal was incomplete without telling them which of their settings it decided
/// was theirs, and the answer decides whether they do anything next.
///
/// What was reversed is said too, in the same sentence. A reversal that put three
/// settings back and left a fourth is not a reversal that failed, and reading only
/// the refusal would have an operator checking three settings that are already
/// right.
pub(super) fn not_put_back(settings: &[String]) -> Problem {
    Problem::new(
        NOT_PUT_BACK,
        "Some settings hold what you chose, so they were left alone",
        "Undoing a change puts back what lemonfiber replaced, and these settings no longer hold \
         what it wrote — so they were changed after it, and writing the old value over that \
         would take away a decision you made. Everything else was put back.",
        Remedy::new("Set them back by hand if the earlier value is the one you want"),
    )
    .with_detail(settings.join(", "))
}

/// The problem naming the credentials a reversal could not read back.
///
/// Named one by one, for the reason [`not_put_back`] names its settings: an operator told
/// only that part of a reversal did not happen needs to know which part, and here the
/// answer decides whether they go and set a password again.
///
/// A warning rather than an error. Everything else was put back, and what was not is a
/// limit of what this machine can still read rather than a failure of this run — the key
/// the record was sealed under is gone, or the record was edited after it was written.
pub(super) fn not_opened(settings: &[String]) -> Problem {
    Problem::new(
        NOT_OPENED,
        "Some settings hold credentials this machine can no longer read back",
        "The journal keeps a credential sealed, under a key kept beside it. These entries \
         would not open — the key is missing, or the record was changed after it was \
         written — so what they held is not something to put back, and they were left \
         exactly as they are. Everything else was put back.",
        Remedy::new("Set them yourself, from wherever the earlier credential came from"),
    )
    .with_detail(settings.join(", "))
}

/// The problem naming the directories a reversal left because something this run did not
/// put there is inside them.
///
/// Named one at a time, for the reason [`not_put_back`] names its settings: which
/// directory it is decides whether an operator does anything next, and *something was
/// left* on its own is a sentence nobody can act on.
///
/// A warning rather than an error, and the same reading as a setting somebody has chosen
/// since. Everything else went back, and what did not is a fact about the machine — a
/// directory two things share — rather than a fault in this run.
pub(super) fn left_holding(paths: &[String]) -> Problem {
    Problem::new(
        STILL_HOLDING,
        "Some directories hold something this run did not put there",
        "A directory lemonfiber made comes off on the way back only while it is empty. \
         These still hold files — something else keeps its own documents in one of them, \
         or you put something there yourself — and taking them would take that with them. \
         Everything else was put back.",
        Remedy::new("Remove them by hand once you have seen what is inside"),
    )
    .with_detail(paths.join(", "))
}

pub(super) fn unkept(secrets: &[String]) -> Problem {
    Problem::new(
        UNKEPT,
        "Some secrets hold a value lemonfiber kept no earlier copy of",
        "Undoing a change to a secret puts back the one value lemonfiber kept before it, and \
         only while what it holds now is what the change left. For these that value is not \
         kept, so they were left exactly as they are. Everything else was put back.",
        Remedy::new("Enter the earlier value again at the terminal if it is the one you want"),
    )
    .with_detail(secrets.join(", "))
}
