//! Each thing lemonfiber keeps, in the operator's words.
//!
//! Keyed by the accessor on [`Paths`] that names the location, because that is what
//! the guard at the bottom reads: the layout is the list of places, and an entry
//! here is somebody having said what a place is for in a sentence an operator can
//! act on. A place the layout gains and this does not is red.

use crate::config::paths::Paths;

use super::{Beside, Kept, Root};

/// One thing kept: the accessor that locates it, what it is, why, and whether it
/// holds a credential.
pub struct Entry {
    /// The accessor on the layout that names this location, spelled as it is
    /// written there — which is what the guard below reads the layout for.
    pub accessor: &'static str,
    /// What it is, in the operator's words.
    pub what: &'static str,
    /// Why it is kept.
    pub why: &'static str,
    /// Whether it holds a credential.
    pub secret: bool,
    /// Where it is, given a layout.
    pub at: fn(&Paths) -> std::path::PathBuf,
}

impl Entry {
    /// This entry against a real layout.
    pub(super) fn against(&self, paths: &Paths) -> Kept {
        Kept {
            what: self.what.to_owned(),
            at: (self.at)(paths).display().to_string(),
            why: self.why.to_owned(),
            secret: self.secret,
        }
    }
}

/// Everything lemonfiber keeps: what an operator answered first, then what can be
/// made again.
pub const EVERY: &[Entry] = &[
    Entry {
        accessor: "env_file",
        what: "the settings file",
        why: "Every answer setup collected, and everything the stack is run with. It is handed \
              to Compose as it stands, and it is the one file to keep if you keep one.",
        secret: true,
        at: Paths::env_file,
    },
    Entry {
        accessor: "setup_progress",
        what: "setup's saved progress",
        why: "What you had answered when you last stopped part-way through setup, so leaving \
              resumes rather than restarts. It carries the credentials you had entered by then, \
              and it is deleted the moment setup finishes.",
        secret: true,
        at: Paths::setup_progress,
    },
    Entry {
        accessor: "admission",
        what: "the web interface's password",
        why: "What proves the password you set for the web interface, kept as an Argon2id \
              verifier and never as the password itself — there is no key that would turn it \
              back. It is what lets the surface be offered to your network at all, and \
              removing it takes that offer away.",
        secret: true,
        at: Paths::admission,
    },
    Entry {
        accessor: "journal",
        what: "the change journal",
        why: "What the last run wrote, so a repair can be put back. The fields that changed and \
              what they were — nothing about you and nothing about what you watch.",
        secret: false,
        at: Paths::journal,
    },
    Entry {
        accessor: "baseline",
        what: "the expected-state baseline",
        why: "What lemonfiber last wrote into each service, which is the only way a later run \
              can tell a value you changed from one it set itself and would otherwise write \
              over.",
        secret: false,
        at: Paths::baseline,
    },
    Entry {
        accessor: "materialised",
        what: "the record of what was written to the stack directory",
        why: "A checksum for each compose file lemonfiber wrote, so a file you edited by hand is \
              recognised as yours rather than replaced on the next run.",
        secret: false,
        at: Paths::materialised,
    },
    Entry {
        accessor: "quality",
        what: "the quality choice",
        why: "Which preset is in force, for everything and per media type, so a later run \
              applies the choice you made rather than the default.",
        secret: false,
        at: Paths::quality,
    },
    Entry {
        accessor: "notifications",
        what: "the notification choice",
        why: "How much you asked to be told about, and the individual events you set apart from \
              that answer.",
        secret: false,
        at: Paths::notifications,
    },
    Entry {
        accessor: "accepted",
        what: "the questions you have already settled",
        why: "The choices whose cost was stated to you once — running with no VPN, or with a \
              provider that forwards no port — so they are not put to you again every run.",
        secret: false,
        at: Paths::accepted,
    },
    Entry {
        accessor: "acknowledged",
        what: "the words you have gone and looked up",
        why: "So a report names a word you already know instead of explaining it again. What \
              one person has been told is not what another has, which is why this is kept here \
              rather than beside the stack.",
        secret: false,
        at: Paths::acknowledged,
    },
    Entry {
        accessor: "stack",
        what: "the materialised stack",
        why: "The compose files Compose is pointed at, written out of the stack description this \
              build carries. Any run that starts something writes them again from scratch.",
        secret: false,
        at: Paths::stack,
    },
    Entry {
        accessor: "service_config",
        what: "each service's own configuration",
        why: "The directories the containers mount and write their own settings, databases and \
              API keys into. What is in them is theirs rather than lemonfiber's, and this is the \
              largest thing here by far.",
        secret: true,
        at: Paths::service_config,
    },
    Entry {
        accessor: "backups",
        what: "the backup archives",
        why: "Each one a copy of the configuration above, so an archive holds whatever that held \
              — the credentials included. Nothing sends one anywhere.",
        secret: true,
        at: Paths::backups,
    },
    Entry {
        accessor: "bundles",
        what: "the support bundles kept for a browser",
        why: "A bundle asked for from the web surface is written here so it can be handed back \
              to the browser that asked for it. Every value in one is replaced by a stand-in \
              before it is written, and nothing sends one anywhere either.",
        secret: false,
        at: Paths::bundles,
    },
    Entry {
        accessor: "storage_state",
        what: "what the disk could do last time",
        why: "Whether importing by hardlink worked when it was last looked at, so losing that \
              capability is noticed rather than showing up as a disk filling twice as fast. \
              Losing this costs one run's history and nothing else.",
        secret: false,
        at: Paths::storage_state,
    },
];

