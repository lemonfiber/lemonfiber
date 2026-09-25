use super::*;
use crate::render::fixtures::*;
use lemonfiber_core::docker::{Condition, State};
use lemonfiber_core::model::{
    LifecycleReport, ResetReport, StackEdit, StatusReport, SupervisionReport,
};
use lemonfiber_core::stack::closure::{Dropped, Filtered, Footprint};

#[test]
fn a_reset_names_every_change_it_would_revert() {
    let report = ResetReport {
        reverted: vec![StackEdit {
            path: "compose.yml".to_owned(),
            diff: "-yours\n+ours\n".to_owned(),
        }],
        reverted_connections: vec!["sonarr → sabnzbd".to_owned()],
        confirmed: false,
    };
    let text = reset(&report).text();
    assert!(text.contains("would revert these 2 change(s)"));
    assert!(text.contains("· sonarr → sabnzbd"));
    assert!(text.contains("compose.yml"));
    assert!(text.contains("-yours"));
    // Confirmed, it reports what it did rather than what it would do.
    let done = ResetReport {
        confirmed: true,
        ..report
    };
    assert!(reset(&done).text().contains("Reverted 2 change(s)"));
    // Nothing to revert is not an empty list.
    let clean = ResetReport {
        reverted: Vec::new(),
        reverted_connections: Vec::new(),
        confirmed: false,
    };
    assert!(reset(&clean).text().contains("already lemonfiber's own"));
}

#[test]
fn a_lifecycle_report_names_the_command_the_drops_and_the_edits_it_kept() {
    let report = LifecycleReport {
        command: vec!["docker".to_owned(), "compose".to_owned()],
        rehearsed: true,
        services: vec![service("sonarr", State::Healthy, None)],
        condition: Some(Condition::Active),
        stack_edits: vec![StackEdit {
            path: "compose.yml".to_owned(),
            diff: "-a\n+b\n".to_owned(),
        }],
        ..a_lifecycle(
            "up",
            a_plan(
                "media",
                vec![Dropped {
                    profile: "usenet".to_owned(),
                    needs: Protocol::Usenet,
                }],
            ),
        )
    };
    let text = lifecycle(&report).text();
    assert!(text.contains("would run:"));
    assert!(text.contains("docker compose"));
    assert!(text.contains("up: media"));
    assert!(text.contains("left out: usenet — no Usenet provider is configured"));
    assert!(text.contains("everything is up"));
    assert!(text.contains("sonarr"));
    assert!(text.contains("kept compose.yml as it is on disk"));
    assert!(text.contains("-a"));
}

/// Told before the bind fails, and told as a fact rather than a refusal.
#[test]
fn a_port_something_else_holds_is_named_on_both_sides_before_the_start() {
    let report = LifecycleReport {
        rehearsed: true,
        port_conflicts: vec![ConflictReport {
            port: 8989,
            wanted_by: "sonarr".to_owned(),
            held_by: "somebody-elses/sonarr".to_owned(),
        }],
        ..a_lifecycle("up", a_plan("media", Vec::new()))
    };
    let text = lifecycle(&report).text();
    assert!(
        text.contains("sonarr wants 8989, held by somebody-elses/sonarr"),
        "{text}"
    );
    assert!(text.contains("migrate beside"), "{text}");
}

/// And a machine running one stack hears nothing about it.
#[test]
fn a_start_with_nothing_in_its_way_says_nothing_about_ports() {
    let report = a_lifecycle("up", a_plan("media", Vec::new()));
    assert!(!lifecycle(&report).text().contains("already answers on"));
}

#[test]
fn a_run_that_was_not_rehearsed_and_reports_no_condition_says_only_what_it_did() {
    let report = LifecycleReport {
        status: Some(0),
        ..a_lifecycle("down", a_plan("media", Vec::new()))
    };
    let text = lifecycle(&report).text();
    assert_eq!(text, "down: media");
}

