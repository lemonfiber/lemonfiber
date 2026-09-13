//! Every command the command line accepts, asked to rehearse, against a real disk.
//!
//! `--dry-run` is declared global, so every subcommand takes it. What that used to
//! mean is that every subcommand *accepted* it, and sixteen of them then went ahead —
//! wired the stack, rotated the credential, wrote the archive — and reported it as a
//! rehearsal. A flag that is accepted and ignored is worse than one that is absent,
//! because the absent one errors and the operator finds out immediately.
//!
//! The decision now lives in one place, `app::rehearsal`, in a match the compiler
//! checks. This is the other half: the claim that a command makes when it says it
//! reports rather than acts is a claim about the world, and only the world can
//! settle it.
//!
//! **What it asserts, and against what.** Each command is dispatched against a
//! context whose configuration home, stack directory and data root are real
//! directories on a real disk, holding the settings a run reads. The tree is hashed
//! before and after, file by file, and a rehearsal that added, removed or changed a
//! byte of it fails here naming the path. That is deliberately not a fake: the writes
//! that hid the longest were the ones that never went through a port at all — the
//! stack being materialised on every lifecycle command, the record of what was
//! materialised, the setup progress file, the quality selection — and a filesystem
//! fake cannot see any of them.
//!
//! The seams that reach *off* this machine are fakes, and that is not a weakening.
//! A real host adapter would install a launch agent into the home directory of
//! whoever ran this, and a real eraser would be handed the paths a `forget` names.
//! Those are recorders: the assertion is that they were asked for nothing, which is
//! the same assertion, made where letting the real thing answer would be damage
//! rather than evidence.
//!
//! **Completeness.** The list below is held against clap's own subcommands, so a new
//! command that nobody decided a rehearsal for fails here as well as failing to
//! compile in `app::rehearsal`. Two subcommands are named as not going through the
//! dispatcher at all, which is a fact about them rather than an exemption: streaming
//! logs and the terminal are not values that arrive once. `help` is clap's, not
//! lemonfiber's, and answers nothing.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::sync::Arc;

use clap::CommandFactory as _;

use lemonfiber::cli::{Cli, STACK};
use lemonfiber_core::app::{
    dispatch, AlertAction, Arranged, Asking, BandwidthAsked, Chosen, Command, Ctx, Decision,
    Keeping, MigrateAction, QualityAction, Removing, Setting, SetupAction, Waiting,
};
use lemonfiber_core::config::{Protocols, Settings};
use lemonfiber_core::doctor::Narrowing;
use lemonfiber_core::platform::Environment;
use lemonfiber_core::ports::hosting::Manager;
use lemonfiber_core::stack::Source;
use lemonfiber_fixtures::erasing::Erasing;
use lemonfiber_fixtures::hosting::Hosting;
use lemonfiber_fixtures::http::{Answer, Fake};
use lemonfiber_fixtures::ports::Stopped;
use lemonfiber_fixtures::pulled::Pulled;
use lemonfiber_fixtures::support::{spoke, Recording, Reporting};
use lemonfiber_fixtures::walking::Walking;

/// The subcommands that never become a [`Command`], and why.
///
/// Named rather than filtered by shape, so that adding a third means saying what it
/// is. Following a log is a stream rather than an answer, and the terminal is a
/// program that then issues commands of its own — each of which arrives here under
/// its own name.
const NOT_DISPATCHED: [&str; 3] = ["logs", "ui", "help"];

