//! Whether an action was given an argument its command has nowhere to put.
//!
//! Apart from the lists it reads, in [`super::takers`], because the two grow for
//! different reasons: a list is a statement about one argument, and this is the one
//! table that holds every argument the carrier knows against them.

use super::takers::{
    TAKES_AGREED, TAKES_AGREEMENT, TAKES_ALLOWANCE, TAKES_APPROVED, TAKES_ARCHIVE, TAKES_BUNDLING,
    TAKES_CAPABILITY, TAKES_CHECK, TAKES_CONSENT, TAKES_DISRUPTION, TAKES_DOWNLOAD, TAKES_FORMS,
    TAKES_ITEM, TAKES_KEPT, TAKES_NAME, TAKES_NARROWING, TAKES_PLUGIN, TAKES_POLICY, TAKES_PRESET,
    TAKES_REASON, TAKES_REQUEST, TAKES_RUN, TAKES_SERVICE, TAKES_SERVICES, TAKES_SETTING,
    TAKES_SHARING, TAKES_SOURCE, TAKES_TERM, TAKES_TIER, TAKES_WAITING,
};
use super::Arguments;
use crate::actions::Refused;
use lemonfiber_core::app::Waiting;
use lemonfiber_core::bundle::Filenames;

/// The argument this action's command has nowhere to put, where one was given.
///
/// Only for a name this surface offers — a name it does not offer is absent before
/// it is anything else, and saying what its arguments should have been would be
/// answering about an action that does not exist.
pub(crate) fn unwanted(action: &str, given: &Arguments, offered: &[&str]) -> Option<Refused> {
    let carried: [(&str, bool, &[&str]); 47] = [
        ("forms", !given.forms.is_empty(), TAKES_FORMS),
        ("services", !given.services.is_empty(), TAKES_SERVICES),
        (
            "wait",
            matches!(given.wait, Waiting::ForTheDownloads),
            TAKES_WAITING,
        ),
        ("service", given.service.is_some(), TAKES_SERVICE),
        ("key", given.key.is_some(), TAKES_SETTING),
        ("value", given.value.is_some(), TAKES_SETTING),
        ("preset", given.preset.is_some(), TAKES_PRESET),
        ("media_type", given.media_type.is_some(), TAKES_PRESET),
        ("archive", given.archive.is_some(), TAKES_ARCHIVE),
        ("at", given.at.is_some(), TAKES_RUN),
        ("name", given.name.is_some(), TAKES_NAME),
        ("libraries", !given.libraries.is_empty(), TAKES_ALLOWANCE),
        ("age_limit", given.age_limit.is_some(), TAKES_ALLOWANCE),
        ("unrated", given.unrated.is_some(), TAKES_ALLOWANCE),
        ("repoint", given.repoint, TAKES_ARCHIVE),
        ("write", given.write, TAKES_BUNDLING),
        ("logs", given.logs.is_some(), TAKES_BUNDLING),
        (
            "filenames",
            matches!(given.filenames, Filenames::Shown),
            TAKES_BUNDLING,
        ),
        ("reveal", !given.reveal.is_empty(), TAKES_BUNDLING),
        ("only", given.only.is_some(), TAKES_NARROWING),
        ("check", given.check.is_some(), TAKES_CHECK),
        ("disruptive", given.disruptive.included(), TAKES_DISRUPTION),
        ("offer", given.offer.is_some(), TAKES_CONSENT),
        ("agreed", !given.agreed.is_empty(), TAKES_AGREED),
        ("confirm", given.confirm, TAKES_AGREEMENT),
        ("item", given.item.is_some(), TAKES_ITEM),
        ("term", given.term.is_some(), TAKES_TERM),
        ("season", given.season.is_some(), TAKES_TERM),
        ("download", given.download.is_some(), TAKES_DOWNLOAD),
        ("policy", given.policy.is_some(), TAKES_POLICY),
        ("requests", given.requests.is_some(), TAKES_POLICY),
        ("days", given.days.is_some(), TAKES_POLICY),
        ("request", given.request.is_some(), TAKES_REQUEST),
        ("reason", given.reason.is_some(), TAKES_REASON),
        ("capability", given.capability.is_some(), TAKES_CAPABILITY),
        ("plugin", given.plugin.is_some(), TAKES_PLUGIN),
        ("source", given.source.is_some(), TAKES_SOURCE),
        ("approved", !given.approved.is_empty(), TAKES_APPROVED),
        ("tier", given.tier.is_some(), TAKES_TIER),
        ("kept", given.kept.is_some(), TAKES_KEPT),
        ("down", given.down.is_some(), TAKES_SHARING),
        ("up", given.up.is_some(), TAKES_SHARING),
        ("active", given.active.is_some(), TAKES_SHARING),
        ("line", given.line.is_some(), TAKES_SHARING),
        ("cap", given.cap.is_some(), TAKES_SHARING),
        ("exceeded", given.exceeded.is_some(), TAKES_SHARING),
        (
            "unrestricted_for",
            given.unrestricted_for.is_some(),
            TAKES_SHARING,
        ),
    ];
    if !offered.contains(&action) {
        return None;
    }
    carried
        .into_iter()
        .find(|(_, was_given, takers)| *was_given && !takers.contains(&action))
        .map(|(argument, _, _)| Refused::Unwanted {
            action: action.to_owned(),
            argument: argument.to_owned(),
        })
}
