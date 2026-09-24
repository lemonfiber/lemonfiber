use super::{
    asked, carried, not_taught_yet, refused, repair, restore, update, verdict, Asked, Rehearsal,
    A_SEARCH_IS_THE_ANSWER, THE_CHECK_IS_THE_DISRUPTION, THE_WALK_IS_THE_OBSERVATION,
};
use crate::app::command::{
    AlertAction, Arranged, Asking, BandwidthAsked, Chosen, Decision, Filling, Keeping, Linking,
    MigrateAction, QualityAction, Removing, Setting,
};
use crate::app::engine::Waiting;
use crate::app::plugins;
use crate::app::setup::SetupAction;
use crate::app::Command;

/// A command that has not been taught to rehearse is refused rather than run.
///
/// Nothing carries this verdict today — every command that changes something has
/// since been taught to say what it would change — so the answer is handed over
/// directly rather than asked of a command. That is the point of it: this is the
/// escape hatch whoever adds the next command gets, and an escape hatch nothing
/// has ever been through is one nobody knows the shape of. What it must do is
/// refuse, and refuse under its own code: an operator who typed `--dry-run` and
/// was quietly run for real is the failure the whole flag exists to prevent, and
/// "this one has not been taught yet" is a different thing to be told from "this
/// one cannot be rehearsed at all".
#[test]
fn a_command_nobody_has_taught_to_rehearse_is_refused_under_its_own_code() {
    let untaught = Asked {
        named: "invent",
        rehearsal: Rehearsal::Untaught,
        disturbs: None,
    };

    // Carried as a `Result` rather than opened with a `let ... else`. The else
    // arm is a region no passing run enters, and the coverage gate counts it
    // against this file — the same reason the test below reaches for
    // `reasoning` instead of `matches!`.
    let answer = verdict(&untaught).map_err(|refusal| refusal.code);

    assert_eq!(answer, Err(not_taught_yet(untaught.named).code));
    assert_ne!(
        answer,
        Err(refused(&untaught, THE_WALK_IS_THE_OBSERVATION).code),
        "not taught yet and cannot be rehearsed are different things to be told"
    );
}

/// The two refusals are told apart by their code, which is what an operator
/// searches for and what a surface keys off.
#[test]
fn the_two_refusals_carry_codes_of_their_own() {
    // `not_taught_yet` builds its refusal from the command's name rather than from
    // its verdict, so any command names one — which matters, because nothing
    // carries `Untaught` today. What is asked here is about the two sentences, not
    // about which command happens to be on the board when somebody reads them.
    let untaught = asked(&Command::Seed);
    let never = asked(&Command::Walkthrough { item: None });
    // Read with `reasoning` rather than with `matches!`, which expands to a match
    // whose second arm is only taken when the assertion is about to fail — a region
    // no passing run enters and one the coverage gate counts.
    assert!(
        reasoning(never.rehearsal).is_some(),
        "a walkthrough should refuse the flag outright"
    );
    // Put through the decision as well as read off the verdict. This crate is
    // compiled twice and each copy is counted on its own, so an arm entered only
    // by the other copy is an arm this one is charged for.
    assert!(
        verdict(&never).is_err(),
        "the decision refuses what cannot be rehearsed"
    );
    assert_ne!(
        not_taught_yet(untaught.named).code,
        refused(&never, "the reason it gives").code,
        "a gap being closed and a limitation that will not change read alike"
    );
}

/// A refusal says which command refused, because an operator running a script
/// sees the message without the command beside it.
#[test]
fn a_refusal_names_the_command_it_refused() {
    let one = asked(&Command::Seed);
    assert!(
        not_taught_yet(one.named).summary.contains("seed"),
        "a refusal that does not name the command leaves the operator guessing"
    );
}

