//! A sample of every command, grouped by what it acts on.

use super::a_plugin_source;
use lemonfiber_core::app::{
    plugins, AlertAction, Arranged, Asking, BandwidthAsked, Chosen, Command, Decision, Filling,
    Keeping, Linking, MigrateAction, QualityAction, Removing, Setting, SetupAction, Waiting,
};
use lemonfiber_core::config::Protocols;
use lemonfiber_core::doctor::Narrowing;

/// The subcommands that never become a [`Command`], and why.
///
/// Named rather than filtered by shape, so that adding a fourth means saying what it
/// is. Following a log is a stream rather than an answer; and the terminal is a
/// program that then issues commands of its own, each of which arrives here under
/// its own name. `help` is clap's, not lemonfiber's, and answers nothing.
///
/// `plugin` was on this list while every word under it was a generated document
/// answering the same on every machine. Two of them are not: one settles what
/// installing somebody else's plugin decides and writes it down, and the other reads
/// that back. So the word is dispatched now and is driven below, which is what makes
/// the sentence this list is for — *reading one asks nothing of this machine* — stop
/// covering a word that writes.
pub(super) const NOT_DISPATCHED: [&str; 3] = ["logs", "ui", "help"];

/// What a rehearsal of each subcommand is driven with.
///
/// One sample per subcommand, chosen to be the shape that changes the most: a
/// `config set` rather than a `config show`, a rotate rather than a read. A sample
/// that took the harmless branch would pass this while saying nothing about the one
/// that matters.
pub(super) fn samples() -> Vec<(&'static str, Command)> {
    let mut all = over_the_stack();
    all.extend(over_the_household());
    all.extend(over_what_this_machine_keeps());
    all
}

/// The stack itself, and the settings that describe it.
fn over_the_stack() -> Vec<(&'static str, Command)> {
    vec![
        ("version", Command::Version),
        ("forms", Command::Forms),
        (
            "migrate",
            Command::Migrate(MigrateAction::Act {
                mode: lemonfiber_core::migration::mode::Mode::Adopt,
                confirmed: true,
            }),
        ),
        ("up", Command::Up { forms: Vec::new() }),
        (
            "down",
            Command::Down {
                forms: Vec::new(),
                wait: Waiting::ForTheDownloads,
            },
        ),
        (
            "switch",
            Command::Switch {
                forms: vec!["library".to_owned()],
            },
        ),
        (
            "restart",
            Command::Restart {
                forms: Vec::new(),
                services: Vec::new(),
            },
        ),
        ("pull", Command::Pull { forms: Vec::new() }),
        ("ps", Command::Ps { forms: Vec::new() }),
        (
            "config",
            Command::ConfigSet(Setting::to("DATA_ROOT", "/srv/library").agreed(true)),
        ),
        (
            "alerts",
            Command::Alerts(AlertAction::Set(
                lemonfiber_core::alert::Appetite::Everything,
            )),
        ),
        (
            "quality",
            Command::Quality(QualityAction::Set {
                preset: lemonfiber_core::quality::Preset::Maximum,
                media_type: None,
                confirm: true,
            }),
        ),
        (
            "doctor",
            Command::Doctor {
                narrowing: Narrowing::Suite,
                disruptive: false,
                accept: Some("storage.one-filesystem".to_owned()),
            },
        ),
        ("watch", Command::Watch { forms: Vec::new() }),
        (
            "hosting",
            Command::Hosting(Keeping::Install {
                what: lemonfiber_core::app::Hostable::Watch,
                forms: Vec::new(),
            }),
        ),
    ]
}

/// The people this stack is for, and what is asked on their behalf.
fn over_the_household() -> Vec<(&'static str, Command)> {
    vec![
        (
            "trace",
            Command::Trace {
                term: "anything".to_owned(),
                season: None,
                searching: true,
            },
        ),
        ("household", Command::Household { member: None }),
        (
            "held",
            Command::Held {
                member: "anybody".to_owned(),
                most: 25,
            },
        ),
        ("walkthrough", Command::Walkthrough { item: None }),
        (
            "explain",
            Command::Explain {
                word: "seeding".to_owned(),
            },
        ),
        ("history", Command::History),
        ("undo", Command::Undo { run: None }),
        ("stuck", Command::Stuck),
        ("front-door", Command::FrontDoor),
        ("outbound", Command::Outbound),
        ("provenance", Command::Provenance),
        ("catalogue", Command::Catalogue),
        // The verb rather than the read, because the read writes nothing under any
        // flag and a rehearsal of it would be proving that a listing lists.
        (
            "wiring",
            Command::Wiring(Linking::Fill(Filling {
                capability: "indexer.search".to_owned(),
                service: "nzbhydra2".to_owned(),
            })),
        ),
        (
            "credentials",
            Command::Credentials(Asking::Rotate {
                credential: "qbittorrent".to_owned(),
            }),
        ),
        ("stored", Command::Stored),
        ("clients", Command::Clients),
        (
            "invite",
            Command::Invite {
                name: "ana".to_owned(),
                allowance: lemonfiber_core::app::Allowance::default(),
            },
        ),
        (
            "reissue",
            Command::Reissue {
                name: "ana".to_owned(),
            },
        ),
        (
            "remove",
            Command::Remove {
                name: "ana".to_owned(),
                confirm: true,
            },
        ),
    ]
}

