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

use acting::exactly_what;
use carriers::{command, naming, refusal, swept, SWEEPS};
use lemonfiber_api::actions::{Arguments, Refused};
use lemonfiber_core::app::{Command, Waiting};

#[test]
fn an_argument_is_taken_exactly_where_the_command_it_reaches_carries_it() {
    let mut wrong: Vec<String> = Vec::new();
    for (argument, give, carries) in SWEEPS {
        wrong.extend(swept(argument, give, carries));
    }
    assert!(wrong.is_empty(), "{wrong:?}");
}

/// The one field the sweeps leave out, and why.
///
/// Every action that takes a name is handed one by `exactly_what`, and the tests that
/// are about a name are the ones that refuse an action without one. A sweep for it
/// would be a sweep every action passes.
const NOT_SWEPT: [&str; 1] = ["name"];

/// Where the carrier is declared, read rather than transcribed.
const CARRIER: &str = "src/actions/asked.rs";

/// What the carrier declares it holds, read off the declaration itself.
///
/// **Read rather than written down here.** This guard used to hold the table below
/// against a second list typed out beside it, which meant a field added to the carrier
/// and to neither list was one it stayed green through — the exact failure it is for.
/// A list compared against another list is a list agreeing with itself.
fn declared() -> Vec<String> {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(CARRIER);
    let source = std::fs::read_to_string(&path).unwrap_or_default();
    let opened = source
        .split_once("pub struct Arguments {")
        .map(|(_, rest)| rest)
        .unwrap_or_default();
    let body = opened.split_once("\n}").map_or("", |(body, _)| body);
    body.lines()
        .filter_map(|line| line.strip_prefix("    pub "))
        .filter_map(|declared| declared.split_once(':'))
        .map(|(field, _)| field.to_owned())
        .collect()
}

#[test]
fn every_argument_the_carrier_holds_is_swept() {
    // A field added to the carrier and not to the table above is a field nothing
    // decides about, which is how one comes to be dropped in the first place.
    let held: Vec<String> = declared()
        .into_iter()
        .filter(|field| !NOT_SWEPT.contains(&field.as_str()))
        .collect();
    assert!(
        held.len() > 20,
        "the carrier could not be read, so this asserts nothing: {held:?}"
    );

    let swept: Vec<&str> = SWEEPS.iter().map(|(argument, _, _)| *argument).collect();

    for field in &held {
        assert!(
            swept.contains(&field.as_str()),
            "the carrier holds `{field}` and nothing sweeps for it"
        );
    }
    for argument in &swept {
        assert!(
            held.iter().any(|field| field == argument) || NOT_SWEPT.contains(argument),
            "`{argument}` is swept for and the carrier no longer holds it"
        );
    }
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

mod carriers;
