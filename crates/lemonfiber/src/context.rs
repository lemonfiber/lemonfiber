//! Where this machine keeps lemonfiber's files, and the capabilities a run holds.
//!
//! Resolving a home directory and reading the settings out of it are the two
//! things every command needs before it can do anything, and both depend on the
//! machine rather than on what was asked. Kept together, and away from the
//! dispatcher, because that is the seam a test cannot cross.
//!
//! Both directories can be named instead of resolved, which is what `--config-dir`
//! and `--data-dir` are for: a second instance on one machine, an install on a
//! removable disk, or a machine whose home directory this program cannot work out at
//! all. The platform is asked only for whichever half was not named.

use std::path::PathBuf;
use std::sync::Arc;

use lemonfiber_adapters::{Daemon, Disk, Launchd, Local, System, Systemd, Unhosted};
use lemonfiber_core::acknowledged::{self, Acknowledged};
use lemonfiber_core::app::Ctx;
use lemonfiber_core::archive::Archiving;
use lemonfiber_core::config::paths::Paths;
use lemonfiber_core::config::{
    data_root_from_env, exposed_from_env, front_door_from_env, household_host_from_env,
    indexer_from_env, ip_echo_from_env, port_forward_from_env, provider_host_from_env,
    reads_as_off, reads_as_on, service_user_from_env, store, Protocols, Reaching, Settings,
    AUTOSTART_ON_BATTERY_KEY, EXPLANATIONS_KEY,
};
use lemonfiber_core::platform::{Environment, HOST_OS};
use lemonfiber_core::ports::hosting::Manager;
use lemonfiber_core::ports::{Host, Runner};
use lemonfiber_core::stack::Source;

use lemonfiber::cli::STACK;

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
    // And what this operator has already gone and found out about, so a report names
    // those words rather than teaching them again.
    crate::render::glossary::settle_known(here().map_or_else(Acknowledged::default, |paths| {
        acknowledged::at(&paths.acknowledged())
    }));
    // And which machine this run is about, before it does anything. Settled here
    // because this is where the one answer for the run already lives: everything
    // below reads it off the settings rather than asking the environment again.
    crate::render::host::settle(&settings.docker);

    // Docker Engine and Docker Desktop are told apart by asking the daemon,
    // which needs the engine adapter. Until then this is what can be seen from
    // here, and nothing yet depends on the difference.
    let environment = Environment::resolve(HOST_OS, false);

    let runner: Arc<dyn Runner> = Arc::new(Local);
    let ctx = Ctx::new(
        Arc::clone(&runner),
        // Both engine seams are built from the one resolved target the settings
        // carry, which is the same field the Compose invocation is built from. That
        // is the whole of what stops the reads and the writes reaching different
        // machines: there is no second place to resolve one.
        Arc::new(Daemon::reaching(settings.docker.clone())),
        Arc::new(System),
        lemonfiber_core::ports::seams::Seams {
            filesystem: Arc::new(Disk),
            ..lemonfiber_adapters::live_reaching(&settings.docker)
        },
        stack,
        settings,
        environment,
    )
    // Which service manager this machine has is decided from the target it was built
    // for and handed in, so the core asks a port rather than the operating system —
    // and a machine whose home directory cannot be found hosts nothing rather than
    // writing a definition into a directory nothing could later find to remove.
    .hosting_with(manager(&runner))
    // A wait says what it is waiting for, and this is where those words go on a
    // terminal. The web surface replaces it with one that says them on the stream a
    // browser holds open, which is the only reason this is a value rather than a
    // call from inside the wait.
    .narrating(Arc::new(crate::engine::Narrating));

    // Where archives are kept, and what packs them. The packing lives in this
    // crate because the crate that reasons about backups keeps no dependency on
    // how a `.tar.gz` is written, so this is the one place the two meet. Absent
    // where this machine would not say where its own files go, which is the same
    // absence every other reader of `here` handles.
    let ctx = match here() {
        None => ctx,
        Some(paths) => ctx
            // What left this machine is written down where this machine keeps its
            // files, which only the edge knows. A run that cannot be told where
            // that is records nothing rather than guessing at a directory.
            .recording_at(paths.outbound())
            .keeping(Archiving {
                paths,
                vault: Arc::new(crate::archive::Tar),
            }),
    };

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

/// This operator's home directory, where the platform will say.
///
/// Its own function rather than a line in the settings, because the strategy it asks
/// has to be brought into scope and doing that inside the record would import a name
/// for the whole of it.
fn home_directory() -> Option<PathBuf> {
    use etcetera::BaseStrategy as _;

    Some(
        etcetera::choose_base_strategy()
            .ok()?
            .home_dir()
            .to_path_buf(),
    )
}

