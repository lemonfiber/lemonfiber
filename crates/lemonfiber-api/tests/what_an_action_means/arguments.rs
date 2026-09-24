//! What each action takes, and what it refuses to be given.

use crate::acting::*;
use crate::{command, naming, nothing, refusal};
use lemonfiber_api::actions::{named, Arguments, Refused};
use lemonfiber_core::app::{Command, QualityAction, Waiting};
use lemonfiber_core::quality::Preset;

#[test]
fn starting_and_stopping_take_the_forms_they_are_given() {
    assert_eq!(
        command("up", naming("tv")),
        Some(Command::Up {
            forms: vec!["tv".to_owned()]
        })
    );
    assert_eq!(
        command("up", nothing()),
        Some(Command::Up { forms: vec![] })
    );
    assert_eq!(
        command("down", naming("tv")),
        Some(Command::Down {
            forms: vec!["tv".to_owned()],
            wait: Waiting::Never
        })
    );
}

#[test]
fn starting_named_services_is_a_different_request_from_bringing_a_form_up() {
    // Not a start with an argument: bringing a form up creates everything its
    // closure holds, and this starts the ones named. Dropping them would start
    // every service the form holds — the answer to a request nobody made.
    let given = Arguments {
        forms: vec!["tv".to_owned()],
        services: vec!["sonarr".to_owned()],
        ..Arguments::default()
    };
    assert_eq!(
        command("up", given),
        Some(Command::Start {
            forms: vec!["tv".to_owned()],
            services: vec!["sonarr".to_owned()]
        })
    );
    // Naming none, it is the whole-form start it says it is.
    assert_eq!(
        command("up", naming("tv")),
        Some(Command::Up {
            forms: vec!["tv".to_owned()]
        })
    );
}

#[test]
fn a_teardown_can_be_asked_to_let_the_downloads_finish_first() {
    let waiting = Arguments {
        forms: vec!["tv".to_owned()],
        wait: Waiting::ForTheDownloads,
        ..Arguments::default()
    };
    assert_eq!(
        command("down", waiting),
        Some(Command::Down {
            forms: vec!["tv".to_owned()],
            wait: Waiting::ForTheDownloads
        })
    );
}

#[test]
fn waiting_and_naming_services_are_refused_together_rather_than_one_being_dropped() {
    // Two arguments, two requests: a teardown that waits, and a stop of named
    // services that leaves the rest of the form running. A run given both would
    // have to pick one, and what is in flight is a question about the download
    // clients a form holds rather than about two named services.
    let both = Arguments {
        forms: vec!["tv".to_owned()],
        services: vec!["sonarr".to_owned()],
        wait: Waiting::ForTheDownloads,
        ..Arguments::default()
    };
    assert_eq!(
        refusal("down", both),
        Some(Refused::Together {
            action: "down".to_owned(),
            argument: "wait".to_owned(),
            alongside: "services".to_owned()
        })
    );
}

#[test]
fn stopping_named_services_is_a_different_request_from_tearing_a_form_down() {
    let given = Arguments {
        forms: vec!["tv".to_owned()],
        services: vec!["sonarr".to_owned()],
        ..Arguments::default()
    };
    assert_eq!(
        command("down", given),
        Some(Command::Halt {
            forms: vec!["tv".to_owned()],
            services: vec!["sonarr".to_owned()]
        })
    );
}

#[test]
fn the_four_actions_that_must_be_told_what_to_act_on_say_so() {
    // A guard is one of them for a different reason from the other three: not that
    // the request has lost its subject, but that a watch with nothing to stop would
    // see the drive vanish and have nothing to do about it.
    for action in ["switch", "restart", "pull", "watch"] {
        assert_eq!(
            refusal(action, nothing()),
            Some(Refused::Missing {
                action: action.to_owned(),
                argument: "forms".to_owned()
            }),
            "{action}"
        );
    }
}