/// What a rehearsal of each subcommand is driven with.
///
/// One sample per subcommand, chosen to be the shape that changes the most: a
/// `config set` rather than a `config show`, a rotate rather than a read. A sample
/// that took the harmless branch would pass this while saying nothing about the one
/// that matters.
fn samples() -> Vec<(&'static str, Command)> {
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
            Command::Update(lemonfiber_core::app::update::Asked {
                service: None,
                confirm: true,
                wait: Waiting::Never,
            }),
        ),
        ("backup", Command::Backup { service: None }),
        (
            "support",
            Command::Support {
                write: true,
                wanted: lemonfiber_core::app::bundle::Wanted::default(),
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
fn beyond_the_command_line() -> Vec<(&'static str, Command)> {
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
        ("self-update", Command::SelfUpdate { to: None }),
        (
            "repair",
            Command::Repair {
                consent: lemonfiber_core::app::repair::Consent::Standing,
                disruptive: false,
            },
        ),
    ]
}

/// Every subcommand clap accepts, so a new one cannot be added without a sample.
fn subcommands() -> BTreeSet<String> {
    Cli::command()
        .get_subcommands()
        .map(|command| command.get_name().to_owned())
        .filter(|name| !NOT_DISPATCHED.contains(&name.as_str()))
        .collect()
}

/// Every file under `root`, with its bytes, so two readings can be compared.
///
/// Read rather than watched, because what matters is whether anything is different
/// afterwards rather than how it got that way, and a directory that was created and
/// removed again left nothing behind — which is what a probe does and is not a
/// change to report.
fn tree(root: &Path) -> BTreeMap<PathBuf, Vec<u8>> {
    let mut held = BTreeMap::new();
    let mut looking = vec![root.to_path_buf()];
    while let Some(here) = looking.pop() {
        let Ok(entries) = std::fs::read_dir(&here) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                looking.push(path);
            } else if let Ok(bytes) = std::fs::read(&path) {
                held.insert(path, bytes);
            }
        }
    }
    held
}

/// A scratch home for one command, holding the settings a run reads.
///
/// Its own directory per command so a failure names the command that caused it, and
/// so one command's writes cannot be another's starting point.
fn scratch(named: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "lemonfiber-rehearsal-{}-{named}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&dir);
    let _ = std::fs::create_dir_all(dir.join("data"));
    let _ = std::fs::write(
        dir.join(".env"),
        format!(
            "DATA_ROOT={}\nLF_PROJECT=lemonfiber\n",
            dir.join("data").display()
        ),
    );
    dir
}

/// What a rehearsal was handed, and what it was asked of afterwards.
struct Watched {
    runner: Arc<Recording>,
    http: Arc<Fake>,
    eraser: Arc<Erasing>,
    hosting: Arc<Hosting>,
}

/// A context that rehearses, over a real disk and recording seams.
fn rehearsing(dir: &Path) -> (Ctx, Watched) {
    let runner = Arc::new(Recording::answering(Ok(spoke(""))));
    let http = Fake::always(Answer::reply(200, "{}"));
    let eraser = Erasing::willing();
    let hosting = Hosting::with(Manager::Launchd);

    let ctx = Ctx::new(
        Arc::clone(&runner) as Arc<dyn lemonfiber_core::ports::Runner>,
        Arc::new(Reporting::absent()),
        Stopped::today(),
        lemonfiber_core::ports::seams::Seams {
            filesystem: Arc::new(lemonfiber_adapters::Disk),
            http: Arc::clone(&http) as Arc<dyn lemonfiber_core::ports::Http>,
            images: Pulled::holding(Vec::new()),
            eraser: Arc::clone(&eraser) as Arc<dyn lemonfiber_core::ports::filesystem::Eraser>,
            occupancy: Walking::holding(Vec::new()),
            hosting: Arc::clone(&hosting) as Arc<dyn lemonfiber_core::ports::Host>,
            ..lemonfiber_adapters::live()
        },
        Source::Embedded(&STACK),
        Settings {
            env_file: Some(dir.join(".env")),
            stack_dir: Some(dir.join("stack")),
            data_root: Some(dir.join("data")),
            protocols: Protocols::both(),
            ..Settings::default()
        },
        Environment::LinuxNative,
    )
    .rehearsing();

    (
        ctx,
        Watched {
            runner,
            http,
            eraser,
            hosting,
        },
    )
}

/// The Compose and Docker words that change something when they are run.
///
/// A closed list rather than "anything at all": a rehearsal reads with the runner —
/// `docker compose version`, `hostname` — and refusing those would be refusing it the
/// facts it reports from.
const CHANGES: [&str; 9] = [
    "up", "down", "start", "stop", "restart", "kill", "rm", "create", "pull",
];