/// The half of the failed-start defect a person sees. The other half is the exit
/// status, and both are read from the same field so a screen and a script cannot
/// come away with different accounts of one run.
#[test]
fn a_run_compose_did_not_carry_out_says_so_rather_than_reading_as_a_success() {
    let failed = LifecycleReport {
        status: Some(1),
        ..a_lifecycle("up", a_plan("media", Vec::new()))
    };
    let said = lifecycle(&failed).text();
    assert!(
        said.contains("up did not finish — Compose exited 1"),
        "{said}"
    );

    // No status at all, on a run that was not a rehearsal, is a process that was
    // signalled rather than one that exited — there is no code to name, and saying
    // one anyway would invent a number Compose never produced.
    let signalled = LifecycleReport {
        status: None,
        ..a_lifecycle("up", a_plan("media", Vec::new()))
    };
    let stopped = lifecycle(&signalled).text();
    assert!(
        stopped.contains("Compose was stopped before it ended"),
        "{stopped}"
    );

    // A rehearsal spawned nothing, so it failed at nothing — saying it did not
    // finish would make `--dry-run` read as a broken command on every run.
    let rehearsed = LifecycleReport {
        status: None,
        rehearsed: true,
        ..a_lifecycle("up", a_plan("media", Vec::new()))
    };
    assert!(!lifecycle(&rehearsed).text().contains("did not finish"));
}

/// A start that declined to start anything says why, and says nothing else.
///
/// The start a login makes declines for three reasons an operator would act on
/// differently — the stack was stopped on purpose, autostart was never asked for,
/// the machine is on its battery — and the plan carried underneath is the plan it
/// did not run. Rendering the profiles, the services and the condition alongside
/// would read as an account of what happened, and an operator skimming it would
/// come away believing their stack came back. So the whole rendering is the one
/// sentence, which is why this asserts the whole of the text rather than a
/// fragment of it.
#[test]
fn a_start_that_declined_says_why_and_nothing_of_the_plan_it_did_not_run() {
    let declined = LifecycleReport {
        held: Some("the stack was stopped on purpose".to_owned()),
        ..a_lifecycle("boot", a_plan("media", Vec::new()))
    };

    assert_eq!(
        lifecycle(&declined).text(),
        "boot: nothing was started — the stack was stopped on purpose"
    );
}

#[test]
fn a_switch_says_what_moved_in_each_direction() {
    let report = LifecycleReport {
        switched: Some(Switched {
            stopped: vec!["qbittorrent".to_owned()],
            started: vec!["sonarr".to_owned()],
            kept: vec!["jellyfin".to_owned()],
            stop_command: None,
        }),
        ..a_lifecycle("switch", a_plan("media", Vec::new()))
    };
    let text = lifecycle(&report).text();
    assert!(text.contains("stopped: qbittorrent"), "{text}");
    assert!(text.contains("started: sonarr"), "{text}");
    assert!(
        text.contains("kept running: jellyfin"),
        "the one that makes the verb worth having: {text}"
    );
}

/// A switch onto the shape the stack is already in moved nothing at all, and that
/// is an answer. Reporting only the profiles would leave the operator to work out
/// for themselves that nothing happened.
#[test]
fn a_switch_that_moved_nothing_says_so_and_still_shows_what_is_up() {
    let report = LifecycleReport {
        switched: Some(Switched {
            stopped: Vec::new(),
            started: Vec::new(),
            kept: vec!["jellyfin".to_owned(), "seerr".to_owned()],
            stop_command: None,
        }),
        ..a_lifecycle("switch", a_plan("media", Vec::new()))
    };

    let text = lifecycle(&report).text();
    assert!(
        text.contains("already in that shape"),
        "the no-op is stated rather than left to be inferred: {text}"
    );
    assert!(
        text.contains("kept running: jellyfin, seerr"),
        "and what is still up is worth more beside it than alone: {text}"
    );
}

/// A direction that moved nothing says nothing, rather than saying "none" three
/// times to an operator who switched onto a stack that was down.
#[test]
fn a_switch_that_moved_nothing_in_a_direction_leaves_that_direction_unsaid() {
    let report = LifecycleReport {
        switched: Some(Switched {
            stopped: Vec::new(),
            started: vec!["sonarr".to_owned()],
            kept: Vec::new(),
            stop_command: None,
        }),
        ..a_lifecycle("switch", a_plan("media", Vec::new()))
    };
    let text = lifecycle(&report).text();
    assert_eq!(text, "switch: media\nstarted: sonarr");
}

