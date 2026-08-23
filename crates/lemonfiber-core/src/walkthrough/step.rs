//! The steps a walkthrough passes through, in order, and what an import did with the
//! file when it got there.
//!
//! Declaration order is walk order, so one step compares less than a later one and the
//! furthest reached is a `max`. The steps deliberately mirror [`crate::trace::Stage`] —
//! the same journey, watched live rather than reconstructed afterwards — with two
//! differences: a walkthrough begins by *choosing* something, which a trace never does
//! because the choosing already happened, and it ends by telling the media server to
//! look, which a trace only observes.

use serde::{Deserialize, Serialize};

use crate::trace::Stage;

/// One step of the walk, ordered from picking something to watching it play.
#[derive(
    Debug,
    Clone,
    Copy,
    Default,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Serialize,
    Deserialize,
    schemars::JsonSchema,
)]
#[serde(rename_all = "kebab-case")]
pub enum Step {
    /// Picking something to add, and confirming it is not already here.
    #[default]
    Choosing,
    /// The indexers are being searched for releases.
    Searching,
    /// A release is being sent to the download client.
    Grabbing,
    /// The download is running.
    Downloading,
    /// The \*arr is moving the finished download into the library.
    Importing,
    /// The media server is being told to look at what arrived.
    Scanning,
    /// It is in the library and playable.
    Available,
}

impl Step {
    /// The step a pipeline stage corresponds to, so a live walk and an after-the-fact
    /// trace agree on where something got to.
    ///
    /// Stages before anything was asked for map to choosing: a walkthrough that has only
    /// just monitored an item has, as far as the operator can see, only chosen it.
    #[must_use]
    pub const fn of_stage(stage: Stage) -> Self {
        match stage {
            Stage::NotMonitored | Stage::Monitored => Self::Choosing,
            Stage::Searching | Stage::Found => Self::Searching,
            Stage::Grabbed => Self::Grabbing,
            Stage::Downloading | Stage::Downloaded => Self::Downloading,
            Stage::Importing => Self::Importing,
            Stage::Imported => Self::Scanning,
            Stage::Available => Self::Available,
        }
    }

    /// What this step is called while it is happening, in the operator's language rather
    /// than the services'. Present tense and unfinished, because it is read as it runs.
    #[must_use]
    pub const fn said(self) -> &'static str {
        match self {
            Self::Choosing => "Choosing something to add",
            Self::Searching => "Searching indexers",
            Self::Grabbing => "Sending to download client",
            Self::Downloading => "Downloading",
            Self::Importing => "Importing",
            Self::Scanning => "Telling the library to look",
            Self::Available => "Available to watch",
        }
    }

    /// Which service does the work of this step, named so the operator learns the shape
    /// of their own stack by watching it run once — which is half the point of the walk.
    #[must_use]
    pub const fn done_by(self) -> &'static str {
        match self {
            Self::Choosing | Self::Searching | Self::Grabbing | Self::Importing => {
                "the library manager"
            }
            Self::Downloading => "the download client",
            Self::Scanning | Self::Available => "the media server",
        }
    }

    /// Whether this step is the end of the walk — the one that means it worked.
    #[must_use]
    pub const fn is_the_end(self) -> bool {
        matches!(self, Self::Available)
    }

    /// Every step in walk order, for a narration that shows what is still to come.
    #[must_use]
    pub const fn all() -> [Self; 7] {
        [
            Self::Choosing,
            Self::Searching,
            Self::Grabbing,
            Self::Downloading,
            Self::Importing,
            Self::Scanning,
            Self::Available,
        ]
    }
}

/// What the import did with the finished download — the difference between one copy of a
/// file and two.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum Link {
    /// The library entry and the download are the same file under two names, which is
    /// what a correctly-shared volume allows and what costs no extra disk.
    Hardlinked,
    /// The file was copied, so it now exists twice. Works, but every import costs its own
    /// size again and seeding a torrent holds the second copy for as long as it seeds.
    Copied,
}

