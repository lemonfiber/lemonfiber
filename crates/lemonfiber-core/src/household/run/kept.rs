//! The asks the core keeps itself, on the household list.

use std::collections::BTreeMap;

use crate::app::Ctx;
use crate::model::{HouseholdReport, MemberRequest};

/// Each ask the core keeps, listed under the member who made it after their other
/// requests, the newest first: those waiting to be filed as the request service's owner,
/// those kept beside other members' requests, then those for music or books, the ones the
/// house now holds read back from their curator first. Each line then names the other
/// members holding a line of the same reference.
pub(super) async fn listed(
    ctx: &Ctx,
    report: &mut HouseholdReport,
    names: &BTreeMap<String, String>,
    reasons: &crate::asking::Reasons,
) {
    crate::collected::run::arrivals(ctx).await;
    let now = ctx.stamp();
    let owners = super::as_owner::kept(ctx);
    let quality = crate::quality::run::recorded_selection(ctx);
    let curated = crate::collected::run::every(ctx);
    let beside = owners
        .beside
        .iter()
        .rev()
        .filter(|ask| ask.standing != crate::collected::store::Standing::Sent)
        .map(|ask| (ask.member.as_str(), ask.line(&quality, now)));
    let lines = super::as_owner::approving::listed(&owners, reasons, &quality, now)
        .chain(beside)
        .chain(curated.iter().rev().map(|ask| {
            (
                ask.member.as_str(),
                crate::collected::run::listed(ask, reasons, now),
            )
        }));
    for (member, line) in lines {
        under(report, names, member, line);
    }
    also_asked(report);
}

/// Name on each line of `report` the other members holding a line of the same reference:
/// one ask, ruled on once for all of them.
pub(super) fn also_asked(report: &mut HouseholdReport) {
    let mut by_reference: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for member in &report.members {
        for line in &member.requests {
            let names = by_reference.entry(line.id.clone()).or_default();
            if !names.contains(&member.name) {
                names.push(member.name.clone());
            }
        }
    }
    for member in &mut report.members {
        for line in &mut member.requests {
            line.also_asked = by_reference
                .get(&line.id)
                .into_iter()
                .flatten()
                .filter(|name| **name != member.name)
                .cloned()
                .collect();
        }
    }
}

/// `line` among the requests of `member`, where the household holds them.
fn under(
    report: &mut HouseholdReport,
    names: &BTreeMap<String, String>,
    member: &str,
    line: MemberRequest,
) {
    let Some(name) = names.get(member) else {
        return;
    };
    if let Some(held) = report.members.iter_mut().find(|one| &one.name == name) {
        held.requests.push(line);
    }
}
