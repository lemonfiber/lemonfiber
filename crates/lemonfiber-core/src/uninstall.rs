//! Taking lemonfiber off this machine, in four removals that are separate decisions.
//!
//! Uninstall is a trust feature: software that is hard to remove is software people
//! hesitate to install. So the whole of this is arranged around two properties.
//!
//! **The operator sees the actual list.** Every container, every image, every path
//! and what it occupies — not "19 containers and 42 GiB". A summary is something an
//! operator agrees to; a list is something they can check.
//!
//! **The library is never bundled.** Exactly one tier reaches the operator's own
//! content, it is never reached as a side effect of another, and it takes an
//! agreement built over the size at stake — so a yes given for one reading of the
//! disk cannot be spent on another.
//!
//! What this module holds is the vocabulary and the judgements over it. Gathering the
//! facts is [`crate::app::uninstall`], which is where the engine, the filesystem and
//! the walk are reached.

mod foreign;
mod outside;
mod tier;

use serde::Serialize;

pub use foreign::{beside, ours, Foreign};
pub use outside::{against, looked_for, Beside, Outside, EVERY as BESIDE};
pub use tier::{Tier, EVERY as TIERS};

use crate::error::Code;

/// Raised when the tier that takes the library was confirmed without its own
/// agreement.
pub const NEEDS_AGREEING: Code = Code::new("GONE-1");

/// Raised when an agreement names a reading of this machine that is not the one
/// standing now.
pub const ANOTHER_READING: Code = Code::new("GONE-2");

/// What sort of thing one line of a manifest is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, schemars::JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum Sort {
    /// A container the engine is holding.
    Container,
    /// A network the containers were on.
    Network,
    /// An image that was pulled.
    Image,
    /// A directory or file on this machine.
    Path,
}

/// One thing a removal reaches, said to be going or said to be kept.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, schemars::JsonSchema)]
pub struct Item {
    /// What it is called — a container name, an image reference, or a full path.
    pub name: String,
    /// Which of the four sorts of thing it is.
    pub sort: Sort,
    /// What it is, in the operator's words.
    pub what: String,
    /// What it occupies, where that is knowable. Absent for a container or a network,
    /// whose room is the image's rather than their own.
    pub bytes: Option<u64>,
    /// Why it is being kept rather than removed, where it is being kept.
    ///
    /// `None` is the ordinary case: this line is going. A reason here is the whole of
    /// how an image shared with another project, or a path this run could not
    /// confirm, stays on the list without being taken.
    pub kept: Option<String>,
    /// Whether it holds a credential, so a report can say what destroying it destroys.
    pub secret: bool,
}

impl Item {
    /// Whether this line is one the removal takes.
    #[must_use]
    pub const fn goes(&self) -> bool {
        self.kept.is_none()
    }
}

/// How much of a manifest was read and how much stood in for what could not be.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, schemars::JsonSchema)]
pub struct Confidence {
    /// Whether every source this tier needed answered.
    pub complete: bool,
    /// What could not be read, each in the words of whatever refused.
    ///
    /// The point of the field: a manifest that is short says so and says why, rather
    /// than reading as a machine with less on it than it has.
    pub unread: Vec<String>,
}

impl Confidence {
    /// A reading where everything answered.
    #[must_use]
    pub fn whole() -> Self {
        Self {
            complete: true,
            unread: Vec::new(),
        }
    }

    /// The same reading, with one source recorded as having not answered.
    #[must_use]
    pub fn short(mut self, why: impl Into<String>) -> Self {
        self.complete = false;
        self.unread.push(why.into());
        self
    }
}

/// One download still coming down when the removal was asked for.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, schemars::JsonSchema)]
pub struct Coming {
    /// What it is, as the client names it.
    pub name: String,
    /// How far along, from zero to a hundred.
    pub progress: u8,
}