/// A switch that stops something runs two commands, and a rehearsal that printed
/// one of them would be a smaller claim than the thing it is rehearsing.
#[test]
fn a_rehearsed_switch_prints_both_invocations_in_the_order_they_would_run() {
    let report = LifecycleReport {
        rehearsed: true,
        command: vec!["docker".to_owned(), "compose".to_owned(), "up".to_owned()],
        switched: Some(Switched {
            stopped: vec!["qbittorrent".to_owned()],
            started: Vec::new(),
            kept: Vec::new(),
            stop_command: Some(vec![
                "docker".to_owned(),
                "compose".to_owned(),
                "stop".to_owned(),
            ]),
        }),
        ..a_lifecycle("switch", a_plan("media", Vec::new()))
    };
    let text = lifecycle(&report).text();
    let stopping = text.find("compose stop");
    let starting = text.find("compose up");
    assert!(
        stopping.is_some() && starting.is_some() && stopping < starting,
        "the stop is printed first, because it runs first: {text}"
    );
}

/// The operator is told what an operation reaches before it reaches it, and the
/// sentence says which way round. The services are the same either way — only the
/// verb changes — so both directions resolve through the same code.
#[test]
fn what_an_operation_affects_is_said_in_the_direction_it_is_going() {
    let plan = a_plan("media", Vec::new());
    let starting = affects(&plan, Doing::Starting).text();
    let stopping = affects(&plan, Doing::Stopping).text();

    assert!(starting.contains(" starts "), "{starting}");
    assert!(stopping.contains(" stops "), "{stopping}");
    assert!(
        starting.contains("sonarr") && stopping.contains("sonarr"),
        "the same services either way: {starting} / {stopping}"
    );
}

/// Naming two forms at once is the documented way to compose them, so the verb has
/// to agree with a plural subject rather than reading as a corner nobody hits.
#[test]
fn two_forms_named_at_once_take_a_plural_verb() {
    let mut plan = a_plan("media", Vec::new());
    plan.forms = vec!["tv".to_owned(), "movies".to_owned()];
    let said = affects(&plan, Doing::Stopping).text();
    assert!(said.contains("tv and movies stop "), "{said}");
}

#[test]
fn a_plan_says_what_starts_and_what_the_configuration_left_out() {
    let text = preview(&a_plan(
        "tv",
        vec![Dropped {
            profile: "torrent".to_owned(),
            needs: Protocol::Torrent,
        }],
    ))
    .text();
    assert!(
        text.contains("tv starts 1 service: sonarr"),
        "the services, counted, in the operator's own words: {text}"
    );
    assert!(text.contains("left out: torrent — no VPN and torrent client are configured"));
}

#[test]
fn two_forms_named_together_take_a_plural_verb() {
    let composed = Plan {
        forms: vec!["full".to_owned(), "proxy".to_owned()],
        ..a_plan("tv", Vec::new())
    };
    let text = preview(&composed).text();
    assert!(
        text.starts_with("full and proxy start 1 service"),
        "a plural subject takes a plural verb: {text}"
    );
}

#[test]
fn a_plan_that_starts_several_services_counts_them_as_several() {
    let several = Plan {
        services: vec!["sonarr".to_owned(), "bazarr".to_owned()],
        ..a_plan("tv", Vec::new())
    };
    let text = preview(&several).text();
    assert!(text.contains("starts 2 services: sonarr, bazarr"), "{text}");
    assert!(
        !text.contains("left out"),
        "nothing was left out, so nothing is said about it: {text}"
    );
}

#[test]
fn every_condition_reads_as_a_sentence() {
    for condition in [
        Condition::Inactive,
        Condition::Degraded,
        Condition::Partial,
        Condition::Active,
    ] {
        assert!(!describe(condition).is_empty());
    }
    assert_eq!(describe(Condition::Inactive), "nothing is running");
}

