//! What a proposed change comes to on the machine it is made on.
//!
//! The diff beside this — what the setting holds and what it would hold — is settled
//! without asking anybody. Everything here needs somebody outside lemonfiber to have
//! been asked: the curators for where they actually file, the download clients for what
//! is still coming down. So both sides are faked — the curators moved into their plugins
//! with each adapter's key held, and a transport answering as the adapters would — and the
//! command is driven the way a surface drives it.
//!
//! From here rather than a `#[cfg(test)]` module, as the wiring and credentials tests
//! are: the app layer is compiled twice, and a branch driven only from the in-crate
//! tests is counted as never run in the copy these binaries link.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use crate::common;
use lemonfiber_core::app::{dispatch, Command, Ctx, Outcome, Setting, Waiting};
use lemonfiber_core::config::{Protocols, Settings};
use lemonfiber_core::ports::http::Http;
use lemonfiber_core::ports::service::RegisteredFolder;
use lemonfiber_core::reconfigure::{Review, Stance};
use lemonfiber_core::stack::Source;
use lemonfiber_fixtures::http::{Answer, Fake};
use lemonfiber_fixtures::support::{Reporting, SeedFs};
use lemonfiber_ports::docker::{Health, Lifecycle};
use lemonfiber_testing::curators::{answered, curated, ALL};

/// The settings these name, spelled once.
const DATA_ROOT: &str = lemonfiber_core::config::DATA_ROOT_KEY;

/// The operator's home: a data location is judged against it, and none of the ones
/// these tests move between lies in it.
const HOME: &str = "/home/op";
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

/// A scratch environment file, holding the data location a move starts from.
fn env_at(name: &str, from: &Path) -> PathBuf {
    let dir = lemonfiber_fixtures::scratch::Scratch::named(&format!("change-{name}")).kept();
    let _ = std::fs::remove_dir_all(&dir);
    let env = dir.join(".env");
    let _ = lemonfiber_core::config::store::set(&env, DATA_ROOT, &from.display().to_string());
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

/// A context over the stack this repository carries, reaching curators that answer.
///
/// `missing` names path fragments the filesystem will not resolve, which is how a host
/// directory that is not there is driven: the curators hold `/data/media/<type>` whatever
/// happens, and what decides a move is whether the directory behind each one exists at
/// the new location.
fn reaching(env: PathBuf, from: &Path, protocols: Protocols, missing: Vec<&'static str>) -> Ctx {
    over(env, from, protocols, Fake::by_path(library()), missing)
}

/// The library each curator holds, at `/data/media/{tv,movies,music}`, as its adapter
/// answers it.
fn library() -> Vec<(&'static str, Answer)> {
    let folder = |id: &str, path: &str| RegisteredFolder {
        id: id.to_owned(),
        path: path.to_owned(),
    };
    vec![(
        curated("root_folders"),
        answered(&[
            folder("1", "/data/media/tv"),
            folder("2", "/data/media/movies"),
            folder("3", "/data/media/music"),
        ]),
    )]
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

/// A context over whatever settings, stack and transport a test hands it, for an
/// operator whose home is [`HOME`] where the settings name none, its curators moved into
/// their plugins and each adapter's key held.
fn stood_up(settings: Settings, stack: Source, http: Arc<Fake>, missing: Vec<&'static str>) -> Ctx {
    let mut ctx = unkeyed(settings, stack, http, missing);
    let adapters: Vec<&str> = ALL.iter().map(|curator| curator.adapter).collect();
    ctx.seams.confined =
        lemonfiber_testing::adapted::Keyed::holding(Arc::clone(&ctx.seams.confined), &adapters);
    ctx
}

/// The same, no adapter having written its key yet.
fn unkeyed(settings: Settings, stack: Source, http: Arc<Fake>, missing: Vec<&'static str>) -> Ctx {
    let settings = Settings {
        home: settings.home.or_else(|| Some(PathBuf::from(HOME))),
        ..settings
    };
    let ctx = lemonfiber_testing::a_context()
        .engine(Arc::new(
            Reporting::holding(&["sonarr"], Lifecycle::Running, Health::Healthy)
                .alongside(lemonfiber_testing::adapted::running(&ALL)),
        ))
        .filesystem(Arc::new(SeedFs::keyed(None).missing(missing)))
        .over(stack)
        .settings(settings)
        .build()
        .with_http(http as Arc<dyn Http>);
    for curator in ALL {
        curator.recorded(&ctx);
    }
    ctx
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
