//! What both halves of the action tests are built from.
//!
//! A shared module rather than a copy in each file: these settle what an action is
//! given and what each argument looks like, and two copies of that would answer the
//! same question differently the first time one of them was updated.
//!
//! `tests/` compiles each file as its own crate and this module into both of them,
//! so anything one half uses is unused in the other — a property of how integration
//! tests are built rather than of what is written here. Both lints are suppressed
//! for that and no other reason; the alternative is a copy per half, which is the
//! thing this module exists to avoid.
#![allow(dead_code, unused_imports)]

pub(crate) use axum::body::to_bytes;
pub(crate) use axum::http::{header, StatusCode};
pub(crate) use lemonfiber_api::actions;
pub(crate) use lemonfiber_api::actions::{
    answering, declined, named, Answering, Arguments, Disturbing, Refused, OFFERED, TAKES_AGREED,
    TAKES_AGREEMENT, TAKES_ALLOWANCE, TAKES_ARCHIVE, TAKES_BUNDLING, TAKES_CHECK, TAKES_CONSENT,
    TAKES_DISRUPTION, TAKES_DOWNLOAD, TAKES_FORMS, TAKES_ITEM, TAKES_NAME, TAKES_NARROWING,
    TAKES_POLICY, TAKES_PRESET, TAKES_REASON, TAKES_REQUEST, TAKES_SERVICE, TAKES_SERVICES,
    TAKES_SETTING, TAKES_TERM, TAKES_WAITING,
};
pub(crate) use lemonfiber_api::events::live::Live;
pub(crate) use lemonfiber_api::guard::Token;
pub(crate) use lemonfiber_api::jobs::Jobs;
pub(crate) use lemonfiber_api::router::Serving;
pub(crate) use lemonfiber_core::app::bundle::Wanted;
pub(crate) use lemonfiber_core::app::repair::Consent;
pub(crate) use lemonfiber_core::app::restore::{Consent as RestoreConsent, Kept};
pub(crate) use lemonfiber_core::app::{
    Answer, Chosen, Command, Ctx, Decision, QualityAction, Waiting,
};
pub(crate) use lemonfiber_core::asking::Policy;
pub(crate) use lemonfiber_core::bundle::Filenames;
pub(crate) use lemonfiber_core::config::Settings;
pub(crate) use lemonfiber_core::doctor::Narrowing;
pub(crate) use lemonfiber_core::platform::Environment;
pub(crate) use lemonfiber_core::quality::Preset;
pub(crate) use lemonfiber_fixtures::ports::{Chance, Idle, Stopped};
pub(crate) use std::collections::BTreeSet;
pub(crate) use std::sync::Arc;

/// Nothing named, which is what most actions are asked with.
pub(crate) fn nothing() -> Arguments {
    Arguments::default()
}

/// One form named, which is what most of the rest are asked with.
pub(crate) fn naming(form: &str) -> Arguments {
    Arguments {
        forms: vec![form.to_owned()],
        ..Arguments::default()
    }
}

/// What an action came to, or nothing where it was refused.
pub(crate) fn command(action: &str, given: Arguments) -> Option<Command> {
    named(action, given).ok()
}

/// Why an action was refused, or nothing where it was not.
pub(crate) fn refusal(action: &str, given: Arguments) -> Option<Refused> {
    named(action, given).err()
}

// ── What the table says each action takes ────────────────────────────────────

