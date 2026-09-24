//! What each installed plugin is doing, beneath its services in the listing.
//!
//! The one read an operator makes when something about the stack surprises them, so
//! it says everything the record knows about a plugin in one place: where it came from
//! and whether anybody vouched for it, what it claims and fills, what it added and what
//! it stands in for, what it may change, where it may reach, what it holds, and when it
//! arrived. A line is left out where there is nothing to say — except where it came
//! from, whether it was reviewed and when it was installed, which are said for every
//! plugin because *not recorded* is itself the answer an operator needs.

use lemonfiber_core::plugin::{Installed, Substituted};

use super::super::super::Lines;

/// Everything the record says a plugin is doing.
pub(super) fn provenance(one: &Installed, substituted: &[Substituted]) -> Lines {
    let mut lines = Lines::default();
    let declared = &one.declared;
    lines.put(format!(
        "    from       {} — {}",
        or_unrecorded(&one.from),
        if declared.reviewed {
            "reviewed"
        } else {
            "unreviewed: nobody vouched for it"
        }
    ));
    lines.put(format!(
        "    installed  {}",
        if one.installed_at.is_empty() {
            "at a moment the record does not hold".to_owned()
        } else {
            format!("at {} (seconds since the epoch)", one.installed_at)
        }
    ));
    if !declared.upstream.is_empty() {
        lines.put(format!(
            "    upstream   {} ({})",
            declared.upstream,
            or_unrecorded(&declared.license)
        ));
    }
    listed(&mut lines, "claims", &declared.claims);
    listed(&mut lines, "fills", &one.provides);
    if !one.contributions.is_empty() {
        lines.put(format!(
            "    adds       {} row{} to registers lemonfiber runs",
            one.contributions.len(),
            lemonfiber_core::plural::s(one.contributions.len())
        ));
    }
    for stands in substituted
        .iter()
        .filter(|stands| stands.plugin == one.plugin)
    {
        lines.put(format!(
            "    stands in  {} fills {}, because you chose it",
            stands.service, stands.capability
        ));
    }
    for change in &declared.overrides {
        lines.put(format!(
            "    may change {} — {}",
            change.setting, change.why
        ));
    }
    listed(&mut lines, "reaches", &declared.reaches);
    for secret in &declared.secrets {
        lines.put(format!(
            "    holds      {} for {} — {}",
            secret.id, secret.of, secret.why
        ));
    }
    lines
}

/// One line naming everything in a list, or nothing where the list is empty.
fn listed(lines: &mut Lines, label: &str, named: &[String]) {
    if !named.is_empty() {
        lines.put(format!("    {label:<10} {}", named.join(", ")));
    }
}

/// A recorded value, or that the record does not hold one.
fn or_unrecorded(value: &str) -> &str {
    if value.is_empty() {
        "not recorded"
    } else {
        value
    }
}
