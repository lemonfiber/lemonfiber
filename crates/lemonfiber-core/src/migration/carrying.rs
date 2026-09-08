//! What taking an existing stack over would come to, and what nothing carries across.
//!
//! An \*arr migrates its database forward on first start, and the binary that did so is
//! the oldest that can open it afterwards. So the version standing on an existing
//! project decides what adopting it means, and a later version already here is a door
//! lemonfiber cannot walk back through.

use crate::model::{CarryingReport, UnsupportedReport};
use crate::ports::docker::Image;

use super::version::{self, Standing};
use super::{image, Ours};

/// What no migration carries across, in any mode.
///
/// Stated rather than discovered, because these are the things an operator finds
/// missing weeks later and has no way to connect back to the day they migrated. A
/// migration that implied it took everything would be the more comfortable claim and
/// the one that costs them that afternoon.
const NEVER_CARRIED: [(&str, &str); 4] = [
    (
        "custom formats you wrote yourself",
        "they are scoring rules particular to your library, and lemonfiber sets its own from the \
         quality preset you choose",
    ),
    (
        "per-indexer tuning",
        "seed ratios, limits and priorities are set against indexers lemonfiber does not know you \
         hold accounts with",
    ),
    (
        "scripts hooked into an *arr's events",
        "a connect script runs a program on your machine, and carrying one across would run \
         somebody's program somewhere it was never pointed at",
    ),
    (
        "watch histories and play state",
        "they live in the media server's own database rather than in the wiring lemonfiber sets \
         up, and nothing here reads them",
    ),
];

/// What no migration carries across, whatever mode it runs in.
#[must_use]
pub fn not_carried() -> Vec<UnsupportedReport> {
    NEVER_CARRIED
        .iter()
        .map(|(what, because)| UnsupportedReport {
            what: (*what).to_owned(),
            because: (*because).to_owned(),
        })
        .collect()
}

/// What adopting each service lemonfiber recognises would come to.
#[must_use]
pub fn carrying(images: &[Image], project: &str, ours: &[Ours]) -> Vec<CarryingReport> {
    let mut found: Vec<CarryingReport> = ours
        .iter()
        .filter_map(|one| {
            let existing = image::standing_on(images, project, &one.image)?;
            Some(verdict(one, &existing))
        })
        .collect();
    found.sort_by(|one, two| one.service.cmp(&two.service));
    found
}

/// What adopting one service would come to, given the version already here.
fn verdict(ours: &Ours, existing: &str) -> CarryingReport {
    let (verdict, because, backup_first, refused) = match version::against(existing, &ours.tag) {
        Standing::Same => (
            "same",
            "the version already here is the one lemonfiber runs, so its database is opened \
             exactly as it stands"
                .to_owned(),
            false,
            false,
        ),
        Standing::Earlier => (
            "upgrade",
            format!(
                "lemonfiber runs {}, which upgrades this database on first start and is a step \
                 nothing walks back — so it is backed up before anything opens it",
                ours.tag
            ),
            true,
            false,
        ),
        Standing::Later => (
            "downgrade",
            format!(
                "this database has been through {existing}, and {} cannot open it afterwards — \
                 lemonfiber will not try, because the attempt is what damages it",
                ours.tag
            ),
            false,
            true,
        ),
        Standing::Untellable => (
            "cannot tell",
            format!(
                "neither {existing} nor {} reads as a version, so which came first cannot be \
                 told from the tags — it is backed up first rather than assumed safe",
                ours.tag
            ),
            true,
            false,
        ),
    };

    CarryingReport {
        service: ours.service.clone(),
        existing: existing.to_owned(),
        ours: ours.tag.clone(),
        verdict: verdict.to_owned(),
        because,
        backup_first,
        refused,
    }
}

#[cfg(test)]
mod tests {
    use super::{carrying, not_carried};
    use crate::migration::tests::{image, ours};

    /// What adopting one service came to, as (verdict, backup first, refused).
    fn taking(existing: &str, pinned: &str) -> Option<(String, bool, bool)> {
        let images = [image(
            &[&format!("linuxserver/sonarr:{existing}")],
            &["media"],
        )];
        let ours = [ours("sonarr", "linuxserver/sonarr", pinned, None)];
        carrying(&images, "media", &ours)
            .first()
            .map(|read| (read.verdict.clone(), read.backup_first, read.refused))
    }

    #[test]
    fn the_version_already_here_being_ours_is_taken_over_as_it_stands() {
        assert_eq!(
            taking("4.0.1", "4.0.1"),
            Some(("same".to_owned(), false, false))
        );
    }

    #[test]
    fn an_older_database_is_upgraded_and_backed_up_before_anything_opens_it() {
        assert_eq!(
            taking("4.0.1", "4.0.2"),
            Some(("upgrade".to_owned(), true, false))
        );
    }

    #[test]
    fn a_database_newer_than_ours_is_refused_rather_than_attempted() {
        assert_eq!(
            taking("4.0.2", "4.0.1"),
            Some(("downgrade".to_owned(), false, true))
        );
    }

    #[test]
    fn versions_that_cannot_be_ordered_are_backed_up_rather_than_assumed_safe() {
        assert_eq!(
            taking("latest", "4.0.1"),
            Some(("cannot tell".to_owned(), true, false))
        );
    }

    #[test]
    fn a_refusal_names_both_versions_so_it_can_be_argued_with() {
        let images = [image(&["linuxserver/sonarr:4.0.9"], &["media"])];
        let ours = [ours("sonarr", "linuxserver/sonarr", "4.0.1", None)];
        let said = carrying(&images, "media", &ours)
            .first()
            .map(|read| read.because.clone())
            .unwrap_or_default();
        assert!(said.contains("4.0.9"), "{said}");
        assert!(said.contains("4.0.1"), "{said}");
    }

    #[test]
    fn a_service_we_run_that_is_not_on_this_project_is_not_reported_as_carried() {
        let images = [image(&["linuxserver/sonarr:4.0.1"], &["other"])];
        let ours = [ours("sonarr", "linuxserver/sonarr", "4.0.1", None)];
        assert!(carrying(&images, "media", &ours).is_empty());
    }

    #[test]
    fn what_would_be_carried_reads_in_a_settled_order() {
        let images = [
            image(&["linuxserver/sonarr:4.0.1"], &["media"]),
            image(&["linuxserver/radarr:5.0.1"], &["media"]),
        ];
        let ours = [
            ours("sonarr", "linuxserver/sonarr", "4.0.1", None),
            ours("radarr", "linuxserver/radarr", "5.0.1", None),
        ];
        let order: Vec<String> = carrying(&images, "media", &ours)
            .into_iter()
            .map(|read| read.service)
            .collect();
        assert_eq!(order, vec!["radarr".to_owned(), "sonarr".to_owned()]);
    }

    #[test]
    fn what_never_carries_across_is_named_rather_than_left_to_be_discovered() {
        let named = not_carried();
        assert!(named.len() >= 4, "{named:?}");
        let all: String = named.iter().map(|item| item.what.clone()).collect();
        assert!(all.contains("custom formats"), "{all}");
    }
}