/// What removing would come to, shown before anything is removed.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, schemars::JsonSchema)]
pub struct Manifest {
    /// Which removal this is.
    pub tier: Tier,
    /// What it takes, in the operator's words.
    pub removes: String,
    /// What it leaves alone, in the operator's words.
    pub keeps: String,
    /// Every line it reaches, each said to be going or said to be kept.
    pub items: Vec<Item>,
    /// What the lines that are going occupy, where that is knowable.
    pub bytes: u64,
    /// What is beneath the data location that the stack did not put there.
    ///
    /// Not a warning. While this is non-empty the data location is never removed as
    /// one tree, and only the stack's own directories beneath it are offered.
    pub foreign: Vec<Foreign>,
    /// Whether the data location is on a network share or a drive that unplugs.
    ///
    /// Said where it is, so removing across a mount an operator forgot was a mount is
    /// something they read before agreeing rather than after.
    pub volume: Option<String>,
    /// What is still coming down, which stopping would interrupt.
    pub coming: Vec<Coming>,
    /// What lemonfiber cannot remove, each with how to remove it by hand.
    pub outside: Vec<Outside>,
    /// Whether a backup was offered before configuration is destroyed, and how.
    pub backup: Option<String>,
    /// How much of this was read, and what could not be.
    pub confidence: Confidence,
    /// What this reading names itself, so an answer says which reading it answered.
    pub agreement: String,
}

/// Something a removal could not take.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, schemars::JsonSchema)]
pub struct Left {
    /// What is still there.
    pub name: String,
    /// What the machine said about it, verbatim.
    pub why: String,
    /// How to finish it by hand.
    pub by_hand: String,
}

/// Whether anything was removed on this run, and what became of it.
///
/// Four states rather than the five a removal passes through. `removing` is the
/// interval between the last two and is said through the narrator as it happens — a
/// value returned at the end cannot be the state a run is in while it runs, and a
/// variant nothing could ever answer with would be a state that is documentation
/// pretending to be a value.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, schemars::JsonSchema)]
#[serde(tag = "state", rename_all = "kebab-case")]
pub enum Removal {
    /// Everything is enumerated with its size, and nothing has been removed.
    Surveyed,
    /// The tier and the manifest were agreed to, and this run changes nothing — the
    /// state a rehearsal ends in.
    Confirmed,
    /// Everything the manifest named as going is gone.
    Complete {
        /// What went, by the name the manifest gave it.
        gone: Vec<String>,
        /// The credentials this destroyed, said rather than left to be inferred.
        credentials: Vec<String>,
    },
    /// Some of it could not be removed, and each of those is named with how to
    /// finish it by hand.
    Partial {
        /// What went, by the name the manifest gave it.
        gone: Vec<String>,
        /// The credentials this destroyed.
        credentials: Vec<String>,
        /// What is still there, and how to remove it.
        left: Vec<Left>,
    },
}

/// A removal, before or after it happened.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, schemars::JsonSchema)]
pub struct Uninstall {
    /// What removing would come to, or what it came to.
    pub manifest: Manifest,
    /// Whether anything was removed on this run.
    pub removal: Removal,
}

/// What the lines that are going occupy.
///
/// Only the lines that are going. An image kept because another project stands on it
/// is on the list and is not room this would get back, and a total that counted it
/// would be a promise the removal does not keep.
#[must_use]
pub fn reclaimable(items: &[Item]) -> u64 {
    items
        .iter()
        .filter(|item| item.goes())
        .filter_map(|item| item.bytes)
        .fold(0, u64::saturating_add)
}

/// What this reading names itself.
///
/// Built over everything an operator reads before agreeing — the tier, every line
/// and whether it is going, the size at stake in the words it is shown in, what is
/// beside the library, and what the volume is. Anything that would make them read it
/// differently makes it a different name, which is what stops a yes given for one
/// reading of a disk from being spent on another.
#[must_use]
pub fn naming(tier: Tier, items: &[Item], foreign: &[Foreign], volume: Option<&str>) -> String {
    let mut words: Vec<String> = vec![tier.name().to_owned()];
    for item in items {
        words.push(format!(
            "{}:{}:{}",
            item.name,
            if item.goes() { "goes" } else { "kept" },
            item.bytes.unwrap_or_default()
        ));
    }
    for found in foreign {
        words.push(format!("beside {}:{}", found.at, found.files));
    }
    // The size in the words it is read in, not only in bytes. What the operator is
    // shown is "1.4 TiB", and it is that sentence they are agreeing to.
    words.push(crate::bytes::humanize(reclaimable(items)));
    words.push(volume.unwrap_or_default().to_owned());

    let read: Vec<&str> = words.iter().map(String::as_str).collect();
    crate::agreement::over(&read)
}

#[cfg(test)]
mod tests {
    use super::{naming, reclaimable, Confidence, Foreign, Item, Sort, Tier};