#[test]
fn every_service_state_reads_as_a_word_and_a_failure_carries_its_code() {
    let services = vec![
        service("a", State::Absent, None),
        service("b", State::Stopped, None),
        service("c", State::Starting, None),
        service("d", State::Running, None),
        service("e", State::Healthy, None),
        service("f", State::Unhealthy, None),
        service("g", State::CrashLooping, None),
        service("h", State::HostManaged, None),
        service("i", State::Failed, Some(137)),
        service("j", State::Failed, None),
    ];
    let text = show(&services).text();
    for word in [
        "absent",
        "stopped",
        "starting",
        "running",
        "healthy",
        "unhealthy",
        "crash-looping",
        "host-managed",
    ] {
        assert!(text.contains(word), "missing {word}");
    }
    // The exit code is the whole reason a failure is not simply "stopped".
    assert!(text.contains("failed (137)"));
    assert!(text.contains("failed         j service"));
}

#[test]
fn a_status_report_leads_with_the_condition() {
    let report = StatusReport {
        forms: vec!["media".to_owned()],
        active_forms: Vec::new(),
        filtered: Vec::new(),
        condition: Condition::Degraded,
        undeclared: Vec::new(),
        services: vec![service("sonarr", State::Unhealthy, None)],
        disturbs: lemonfiber_core::model::Disturbances::all(lemonfiber_core::app::PATIENCE),
        unsupported: Vec::new(),
    };
    let text = status(&report).text();
    assert!(text.starts_with("running, and something needs attention"));
    assert!(text.contains("unhealthy"));
}

/// What a service is for rides the line an operator is already reading, which is
/// the whole of what having it available where a service is referenced comes to.
#[test]
fn a_service_is_listed_with_what_it_does_for_the_operator() {
    let text = show(&[service("sonarr", State::Healthy, None)]).text();

    assert!(text.contains("what sonarr is for"), "{text}");
    assert!(
        text.contains("sonarr service — what sonarr is for"),
        "and after the name rather than in front of the state: {text}"
    );
}

/// A stack that says nothing about a service gets no dangling dash, because a
/// punctuation mark with nothing after it reads as a rendering that broke.
#[test]
fn a_service_the_stack_describes_with_nothing_is_listed_without_a_dash() {
    let quiet = Service {
        describes: String::new(),
        ..service("sonarr", State::Healthy, None)
    };
    let text = show(&[quiet]).text();

    assert!(!text.contains('—'), "{text}");
    assert!(text.contains("sonarr service"), "{text}");
}

/// A description is prose out of a file an operator can edit, and this is the one
/// new place such prose reaches a terminal — so what a terminal obeys is taken out
/// of it, and taken out before anything is measured.
///
/// Two things are being held here rather than one. A control sequence in the
/// middle of a description would clear the screen the listing was being drawn on;
/// a newline in it would let one service's description draw a line that looked
/// like the next service's row, which is how a stack description comes to hide a
/// service that is down behind a service that is not.
#[test]
fn a_description_cannot_take_over_the_screen_or_forge_a_row() {
    let hostile = Service {
        describes: "fine\u{1b}[2J\nqbittorrent     healthy        all is well".to_owned(),
        ..service("sonarr", State::Healthy, None)
    };
    let text = show(&[hostile]).text();

    assert!(!text.contains('\u{1b}'), "{text:?}");
    assert_eq!(
        text.lines().count(),
        1,
        "one service is one row, whatever the description says: {text:?}"
    );
}

/// A container nobody declared is shown with an unknown description rather than
/// dropped from the listing, and shown apart from the services — the one thing
/// that can be said about it is that nothing here knows what it is.
#[test]
fn a_container_the_stack_never_declared_is_shown_with_an_unknown_description() {
    let report = StatusReport {
        forms: Vec::new(),
        active_forms: Vec::new(),
        filtered: Vec::new(),
        condition: Condition::Inactive,
        undeclared: vec![Undeclared {
            id: "something-of-their-own".to_owned(),
            state: State::Running,
            describes: lemonfiber_core::docker::UNDESCRIBED.to_owned(),
        }],
        services: vec![service("sonarr", State::Absent, None)],
        disturbs: lemonfiber_core::model::Disturbances::all(lemonfiber_core::app::PATIENCE),
        unsupported: Vec::new(),
    };
    let text = status(&report).text();

    assert!(
        text.contains(
            "1 container running under this project that the stack does not \
                       declare:"
        ),
        "{text}"
    );
    assert!(text.contains("something-of-their-own"), "{text}");
    assert!(text.contains("not declared by this stack"), "{text}");
}

