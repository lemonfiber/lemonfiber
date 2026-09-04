//! Which argument reaches which action, and what refusing one looks like.
//!
//! Apart from what an action *means* because it is a different claim. That one is
//! about the set of actions: every name reaches one of the core's own commands, and a
//! name outside the set is refused rather than invented. This one is about what each
//! of them may be *given* — the rule that an action accepts an argument only where
//! the command it reaches has somewhere to put it, and refuses it by name otherwise.
//!
//! **Taking an argument and dropping it is what this exists to catch**, and dropping
//! one is invisible from outside: the request is carried out, as something else. So
//! every argument the carrier holds is swept over every action offered, and what
//! counts as having arrived is asserted on the command rather than on the reply.
//!
//! Driven from outside the crate, because what a caller can reach is the thing worth
//! holding still.

mod acting;

use acting::{
    exactly_what, AGE, ARCHIVE, FOLLOWED, ITEM, LIBRARY, LOGS, NARROWED, OFFER, SEASON, WARNED,
};
use lemonfiber_api::actions::{named, Arguments, Disturbing, Refused, OFFERED};
use lemonfiber_core::app::bundle::Wanted;
use lemonfiber_core::app::repair::Consent;
use lemonfiber_core::app::restore::{Consent as RestoreConsent, Kept};
use lemonfiber_core::app::{Command, QualityAction, Waiting};
use lemonfiber_core::bundle::Filenames;
use lemonfiber_core::doctor::Narrowing;

/// One form named, which is what most of the rest are asked with.
fn naming(form: &str) -> Arguments {
    Arguments {
        forms: vec![form.to_owned()],
        ..Arguments::default()
    }
}

/// What an action came to, or nothing where it was refused.
fn command(action: &str, given: Arguments) -> Option<Command> {
    named(action, given).ok()
}

/// Why an action was refused, or nothing where it was not.
fn refusal(action: &str, given: Arguments) -> Option<Refused> {
    named(action, given).err()
}

/// Whether the command has the forms it was given in it.
fn carries_forms(command: &Command) -> bool {
    match command {
        Command::Up { forms }
        | Command::Down { forms, .. }
        | Command::Switch { forms }
        | Command::Pull { forms }
        | Command::Watch { forms }
        | Command::Start { forms, .. }
        | Command::Halt { forms, .. }
        | Command::Restart { forms, .. } => !forms.is_empty(),
        _ => false,
    }
}

/// Whether the command has the services it was given in it.
/// Whether the command has the wait it was asked for in it.
fn carries_wait(command: &Command) -> bool {
    matches!(
        command,
        Command::Down {
            wait: Waiting::ForTheDownloads,
            ..
        }
    )
}

fn carries_services(command: &Command) -> bool {
    match command {
        Command::Start { services, .. }
        | Command::Halt { services, .. }
        | Command::Restart { services, .. } => !services.is_empty(),
        _ => false,
    }
}

/// Whether the command has the setting it was given in it.
fn carries_setting(command: &Command) -> bool {
    matches!(command, Command::ConfigSet { .. })
}

/// Whether the command has the quality it was given in it.
fn carries_preset(command: &Command) -> bool {
    matches!(
        command,
        Command::Quality(QualityAction::Set { .. }) | Command::QualityMusic { .. }
    )
}

/// Whether the command has the media type it was given in it.
fn carries_media_type(command: &Command) -> bool {
    matches!(
        command,
        Command::Quality(QualityAction::Set {
            media_type: Some(_),
            ..
        }) | Command::QualityMusic { .. }
    )
}

/// Whether the command has the operator's agreement in it.
///
/// A bundle carries it inside what was asked for rather than beside it, because the
/// setting to be shown and the agreement to show it are one decision.
fn carries_agreement(command: &Command) -> bool {
    matches!(
        command,
        Command::Quality(QualityAction::Set { confirm: true, .. })
            | Command::QualityUpgrade { confirm: true }
            | Command::Reset { confirm: true }
            | Command::Forget { confirm: true }
            | Command::Space { confirm: true }
            | Command::Remove { confirm: true, .. }
            | Command::Restore {
                consent: RestoreConsent::Given { .. } | RestoreConsent::Standing,
                ..
            }
            | Command::Support {
                wanted: Wanted {
                    confirmed: true,
                    ..
                },
                ..
            }
    ) || carries_a_yes(command)
}

