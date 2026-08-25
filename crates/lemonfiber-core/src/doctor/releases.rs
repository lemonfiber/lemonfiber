//! Judging the chosen quality against what the indexers actually carry.
//!
//! A demanding preset can ask for releases that do not exist — 4K where the indexer
//! only has 1080p — and the operator is then left wondering why nothing downloads. The
//! easy misread is that the indexer is broken. This check tells the two apart: it
//! searches for something the operator is actually missing and reads the result against
//! the profile in force.
//!
//! The distinction is the honesty contract at work. A search that cannot run — the
//! service or its indexers unreachable — establishes nothing about releases, so it is
//! *unverified*, the indexer-failure case. A search that runs cleanly and finds
//! releases the profile would grab is a *pass*; one that finds releases the profile
//! wants none of, or finds nothing at all, is a *warning* the operator can act on by
//! easing the preset — never confused with the indexer having failed.
//!
//! The search hits the indexers live and spends one against the daily cap for every
//! service it searches, so it is only run where it was asked for. Nothing on the
//! operator's own machine is disturbed by it — what it spends belongs to the indexers,
//! which is a cost to state rather than a thing to take away.
//!
//! An ordinary run reports it unverified rather than skipped. Skipped is for a check
//! that does not apply, and this one applies perfectly well: television and film are
//! configured, content is wanted, and the only reason nothing was established is that
//! no search was spent on it. Calling that inapplicable would let a stack read as
//! healthy on the strength of a search nobody made.

use std::sync::Arc;

use async_trait::async_trait;

use super::{Category, Check, Finding, Verdict};
use crate::doctor::credentials::Target;
use crate::error::{Code, Problem, Remedy, Severity, State};
use crate::ports::filesystem::FileSystem;
use crate::ports::http::Http;
use crate::ports::service::{QualityReleases, ReleaseProbe};
use crate::recyclarr::Kind;

/// Raised when releases exist but the profile — the chosen quality included — wants
/// none of them.
pub const PRESET_UNMET: Code = Code::new("QUAL-2");

/// Raised when a clean search turns up nothing at all for wanted content.
pub const NONE_AVAILABLE: Code = Code::new("QUAL-3");

/// The id under which the check reports where there is nothing to run it against.
const NONE: &str = "services.releases";

/// What this check says when the operator has not asked for it.
///
/// It says what running it costs and for how long, which is what an operator has to
/// weigh before opting in: the searches are real, they are spent against the daily cap
/// the indexers hold the operator to, and the check is abandoned at its budget rather
/// than left to run.
///
/// A number of searches rather than something that stops working, because that is what
/// this actually costs. The stack carries on throughout.
fn not_asked_for() -> String {
    format!(
        "a live release search is only run where it is asked for: it spends one real search \
         per service against the indexers, counted towards the daily cap they hold the \
         operator to, {}",
        crate::doctor::disturbing_for(crate::doctor::CHECK_BUDGET)
    )
}

/// How to get a real answer, said the way the other opt-in check says it.
fn how_to_ask() -> Remedy {
    Remedy::new("Run the search when one can be spent against the indexers")
        .with_detail("lemonfiber doctor --only services.releases --disruptive")
}

/// Searches the resolution services for the operator's wanted content, so a preset that
/// asks for releases the indexers do not carry is told apart from an indexer failure.
pub struct ReleasesCheck {
    http: Arc<dyn Http>,
    filesystem: Arc<dyn FileSystem>,
    targets: Vec<Target>,
    disruptive: bool,
}

impl ReleasesCheck {
    /// A check over the resolution `targets` reachable through `http` and `filesystem`.
    /// It searches live only when `disruptive`, so an ordinary run does not query the
    /// indexer.
    #[must_use]
    pub fn new(
        http: Arc<dyn Http>,
        filesystem: Arc<dyn FileSystem>,
        targets: Vec<Target>,
        disruptive: bool,
    ) -> Self {
        Self {
            http,
            filesystem,
            targets,
            disruptive,
        }
    }

