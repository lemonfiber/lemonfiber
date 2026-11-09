//! The asks for music and books the core keeps, on the household list.

use std::collections::BTreeMap;

use crate::app::Ctx;
use crate::model::HouseholdReport;

/// Each ask for music or books the core keeps, listed under the member who made it after
/// their other requests, the newest first.
pub(super) fn listed(
    ctx: &Ctx,
    report: &mut HouseholdReport,
    names: &BTreeMap<String, String>,
    reasons: &crate::asking::Reasons,
) {
    let now = ctx.seams.clock.now();
    for ask in crate::collected::run::every(ctx).iter().rev() {
        let Some(name) = names.get(&ask.member) else {
            continue;
        };
        if let Some(member) = report.members.iter_mut().find(|one| &one.name == name) {
            member
                .requests
                .push(crate::collected::run::listed(ask, reasons, now));
        }
    }
}
