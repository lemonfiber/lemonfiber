use super::*;
use crate::app::engine::Waiting;
use crate::app::plugins::Asked;

/// A patience unlike any default, so a length read from it cannot be a
/// constant that happens to match.
const WAITED: Duration = Duration::from_secs(41);

#[test]
fn starting_is_bounded_by_the_patience_the_run_was_given() {
    let command = Command::Up {
        forms: vec!["media".to_owned()],
    };

    assert_eq!(
        of(&command, WAITED),
        Some(Disturbance::Bounded(WAITED)),
        "a start is held to the same wait that decides when it gives up"
    );
}

#[test]
fn every_verb_that_starts_something_is_bounded_the_same_way() {
    let starting = [
        Command::Up { forms: Vec::new() },
        Command::Start {
            forms: Vec::new(),
            services: vec!["sonarr".to_owned()],
        },
        Command::Restart {
            forms: Vec::new(),
            services: Vec::new(),
        },
        Command::Switch { forms: Vec::new() },
    ];

    for command in starting {
        assert_eq!(
            of(&command, WAITED),
            Some(Disturbance::Bounded(WAITED)),
            "{command:?} waits for services to settle and is bounded by it"
        );
    }
}

#[test]
fn a_teardown_that_waits_for_downloads_has_nothing_bounding_it() {
    let command = Command::Down {
        forms: Vec::new(),
        wait: Waiting::ForTheDownloads,
    };

    assert_eq!(
        of(&command, WAITED),
        Some(Disturbance::Until(Awaiting::Downloads)),
        "how long a torrent takes belongs to whoever is seeding it"
    );
}

#[test]
fn a_teardown_that_does_not_wait_is_bounded_by_the_engines_grace() {
    let command = Command::Down {
        forms: Vec::new(),
        wait: Waiting::Never,
    };

    assert_eq!(
        of(&command, WAITED),
        Some(Disturbance::Bounded(GRACE)),
        "a stop is held to what the engine gives a service, not to the settle wait"
    );
}

#[test]
fn stopping_named_services_is_the_same_stop() {
    let command = Command::Halt {
        forms: Vec::new(),
        services: vec!["sonarr".to_owned()],
    };

    assert_eq!(of(&command, WAITED), Some(Disturbance::Bounded(GRACE)));
}

/// Removing a plugin stops its containers through the engine's own stop, so it is
/// held to the same grace and said the same way; updating one changes what is
/// running, like a switch. Installing one takes nothing away from anybody, and
/// reading what is installed takes nothing at all.
#[test]
fn removing_or_updating_a_plugin_takes_something_away_and_installing_one_does_not() {
    let removing = Command::Plugins(Asked::Remove {
        plugin: "komga".to_owned(),
    });
    assert_eq!(of(&removing, WAITED), Some(Disturbance::Bounded(GRACE)));
    let updating = Command::Plugins(Asked::Update {
        path: std::path::PathBuf::from("komga"),
    });
    assert_eq!(
        of(&updating, WAITED),
        Some(Disturbance::Bounded(WAITED)),
        "an update changes what is running, and is held to the settle wait a switch is"
    );

    for quiet in [
        Command::Plugins(Asked::Install {
            path: std::path::PathBuf::from("komga"),
        }),
        Command::Plugins(Asked::Installed),
    ] {
        assert_eq!(of(&quiet, WAITED), None, "{quiet:?} stops nothing");
    }
}

#[test]
fn reading_takes_nothing_away_and_says_nothing() {
    let reading = [Command::Version, Command::Forms, Command::History];

    for command in reading {
        assert_eq!(
            of(&command, WAITED),
            None,
            "{command:?} disturbs nobody, and a line saying so teaches an operator to skip it"
        );
    }
}

#[test]
fn a_bounded_length_is_said_in_the_one_place_a_length_becomes_words() {
    let said = sentence(Disturbance::Bounded(WAITED));

    assert!(
        said.contains(&crate::doctor::disturbing_for(WAITED)),
        "a length typed into a sentence goes stale the day the bound moves: {said}"
    );
}

#[test]
fn an_unbounded_operation_says_what_it_is_waiting_for() {
    let said = sentence(Disturbance::Until(Awaiting::Downloads));

    assert!(
        said.contains("coming down"),
        "open-ended is half an answer without what it is open-ended on: {said}"
    );
    assert!(
        !said.contains("seconds"),
        "a wait with no bound must not be given one in words: {said}"
    );
}