#[test]
fn changing_a_setting_needs_both_halves_of_it() {
    let key_only = Arguments {
        key: Some("DATA_ROOT".to_owned()),
        ..Arguments::default()
    };
    let value_only = Arguments {
        value: Some("/srv".to_owned()),
        ..Arguments::default()
    };
    assert_eq!(
        refusal("config-set", value_only),
        Some(Refused::Missing {
            action: "config-set".to_owned(),
            argument: "key".to_owned()
        })
    );
    assert_eq!(
        refusal("config-set", key_only),
        Some(Refused::Missing {
            action: "config-set".to_owned(),
            argument: "value".to_owned()
        })
    );
}

#[test]
fn a_quality_choice_reaches_the_preset_it_names() {
    let given = Arguments {
        preset: Some("balanced".to_owned()),
        ..Arguments::default()
    };
    assert_eq!(
        command("quality-set", given),
        Some(Command::Quality(QualityAction::Set {
            preset: Preset::Balanced,
            media_type: None,
            confirm: false
        }))
    );
}

#[test]
fn choosing_for_music_chooses_a_format_rather_than_a_resolution() {
    // Music has no resolution, so the same request means a different command —
    // the fork the command line takes, taken the same way here.
    let given = Arguments {
        preset: Some("lossless".to_owned()),
        media_type: Some("music".to_owned()),
        ..Arguments::default()
    };
    assert!(matches!(
        command("quality-set", given),
        Some(Command::QualityMusic { .. })
    ));
}

#[test]
fn a_quality_choice_with_no_preset_at_all_says_which_argument_it_wanted() {
    assert_eq!(
        refusal("quality-set", nothing()),
        Some(Refused::Missing {
            action: "quality-set".to_owned(),
            argument: "preset".to_owned()
        })
    );
}

#[test]
fn a_preset_that_names_nothing_is_refused_with_what_it_could_have_said() {
    let bad_preset = Arguments {
        preset: Some("cinematic".to_owned()),
        ..Arguments::default()
    };
    let bad_music = Arguments {
        preset: Some("cinematic".to_owned()),
        media_type: Some("music".to_owned()),
        ..Arguments::default()
    };
    let bad_type = Arguments {
        preset: Some("balanced".to_owned()),
        media_type: Some("podcasts".to_owned()),
        ..Arguments::default()
    };
    for (given, argument) in [
        (bad_preset, "preset"),
        (bad_music, "preset"),
        (bad_type, "media_type"),
    ] {
        let refused = refusal("quality-set", given);
        assert!(
            matches!(&refused, Some(Refused::Unrecognised { argument: named, offered })
                if named == argument && !offered.is_empty()),
            "{argument}: {refused:?}"
        );
    }
}

/// A word about unrated content this build does not know is refused with the ones it
/// does, rather than falling to whichever answer is safer.
///
/// Falling to the safe answer is the tempting mistake and it is the wrong one: a caller
/// who wrote `allow` and meant it would be given `block` because of a spelling, and
/// would find out when somebody in the house could not find half the library.
#[test]
fn a_word_about_unrated_content_this_build_does_not_know_is_refused() {
    let mistaken = Arguments {
        name: Some("ana".to_owned()),
        age_limit: Some(12),
        unrated: Some("hide".to_owned()),
        ..Arguments::default()
    };

    let refused = refusal("invite", mistaken);

    assert!(
        matches!(&refused, Some(Refused::Unrecognised { argument, offered })
            if argument == "unrated" && offered.contains("block") && offered.contains("allow")),
        "{refused:?}"
    );
}

/// Each word this build does know reaches the command carrying the choice it names.
#[test]
fn each_word_about_unrated_content_reaches_the_choice_it_names() {
    for (written, chosen) in [
        ("block", lemonfiber_core::ports::service::Unrated::HeldBack),
        (
            "allow",
            lemonfiber_core::ports::service::Unrated::LetThrough,
        ),
    ] {
        let given = Arguments {
            name: Some("ana".to_owned()),
            age_limit: Some(12),
            unrated: Some(written.to_owned()),
            ..Arguments::default()
        };

        let reached = named("invite", given);

        assert!(
            matches!(&reached, Ok(Command::Invite { allowance, .. })
                if allowance.unrated == Some(chosen)),
            "{written}: {reached:?}"
        );
    }
}