    /// A line that is going, occupying the given room.
    fn going(name: &str, bytes: u64) -> Item {
        Item {
            name: name.to_owned(),
            sort: Sort::Image,
            what: "an image".to_owned(),
            bytes: Some(bytes),
            kept: None,
            secret: false,
        }
    }

    /// The same line, kept for a stated reason.
    fn kept(name: &str, bytes: u64, why: &str) -> Item {
        Item {
            kept: Some(why.to_owned()),
            ..going(name, bytes)
        }
    }

    #[test]
    fn only_what_is_going_counts_towards_what_would_be_freed() {
        let items = vec![
            going("linuxserver/sonarr:4.0.15", 400),
            kept("postgres:16", 900, "another project is standing on it"),
        ];

        assert_eq!(reclaimable(&items), 400);
        assert!(items.first().is_some_and(Item::goes));
        assert!(items.get(1).is_some_and(|item| !item.goes()));
    }

    #[test]
    fn a_line_with_no_knowable_size_does_not_break_the_total() {
        let items = vec![
            Item {
                bytes: None,
                ..going("sonarr", 0)
            },
            going("radarr", 40),
        ];

        assert_eq!(reclaimable(&items), 40);
    }

    #[test]
    fn a_total_that_would_overflow_saturates_rather_than_wrapping() {
        let items = vec![going("a", u64::MAX), going("b", 1)];

        assert_eq!(reclaimable(&items), u64::MAX);
    }

    /// The whole of what the agreement is for: the same reading names itself the
    /// same way twice, so an answer can be checked against a fresh look.
    #[test]
    fn the_same_reading_names_itself_the_same_way() {
        let items = vec![going("a", 10)];
        let name = naming(Tier::Media, &items, &[], None);

        assert_eq!(name, naming(Tier::Media, &items, &[], None));
        assert_eq!(name.len(), 8, "{name}");
    }

    /// A tier changed is a different reading. An operator who read what removing the
    /// containers would do has not agreed to losing the library.
    #[test]
    fn a_different_tier_is_a_different_reading() {
        let items = vec![going("a", 10)];

        assert_ne!(
            naming(Tier::Media, &items, &[], None),
            naming(Tier::Configuration, &items, &[], None)
        );
    }

    /// A size changed is a different reading, which is the requirement that a
    /// confirmation state the volume at stake, held as a property.
    #[test]
    fn a_disk_that_has_moved_since_it_was_read_is_a_different_reading() {
        assert_ne!(
            naming(Tier::Media, &[going("a", 10)], &[], None),
            naming(Tier::Media, &[going("a", 2_000_000_000_000)], &[], None)
        );
    }

    /// An image that has since become shared is kept rather than taken, and that is
    /// a different reading too — the list the operator agreed to has changed.
    #[test]
    fn a_line_that_has_become_kept_is_a_different_reading() {
        assert_ne!(
            naming(Tier::Services, &[going("a", 10)], &[], None),
            naming(Tier::Services, &[kept("a", 10, "shared")], &[], None)
        );
    }

    /// Something appearing beside the library since it was read is a different
    /// reading: the operator agreed to a disk that had nothing of theirs on it.
    #[test]
    fn something_found_beside_the_library_is_a_different_reading() {
        let found = vec![Foreign {
            at: "Photographs".to_owned(),
            files: 9_000,
            bytes: 40,
        }];

        assert_ne!(
            naming(Tier::Media, &[going("a", 10)], &[], None),
            naming(Tier::Media, &[going("a", 10)], &found, None)
        );
    }

    /// And a data location that turns out to be a network share is a different
    /// reading, which is the additional confirmation that case asks for.
    #[test]
    fn a_removal_that_crosses_a_network_share_is_a_different_reading() {
        assert_ne!(
            naming(Tier::Media, &[going("a", 10)], &[], None),
            naming(Tier::Media, &[going("a", 10)], &[], Some("an SMB share"))
        );
    }

    #[test]
    fn a_reading_that_could_not_be_completed_says_what_it_could_not_read() {
        let whole = Confidence::whole();
        assert!(whole.complete && whole.unread.is_empty());

        let short = whole.short("the container engine is not running");
        assert!(!short.complete);
        assert_eq!(short.unread.len(), 1);
        assert!(short
            .unread
            .first()
            .is_some_and(|why| why.contains("engine")));
    }
}
