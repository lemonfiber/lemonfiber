//! What a proposed change comes to on the machine it is made on.
//!
//! The diff beside this — what the setting holds and what it would hold — is settled
//! without asking anybody. Everything here needs somebody outside lemonfiber to have
//! been asked: the \*arrs for where they actually file, the download clients for what
//! is still coming down. So both sides are faked — a filesystem handing back each
//! \*arr's key, and a transport answering as the services would — and the command is
//! driven the way a surface drives it.
//!
//! From here rather than a `#[cfg(test)]` module, as the wiring and credentials tests
//! are: the app layer is compiled twice, and a branch driven only from the in-crate
//! tests is counted as never run in the copy these binaries link.

mod common;

use std::path::{Path, PathBuf};
use std::sync::Arc;

use lemonfiber_core::app::{dispatch, Command, Ctx, Outcome, Setting, Waiting};
use lemonfiber_core::config::{Protocols, Settings};
use lemonfiber_core::platform::Environment;
use lemonfiber_core::ports::http::Http;
use lemonfiber_core::reconfigure::{Review, Stance};
use lemonfiber_core::stack::Source;
use lemonfiber_fixtures::downloads::SAB_KEY_INI;
use lemonfiber_fixtures::http::Fake;
use lemonfiber_fixtures::ports::Stopped;
use lemonfiber_fixtures::support::{seeding_routes, spoke, Reporting, Scripted, SeedFs};
use lemonfiber_ports::docker::{Health, Lifecycle};

/// A Servarr config carrying a key, as one reads from disk.
const CONFIG: &str = "<Config><ApiKey>a1b2c3d4e5</ApiKey></Config>";

/// The settings these name, spelled once.
const DATA_ROOT: &str = lemonfiber_core::config::DATA_ROOT_KEY;
const TORRENT: &str = lemonfiber_core::config::TORRENT_KEY;
const USENET: &str = lemonfiber_core::config::USENET_KEY;

/// What the operator said about a change beyond what the change is.
#[derive(Clone, Copy)]
struct Said {
    /// Whether they agreed to what it costs.
    confirmed: bool,
    /// Whether they asked for what is still coming down to finish first.
    waiting: Waiting,
}

/// Nothing said: the plain run, which stages a consequential change.
const UNSAID: Said = Said {
    confirmed: false,
    waiting: Waiting::Never,
};

/// The same, having agreed to what it costs.
const AGREED: Said = Said {
    confirmed: true,
    waiting: Waiting::Never,
};

/// A scratch environment file, holding the data location a move starts from and the
/// download client's recorded password — without which nothing can ask qBittorrent
/// what is still coming down.
fn env_at(name: &str, from: &Path) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("lemonfiber-change-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let env = dir.join(".env");
    let _ = lemonfiber_core::config::store::set(&env, DATA_ROOT, &from.display().to_string());
    let _ = lemonfiber_core::config::store::set(
        &env,
        lemonfiber_core::config::QBITTORRENT_PASSWORD_KEY,
        &["minted", "-earlier"].concat(),
    );
    env
}

/// What lemonfiber last wrote, put where a change reads it back — so the change under
/// test is not the first this machine has seen and has a record to judge against.
fn remembering(env: &Path, key: &str, value: &str) {
    let record = format!(
        r#"{{"services":{{"lemonfiber:settings":{{"{key}":{{"value":"{value}","at":"1"}}}}}}}}"#
    );
    let _ = std::fs::write(env.with_file_name("baseline.json"), record);
}

/// A context over the stack this repository carries, reaching \*arrs that answer.
///
/// `missing` names path fragments the filesystem will not resolve, which is how a host
/// directory that is not there is driven: the \*arrs hold `/data/media/<type>` whatever
/// happens, and what decides a move is whether the directory behind each one exists at
/// the new location.
fn reaching(env: PathBuf, from: &Path, protocols: Protocols, missing: Vec<&'static str>) -> Ctx {
    over(
        env,
        from,
        protocols,
        Fake::by_path(seeding_routes()),
        missing,
    )
}

/// The same, over a transport a test supplies.
fn over(
    env: PathBuf,
    from: &Path,
    protocols: Protocols,
    http: Arc<Fake>,
    missing: Vec<&'static str>,
) -> Ctx {
    stood_up(
        Settings {
            env_file: Some(env),
            data_root: Some(from.to_path_buf()),
            protocols,
            ..Settings::default()
        },
        Source::External(common::stack::project()),
        http,
        missing,
    )
}

/// A context over whatever settings, stack and transport a test hands it.
fn stood_up(settings: Settings, stack: Source, http: Arc<Fake>, missing: Vec<&'static str>) -> Ctx {
    Ctx::new(
        Arc::new(Scripted(Ok(spoke("")))),
        Arc::new(Reporting::holding(
            &["sonarr"],
            Lifecycle::Running,
            Health::Healthy,
        )),
        Stopped::today(),
        lemonfiber_ports::seams::Seams {
            filesystem: Arc::new(SeedFs::keyed(Some(CONFIG), Some(SAB_KEY_INI)).missing(missing)),
            ..lemonfiber_adapters::live()
        },
        stack,
        settings,
        Environment::MacOs,
    )
    .with_http(http as Arc<dyn Http>)
}

/// The proposal, or nothing where the answer was not a settings one.
async fn changing(ctx: &Ctx, key: &str, value: &str, said: Said) -> Option<Review> {
    match dispatch(
        Command::ConfigSet(
            Setting::to(key, value)
                .agreed(said.confirmed)
                .waiting(said.waiting),
        ),
        ctx,
    )
    .await
    {
        Ok(Outcome::Config(report)) => report.review,
        _ => None,
    }
}

/// Where the proposal stands.
fn stance(review: Option<&Review>) -> Option<Stance> {
    review.map(|review| review.stance)
}

/// Why nothing was written, where nothing was.
fn refusal(review: Option<&Review>) -> Option<String> {
    review.and_then(|review| review.refusal.clone())
}

/// What the environment file holds for a setting now.
fn held(env: &Path, key: &str) -> Option<String> {
    lemonfiber_core::config::store::read(env)
        .ok()
        .and_then(|file| file.get(key).map(str::to_owned))
}

// ── Moving the data location ─────────────────────────────────────────────────

// ── Adding and dropping a way of downloading ─────────────────────────────────

// ── What the change cannot get past ──────────────────────────────────────────

// ── An edit made outside lemonfiber ──────────────────────────────────────────

mod edited;
mod moving;
mod protocols;