/// The two directories everything above sits under.
pub(super) fn roots(paths: &Paths) -> Vec<Root> {
    vec![
        Root {
            at: paths.config_dir().display().to_string(),
            what: "Your answers, and lemonfiber's memory of what it wrote. Small, worth keeping a \
                   copy of, and the half that cannot be made again."
                .to_owned(),
        },
        Root {
            at: paths.data_dir().display().to_string(),
            what: "What can be made again: the materialised stack, each service's own \
                   configuration, the archives and the bundles."
                .to_owned(),
        },
    ]
}

/// What is on this machine that lemonfiber neither keeps nor removes.
///
/// An operator reading a list of what is stored is owed the reason a thing is absent
/// from it as much as they are owed the entries — and the library is the absence
/// that would otherwise read as an oversight.
#[must_use]
pub fn beside() -> Vec<Beside> {
    vec![
        Beside {
            what: "your library and your downloads".to_owned(),
            why: "Written by the services under the path you chose, and yours. lemonfiber never \
                  reads what is in them and never removes them."
                .to_owned(),
        },
        Beside {
            what: "the containers, images and volumes".to_owned(),
            why: "The container engine's. lemonfiber starts and stops them and owns none of \
                  them; `docker` is what lists and removes them."
                .to_owned(),
        },
        Beside {
            what: "anything you installed yourself".to_owned(),
            why: "Docker, a media server running natively, a tunnel client. lemonfiber did not \
                  put them there and does not take them away."
                .to_owned(),
        },
    ]
}

#[cfg(test)]
mod tests {
    use super::EVERY;

    /// The layout's own source, read at compile time.
    ///
    /// The layout is the list of places lemonfiber writes. Reading it is what makes
    /// the disclosure hold: a place added there and not here is a thing kept on
    /// somebody's machine that nothing tells them about, and it is exactly the kind
    /// of addition nobody thinks to mention.
    const LAYOUT: &str = include_str!("../config/paths.rs");

    /// Every location the layout names, by the accessor that names it.
    ///
    /// The accessors that answer with a path of their own. The two that answer with
    /// the directories those sit under are the roots, and they are listed as roots
    /// rather than as things kept.
    ///
    /// One line at a time, which is what it can see: an accessor whose signature was
    /// wrapped across two lines would be invisible here, and the floor asserted below
    /// catches a reader that has stopped working rather than one that missed a single
    /// place. Every signature in that file fits on one line today and the formatter
    /// keeps it that way; a longer one is the shape to watch for.
    fn located() -> Vec<&'static str> {
        LAYOUT
            .lines()
            .filter_map(|line| line.trim().strip_prefix("pub fn "))
            .filter(|rest| rest.contains("(&self) -> PathBuf {"))
            .filter_map(|rest| rest.split_once('(').map(|(name, _)| name))
            .collect()
    }

    #[test]
    fn every_place_the_layout_names_is_disclosed() {
        let places = located();
        let counted = places.len();
        assert!(
            counted > 10,
            "the layout was read as naming {counted} places, which means this is reading the \
             wrong file and is about to agree with itself"
        );
        let disclosed: Vec<&str> = EVERY.iter().map(|entry| entry.accessor).collect();
        let unsaid: Vec<&&str> = places
            .iter()
            .filter(|place| !disclosed.contains(place))
            .collect();
        assert!(
            unsaid.is_empty(),
            "lemonfiber writes these and nothing tells the operator they are there: {unsaid:?}"
        );
    }

    /// And the other direction, which is not the same test: an entry for a place the
    /// layout no longer has would tell somebody about a file that is not on their
    /// machine, and send them looking for it.
    #[test]
    fn nothing_is_disclosed_that_the_layout_no_longer_names() {
        let places = located();
        let gone: Vec<&str> = EVERY
            .iter()
            .map(|entry| entry.accessor)
            .filter(|accessor| !places.contains(accessor))
            .collect();
        assert!(
            gone.is_empty(),
            "these are disclosed as kept and the layout names no such place: {gone:?}"
        );
    }

    #[test]
    fn every_entry_is_named_something_and_says_why_it_is_kept() {
        let silent: Vec<&str> = EVERY
            .iter()
            .filter(|entry| {
                entry.what.split_whitespace().count() < 2
                    || entry.why.split_whitespace().count() < 8
            })
            .map(|entry| entry.accessor)
            .collect();
        assert!(
            silent.is_empty(),
            "these are disclosed and the disclosure says nothing: {silent:?}"
        );
    }
}