/// This machine's own service manager, reached where it keeps its definitions.
///
/// The directories are the platforms' own rather than lemonfiber's: a launch agent
/// is only loaded from `~/Library/LaunchAgents`, and a user unit only from the XDG
/// configuration directory, so neither is somewhere this program gets to choose.
fn manager(runner: &Arc<dyn Runner>) -> Arc<dyn Host> {
    use etcetera::BaseStrategy as _;

    let Ok(strategy) = etcetera::choose_base_strategy() else {
        return Arc::new(Unhosted);
    };
    match HOST_OS.manager() {
        Manager::Launchd => Arc::new(Launchd::over(
            strategy.home_dir().join("Library/LaunchAgents"),
            Arc::clone(runner),
        )),
        Manager::Systemd => Arc::new(Systemd::over(
            strategy.config_dir().join("systemd/user"),
            Arc::clone(runner),
        )),
        Manager::Unsupported => Arc::new(Unhosted),
    }
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
        project: lemonfiber_core::config::project_from_env(&recorded),
        quiet: lemonfiber_core::config::quiet_from_env(&recorded),
        overlays: lemonfiber_core::config::overlay_from_env(&recorded),
        protocols: Protocols::from_env(&recorded),
        ip_echo: ip_echo_from_env(&recorded),
        data_root: data_root_from_env(&recorded),
        storage_state: here().map(|paths| paths.storage_state()),
        service_user: service_user_from_env(&recorded),
        port_forward: port_forward_from_env(&recorded),
        indexer: indexer_from_env(&recorded),
        admission: here().map(|paths| paths.admission()),
        household_host: household_host_from_env(&recorded),
        exposed: exposed_from_env(&recorded),
        unmanaged: lemonfiber_core::config::unmanaged_from_env(&recorded),
        front_door: front_door_from_env(&recorded),
        reaching: Reaching::from_env(&recorded),
        provider_host: provider_host_from_env(&recorded),
        // What a hosted service names as the program to run, which only this process
        // can say. Absent where the platform will not, which hosting refuses on rather
        // than installing a service against a guessed path.
        program: std::env::current_exe().ok(),
        hosted: here().map(|paths| paths.hosted()),
        // Where the tools that install programs leave a record of having done so,
        // which is the only thing this is read for.
        home: home_directory(),
        // Which engine this run operates, resolved once from the environment and
        // from Docker's own records. Read here rather than by whoever needs it, so
        // the Engine API client, the image listing, the Compose invocation and the
        // diagnosis all obey one answer.
        docker: lemonfiber_adapters::docker_target(),
        // On unless it is explicitly turned off: somebody meeting this vocabulary
        // does not know there is a setting to look for, and somebody who wants the
        // explanations gone knows exactly what they want to stop.
        explanations: !recorded.get(EXPLANATIONS_KEY).is_some_and(reads_as_off),
        // Off unless it is explicitly turned on, which is the other way round from the
        // explanations above and deliberately so: a stack started on a battery costs an
        // afternoon of it, and that is a thing to have asked for.
        autostart_on_battery: recorded
            .get(AUTOSTART_ON_BATTERY_KEY)
            .is_some_and(reads_as_on),
        env_file,
        stack_dir: stack_directory(),
    }
}

/// Record that a word has been gone and found out about.
///
/// Beside where the record is read, because both answer the same question about
/// where this machine keeps its files.
///
/// Best effort and silent: the answer has already been given, and a record that
/// could not be written costs one repeated explanation rather than anything the
/// operator needs telling about now. Written only where it actually changed, so
/// asking about the same word twice touches nothing.
///
/// A rehearsal writes nothing, which this is not exempt from.
pub(crate) fn remember(words: &[&str], rehearsing: bool) {
    if rehearsing {
        return;
    }
    let Some(paths) = here() else {
        return;
    };
    let path = paths.acknowledged();
    let mut known = acknowledged::at(&path);
    // Read and written once however many words were opened at a time, which is what
    // a pane does — the file is small, but a screenful of separate rewrites is a
    // screenful of chances for two of them to lose each other's.
    // Every word is taken, and `|=` is what says so: `any` and `fold` with `||`
    // both short-circuit, so a screenful would record its first new word and
    // silently drop the rest. Clippy suggests `any` here and is wrong, because it
    // cannot see that taking a word is what changes the record.
    let mut changed = false;
    for word in words {
        changed |= known.take(word);
    }
    if !changed {
        return;
    }
    if let Some(text) = known.to_json() {
        let _ = std::fs::write(&path, text);
    }
}

/// The two base directories lemonfiber's own files sit beneath, where the operator
/// named one instead of the platform's.
///
/// The pair rather than one directory, because the two mean different things to the
/// operating system and to whoever backs this machine up: configuration is small and
/// worth keeping, and the data directory holds what can be made again. An escape
/// hatch that collapsed them would make the distinction unavailable to anybody who
/// took it.
#[derive(Debug)]
struct Roots {
    /// Where configuration goes, instead of the directory this platform names.
    config: Option<PathBuf>,
    /// Where regenerable data goes, instead of the directory this platform names.
    data: Option<PathBuf>,
}

/// What this run was told about where its own files go.
///
/// Settled once and read by everything that asks, rather than handed to each reader:
/// where lemonfiber keeps its files is a property of the run, the same way the
/// explanations setting is, and there are six readers of it here. A reader told
/// separately is a reader that can be told something different, and a run that
/// answered two ways about where its own files are would write half of them
/// somewhere nothing later looks.
static ROOTS: std::sync::OnceLock<Roots> = std::sync::OnceLock::new();

/// Take where the operator wants lemonfiber's own files kept, for the rest of the run.
///
/// A second call is ignored rather than obeyed, which is what the answer being a
/// property of the run means: the flags are read once, before anything has asked
/// where anything is.
pub(crate) fn settle_roots(config: Option<PathBuf>, data: Option<PathBuf>) {
    let _ = ROOTS.set(Roots { config, data });
}

pub(crate) fn here() -> Option<Paths> {
    use etcetera::BaseStrategy as _;

    let given = ROOTS.get();
    let config = given.and_then(|roots| roots.config.as_deref());
    let data = given.and_then(|roots| roots.data.as_deref());
    // Named both ways, and the platform is never asked. Worth the separate arm: a
    // machine whose home directory cannot be resolved is exactly the machine
    // somebody reaches for these flags on, and falling through here would answer it
    // with the absence the flags were given to fill.
    if let (Some(config), Some(data)) = (config, data) {
        return Some(Paths::rooted(config, data));
    }

    let strategy = etcetera::choose_base_strategy().ok()?;
    let platform_config = strategy.config_dir();
    let platform_data = strategy.data_dir();
    Some(Paths::rooted(
        config.unwrap_or(&platform_config),
        data.unwrap_or(&platform_data),
    ))
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
