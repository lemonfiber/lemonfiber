//! Whether an existing layout can hold a hardlink, and what it costs where it cannot.
//!
//! A hardlink is a second name for the same file, so importing a finished download
//! costs nothing and the file is never copied. It only works within one filesystem, and
//! some filesystems cannot do it at all. A layout that keeps downloads on one and the
//! library on another therefore turns every import into a copy — quietly, and for as
//! long as the setup lasts.
//!
//! Reported and never acted on. The layout is the operator's, the library in it is the
//! part no backup here covers, and correctness does not outrank their data: this says
//! what it costs and what would fix it, and leaves the doing to them.

use std::collections::BTreeMap;
use std::path::PathBuf;

use crate::bytes::humanize;
use crate::model::LinkingReport;
use crate::ports::filesystem::StorageFacts;

/// What the existing layout costs, or nothing where it costs nothing.
///
/// Nothing is the ordinary answer on a setup that already keeps its data under one
/// mount, and saying nothing about it is right: an operator whose layout is fine does
/// not need a paragraph telling them so.
#[must_use]
pub fn linking(seen: &[(PathBuf, StorageFacts)]) -> Option<LinkingReport> {
    if seen.is_empty() {
        return None;
    }

    let mut mounts: BTreeMap<PathBuf, &StorageFacts> = BTreeMap::new();
    for (_, facts) in seen {
        mounts.insert(facts.point.clone(), facts);
    }

    let cannot: Vec<(&PathBuf, &StorageFacts)> = mounts
        .iter()
        .filter(|(_, facts)| facts.kind.limitation().is_some())
        .map(|(point, facts)| (point, *facts))
        .collect();

    let because = if let Some((point, facts)) = cannot.first() {
        let limitation = facts
            .kind
            .limitation()
            .unwrap_or("it cannot hold a hardlink");
        format!(
            "{} is {}, and {limitation}",
            point.display(),
            facts.kind.label()
        )
    } else if mounts.len() > 1 {
        let across: Vec<String> = mounts
            .keys()
            .map(|point| point.display().to_string())
            .collect();
        format!(
            "this setup keeps its data on {} separate filesystems — {} — and a hardlink \
             cannot reach from one to another",
            mounts.len(),
            across.join(", ")
        )
    } else {
        return None;
    };

    let room = mounts
        .values()
        .map(|facts| facts.available)
        .min()
        .unwrap_or_default();

    Some(LinkingReport {
        links: false,
        because,
        cost: format!(
            "every import copies the file instead of naming it twice, so a finished \
             download and its imported copy both take room until the download is \
             removed — and the library grows by the size of everything in it a second \
             time. There is {} free where there is least of it.",
            humanize(room)
        ),
        remedy: "keeping downloads and the library under one filesystem lets imports be \
                 hardlinks again, which costs no room at all"
            .to_owned(),
        forced: false,
        filesystems: mounts.values().map(|facts| facts.kind.label()).collect(),
    })
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::linking;
    use crate::ports::filesystem::{FsKind, StorageFacts};

    fn facts(point: &str, kind: FsKind) -> StorageFacts {
        StorageFacts {
            point: PathBuf::from(point),
            kind,
            removable: false,
            available: 100_000_000_000,
            total: 500_000_000_000,
        }
    }

    fn under(point: &str, kind: FsKind) -> (PathBuf, StorageFacts) {
        (PathBuf::from(format!("{point}/data")), facts(point, kind))
    }

    fn linking_kind() -> FsKind {
        FsKind::Linking("apfs".to_owned())
    }

    #[test]
    fn a_layout_already_under_one_mount_costs_nothing_and_is_not_mentioned() {
        let seen = [under("/srv", linking_kind())];
        assert!(linking(&seen).is_none());
    }

    #[test]
    fn nothing_mounted_says_nothing() {
        assert!(linking(&[]).is_none());
    }

    #[test]
    fn two_filesystems_cannot_hardlink_between_them() {
        let seen = [under("/srv", linking_kind()), under("/mnt", linking_kind())];
        let found = linking(&seen);
        let said = found.as_ref().map(|read| read.because.clone());
        assert_eq!(
            found.as_ref().map(|read| read.links),
            Some(false),
            "{found:?}"
        );
        let said = said.unwrap_or_default();
        assert!(said.contains("/mnt") && said.contains("/srv"), "{said}");
    }

    #[test]
    fn a_filesystem_that_cannot_link_is_named_with_the_reason_it_cannot() {
        let seen = [under("/srv", FsKind::ExFat)];
        let found = linking(&seen);
        assert_eq!(
            found.as_ref().map(|read| read.links),
            Some(false),
            "{found:?}"
        );
        let said = found.map(|read| read.because).unwrap_or_default();
        assert!(said.contains("exFAT"), "{said}");
    }

    #[test]
    fn the_reason_a_filesystem_cannot_outranks_there_being_several() {
        let seen = [under("/srv", FsKind::ExFat), under("/mnt", linking_kind())];
        let said = linking(&seen).map(|read| read.because).unwrap_or_default();
        assert!(said.contains("exFAT"), "{said}");
    }

    #[test]
    fn the_cost_is_quantified_rather_than_described() {
        let seen = [under("/srv", linking_kind()), under("/mnt", linking_kind())];
        let cost = linking(&seen).map(|read| read.cost).unwrap_or_default();
        assert!(cost.contains("free") && cost.contains("copies"), "{cost}");
    }

    #[test]
    fn a_remedy_is_offered_and_never_forced() {
        let seen = [under("/srv", linking_kind()), under("/mnt", linking_kind())];
        let found = linking(&seen);
        assert_eq!(
            found.as_ref().map(|read| read.forced),
            Some(false),
            "the operator decides: {found:?}"
        );
        let remedy = found.map(|read| read.remedy).unwrap_or_default();
        assert!(remedy.contains("one filesystem"), "{remedy}");
    }

    #[test]
    fn the_room_reported_is_the_least_there_is_anywhere() {
        let mut tight = facts("/mnt", linking_kind());
        tight.available = 1;
        let seen = [
            under("/srv", linking_kind()),
            (PathBuf::from("/mnt/x"), tight),
        ];
        let cost = linking(&seen).map(|read| read.cost).unwrap_or_default();
        assert!(cost.contains("1 B"), "{cost}");
    }

    #[test]
    fn two_paths_on_one_mount_are_one_filesystem() {
        let seen = [
            (PathBuf::from("/srv/one"), facts("/srv", linking_kind())),
            (PathBuf::from("/srv/two"), facts("/srv", linking_kind())),
        ];
        assert!(linking(&seen).is_none());
    }
}