/// Reading a credential and rotating one arrive as the same command, and only one
/// of them changes anything. A verdict taken at the outer variant would hold the
/// read to a rule written for the write.
#[test]
fn reading_a_credential_is_not_read_the_way_rotating_one_is() {
    assert!(asked(&Command::Credentials(Asking::Read)).rehearsal == Rehearsal::Reads);
    assert!(
        asked(&Command::Credentials(Asking::Rotate {
            credential: "qbittorrent".to_owned(),
        }))
        .rehearsal
            == Rehearsal::Reports
    );
}

/// The reason a verdict gives, where it gives one.
///
/// Here rather than on `Rehearsal` because here is the only place that asks. A
/// verdict carries its reason and `permitted` reads it out of the pattern, so an
/// accessor beside it was a second way to ask one question — and the shipped
/// build never called it, which is a whole function's worth of lines nothing
/// enters and the coverage gate counts.
fn reasoning(rehearsal: Rehearsal) -> Option<&'static str> {
    match rehearsal {
        Rehearsal::Cannot(why) => Some(why),
        Rehearsal::Reads | Rehearsal::Reports | Rehearsal::Untaught => None,
    }
}

/// A trace, asked with and without a live search.
fn tracing(searching: bool) -> Command {
    Command::Trace {
        term: "anything".to_owned(),
        season: None,
        searching,
    }
}

/// A doctor run, disruptive or not, acknowledging nothing.
fn examining(disruptive: bool) -> Command {
    Command::Doctor {
        narrowing: crate::doctor::Narrowing::Suite,
        disruptive,
        accept: None,
    }
}

/// A support bundle, written out or only described.
fn bundling(write: bool) -> Command {
    Command::Support {
        write,
        wanted: crate::bundle::run::Wanted::default(),
        dest: crate::app::support::Destination::Kept,
    }
}

/// The three that refuse the flag for good say so, the read half of each command
/// that shares a name with one does not, and the reason reaches the operator.
///
/// Driven rather than read, because the two halves of `trace` and of `doctor` are
/// one word to an operator and two arms here, and an arm nothing reaches is an arm
/// whose pattern could be wrong in either direction without anything saying so.
#[test]
fn what_cannot_be_rehearsed_is_told_from_the_read_beside_it() {
    for (command, refuses) in [
        (tracing(true), true),
        (tracing(false), false),
        (examining(true), true),
        (examining(false), false),
        (Command::Walkthrough { item: None }, true),
        (bundling(false), false),
    ] {
        let asked = asked(&command);
        assert!(
            reasoning(asked.rehearsal).is_some() == refuses,
            "{command:?} was read as the wrong one of the two"
        );
        // The reason reaches the operator rather than staying in the source, which
        // is the difference between refusing and refusing usefully.
        if let Some(why) = reasoning(asked.rehearsal) {
            assert!(refused(&asked, why).meaning.contains(why), "{command:?}");
        }
    }
}

/// Every command, against the answer a rehearsal of it gives.
///
/// The match is exhaustive, so the compiler already refuses a new command with no
/// verdict. What it cannot refuse is a *wrong* verdict, or a pattern catching more
/// than it means — and an arm nothing drives is an arm whose pattern could be wrong
/// in either direction with nothing to say so. Forty-five of these had never been
/// read by anything.
///
/// Here rather than beside the integration test that dispatches them, and the
/// reason is not tidiness. This file is compiled twice — once into the binary and
/// once for its own tests — and a table living in another crate leaves this copy's
/// arms unentered however thoroughly the other copy is driven. The coverage gate
/// counts both, which is how a module every line of which is reached came to read
/// as ninety-four per cent.
///
/// The sub-patterns are listed beside the variants they split, because the split is
/// where the mistake lives: `Support { write: false }` and `Support { .. }` are one
/// word to an operator and two arms here.
///
/// One group below per verdict, and the verdict written once for the group rather
/// than once per row. A row under the wrong heading used to be a row that still
/// declared the right answer beside itself, and read correctly while sitting in the
/// wrong place; now the heading is the answer.
fn every_verdict() -> Vec<(Command, Rehearsal)> {
    fn under(commands: Vec<Command>, verdict: Rehearsal) -> Vec<(Command, Rehearsal)> {
        commands
            .into_iter()
            .map(|command| (command, verdict))
            .collect()
    }

    let mut every = under(reads(), Rehearsal::Reads);
    every.extend(refused_for_good());
    every.extend(under(reports(), Rehearsal::Reports));
    every.extend(under(untaught(), Rehearsal::Untaught));
    every
}