/// A stranger that failed is said to have failed, and nothing more.
///
/// The word is the whole of the answer here: a declared service that failed is
/// shown with the code it exited on, and there is no service behind a container
/// the manifest never named to ask one of. Two spellings of the same state on one
/// screen would read as two different things having happened.
#[test]
fn a_stranger_that_failed_is_worded_without_an_exit_code_to_quote() {
    let report = StatusReport {
        forms: Vec::new(),
        active_forms: Vec::new(),
        filtered: Vec::new(),
        condition: Condition::Inactive,
        undeclared: vec![Undeclared {
            id: "something-that-fell-over".to_owned(),
            state: State::Failed,
            describes: lemonfiber_core::docker::UNDESCRIBED.to_owned(),
        }],
        services: vec![service("sonarr", State::Absent, None)],
        disturbs: lemonfiber_core::model::Disturbances::all(lemonfiber_core::app::PATIENCE),
        unsupported: Vec::new(),
    };
    let text = status(&report).text();

    assert!(text.contains("something-that-fell-over"), "{text}");
    assert!(text.contains("failed"), "{text}");
    assert!(
        !text.contains("exit"),
        "there is no service behind a stranger to ask for a code: {text}"
    );
}

/// And an ordinary stack says nothing about it at all, rather than carrying a
/// heading over an empty list on every run.
#[test]
fn a_stack_with_nothing_strange_running_says_nothing_about_strangers() {
    let report = StatusReport {
        forms: Vec::new(),
        active_forms: Vec::new(),
        filtered: Vec::new(),
        condition: Condition::Inactive,
        undeclared: Vec::new(),
        services: vec![service("sonarr", State::Absent, None)],
        disturbs: lemonfiber_core::model::Disturbances::all(lemonfiber_core::app::PATIENCE),
        unsupported: Vec::new(),
    };

    assert!(!status(&report).text().contains("does not declare"));
}

/// A stack the operator maintains, carrying a declaration lemonfiber cannot reach.
#[test]
fn a_service_lemonfiber_cannot_speak_to_is_named_with_why_and_is_not_a_fault() {
    let report = StatusReport {
        forms: Vec::new(),
        active_forms: Vec::new(),
        filtered: Vec::new(),
        condition: Condition::Active,
        services: vec![service("theirs", State::Healthy, None)],
        unsupported: vec![UnsupportedReport {
            what: "theirs".to_owned(),
            because: "it declares an API of the Servarr shape and no port to publish".to_owned(),
        }],
        undeclared: Vec::new(),
        disturbs: lemonfiber_core::model::Disturbances::all(lemonfiber_core::app::PATIENCE),
    };
    let text = status(&report).text();
    assert!(text.contains("needs to know what they are"), "{text}");
    assert!(text.contains("theirs — it declares an API"), "{text}");
    // Still up, and still reported as up: what is unsupported is a feature, not
    // the service.
    assert!(text.contains("everything is up"), "{text}");
}

/// And nothing is said at all where there is nothing to say, which is every stack
/// this build ships.
#[test]
fn a_stack_with_nothing_unsupported_says_nothing_about_it() {
    let report = StatusReport {
        forms: Vec::new(),
        active_forms: Vec::new(),
        filtered: Vec::new(),
        condition: Condition::Active,
        services: vec![service("sonarr", State::Healthy, None)],
        unsupported: Vec::new(),
        undeclared: Vec::new(),
        disturbs: lemonfiber_core::model::Disturbances::all(lemonfiber_core::app::PATIENCE),
    };
    assert!(!status(&report).text().contains("needs to know"));
}

#[test]
fn a_watch_reports_whether_it_stopped_what_it_guarded() {
    assert!(watch(&a_watch()).text().contains("stopped: media"));
    let stranded = SupervisionReport {
        stopped: false,
        ..a_watch()
    };
    assert!(watch(&stranded).text().contains("could not stop media"));
}