/// The payload states every situation there is, and nothing else.
///
/// This is the join that keeps the two halves from drifting apart. A new
/// situation fails to compile in [`Situation::called`], which sends whoever
/// added it to [`Situation::EVERY`], which lands here — and this stays red
/// until the payload has grown a field for it. Without the join, a situation
/// could be added, routed, and held to a clock while every surface went on
/// publishing the four it knew about, which reads to an operator as though
/// the new verb costs nothing.
///
/// Both directions, because a field nothing routes to is the same silence
/// wearing the opposite hat: a published length no command is ever held to.
#[test]
fn the_payload_states_every_situation_there_is_and_nothing_else() {
    let published =
        serde_json::to_value(crate::model::Disturbances::all(WAITED)).unwrap_or_default();
    let fields = published.as_object().cloned().unwrap_or_default();

    assert!(
        !fields.is_empty(),
        "the payload serialised to nothing, so the rest of this rule read nothing"
    );

    for situation in Situation::EVERY.iter().copied() {
        assert!(
            fields.contains_key(situation.called()),
            "{situation:?} is a situation the stack can be put in and no surface \
             can read what it costs"
        );
    }

    // Both sides are named before the assertion rather than inside its message.
    // A message is rendered only where the assertion fails, so a run that passes
    // never enters it — and the coverage gate counts a rendering no run reaches
    // against every covered line beside it.
    let published: Vec<&String> = fields.keys().collect();
    let known: Vec<&str> = Situation::EVERY
        .iter()
        .copied()
        .map(Situation::called)
        .collect();
    assert_eq!(
        published.len(),
        known.len(),
        "the payload publishes a length nothing is ever held to: {published:?} \
         against {known:?}"
    );
}

/// What a surface reads off the payload is what the command it then runs is
/// held to.
///
/// The whole point of publishing the lengths is that a surface states one and
/// then acts; if the two were arrived at separately, the day they diverged
/// would be a day an operator was told a number nothing honoured — and
/// nothing on either side would ever compare them, so it would never be
/// found. This is that comparison, made once, for every command that has a
/// length at all.
#[test]
fn the_payload_and_the_command_agree_on_every_length() {
    let forms = || vec!["media".to_owned()];
    let services = || vec!["sonarr".to_owned()];
    let payload = crate::model::Disturbances::all(WAITED);

    let pairs = [
        (Command::Up { forms: forms() }, payload.starting),
        (
            Command::Start {
                forms: forms(),
                services: services(),
            },
            payload.starting,
        ),
        (
            Command::Down {
                forms: forms(),
                wait: Waiting::Never,
            },
            payload.stopping,
        ),
        (
            Command::Halt {
                forms: forms(),
                services: services(),
            },
            payload.stopping,
        ),
        (
            Command::Down {
                forms: forms(),
                wait: Waiting::ForTheDownloads,
            },
            payload.stopping_after_downloads,
        ),
        (
            Command::Restart {
                forms: forms(),
                services: services(),
            },
            payload.restarting,
        ),
        (Command::Switch { forms: forms() }, payload.switching),
    ];

    for (command, published) in pairs {
        // Compared as options rather than unwrapped, so that a command which
        // stops having a length at all fails here saying which one, instead of
        // ending the run at a line that names nothing.
        assert_eq!(
            of(&command, WAITED).map(crate::model::TakesAway::from),
            Some(published),
            "{command:?} is held to one length and the payload publishes another, \
             so a surface states a number no run honours"
        );
    }
}

/// Addressing a form and addressing the services inside it take the same
/// length away.
///
/// This is what lets one published field answer for two commands. It is
/// asserted rather than assumed, because the day a service stop is held to a
/// different clock from a teardown, the payload has to grow a field — and the
/// failure that says so has to arrive here, not in an operator being told the
/// wrong number.
#[test]
fn a_form_and_the_services_inside_it_are_held_to_the_same_clock() {
    let forms = vec!["media".to_owned()];
    let services = vec!["sonarr".to_owned()];

    let same = [
        (
            Command::Up {
                forms: forms.clone(),
            },
            Command::Start {
                forms: forms.clone(),
                services: services.clone(),
            },
        ),
        (
            Command::Down {
                forms: forms.clone(),
                wait: Waiting::Never,
            },
            Command::Halt {
                forms: forms.clone(),
                services,
            },
        ),
    ];

    for (whole, named) in same {
        assert_eq!(
            of(&whole, WAITED),
            of(&named, WAITED),
            "{whole:?} and {named:?} no longer cost the same, so one published \
             field can no longer answer for both"
        );
    }
}

/// The published lengths are the knob's, not a constant's.
///
/// An operator on a slow disk runs with a longer patience, and a payload that
/// answered from a constant would state the length somebody else's stack was
/// held to — plausibly, and wrong in exactly the case the knob exists for.
#[test]
fn a_longer_patience_is_a_longer_published_start() {
    let brief = crate::model::Disturbances::all(Duration::from_secs(30));
    let patient = crate::model::Disturbances::all(Duration::from_secs(600));

    assert_ne!(
        brief.starting, patient.starting,
        "the published start ignores the knob it is supposed to be reading"
    );
    assert_eq!(
        brief.stopping, patient.stopping,
        "stopping is held to the engine's grace, which the start knob does not move"
    );
}