/// Drive one command as a rehearsal and say nothing changed, or why that is wrong.
async fn unchanged(named: &str, command: Command) -> Result<(), String> {
    let dir = scratch(named);
    let before = tree(&dir);
    let (ctx, watched) = rehearsing(&dir);

    // The answer is not asserted on. A rehearsal of `restore` against an archive that
    // is not there fails, and so it should — what is being asked here is what it left
    // behind on its way to failing.
    let _ = dispatch(command, &ctx).await;

    let after = tree(&dir);
    let mut wrong = Vec::new();
    for (path, bytes) in &after {
        match before.get(path) {
            None => wrong.push(format!("wrote {}", path.display())),
            Some(was) if was != bytes => wrong.push(format!("changed {}", path.display())),
            Some(_) => {}
        }
    }
    for path in before.keys() {
        if !after.contains_key(path) {
            wrong.push(format!("removed {}", path.display()));
        }
    }

    for argv in watched.runner.seen() {
        if argv.iter().any(|word| CHANGES.contains(&word.as_str())) {
            wrong.push(format!("ran `{}`", argv.join(" ")));
        }
    }
    for request in watched.http.requests() {
        if request.method != lemonfiber_core::ports::http::Method::Get {
            wrong.push(format!("sent {:?} {}", request.method, request.url));
        }
    }
    for path in watched.eraser.asked() {
        wrong.push(format!("asked to erase {}", path.display()));
    }
    for placed in watched.hosting.placed() {
        wrong.push(format!("installed the service `{}`", placed.name));
    }
    for name in watched.hosting.withdrawn() {
        wrong.push(format!("removed the service `{name}`"));
    }

    let _ = std::fs::remove_dir_all(&dir);
    if wrong.is_empty() {
        Ok(())
    } else {
        Err(format!("`{named}` --dry-run {}", first_few(&wrong)))
    }
}

/// How many of a command's findings are worth printing.
///
/// A rehearsed lifecycle command wrote the entire embedded stack, which is some
/// hundreds of files on one line — a failure nobody reads is a failure that gets
/// rerun rather than fixed. Enough to see the shape, and the count for the scale.
const FEW: usize = 4;

/// The first few findings and how many there were, as one phrase.
fn first_few(wrong: &[String]) -> String {
    let shown = wrong
        .iter()
        .take(FEW)
        .cloned()
        .collect::<Vec<_>>()
        .join(", ");
    match wrong.len().checked_sub(FEW) {
        None | Some(0) => shown,
        Some(rest) => format!("{shown}, and {rest} more"),
    }
}

/// The requirement in one sentence, over every command the command line has.
#[tokio::test]
async fn no_command_changes_anything_when_it_is_rehearsing() {
    let mut failed = Vec::new();
    for (named, command) in samples().into_iter().chain(beyond_the_command_line()) {
        if let Err(why) = unchanged(named, command).await {
            failed.push(why);
        }
    }
    assert!(
        failed.is_empty(),
        "a rehearsal changed this machine:\n  {}",
        failed.join("\n  ")
    );
}

/// A subcommand nobody decided a rehearsal for fails here, not in production.
///
/// The compiler already refuses a new `Command` with no verdict. This is the other
/// direction: a verdict decided and never driven is a verdict nothing has checked.
#[test]
fn every_subcommand_the_command_line_accepts_is_driven_here() {
    let sampled: BTreeSet<String> = samples()
        .into_iter()
        .map(|(named, _)| named.to_owned())
        .collect();
    let missing: Vec<String> = subcommands().difference(&sampled).cloned().collect();
    assert!(
        missing.is_empty(),
        "these subcommands take `--dry-run` and nothing here asks what that does: {}",
        missing.join(", ")
    );
}

/// And the other way, so a sample for a subcommand that no longer exists is noticed
/// rather than quietly driving something the operator cannot ask for.
#[test]
fn nothing_is_driven_here_that_the_command_line_does_not_accept() {
    let offered = subcommands();
    let stray: Vec<&str> = samples()
        .iter()
        .map(|(named, _)| *named)
        .filter(|named| !offered.contains(*named))
        .collect();
    assert!(
        stray.is_empty(),
        "these are driven as subcommands and are not ones: {}",
        stray.join(", ")
    );
}