impl Link {
    /// What this means for the operator's disk, said once, concretely, at the moment they
    /// can see the file it happened to.
    ///
    /// The walkthrough is the natural place for this: an explanation of hardlinks in the
    /// abstract is a documentation page nobody reads, and the same explanation attached to
    /// a file that just landed is a thing understood.
    #[must_use]
    pub const fn consequence(self) -> &'static str {
        match self {
            Self::Hardlinked => {
                "the library and the download are one file under two names, so this cost \
                 no extra disk"
            }
            Self::Copied => {
                "this was copied rather than hardlinked, so it is on disk twice — every \
                 import will cost its own size again until the download is removed"
            }
        }
    }

    /// What to do about it, where there is anything to do.
    #[must_use]
    pub const fn remedy(self) -> Option<&'static str> {
        match self {
            Self::Hardlinked => None,
            Self::Copied => Some(
                "Put the downloads and the library on one volume, mounted at one path in \
                 both containers — `lemonfiber doctor` names the mismatch",
            ),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{Link, Step};
    use crate::trace::Stage;

    #[test]
    fn the_steps_are_declared_in_the_order_they_happen() {
        let mut walked = Step::all().to_vec();
        walked.sort_unstable();
        assert_eq!(
            walked,
            Step::all().to_vec(),
            "declaration order is walk order"
        );
        assert!(Step::Choosing < Step::Available);
        assert!(Step::all().last().copied().is_some_and(Step::is_the_end));
    }

    #[test]
    fn only_the_last_step_means_it_worked() {
        let ends: Vec<Step> = Step::all().into_iter().filter(|s| s.is_the_end()).collect();
        assert_eq!(ends, vec![Step::Available]);
    }

    #[test]
    fn every_pipeline_stage_lands_on_a_step() {
        // The live walk and the after-the-fact trace have to agree on where something
        // got to, so every stage the trace knows maps onto a step the walk narrates.
        let stages = [
            (Stage::NotMonitored, Step::Choosing),
            (Stage::Monitored, Step::Choosing),
            (Stage::Searching, Step::Searching),
            (Stage::Found, Step::Searching),
            (Stage::Grabbed, Step::Grabbing),
            (Stage::Downloading, Step::Downloading),
            (Stage::Downloaded, Step::Downloading),
            (Stage::Importing, Step::Importing),
            (Stage::Imported, Step::Scanning),
            (Stage::Available, Step::Available),
        ];
        for (stage, step) in stages {
            assert_eq!(Step::of_stage(stage), step, "{stage:?}");
        }
    }

    #[test]
    fn the_stage_mapping_never_goes_backwards() {
        // A later stage can never map to an earlier step, or a walk would appear to
        // retreat while the pipeline advanced.
        let ordered = [
            Stage::NotMonitored,
            Stage::Monitored,
            Stage::Searching,
            Stage::Found,
            Stage::Grabbed,
            Stage::Downloading,
            Stage::Downloaded,
            Stage::Importing,
            Stage::Imported,
            Stage::Available,
        ];
        for pair in ordered.windows(2) {
            let (earlier, later) = (pair.first().copied(), pair.last().copied());
            let steps = earlier
                .zip(later)
                .map(|(a, b)| (Step::of_stage(a), Step::of_stage(b)));
            assert!(steps.is_some_and(|(a, b)| a <= b), "{pair:?}");
        }
    }

    #[test]
    fn every_step_says_what_it_is_doing_and_who_is_doing_it() {
        for step in Step::all() {
            assert!(!step.said().is_empty(), "{step:?}");
            assert!(step.done_by().starts_with("the"), "{step:?}");
            // Present tense and unfinished: it is read while it runs.
            assert!(!step.said().ends_with('.'), "{step:?}");
        }
    }

    #[test]
    fn a_copy_is_explained_and_a_hardlink_is_not_a_problem() {
        assert!(Link::Copied.consequence().contains("twice"));
        assert!(
            Link::Copied.remedy().is_some(),
            "a copy has something to do about it"
        );
        assert!(Link::Hardlinked.consequence().contains("no extra disk"));
        assert_eq!(
            Link::Hardlinked.remedy(),
            None,
            "nothing to fix when it worked"
        );
    }
}