#[test]
fn an_argument_no_action_takes_is_refused_rather_than_ignored() {
    // A caller who spelled `form` where `forms` was meant has been told, instead
    // of watching a whole form stop.
    let mistyped = serde_json::from_str::<Arguments>(r#"{"form":"tv"}"#);
    assert!(mistyped.is_err());
    assert!(serde_json::from_str::<Arguments>("{}").is_ok());
}

/// A policy this build does not know is refused with the ones it does.
///
/// A caller who wrote a word and meant it must not be given a different arrangement
/// because of a spelling — which on this argument would be a household trusted where
/// it asked to be held to something.
#[test]
fn a_policy_this_build_does_not_know_is_refused_with_the_ones_it_does() {
    let mistaken = Arguments {
        policy: Some("generous".to_owned()),
        ..Arguments::default()
    };

    let refused = refusal("household-allow", mistaken);

    assert!(
        matches!(&refused, Some(Refused::Unrecognised { argument, offered })
            if argument == "policy"
                && offered.contains("trusted")
                && offered.contains("everything-waits")),
        "{refused:?}"
    );
}

/// Half a limit is refused as the half that is missing, not completed for the caller.
///
/// A number over no period, and a period allowing no number, are each a household
/// held to something nobody chose.
#[test]
fn half_a_limit_is_refused_as_the_half_that_is_missing() {
    for (requests, days, wanted) in [(Some(5_u32), None, "days"), (None, Some(7_u32), "requests")] {
        let given = Arguments {
            requests,
            days,
            ..Arguments::default()
        };

        let refused = refusal("household-allow", given);

        assert!(
            matches!(&refused, Some(Refused::Missing { action, argument })
                if action == "household-allow" && argument == wanted),
            "{requests:?}/{days:?}: {refused:?}"
        );
    }
}

/// Naming nobody is the household's own choice rather than an omission.
///
/// The one action addressed to a person that does not require one: a choice with
/// nobody named is about the house, which is a request in its own right.
#[test]
fn a_choice_naming_nobody_is_the_households_own() {
    let given = Arguments {
        policy: Some("trusted".to_owned()),
        ..Arguments::default()
    };

    let reached = named("household-allow", given);

    assert!(
        matches!(&reached, Ok(Command::Allowing(chosen))
            if chosen.member.is_none()
                && chosen.policy == Some(lemonfiber_core::asking::Policy::Trusted)),
        "{reached:?}"
    );
}

/// A decision with no request has lost its subject, and is refused rather than
/// answered with every waiting request.
#[test]
fn a_decision_with_no_request_is_refused() {
    for action in ["household-approve", "household-decline"] {
        let given = Arguments {
            reason: (action == "household-decline").then(|| "no room".to_owned()),
            ..Arguments::default()
        };

        let refused = refusal(action, given);

        assert!(
            matches!(&refused, Some(Refused::Missing { argument, .. }) if argument == "request"),
            "{action}: {refused:?}"
        );
    }
}

/// Turning a request down without a reason is refused by name.
///
/// A refusal with nothing beside it is indistinguishable from being ignored, which
/// is the whole of what the reason is for.
#[test]
fn turning_a_request_down_without_a_reason_is_refused() {
    let given = Arguments {
        request: Some(7),
        ..Arguments::default()
    };

    let refused = refusal("household-decline", given);

    assert!(
        matches!(&refused, Some(Refused::Missing { action, argument })
            if action == "household-decline" && argument == "reason"),
        "{refused:?}"
    );
}

/// An approval cannot carry a reason at all, and is refused for it by name.
///
/// What an approval owes the person who asked is the thing they asked for, so a
/// reason named to one is an argument its command has nowhere to put.
#[test]
fn an_approval_cannot_carry_a_reason() {
    let given = Arguments {
        request: Some(7),
        reason: Some("no room".to_owned()),
        ..Arguments::default()
    };

    let refused = refusal("household-approve", given);

    assert!(
        matches!(&refused, Some(Refused::Unwanted { argument, .. }) if argument == "reason"),
        "{refused:?}"
    );
}