/// The commands a rehearsal runs exactly as it always does.
///
/// Nothing here reaches for anything it could put back, so there is nothing to
/// hold off on and no report to give in place of the work.
fn reads() -> Vec<Command> {
    vec![
        Command::Version,
        Command::Forms,
        Command::Preview {
            forms: vec!["library".to_owned()],
        },
        Command::ConfigGet {
            key: "DATA_ROOT".to_owned(),
        },
        Command::ConfigShow,
        Command::History,
        Command::Ps { forms: Vec::new() },
        Command::Stuck,
        Command::FrontDoor,
        Command::Explain {
            word: "seeding".to_owned(),
        },
        Command::Glossary,
        Command::Clients,
        Command::Catalogue,
        Command::Outbound,
        Command::Provenance,
        Command::Stored,
        Command::Plugins(plugins::Asked::Installed),
        Command::Archives,
        Command::Migrate(MigrateAction::Survey),
        Command::Credentials(Asking::Read),
        Command::Credentials(Asking::Reveal {
            credential: "qbittorrent".to_owned(),
            confirmed: true,
        }),
        Command::Hosting(Keeping::Read),
        Command::Setup(SetupAction::Where),
        Command::SelfUpdate { to: None },
        bundling(false),
        tracing(false),
        examining(false),
    ]
}

/// The three that refuse the flag for good, each with its own reason.
///
/// Listed with their reasons rather than under one verdict, because here the
/// reason is the whole of the verdict: they differ in nothing else.
fn refused_for_good() -> Vec<(Command, Rehearsal)> {
    vec![
        (tracing(true), Rehearsal::Cannot(A_SEARCH_IS_THE_ANSWER)),
        (
            examining(true),
            Rehearsal::Cannot(THE_CHECK_IS_THE_DISRUPTION),
        ),
        (
            Command::Walkthrough { item: None },
            Rehearsal::Cannot(THE_WALK_IS_THE_OBSERVATION),
        ),
    ]
}

/// The commands that report instead of acting.
///
/// Each builds what it would have filled in and stops before the step it cannot
/// take back.
///
/// Three lists rather than one, because one of them outgrew the length rule and
/// the seams were already written into it as comments. They are the three ways a
/// command comes to rehearse: it always did, it was taught to, or it already had
/// an answer for an unconfirmed run and a rehearsal is that answer with the yes
/// taken back on the way in.
fn reports() -> Vec<Command> {
    let mut every = always_reported();
    every.extend(taught_to_report());
    every.extend(answering_twice());
    every
}