/// Exactly what the table says this action takes, and nothing else.
///
/// Built from the same lists the translation refuses by, so the arguments under
/// test and the arguments the rule governs cannot come apart. A carrier holding
/// everything any action could need is what hid this: the one sweep shaped to
/// catch a dropped argument named `forms`, `key`, `value` and `preset`, and never
/// sent `services` or `confirm`.
pub(crate) fn exactly_what(action: &str) -> Arguments {
    let takes = |takers: &[&str]| takers.contains(&action);
    Arguments {
        forms: if takes(TAKES_FORMS) {
            vec!["tv".to_owned()]
        } else {
            Vec::new()
        },
        // Everything that takes services, apart from the one that takes a wait as
        // well: those two name different requests, so an action given both would
        // be given a pair no run can carry. The teardown is handed its wait and
        // the stop of named services beside it is left to the tests that are
        // about that request.
        services: if takes(TAKES_SERVICES) && !takes(TAKES_WAITING) {
            vec!["sonarr".to_owned()]
        } else {
            Vec::new()
        },
        wait: takes(TAKES_WAITING).into(),
        service: takes(TAKES_SERVICE).then(|| "sonarr".to_owned()),
        key: takes(TAKES_SETTING).then(|| "DATA_ROOT".to_owned()),
        value: takes(TAKES_SETTING).then(|| "/srv".to_owned()),
        preset: takes(TAKES_PRESET).then(|| "balanced".to_owned()),
        media_type: takes(TAKES_PRESET).then(|| "tv".to_owned()),
        archive: takes(TAKES_ARCHIVE).then(|| ARCHIVE.to_owned()),
        name: takes(TAKES_NAME).then(|| "ana".to_owned()),
        repoint: takes(TAKES_ARCHIVE),
        write: takes(TAKES_BUNDLING),
        logs: takes(TAKES_BUNDLING).then_some(LOGS),
        filenames: takes(TAKES_BUNDLING).into(),
        reveal: if takes(TAKES_BUNDLING) {
            vec!["INDEXER_KEY".to_owned()]
        } else {
            Vec::new()
        },
        only: takes(TAKES_NARROWING).then(|| NARROWED.to_owned()),
        check: takes(TAKES_CHECK).then(|| WARNED.to_owned()),
        disruptive: takes(TAKES_DISRUPTION).into(),
        offer: takes(TAKES_CONSENT).then(|| OFFER.to_owned()),
        agreed: if takes(TAKES_AGREED) {
            vec![WARNED.to_owned()]
        } else {
            Vec::new()
        },
        confirm: takes(TAKES_AGREEMENT),
        item: takes(TAKES_ITEM).then(|| ITEM.to_owned()),
        libraries: if takes(TAKES_ALLOWANCE) {
            vec![LIBRARY.to_owned()]
        } else {
            Vec::new()
        },
        age_limit: takes(TAKES_ALLOWANCE).then_some(AGE),
        unrated: takes(TAKES_ALLOWANCE).then(|| UNRATED.to_owned()),
        term: takes(TAKES_TERM).then(|| FOLLOWED.to_owned()),
        season: takes(TAKES_TERM).then_some(SEASON),
        download: takes(TAKES_DOWNLOAD).then(|| DOWNLOAD.to_owned()),
        policy: takes(TAKES_POLICY).then(|| POLICY.to_owned()),
        // Both halves or neither: one alone is half a limit, which the translation
        // refuses — so an action given only one would be refused for the argument it
        // was *not* given, and every sweep over it would read as something else.
        requests: takes(TAKES_POLICY).then_some(ALLOWED),
        days: takes(TAKES_POLICY).then_some(PERIOD),
        request: takes(TAKES_REQUEST).then_some(WAITING),
        reason: takes(TAKES_REASON).then(|| REASON.to_owned()),
    }
}

/// A backup name, as one is written under.
pub(crate) const ARCHIVE: &str = "lemonfiber-full-1700000000.tar.gz";

/// A log window that is not the one a bundle takes when nothing is asked for, so a
/// command carrying the default cannot pass for one carrying what was given.
pub(crate) const LOGS: u32 = 12;

/// One thing to walk end to end, named the way somebody would say it.
pub(crate) const ITEM: &str = "Sintel";

/// One show to follow, named the way somebody would say it. Not the same words as the
/// thing a walk is given, so a command carrying one cannot pass for one carrying the
/// other — which is the mistake the two arguments are apart to prevent.
pub(crate) const FOLLOWED: &str = "The Expanse";

/// A season to narrow a followed show to, and not the first, so a command that dropped
/// it and defaulted cannot pass for one that carried it.
pub(crate) const SEASON: u32 = 2;

/// A check by the name a finding gives it — the one a warning is answered for, and
/// the one a repair is agreed to for.
pub(crate) const WARNED: &str = "vpn.unprotected";

/// A check to narrow a run to. Not the one a warning is answered for, so a command
/// carrying the narrowing cannot pass for one carrying the accepted check.
pub(crate) const NARROWED: &str = "vpn.killswitch";

/// An offer, as one names itself. Not one any real offer would produce, so consent
/// carrying it can only have come from here.
pub(crate) const OFFER: &str = "0f0f0f0f";

/// One library an invitation may be narrowed to, named the way a media server names
/// one rather than by the identifier it tells them apart by.
pub(crate) const LIBRARY: &str = "Films";

/// One age a limit may be given as. Not nought, so a command carrying a limit cannot
/// pass for one carrying none.
pub(crate) const AGE: u32 = 12;

/// What to do about unrated content, as a request body writes it. The word that is
/// *not* what a restriction defaults to, so a command carrying the choice cannot pass
/// for one carrying the default.
pub(crate) const UNRATED: &str = "allow";

/// One completed download, as a client and the disk account both name it. Not the
/// words a walk or a trace is given, so a command carrying this cannot pass for one
/// carrying either of those.
pub(crate) const DOWNLOAD: &str = "A.Show.S01E01.1080p";
/// A policy, as a request body writes it. The one that lives inside a limit, so the
/// limit beside it is not an argument the translation could drop without noticing.
pub(crate) const POLICY: &str = "within-a-limit";

/// How many requests a period allows. Not one, so a command carrying it cannot pass
/// for one carrying a figure anything might have defaulted to.
pub(crate) const ALLOWED: u32 = 5;

/// How long that period is. Not seven, so the two numbers cannot be swapped without
/// a case noticing.
pub(crate) const PERIOD: u32 = 30;

/// One waiting request, by the number the request service would file it under. Not
/// nought, so a command carrying it cannot pass for one carrying nothing.
pub(crate) const WAITING: i64 = 7;

/// Why a request was turned down, in the operator's own words.
pub(crate) const REASON: &str = "there is no room this month";
