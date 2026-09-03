//! What each action may be given, as the tests either side of it name it.
//!
//! Shared because two files ask about the same input from different sides. One holds
//! every action to reaching a command; the other holds every argument to reaching only
//! the actions whose command can carry it. Both have to start from *exactly what this
//! action takes* — a request short of something is refused for the omission, and one
//! carrying something extra is refused for that instead, and either refusal would be
//! read as being about the name rather than about the argument.
//!
//! Built from the same lists the translation refuses by, so what is under test and
//! what the rule governs cannot come apart. A second copy in each file would drift,
//! and it would drift silently: both would still pass.

use lemonfiber_api::actions::{
    Arguments, TAKES_AGREED, TAKES_AGREEMENT, TAKES_ALLOWANCE, TAKES_ARCHIVE, TAKES_BUNDLING,
    TAKES_CHECK, TAKES_CONSENT, TAKES_DISRUPTION, TAKES_FORMS, TAKES_ITEM, TAKES_NAME,
    TAKES_NARROWING, TAKES_PRESET, TAKES_SERVICE, TAKES_SERVICES, TAKES_SETTING, TAKES_WAITING,
};

/// A backup name, as one is written under.
pub(crate) const ARCHIVE: &str = "lemonfiber-full-1700000000.tar.gz";

/// A log window that is not the one a bundle takes when nothing is asked for, so a
/// command carrying the default cannot pass for one carrying what was given.
pub(crate) const LOGS: u32 = 12;

/// One thing to walk end to end, named the way somebody would say it.
pub(crate) const ITEM: &str = "Sintel";

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
        libraries: if takes(TAKES_ALLOWANCE) {
            vec![LIBRARY.to_owned()]
        } else {
            Vec::new()
        },
        age_limit: takes(TAKES_ALLOWANCE).then_some(AGE),
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
    }
}
