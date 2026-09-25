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
mod tests;