/// The ones that reported before any of this: the flag was read where it mattered
/// and the write was never reached.
fn always_reported() -> Vec<Command> {
    vec![
        Command::Up { forms: Vec::new() },
        Command::Start {
            forms: Vec::new(),
            services: Vec::new(),
        },
        Command::Down {
            forms: Vec::new(),
            wait: Waiting::Never,
        },
        Command::Halt {
            forms: Vec::new(),
            services: Vec::new(),
        },
        Command::Switch {
            forms: vec!["library".to_owned()],
        },
        Command::Restart {
            forms: Vec::new(),
            services: Vec::new(),
        },
        Command::Pull { forms: Vec::new() },
        Command::ConfigSet(Setting::to("DATA_ROOT", "/srv/library").agreed(true)),
        Command::Quality(QualityAction::Set {
            preset: crate::quality::Preset::Maximum,
            media_type: None,
            confirm: true,
        }),
        Command::Alerts(AlertAction::Set(crate::alert::Appetite::Everything)),
        Command::QualityMusic {
            format: crate::audio::Format::Lossless,
        },
        Command::Household { member: None },
        Command::Held {
            member: "ana".to_owned(),
            most: 25,
        },
        Command::Allowing(Chosen::default()),
        Command::Deciding(Decision {
            request: 1,
            answer: crate::app::Answer::LetThrough,
        }),
        Command::Expiring(Arranged::After(30)),
        Command::Hosting(Keeping::Install {
            what: crate::app::Hostable::Watch,
            forms: Vec::new(),
        }),
        Command::Invite {
            name: "ana".to_owned(),
            allowance: crate::app::Allowance::default(),
        },
        Command::Reissue {
            name: "ana".to_owned(),
        },
        Command::Forget { confirm: true },
        Command::Space { confirm: true },
        Command::StopSeeding {
            download: "anything".to_owned(),
            agreement: None,
        },
        Command::Bandwidth(BandwidthAsked {
            down: Some("20".to_owned()),
            ..BandwidthAsked::default()
        }),
        Command::Uninstall(Removing {
            tier: crate::uninstall::Tier::Configuration,
            confirm: true,
            agreement: None,
            waiting: Waiting::Never,
        }),
    ]
}

/// Taught to report rather than to act: each stops short of the write and says
/// what the write would have been.
fn taught_to_report() -> Vec<Command> {
    vec![
        examining_accepting(),
        Command::Watch { forms: Vec::new() },
        Command::Undo { run: None },
        Command::Credentials(Asking::Rotate {
            credential: "qbittorrent".to_owned(),
        }),
        Command::Setup(SetupAction::Apply),
        Command::Backup { service: None },
        Command::Plugins(plugins::Asked::Install {
            path: std::path::PathBuf::from("/srv/komga"),
        }),
        Command::Plugins(plugins::Asked::Update {
            path: std::path::PathBuf::from("komga"),
        }),
        Command::Plugins(plugins::Asked::Remove {
            plugin: "komga".to_owned(),
        }),
    ]
}

/// The ones that answer twice.
///
/// Here rather than under `Untaught` because the answer each gives unconfirmed is
/// the report a rehearsal wants, and `carried` is what takes the yes back on the
/// way in.
fn answering_twice() -> Vec<Command> {
    vec![
        Command::Migrate(MigrateAction::Act {
            mode: crate::migration::mode::Mode::Adopt,
            confirmed: true,
        }),
        Command::Remove {
            name: "ana".to_owned(),
            confirm: true,
        },
        Command::QualityUpgrade { confirm: true },
        Command::Repair {
            consent: crate::repair::run::Consent::Standing,
            disruptive: false,
        },
        Command::Reset { confirm: true },
        Command::Update(crate::update::run::Asked {
            service: None,
            confirm: true,
            wait: Waiting::Never,
        }),
        Command::Restore {
            archive: crate::app::restore::Kept::Named("anything".to_owned()),
            repoint: false,
            consent: crate::app::restore::Consent::Standing,
        },
        bundling(true),
        Command::Seed,
        Command::Adopt,
    ]
}

/// The commands that change something and have not been taught to say what.
///
/// Empty, and kept rather than deleted: the verdict it stands for is still one of
/// the four, a command added tomorrow can still be given it, and a reader comparing
/// this table with the match wants to see that nothing carries it rather than to
/// find the row missing. `every_command_is_answered_the_way_the_table_says` reads
/// it whatever it holds.
fn untaught() -> Vec<Command> {
    Vec::new()
}

/// A doctor run acknowledging a finding, which is the third of its three arms.
fn examining_accepting() -> Command {
    Command::Doctor {
        narrowing: crate::doctor::Narrowing::Suite,
        disruptive: false,
        accept: Some("storage.one-filesystem".to_owned()),
    }
}