/// Whether a repairing run was told yes, in either of the two ways of saying it.
///
/// A repair carries the agreement inside the consent rather than beside it, because
/// what was agreed to and whether anything was are one decision. An offer is the
/// only shape that carries no yes, which is what makes it the offer.
fn carries_a_yes(command: &Command) -> bool {
    match command {
        Command::Repair { consent, .. } => !matches!(consent, Consent::Offer),
        _ => false,
    }
}

/// Whether the command has the check whose warning is being answered in it.
fn carries_check(command: &Command) -> bool {
    matches!(command, Command::Doctor { accept: Some(check), .. } if check == WARNED)
}

/// Whether the command was told to do the widened thing rather than the plain one.
///
/// A trace carries it as `searching`. The word buys the same thing at every door that
/// takes it — the widened form of a read, which reaches past this machine — and each
/// command holds it under its own name for that.
fn carries_disruption(command: &Command) -> bool {
    matches!(
        command,
        Command::Repair {
            disruptive: true,
            ..
        } | Command::Doctor {
            disruptive: true,
            ..
        } | Command::Trace {
            searching: true,
            ..
        }
    )
}

/// Whether the command has what the consent was read in.
///
/// Two actions, and each names its own half: a repair's yes names the offer it was
/// read in, a restore's names the listing.
fn carries_offer(command: &Command) -> bool {
    match command {
        Command::Repair {
            consent: Consent::Given { offer, .. },
            ..
        } => offer == OFFER,
        Command::Restore {
            consent: RestoreConsent::Given { listing },
            ..
        } => listing == OFFER,
        _ => false,
    }
}

/// Whether the command has the repairs that were agreed to.
fn carries_agreed(command: &Command) -> bool {
    matches!(
        command,
        Command::Repair {
            consent: Consent::Given { repairs, .. },
            ..
        } if repairs.iter().any(|check| check == WARNED)
    )
}

/// Whether the command has the one service it was given in it.
fn carries_service(command: &Command) -> bool {
    matches!(command, Command::Backup { service: Some(_) })
}

/// Whether the command has the archive it was named in it, as a name rather than a
/// path — which is the whole of what a browser may ask to be read.
fn carries_archive(command: &Command) -> bool {
    matches!(command, Command::Restore { archive: Kept::Named(name), .. } if name == ARCHIVE)
}

/// Whether the command has the accepted re-point in it.
fn carries_repoint(command: &Command) -> bool {
    matches!(command, Command::Restore { repoint: true, .. })
}

/// Whether the command was told to produce the file rather than describe one.
fn carries_write(command: &Command) -> bool {
    matches!(command, Command::Support { write: true, .. })
}

/// Whether the command has the log window it was given in it.
fn carries_logs(command: &Command) -> bool {
    matches!(command, Command::Support { wanted, .. } if wanted.lines == LOGS)
}

/// Whether the command was told to leave media filenames as they are.
fn carries_filenames(command: &Command) -> bool {
    matches!(
        command,
        Command::Support {
            wanted: Wanted {
                filenames: Filenames::Shown,
                ..
            },
            ..
        }
    )
}

/// Whether the command has the settings it was told to show as they are.
fn carries_reveal(command: &Command) -> bool {
    matches!(command, Command::Support { wanted, .. } if !wanted.reveal.is_empty())
}

fn give_forms(given: &mut Arguments) {
    given.forms = vec!["tv".to_owned()];
}

fn give_services(given: &mut Arguments) {
    given.services = vec!["sonarr".to_owned()];
}

fn give_wait(given: &mut Arguments) {
    given.wait = Waiting::ForTheDownloads;
}

fn give_key(given: &mut Arguments) {
    given.key = Some("DATA_ROOT".to_owned());
}

fn give_value(given: &mut Arguments) {
    given.value = Some("/srv".to_owned());
}

fn give_preset(given: &mut Arguments) {
    given.preset = Some("balanced".to_owned());
}

fn give_media_type(given: &mut Arguments) {
    given.media_type = Some("tv".to_owned());
}

fn give_agreement(given: &mut Arguments) {
    given.confirm = true;
}

fn give_service(given: &mut Arguments) {
    given.service = Some("sonarr".to_owned());
}

fn give_archive(given: &mut Arguments) {
    given.archive = Some(ARCHIVE.to_owned());
}

fn give_repoint(given: &mut Arguments) {
    given.repoint = true;
}

fn give_write(given: &mut Arguments) {
    given.write = true;
}

