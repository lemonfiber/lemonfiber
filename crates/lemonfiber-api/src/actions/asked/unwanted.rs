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

/// One argument the carrier holds, the actions whose command has somewhere to put it,
/// and whether a request gave it.
pub(crate) struct Taken {
    /// Its name, as a request body spells it.
    pub(crate) name: &'static str,
    /// Every action whose command carries it.
    pub(crate) takers: &'static [&'static str],
    /// Whether a request gave it, rather than leaving it at what giving nothing reads as.
    pub(crate) given: fn(&Arguments) -> bool,
}

/// Every argument the carrier holds, against the actions that take it.
///
/// The one table both questions are asked of: whether an action was given something
/// its command has nowhere to put, and which arguments the contract lists an action as
/// taking.
pub(crate) const TAKEN: &[Taken] = &[
    Taken {
        name: "forms",
        takers: TAKES_FORMS,
        given: |arguments| !arguments.forms.is_empty(),
    },
    Taken {
        name: "services",
        takers: TAKES_SERVICES,
        given: |arguments| !arguments.services.is_empty(),
    },
    Taken {
        name: "wait",
        takers: TAKES_WAITING,
        given: |arguments| matches!(arguments.wait, Waiting::ForTheDownloads),
    },
    Taken {
        name: "service",
        takers: TAKES_SERVICE,
        given: |arguments| arguments.service.is_some(),
    },
    Taken {
        name: "key",
        takers: TAKES_SETTING,
        given: |arguments| arguments.key.is_some(),
    },
    Taken {
        name: "value",
        takers: TAKES_SETTING,
        given: |arguments| arguments.value.is_some(),
    },
    Taken {
        name: "preset",
        takers: TAKES_PRESET,
        given: |arguments| arguments.preset.is_some(),
    },
    Taken {
        name: "media_type",
        takers: TAKES_PRESET,
        given: |arguments| arguments.media_type.is_some(),
    },
    Taken {
        name: "archive",
        takers: TAKES_ARCHIVE,
        given: |arguments| arguments.archive.is_some(),
    },
    Taken {
        name: "at",
        takers: TAKES_RUN,
        given: |arguments| arguments.at.is_some(),
    },
    Taken {
        name: "name",
        takers: TAKES_NAME,
        given: |arguments| arguments.name.is_some(),
    },
    Taken {
        name: "libraries",
        takers: TAKES_ALLOWANCE,
        given: |arguments| !arguments.libraries.is_empty(),
    },
    Taken {
        name: "age_limit",
        takers: TAKES_ALLOWANCE,
        given: |arguments| arguments.age_limit.is_some(),
    },
    Taken {
        name: "unrated",
        takers: TAKES_ALLOWANCE,
        given: |arguments| arguments.unrated.is_some(),
    },
    Taken {
        name: "repoint",
        takers: TAKES_ARCHIVE,
        given: |arguments| arguments.repoint,
    },
    Taken {
        name: "write",
        takers: TAKES_BUNDLING,
        given: |arguments| arguments.write,
    },
    Taken {
        name: "logs",
        takers: TAKES_BUNDLING,
        given: |arguments| arguments.logs.is_some(),
    },
    Taken {
        name: "filenames",
        takers: TAKES_BUNDLING,
        given: |arguments| matches!(arguments.filenames, Filenames::Shown),
    },
    Taken {
        name: "reveal",
        takers: TAKES_BUNDLING,
        given: |arguments| !arguments.reveal.is_empty(),
    },
    Taken {
        name: "only",
        takers: TAKES_NARROWING,
        given: |arguments| arguments.only.is_some(),
    },
    Taken {
        name: "check",
        takers: TAKES_CHECK,
        given: |arguments| arguments.check.is_some(),
    },
    Taken {
        name: "disruptive",
        takers: TAKES_DISRUPTION,
        given: |arguments| arguments.disruptive.included(),
    },
    Taken {
        name: "offer",
        takers: TAKES_CONSENT,
        given: |arguments| arguments.offer.is_some(),
    },
    Taken {
        name: "agreed",
        takers: TAKES_AGREED,
        given: |arguments| !arguments.agreed.is_empty(),
    },
    Taken {
        name: "confirm",
        takers: TAKES_AGREEMENT,
        given: |arguments| arguments.confirm,
    },
    Taken {
        name: "item",
        takers: TAKES_ITEM,
        given: |arguments| arguments.item.is_some(),
    },
    Taken {
        name: "term",
        takers: TAKES_TERM,
        given: |arguments| arguments.term.is_some(),
    },
    Taken {
        name: "season",
        takers: TAKES_TERM,
        given: |arguments| arguments.season.is_some(),
    },
    Taken {
        name: "download",
        takers: TAKES_DOWNLOAD,
        given: |arguments| arguments.download.is_some(),
    },
    Taken {
        name: "policy",
        takers: TAKES_POLICY,
        given: |arguments| arguments.policy.is_some(),
    },
    Taken {
        name: "requests",
        takers: TAKES_POLICY,
        given: |arguments| arguments.requests.is_some(),
    },
    Taken {
        name: "days",
        takers: TAKES_POLICY,
        given: |arguments| arguments.days.is_some(),
    },
    Taken {
        name: "request",
        takers: TAKES_REQUEST,
        given: |arguments| arguments.request.is_some(),
    },
    Taken {
        name: "reason",
        takers: TAKES_REASON,
        given: |arguments| arguments.reason.is_some(),
    },
    Taken {
        name: "capability",
        takers: TAKES_CAPABILITY,
        given: |arguments| arguments.capability.is_some(),
    },
    Taken {
        name: "plugin",
        takers: TAKES_PLUGIN,
        given: |arguments| arguments.plugin.is_some(),
    },
    Taken {
        name: "source",
        takers: TAKES_SOURCE,
        given: |arguments| arguments.source.is_some(),
    },
    Taken {
        name: "approved",
        takers: TAKES_APPROVED,
        given: |arguments| !arguments.approved.is_empty(),
    },
    Taken {
        name: "inputs",
        takers: TAKES_APPROVED,
        given: |arguments| !arguments.inputs.is_empty(),
    },
    Taken {
        name: "tier",
        takers: TAKES_TIER,
        given: |arguments| arguments.tier.is_some(),
    },
    Taken {
        name: "kept",
        takers: TAKES_KEPT,
        given: |arguments| arguments.kept.is_some(),
    },
    Taken {
        name: "down",
        takers: TAKES_SHARING,
        given: |arguments| arguments.down.is_some(),
    },
    Taken {
        name: "up",
        takers: TAKES_SHARING,
        given: |arguments| arguments.up.is_some(),
    },
    Taken {
        name: "active",
        takers: TAKES_SHARING,
        given: |arguments| arguments.active.is_some(),
    },
    Taken {
        name: "line",
        takers: TAKES_SHARING,
        given: |arguments| arguments.line.is_some(),
    },
    Taken {
        name: "cap",
        takers: TAKES_SHARING,
        given: |arguments| arguments.cap.is_some(),
    },
    Taken {
        name: "exceeded",
        takers: TAKES_SHARING,
        given: |arguments| arguments.exceeded.is_some(),
    },
    Taken {
        name: "unrestricted_for",
        takers: TAKES_SHARING,
        given: |arguments| arguments.unrestricted_for.is_some(),
    },
];

/// The argument this action's command has nowhere to put, where one was given.
///
/// Only for a name this surface offers — a name it does not offer is absent before
/// it is anything else, and saying what its arguments should have been would be
/// answering about an action that does not exist.
pub(crate) fn unwanted(action: &str, given: &Arguments, offered: &[&str]) -> Option<Refused> {
    if !offered.contains(&action) {
        return None;
    }
    TAKEN
        .iter()
        .find(|taken| (taken.given)(given) && !taken.takers.contains(&action))
        .map(|taken| Refused::Unwanted {
            action: action.to_owned(),
            argument: taken.name.to_owned(),
        })
}