/// Every command gives the verdict this module says it gives.
#[test]
fn every_command_is_answered_the_way_the_table_says() {
    // `then_some` rather than an `if` with a body, and the name built before
    // the comparison rather than inside it. A block that runs only when a row
    // disagrees is a block nothing enters while the table is right, and the
    // coverage gate counts regions: this test passing is exactly the condition
    // under which the old shape read as an unreached line. Naming all
    // sixty-five commands to use none of them costs nothing here.
    let wrong: Vec<String> = every_verdict()
        .into_iter()
        .filter_map(|(command, expected)| {
            let named = format!("{command:?}");
            (asked(&command).rehearsal != expected).then_some(named)
        })
        .collect();

    assert!(
        wrong.is_empty(),
        "these were read as a verdict other than the one declared for them: {wrong:?}"
    );
}

/// A rehearsal of the eight that answer twice is the answer they already give
/// unconfirmed, so the go-ahead is taken back on the way in.
#[test]
fn a_rehearsal_carries_the_confirmable_commands_without_their_yes() {
    let rehearsing = crate::test_support::a_context().build().rehearsing();
    for asked in [
        Command::Reset { confirm: true },
        Command::Remove {
            name: "ana".to_owned(),
            confirm: true,
        },
        Command::QualityUpgrade { confirm: true },
        Command::Migrate(MigrateAction::Act {
            mode: crate::migration::mode::Mode::Adopt,
            confirmed: true,
        }),
        Command::Update(update::Asked {
            service: None,
            confirm: true,
            wait: Waiting::Never,
        }),
        Command::Restore {
            archive: restore::Kept::Named("anything".to_owned()),
            repoint: false,
            consent: restore::Consent::Standing,
        },
        Command::Repair {
            consent: repair::Consent::Standing,
            disruptive: false,
        },
        bundling(true),
    ] {
        let carried = carried(asked.clone(), &rehearsing);
        assert_ne!(carried, asked, "{asked:?} kept the yes it was given");
    }
}

/// And what a rehearsal of a support bundle carries is the read beside it, rather
/// than merely something other than what was asked.
///
/// Its own case because `assert_ne!` above is satisfied by any difference, and the
/// difference that matters here is which of the command's two halves runs: the
/// describing one, which is already classified as changing nothing.
#[test]
fn a_rehearsed_support_bundle_is_the_run_that_describes_one() {
    let rehearsing = crate::test_support::a_context().build().rehearsing();
    assert_eq!(carried(bundling(true), &rehearsing), bundling(false));
}

/// And a real run carries exactly what it was handed, because the withholding is
/// what a rehearsal is rather than something the dispatcher does to everybody.
#[test]
fn a_real_run_carries_the_command_it_was_given() {
    let real = crate::test_support::a_context().build();
    let asked = Command::Reset { confirm: true };
    assert_eq!(carried(asked.clone(), &real), asked);
}

/// The same split, on the three other commands that carry a read and a write under
/// one name.
#[test]
fn every_command_that_carries_both_is_split_on_which_it_is() {
    for (command, expected) in [
        (Command::Migrate(MigrateAction::Survey), Rehearsal::Reads),
        (Command::Hosting(Keeping::Read), Rehearsal::Reads),
        (Command::Setup(SetupAction::Where), Rehearsal::Reads),
        (Command::Setup(SetupAction::Apply), Rehearsal::Reports),
        (Command::Wiring(Linking::Read), Rehearsal::Reads),
        (
            Command::Wiring(Linking::Fill(Filling {
                capability: "indexer.search".to_owned(),
                service: "nzbhydra2".to_owned(),
            })),
            Rehearsal::Reports,
        ),
    ] {
        assert!(
            asked(&command).rehearsal == expected,
            "{command:?} was read as the wrong half of what it carries"
        );
    }
}