    /// Search one service and read the result into a verdict.
    async fn probe(&self, target: &Target, kind: Kind) -> Verdict {
        let media = kind.media_type();
        let Some(service) = target.open(&self.http, self.filesystem.as_ref()).await else {
            return Verdict::Skipped {
                reason: format!(
                    "{media} has not finished starting, so its releases could not be searched"
                ),
            };
        };
        match service.probe_releases(kind.release_id_param()).await {
            // The search could not run: nothing was learned about releases, only that
            // the service or its indexers could not be reached — the indexer-failure
            // case, kept as its own thing rather than dressed as a preset problem.
            Err(failure) => Verdict::Unverified {
                reason: format!(
                    "the {media} release search could not be run, so the preset could not be \
                     judged against what is available: {failure}"
                ),
                remedy: Remedy::new("Check the indexer is reachable and working, then check again"),
            },
            Ok(ReleaseProbe::NothingWanted) => Verdict::Skipped {
                reason: format!(
                    "nothing is wanted in {media}, so there is nothing to search for yet"
                ),
            },
            Ok(ReleaseProbe::Matching) => Verdict::Pass {
                note: Some(format!(
                    "releases meeting the chosen quality are available for {media}"
                )),
            },
            Ok(ReleaseProbe::NoneMatch) => Verdict::Warn(unmet(media)),
            Ok(ReleaseProbe::NoneFound) => Verdict::Warn(none_available(media)),
        }
    }
}

#[async_trait]
impl Check for ReleasesCheck {
    fn category(&self) -> Category {
        Category::Services
    }

    async fn run(&self) -> Vec<Finding> {
        let resolution: Vec<(&Target, Kind)> = self
            .targets
            .iter()
            .filter_map(|target| Kind::for_section(&target.id).map(|kind| (target, kind)))
            .collect();

        if resolution.is_empty() {
            return vec![Finding::in_category(
                Category::Services,
                NONE,
                "Releases for the chosen quality",
                Verdict::Skipped {
                    reason:
                        "no television or film service is configured, so there are no releases \
                             to check"
                            .to_owned(),
                },
            )];
        }

        if !self.disruptive {
            return vec![Finding::in_category(
                Category::Services,
                NONE,
                "Releases for the chosen quality",
                Verdict::Unverified {
                    reason: not_asked_for(),
                    remedy: how_to_ask(),
                },
            )];
        }

        let mut findings = Vec::new();
        for (target, kind) in resolution {
            let verdict = self.probe(target, kind).await;
            findings.push(
                Finding::in_category(
                    Category::Services,
                    &format!("services.releases.{}", target.id),
                    &format!("Releases for {} at the chosen quality", kind.media_type()),
                    verdict,
                )
                // Named, so a service that cannot search because the indexer beneath it
                // is down reads as one problem rather than two — and so a failure here
                // is quoted with what the service itself said about it.
                .about(&target.id),
            );
        }
        findings
    }
}

/// Releases came back but the profile wants none of them: they exist, they just do not
/// meet the chosen quality, so the preset — not the indexer — is what leaves the search
/// empty of anything grabbable.
fn unmet(media: &str) -> Problem {
    Problem::new(
        PRESET_UNMET,
        Severity::Warning,
        format!("The chosen quality finds no {media} releases"),
        format!(
            "Releases for wanted {media} are available, but the chosen quality preset wants none \
             of them — what is out there does not meet it. The indexer is working; the preset is \
             simply stricter than what can be found. Nothing is broken, and the wanted content \
             stays wanted until a matching release appears."
        ),
        Remedy::new(format!(
            "Choose a less demanding quality preset for {media}, or wait for a matching release \
             to appear"
        )),
    )
    .in_state(State::Guided)
}

/// A clean search turned up nothing at all — distinct from the indexer failing, which
/// would not have answered.
fn none_available(media: &str) -> Problem {
    Problem::new(
        NONE_AVAILABLE,
        Severity::Warning,
        format!("No {media} releases were found for wanted content"),
        format!(
            "The indexer answered but returned no releases at all for wanted {media} — few or \
             none are available right now. This is not an indexer failure, which would not have \
             answered; there is simply nothing to grab yet."
        ),
        Remedy::new(
            "Check the indexer carries this content, or wait for releases to appear; no action is \
             needed if the content is merely not out yet",
        ),
    )
    .in_state(State::Guided)
}