/// What this machine keeps, and the commands that take it away or put it back.
fn over_what_this_machine_keeps() -> Vec<(&'static str, Command)> {
    vec![
        ("forget", Command::Forget { confirm: true }),
        (
            "uninstall",
            Command::Uninstall(Removing {
                tier: lemonfiber_core::uninstall::Tier::Configuration,
                confirm: true,
                agreement: None,
                waiting: Waiting::Never,
            }),
        ),
        ("space", Command::Space { confirm: true }),
        (
            "stop-seeding",
            Command::StopSeeding {
                download: "anything".to_owned(),
                agreement: None,
            },
        ),
        (
            "bandwidth",
            Command::Bandwidth(BandwidthAsked {
                down: Some("20".to_owned()),
                ..BandwidthAsked::default()
            }),
        ),
        ("seed", Command::Seed),
        ("adopt", Command::Adopt),
        ("reset", Command::Reset { confirm: true }),
        (
            "update",
            Command::Update(lemonfiber_core::update::run::Asked {
                service: None,
                confirm: true,
                wait: Waiting::Never,
            }),
        ),
        ("backup", Command::Backup { service: None }),
        // The one write under `plugin`, and the shape a rehearsal most easily gets
        // wrong: everything before the write settles what the install decides, so a
        // handler reading the flag late would have the record on disk before it
        // decided not to write one.
        (
            "plugin",
            Command::Plugins(plugins::Asked::Install {
                path: a_plugin_source(),
            }),
        ),
        (
            "support",
            Command::Support {
                write: true,
                wanted: lemonfiber_core::bundle::run::Wanted::default(),
                dest: lemonfiber_core::app::support::Destination::Kept,
            },
        ),
        (
            "restore",
            Command::Restore {
                archive: lemonfiber_core::app::restore::Kept::Named("anything".to_owned()),
                repoint: false,
                consent: lemonfiber_core::app::restore::Consent::Standing,
            },
        ),
        ("setup", Command::Setup(SetupAction::Apply)),
    ]
}

/// The commands no subcommand of its own carries, driven all the same.
///
/// The dashboard and the web surface reach these, and both hand the core the same
/// context the command line does — so a rehearsal of one is a rehearsal, and there
/// is no reason for them to be the only ones nothing asks.
pub(super) fn beyond_the_command_line() -> Vec<(&'static str, Command)> {
    vec![
        (
            "preview",
            Command::Preview {
                forms: vec!["library".to_owned()],
            },
        ),
        (
            "start",
            Command::Start {
                forms: Vec::new(),
                services: Vec::new(),
            },
        ),
        (
            "halt",
            Command::Halt {
                forms: Vec::new(),
                services: Vec::new(),
            },
        ),
        (
            "config-get",
            Command::ConfigGet {
                key: "DATA_ROOT".to_owned(),
            },
        ),
        ("config-show", Command::ConfigShow),
        ("glossary", Command::Glossary),
        ("archives", Command::Archives),
        ("quality-upgrade", Command::QualityUpgrade { confirm: true }),
        (
            "quality-music",
            Command::QualityMusic {
                format: lemonfiber_core::audio::Format::Lossless,
            },
        ),
        ("allowing", Command::Allowing(Chosen::default())),
        (
            "deciding",
            Command::Deciding(Decision {
                request: 1,
                answer: lemonfiber_core::app::Answer::LetThrough,
            }),
        ),
        ("expiring", Command::Expiring(Arranged::After(30))),
        // `up --at-boot` is a flag rather than a subcommand, so the sample above
        // cannot reach it — and it is the one command here that writes three records
        // of its own, which is exactly the shape a rehearsal gets wrong.
        ("at-boot", Command::AtBoot),
        ("self-update", Command::SelfUpdate { to: None }),
        (
            "repair",
            Command::Repair {
                consent: lemonfiber_core::repair::run::Consent::Standing,
                disruptive: false,
            },
        ),
        // The other half of `undo`. Naming a run reaches a different function from
        // naming none — one reads the journal for the last repair, the other for the
        // run a stamp names — and only the second judges the whole run before acting.
        (
            "undo-a-named-run",
            Command::Undo {
                run: Some("2026-07-30T00:00:00Z".to_owned()),
            },
        ),
        // The read half of a support bundle, which is what a rehearsal of the writing
        // half becomes. Driven under its own name so a failure says which of the two
        // wrote something.
        (
            "support-described",
            Command::Support {
                write: false,
                wanted: lemonfiber_core::bundle::run::Wanted::default(),
                dest: lemonfiber_core::app::support::Destination::Kept,
            },
        ),
        // Answering a setup question, which writes the resumable progress file — the
        // one write in the configuration home that no other sample here reaches.
        (
            "setup-answer",
            Command::Setup(SetupAction::Answer(
                lemonfiber_core::wizard::Answer::Protocols(Protocols::both()),
            )),
        ),
    ]
}