fn give_logs(given: &mut Arguments) {
    given.logs = Some(LOGS);
}

fn give_filenames(given: &mut Arguments) {
    given.filenames = Filenames::Shown;
}

fn give_reveal(given: &mut Arguments) {
    given.reveal = vec!["INDEXER_KEY".to_owned()];
}

/// Whether the command has the one thing it was told to add in it.
fn carries_item(command: &Command) -> bool {
    matches!(command, Command::Walkthrough { item: Some(named) } if named == ITEM)
}

fn give_item(given: &mut Arguments) {
    given.item = Some(ITEM.to_owned());
}

/// Whether the command has the title it was told to follow in it.
fn carries_term(command: &Command) -> bool {
    matches!(command, Command::Trace { term, .. } if term == FOLLOWED)
}

fn give_term(given: &mut Arguments) {
    given.term = Some(FOLLOWED.to_owned());
}

/// Whether the command has the season the following was narrowed to in it.
fn carries_season(command: &Command) -> bool {
    matches!(
        command,
        Command::Trace {
            season: Some(SEASON),
            ..
        }
    )
}

fn give_season(given: &mut Arguments) {
    given.season = Some(SEASON);
}

/// Whether the command has the libraries it was told they may open in it.
fn carries_libraries(command: &Command) -> bool {
    matches!(command, Command::Invite { allowance, .. }
        if allowance.libraries.iter().any(|named| named == LIBRARY))
}

fn give_libraries(given: &mut Arguments) {
    given.libraries = vec![LIBRARY.to_owned()];
}

/// Whether the command has the age limit it was given in it, as the age it was given
/// as — the media server keeps an age, so there is nothing here to translate.
fn carries_age_limit(command: &Command) -> bool {
    matches!(command, Command::Invite { allowance, .. } if allowance.age_limit == Some(AGE))
}

fn give_age_limit(given: &mut Arguments) {
    given.age_limit = Some(AGE);
}

fn give_check(given: &mut Arguments) {
    given.check = Some(WARNED.to_owned());
}

/// Whether the command has the narrowing it was given in it.
fn carries_narrowing(command: &Command) -> bool {
    matches!(
        command,
        Command::Doctor {
            narrowing: Narrowing::Check(id),
            ..
        } if id == NARROWED
    )
}

fn give_narrowing(given: &mut Arguments) {
    given.only = Some(NARROWED.to_owned());
}

fn give_disruption(given: &mut Arguments) {
    given.disruptive = Disturbing::Included;
}

// One at a time, like every other sweep. The action that takes them was given both
// already; an action that takes neither must be refused for the one being swept for
// rather than for its companion.
fn give_offer(given: &mut Arguments) {
    given.offer = Some(OFFER.to_owned());
}

fn give_agreed(given: &mut Arguments) {
    given.agreed = vec![WARNED.to_owned()];
}

