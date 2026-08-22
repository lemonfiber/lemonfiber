//! Where this machine keeps lemonfiber's files, and the capabilities a run holds.
//!
//! Resolving a home directory and reading the settings out of it are the two
//! things every command needs before it can do anything, and both depend on the
//! machine rather than on what was asked. Kept together, and away from the
//! dispatcher, because that is the seam a test cannot cross.

use std::path::PathBuf;
use std::sync::Arc;

use lemonfiber_core::adapters::{Daemon, Disk, Local, System};
use lemonfiber_core::app::Ctx;
use lemonfiber_core::config::paths::Paths;
use lemonfiber_core::config::{
    data_root_from_env, indexer_from_env, ip_echo_from_env, port_forward_from_env, reads_as_off,
    service_user_from_env, store, Protocols, Settings, EXPLANATIONS_KEY,
};
use lemonfiber_core::platform::{Environment, HOST_OS};
use lemonfiber_core::stack::Source;

use crate::cli::STACK;

/// Everything a command needs that the command itself does not carry.
pub(crate) fn context(stack_dir: Option<PathBuf>, dry_run: bool, force: bool) -> Ctx {
    // The path outlives the process, and `Source` is Copy so it can be handed
    // around freely; leaking one allocation at startup buys both.
    let stack = match stack_dir {
        Some(path) => Source::External(Box::leak(path.into_boxed_path())),
        None => Source::Embedded(&STACK),
    };

    let settings = read_settings();
    // Settled here rather than at each of the surfaces that explains something,
    // because it is a property of the run: the two places that build a context are
    // the only two that could be told, and neither can then be told wrong.
    crate::render::glossary::settle(settings.explanations);

    // Docker Engine and Docker Desktop are told apart by asking the daemon,
    // which needs the engine adapter. Until then this is what can be seen from
    // here, and nothing yet depends on the difference.
    let environment = Environment::resolve(HOST_OS, false);

    let ctx = Ctx::new(
        Arc::new(Local),
        Arc::new(Daemon::local()),
        Arc::new(System),
        Arc::new(Disk),
        stack,
        settings,
        environment,
    );

    // A rehearsal takes nothing, so the two never both apply — but a rehearsal that
    // was also asked to force is a rehearsal, because the harmless reading of an
    // ambiguous pair of flags is the one to take.
    if dry_run {
        return ctx.rehearsing();
    }
    if force {
        return ctx.forcing();
    }
    ctx
}

/// The operator's settings, read from their file as it stands now.
///
/// Read fresh rather than passed around, because setup writes the file mid-run:
/// the settings this process started with predate what it just applied, and
/// starting the stack against the stale set would run the wrong thing.
pub(crate) fn read_settings() -> Settings {
    let env_file = configuration_file();
    let recorded = env_file
        .as_deref()
        .and_then(|path| store::read(path).ok())
        .unwrap_or_default();

    Settings {
        protocols: Protocols::from_env(&recorded),
        ip_echo: ip_echo_from_env(&recorded),
        data_root: data_root_from_env(&recorded),
        storage_state: here().map(|paths| paths.storage_state()),
        service_user: service_user_from_env(&recorded),
        port_forward: port_forward_from_env(&recorded),
        indexer: indexer_from_env(&recorded),
        // On unless it is explicitly turned off: somebody meeting this vocabulary
        // does not know there is a setting to look for, and somebody who wants the
        // explanations gone knows exactly what they want to stop.
        explanations: !recorded.get(EXPLANATIONS_KEY).is_some_and(reads_as_off),
        env_file,
        stack_dir: stack_directory(),
        ..Settings::default()
    }
}

pub(crate) fn here() -> Option<Paths> {
    use etcetera::BaseStrategy as _;

    let strategy = etcetera::choose_base_strategy().ok()?;
    Some(Paths::rooted(&strategy.config_dir(), &strategy.data_dir()))
}

/// The operator's settings file, whether or not it exists yet.
///
/// Named even when absent, because `config set` has to be able to create it —
/// refusing to name a file until it exists would make setting the first setting
/// impossible.
pub(crate) fn configuration_file() -> Option<PathBuf> {
    here().map(|paths| paths.env_file())
}

/// Where an embedded stack is written so Compose can read it.
pub(crate) fn stack_directory() -> Option<PathBuf> {
    here().map(|paths| paths.stack())
}