#[test]
fn a_rehearsed_watch_prints_the_invocation_it_would_run_and_the_guard_it_would_keep() {
    let rehearsed = SupervisionReport {
        forms: Vec::new(),
        would: Some(Vigil {
            root: "/srv/library".to_owned(),
            every: 5,
            command: vec!["docker".to_owned(), "compose".to_owned(), "stop".to_owned()],
        }),
        ..a_watch()
    };
    let text = watch(&rehearsed).text();
    assert!(text.contains("docker compose stop"), "{text}");
    assert!(text.contains("/srv/library"), "{text}");
    assert!(text.contains("every 5s"), "{text}");
    assert!(
        text.contains("the whole stack"),
        "naming no form is a guard over all of them: {text}"
    );
    assert!(
        !text.contains("the watch ended"),
        "nothing ended, and saying so would be the rehearsal claiming to be the thing: {text}"
    );
}

/// A rehearsed watch over named forms says which ones it would stop.
///
/// The counterpart above names none and gets the sentence for all of them, and both
/// halves are worth pinning because an operator setting a guard is deciding exactly
/// this: whether the thing that goes down when the drive does is the part they meant
/// or the whole stack. A line that ended in nothing would read as a guard that stops
/// nothing, and one that said "the whole stack" over two named forms would read as a
/// far bigger promise than the guard makes.
#[test]
fn a_rehearsed_watch_over_named_forms_says_which_ones_it_would_stop() {
    let rehearsed = SupervisionReport {
        forms: vec!["library".to_owned(), "search".to_owned()],
        would: Some(Vigil {
            root: "/srv/library".to_owned(),
            every: 5,
            command: vec!["docker".to_owned(), "compose".to_owned(), "stop".to_owned()],
        }),
        ..a_watch()
    };

    let text = watch(&rehearsed).text();

    assert!(text.contains("stopping library, search"), "{text}");
    assert!(
        !text.contains("the whole stack"),
        "a guard over two named forms was described as one over all of them: {text}"
    );
}

#[test]
fn a_watch_read_by_a_script_is_the_envelope_every_other_answer_arrives_in() {
    // Through the outcome rather than through a rendering of its own, so a
    // guard cannot come to describe its ending differently from the rest.
    let json =
        crate::render::machine_readable(&lemonfiber_core::app::Outcome::Watch(a_watch())).text();
    assert!(json.contains(r#""kind":"watch""#), "{json}");
    assert!(json.contains(r#""stopped":true"#), "{json}");
}

/// Each way a footprint can stand, said as the stack's estimate every time.
#[test]
fn a_preview_states_the_memory_the_stack_estimates() {
    let said = |estimated_mib, unestimated: &[&str]| {
        let plan = Plan {
            footprint: Footprint {
                estimated_mib,
                unestimated: unestimated.iter().map(|id| (*id).to_owned()).collect(),
            },
            ..a_plan("tv", Vec::new())
        };
        preview(&plan).text()
    };
    assert!(!said(0, &[]).contains("memory"));
    assert!(said(0, &["sonarr"]).contains("the stack states no memory estimate for sonarr"));
    assert!(said(900, &[]).contains("the stack estimates about 900 MiB of memory"));
    let partial = said(900, &["sonarr", "bazarr"]);
    assert!(
        partial.contains(
            "about 900 MiB of memory, leaving out sonarr, bazarr, which state no estimate"
        ),
        "{partial}"
    );
}

/// Status says which forms are running and what they left out, beside the services.
#[test]
fn a_status_report_names_the_running_forms_and_what_they_left_out() {
    let report = StatusReport {
        forms: Vec::new(),
        active_forms: vec!["tv".to_owned(), "movies".to_owned()],
        filtered: vec![Filtered {
            id: "gluetun".to_owned(),
            name: "Gluetun".to_owned(),
            profile: "torrent".to_owned(),
            needs: Protocol::Torrent,
            forms: vec!["tv".to_owned(), "movies".to_owned()],
        }],
        condition: Condition::Active,
        undeclared: Vec::new(),
        services: vec![service("sonarr", State::Healthy, None)],
        disturbs: lemonfiber_core::model::Disturbances::all(lemonfiber_core::app::PATIENCE),
        unsupported: Vec::new(),
    };
    let text = status(&report).text();
    assert!(text.contains("running for: tv, movies"), "{text}");
    assert!(
        text.contains("left out: Gluetun — no VPN and torrent client are configured"),
        "{text}"
    );
}