/// One argument the carrier holds: its name, how to give it, and what it looks like
/// to have arrived on the command the action reached.
type Sweep = (&'static str, fn(&mut Arguments), fn(&Command) -> bool);

/// Every argument the carrier holds: how to give it, and what it looks like to have
/// arrived.
///
/// One row per argument rather than one test per argument, because the rule is one
/// thing: an action may accept an argument only if the command it reaches has
/// somewhere to put it, and must refuse it by that name otherwise.
const SWEEPS: [Sweep; 25] = [
    ("forms", give_forms, carries_forms),
    ("services", give_services, carries_services),
    ("wait", give_wait, carries_wait),
    ("service", give_service, carries_service),
    ("key", give_key, carries_setting),
    ("value", give_value, carries_setting),
    ("preset", give_preset, carries_preset),
    ("media_type", give_media_type, carries_media_type),
    ("archive", give_archive, carries_archive),
    ("repoint", give_repoint, carries_repoint),
    ("write", give_write, carries_write),
    ("logs", give_logs, carries_logs),
    ("filenames", give_filenames, carries_filenames),
    ("reveal", give_reveal, carries_reveal),
    ("only", give_narrowing, carries_narrowing),
    ("check", give_check, carries_check),
    ("disruptive", give_disruption, carries_disruption),
    ("offer", give_offer, carries_offer),
    ("agreed", give_agreed, carries_agreed),
    ("confirm", give_agreement, carries_agreement),
    ("item", give_item, carries_item),
    ("libraries", give_libraries, carries_libraries),
    ("age_limit", give_age_limit, carries_age_limit),
    ("term", give_term, carries_term),
    ("season", give_season, carries_season),
];

/// Every offered action given one argument on top of what it takes, gathering what
/// is wrong.
///
/// An action that takes the argument already had it and is unchanged; one that does
/// not is being given something its command cannot carry, and must say so. Taking it
/// and dropping it is what this exists to catch, and dropping it is invisible from
/// outside — the request is carried out, as something else.
fn swept(argument: &str, give: fn(&mut Arguments), carries: fn(&Command) -> bool) -> Vec<String> {
    let mut wrong: Vec<String> = Vec::new();
    for action in OFFERED {
        let mut given = exactly_what(action);
        give(&mut given);
        match named(action, given) {
            Ok(command) if carries(&command) => {}
            Ok(command) => {
                wrong.push(format!(
                    "{action} took `{argument}` and dropped it: {command:?}"
                ));
            }
            Err(Refused::Unwanted {
                argument: named, ..
            }) if named == argument => {}
            // Refused by name for the other reason there is: it and something the
            // action already holds name two different requests. That refusal names
            // both of them, and either half is this argument being refused rather
            // than dropped, which is the whole of what this sweeps for.
            Err(Refused::Together {
                argument: named,
                alongside,
                ..
            }) if named == argument || alongside == argument => {}
            Err(refused) => {
                wrong.push(format!(
                    "{action} was refused for something else: {refused:?}"
                ));
            }
        }
    }
    wrong
}

#[test]
fn an_argument_is_taken_exactly_where_the_command_it_reaches_carries_it() {
    let mut wrong: Vec<String> = Vec::new();
    for (argument, give, carries) in SWEEPS {
        wrong.extend(swept(argument, give, carries));
    }
    assert!(wrong.is_empty(), "{wrong:?}");
}

#[test]
fn every_argument_the_carrier_holds_is_swept() {
    // A field added to the carrier and not to the table above is a field nothing
    // decides about, which is how one comes to be dropped in the first place.
    let held = [
        "forms",
        "services",
        "wait",
        "service",
        "key",
        "value",
        "preset",
        "media_type",
        "archive",
        "repoint",
        "write",
        "logs",
        "filenames",
        "reveal",
        "only",
        "check",
        "disruptive",
        "offer",
        "agreed",
        "confirm",
        "item",
        "libraries",
        "age_limit",
        "term",
        "season",
    ];
    let swept: Vec<&str> = SWEEPS.iter().map(|(argument, _, _)| *argument).collect();
    assert_eq!(swept, held);
}

#[test]
fn stopping_a_whole_stack_is_not_gated_the_way_a_reset_is() {
    // A teardown removes what a form started and `up` puts it back, so there is no
    // cost to agree to in advance, and the command line declares no such flag on
    // it. The command it reaches carries no agreement at all.
    assert_eq!(
        command("down", naming("tv")),
        Some(Command::Down {
            forms: vec!["tv".to_owned()],
            wait: Waiting::Never
        })
    );
    let agreed = Arguments {
        forms: vec!["tv".to_owned()],
        confirm: true,
        ..Arguments::default()
    };
    assert_eq!(
        refusal("down", agreed),
        Some(Refused::Unwanted {
            action: "down".to_owned(),
            argument: "confirm".to_owned()
        })
    );
}

#[test]
fn stopping_named_services_takes_no_agreement_either() {
    let agreed = Arguments {
        forms: vec!["tv".to_owned()],
        services: vec!["sonarr".to_owned()],
        confirm: true,
        ..Arguments::default()
    };
    assert_eq!(
        refusal("down", agreed),
        Some(Refused::Unwanted {
            action: "down".to_owned(),
            argument: "confirm".to_owned()
        })
    );
}

#[test]
fn choosing_for_music_takes_the_agreement_and_drops_it_as_the_command_line_does() {
    // Picking an audio format is not a choice this host has to transcode for, so
    // the agreement has nowhere to go — and `quality set --for music --confirm`
    // drops it too. Refusing it here would make this surface the stricter of the
    // two, which is the same divergence as offering something the other cannot.
    let given = Arguments {
        preset: Some("lossless".to_owned()),
        media_type: Some("music".to_owned()),
        confirm: true,
        ..Arguments::default()
    };
    assert!(matches!(
        command("quality-set", given),
        Some(Command::QualityMusic { .. })
    ));
}
