//! Turning what the command line accepted into what the core understands.
//!
//! A subcommand is the surface's vocabulary and a [`Command`] is the core's, and
//! this is the whole of the mapping between them. Kept apart from the dispatcher
//! so that what a request *means* can be read, and proven, without going through
//! everything that happens to it afterwards.

use lemonfiber_core::alert::Appetite;
use lemonfiber_core::app::plugins;
use lemonfiber_core::app::support::Destination;
use lemonfiber_core::app::{
    AlertAction, Asking, BandwidthAsked, Command, Filling, Hostable, Keeping, Linking,
    MigrateAction, QualityAction, Removing, Setting,
};
use lemonfiber_core::audio::Format;
use lemonfiber_core::bundle::run::Wanted;
use lemonfiber_core::doctor::Narrowing;
use lemonfiber_core::migration::mode::Mode;
use lemonfiber_core::quality::Preset;
use lemonfiber_core::recyclarr::Kind;
use lemonfiber_core::uninstall::Tier;
use lemonfiber_core::update::run as update;

use crate::exit::USAGE;
use crate::say::complain;
use lemonfiber::cli::{
    AlertCommand, Asked, Authoring, ConfigAction, HostingCommand, Kept, MigrateCommand,
    PluginCommand, QualityCommand, RawBandwidth, RawCredentials, RawRemoval, RawRemoving,
    UpdateCommand, WiringCommand,
};

mod people;

pub(crate) use people::{held, household, invitation};

/// What a support bundle was asked to hold, and where it goes.
///
/// A shell has a filesystem in front of it, so a bundle asked for without a path
/// goes beside the operator rather than into a directory they would have to be
/// told about — which is the one thing this decides that a browser's request
/// cannot, and the reason the translation is not the same on both surfaces.
pub(crate) fn bundling(asked: Asked) -> Command {
    Command::Support {
        write: asked.write,
        wanted: Wanted::asked(
            asked.logs,
            asked.filenames.into(),
            asked.reveal,
            asked.confirm,
        ),
        dest: asked.out.map_or(Destination::Beside, Destination::At),
    }
}

/// Who an invitation is for, and what the account is to let them watch.
///
/// Reading what this stack wires to what, or changing one of those links.
///
/// The read is the bare word and the write is a verb under it, so a command line
/// that asks for a listing cannot be one that changes a stack by being mistyped.
pub(crate) fn wiring(fill: Option<WiringCommand>) -> Command {
    Command::Wiring(match fill {
        None => Linking::Read,
        Some(WiringCommand::Fill {
            capability,
            service,
        }) => Linking::Fill(Filling {
            capability,
            service,
        }),
    })
}

/// What was asked about what this machine keeps running.
///
/// **Naming nothing is a reading rather than an omission.** The two words underneath
/// change what this machine does at every login, and a bare word that installed
/// something would be exactly the side effect this feature refuses to be.
pub(crate) fn hosting(action: Option<HostingCommand>) -> Command {
    Command::Hosting(match action {
        None => Keeping::Read,
        Some(HostingCommand::Install { what, forms }) => Keeping::Install {
            what: kept(what),
            forms,
        },
        Some(HostingCommand::Remove { what }) => Keeping::Remove { what: kept(what) },
    })
}

/// The command line's word for one of them, as the core names it.
const fn kept(what: Kept) -> Hostable {
    match what {
        Kept::Watch => Hostable::Watch,
        Kept::Expiring => Hostable::Expiring,
        Kept::Boot => Hostable::Boot,
    }
}

/// Which setting the operator is reading or changing.
/// What a migration command asks for, with nothing named meaning survey.
///
/// Surveying is the default because it changes nothing, and because an operator who
/// typed `migrate` to see what is here should not have taken over their own stack by
/// doing so.
pub(crate) fn migrating(action: Option<&MigrateCommand>) -> MigrateAction {
    let Some(asked) = action else {
        return MigrateAction::Survey;
    };
    let (mode, confirmed) = match asked {
        MigrateCommand::Adopt { confirm } => (Mode::Adopt, *confirm),
        MigrateCommand::Import { confirm } => (Mode::Import, *confirm),
        MigrateCommand::Beside { confirm } => (Mode::Beside, *confirm),
        MigrateCommand::Replace { confirm } => (Mode::Replace, *confirm),
    };
    MigrateAction::Act { mode, confirmed }
}

pub(crate) fn configuration(action: ConfigAction) -> Command {
    match action {
        ConfigAction::Get { key } => Command::ConfigGet { key },
        ConfigAction::Set {
            key,
            value,
            confirm,
            wait,
        } => Command::ConfigSet(
            Setting::to(&key, &value)
                .agreed(confirm)
                .waiting(wait.into()),
        ),
        ConfigAction::Show => Command::ConfigShow,
    }
}

/// How much the operator asked to be told, or the code to exit with for a preset this
/// build does not offer.
///
/// A name it does not know is a mistake to correct rather than a reason to fall back to
/// the quiet default, which would leave them believing they had changed something.
pub(crate) fn alerts(action: AlertCommand) -> Result<Command, u8> {
    let chosen = match action {
        AlertCommand::Show => AlertAction::Show,
        AlertCommand::Set { preset } => {
            let Some(wanted) = Appetite::from_label(&preset) else {
                complain!(
                    "error: no notification preset named `{preset}` \
                     (try problems-only, with-completions, or everything)"
                );
                return Err(USAGE);
            };
            AlertAction::Set(wanted)
        }
    };
    Ok(Command::Alerts(chosen))
}

pub(crate) fn quality(action: QualityCommand) -> Result<Command, u8> {
    let action = match action {
        QualityCommand::Show => QualityAction::Show,
        QualityCommand::Set {
            preset,
            media_type,
            confirm,
        } => {
            // Music has no resolution: `--for music` chooses an audio format instead of a
            // resolution preset, and reaches the service rather than only recording, so it
            // routes to its own command.
            if media_type.as_deref() == Some("music") {
                let Some(format) = Format::from_label(&preset) else {
                    complain!(
                        "error: no music format named `{preset}` \
                         (try compact, lossless, or hi-res)"
                    );
                    return Err(USAGE);
                };
                return Ok(Command::QualityMusic { format });
            }
            let Some(preset) = Preset::from_label(&preset) else {
                complain!(
                    "error: no quality preset named `{preset}` \
                     (try space-saving, balanced, high-quality, or maximum)"
                );
                return Err(USAGE);
            };
            if let Some(media_type) = &media_type {
                if !Kind::ALL.iter().any(|kind| kind.media_type() == media_type) {
                    complain!(
                        "error: no media type named `{media_type}` (try tv, movies, or music)"
                    );
                    return Err(USAGE);
                }
            }
            QualityAction::Set {
                preset,
                media_type,
                confirm,
            }
        }
        QualityCommand::Reapply => QualityAction::Reapply,
        // Upgrade is its own command, not a quality action, since it reaches the
        // services rather than only reading or writing the recorded choice.
        QualityCommand::Upgrade { confirm } => return Ok(Command::QualityUpgrade { confirm }),
    };
    Ok(Command::Quality(action))
}

/// What a trace is asked about, from the words it was typed as.
///
/// The term is taken as words so it can be typed unquoted; joined back into the title
/// as said. The searching form is the one that reaches past this machine, spending a
/// real search against the indexers' daily allowance, so it happens only where the
/// flag asked for it.
pub(crate) fn traced(term: &[String], season: Option<u32>, search: bool) -> Command {
    Command::Trace {
        term: term.join(" "),
        season,
        searching: search,
    }
}

/// What was asked about the line, carried as it was written.
///
/// Not one value is interpreted here. What `50%` means, what may be lifted for how
/// long, and what a cap needs beside it are all decisions the core makes for every
/// surface at once — a shell that read them would be a second answer to the same
/// question, and the two would part company on the first change to either.
pub(crate) fn sharing(asked: RawBandwidth) -> Command {
    Command::Bandwidth(BandwidthAsked {
        down: asked.down,
        up: asked.up,
        active: asked.active,
        line: asked.line,
        cap: asked.cap,
        exceeded: asked.when_exceeded,
        unrestricted_for: asked.unrestricted_for,
    })
}

/// A restart of named services, or of everything the form holds where none are named.
pub(crate) fn restarting(form: String, services: Vec<String>) -> Command {
    Command::Restart {
        forms: vec![form],
        services,
    }
}

/// Which completed download to stop seeding, and the offer being answered.
///
/// A rename and nothing else, which is what makes it worth writing down: the command
/// line calls the answer `--offer`, because what an operator types is the name the run
/// before it printed, and the core calls it the agreement, because what it does with
/// it is compare it against the offer standing now. One word for one thing on each
/// side, and this is the whole of the join.
///
/// Nothing typed is nothing agreed to. A flag given empty is an answer to no offer,
/// and carrying it would be a name the core goes and fails to match, so it is dropped
/// here, where the emptiness is visible, rather than travelling as one.
pub(crate) fn letting(download: String, offer: Option<String>) -> Command {
    Command::StopSeeding {
        download,
        agreement: offer.filter(|named| !named.trim().is_empty()),
    }
}

/// Which removal was asked for, and what was answered about it.
///
/// The four words are a `ValueEnum` on this surface, so a fifth word is refused by
/// the parser before anything here runs — which is why this translation cannot fail
/// and the core's own refusal for an unrecognised removal belongs to the surface that
/// takes one as free text.
///
/// Nothing typed is nothing agreed to, for the reason [`letting`] drops an empty
/// offer: a flag given empty is an answer to no listing, and carrying it would be a
/// name the core goes and fails to match.
pub(crate) fn removing(asked: RawRemoving) -> Command {
    let tier = match asked.tier {
        RawRemoval::Stop => Tier::Stop,
        RawRemoval::Services => Tier::Services,
        RawRemoval::Configuration => Tier::Configuration,
        RawRemoval::Media => Tier::Media,
    };
    Command::Uninstall(
        Removing::surveying(tier)
            .confirmed(asked.confirm)
            .agreeing(asked.agreed.filter(|named| !named.trim().is_empty()))
            .waiting(asked.wait.into()),
    )
}

/// The diagnosis a plain run asks for, narrowed as it was asked to be.
///
/// Named apart because the arm it came from carries a fork of its own — a run that
/// mends returns before this is reached — and the two together are longer than the
/// table has room for.
pub(crate) fn diagnosing(
    only: Option<&str>,
    disruptive: bool,
    accept: Option<String>,
) -> Result<Command, u8> {
    narrowed(only).map(|narrowing| Command::Doctor {
        narrowing,
        disruptive,
        accept,
    })
}

/// What a diagnosis was narrowed to, or the code to exit with for a name that is
/// neither a category nor a check inside one.
///
/// A name lemonfiber does not know is a mistake to correct rather than a request to
/// run everything — refused here, before the core is reached. Whether a stack reports
/// the check named is a question only the run can answer, and it answers it.
fn narrowed(only: Option<&str>) -> Result<Narrowing, u8> {
    match only.map(Narrowing::parse) {
        Some(None) => {
            let named = only.unwrap_or_default();
            complain!("error: no diagnostic category or check named `{named}`");
            Err(USAGE)
        }
        Some(Some(narrowing)) => Ok(narrowing),
        None => Ok(Narrowing::Suite),
    }
}

/// What is being asked about the credentials this stack holds.
///
/// Naming nothing is the reading. Naming one to reveal or one to rotate is an act on
/// a line of that reading, and the two cannot arrive together — the command line
/// refuses the pair, so there is no order of precedence here to get wrong.
///
/// The confirmation belongs to the reveal and to nothing else. Carried through
/// rather than acted on here, because what an unconfirmed reveal answers with is a
/// warning that has to be written once, where the value would otherwise be.
pub(crate) fn credentials(asked: RawCredentials) -> Command {
    Command::Credentials(match (asked.reveal, asked.rotate) {
        (Some(credential), _) => Asking::Reveal {
            credential,
            confirmed: asked.confirm,
        },
        (None, Some(credential)) => Asking::Rotate { credential },
        (None, None) => Asking::Read,
    })
}

/// The one thing a walk was asked for, or nothing at all.
///
/// Taken as words so it can be typed unquoted, and joined back into the title as
/// said. Nothing named is a request in its own right rather than an omission: a
/// walk asked for nothing in particular suggests something likely to work, which
/// is what an operator with an empty library needs.
pub(crate) fn named(words: &[String]) -> Option<String> {
    let said = words.join(" ");
    (!said.trim().is_empty()).then_some(said)
}

/// Which of the two things that can be moved forward was named, as the core carries it.
///
/// Apart from the arm that reads it for the reason the bundle beside it is: the stack's
/// three fields spelled out twice is nine lines of the one function that has to stay
/// readable, and the wait is the flag a teardown spells the same way.
///
/// The two go to different commands rather than to one carrying a mode. What each
/// answers with does not resemble the other — a list of services and the steps they
/// would take, against where one binary stands and which tool owns it — so a shared
/// shape would be a shape neither of them fits.
pub(crate) fn moving(object: UpdateCommand) -> Command {
    match object {
        UpdateCommand::Stack {
            service,
            confirm,
            wait,
        } => Command::Update(update::Asked {
            service,
            confirm,
            wait: wait.into(),
        }),
        UpdateCommand::Itself { to } => Command::SelfUpdate { to },
    }
}

/// Which of the two doors a word under `plugin` goes through.
///
/// A value rather than a routing decision taken here, for the reason the rest of
/// this file is a mapping: which door a word takes is what the vocabulary says, and
/// walking through it is the caller's errand.
pub(crate) enum Under {
    /// A value that arrives once, dispatched like any other verb.
    Dispatched(Command),
    /// A document this build generated at compile time, answered with no stack.
    Published(Authoring),
}

/// Which door this word goes through, and the value it goes through it as.
///
/// The five documents answer the same on a machine with nothing installed as on one
/// running everything, so there is no stack to ask and nothing to decide; the four
/// verbs are about this machine and go where every other verb goes.
pub(crate) fn plugin(read: PluginCommand) -> Under {
    match read {
        PluginCommand::Install { path } => {
            Under::Dispatched(Command::Plugins(plugins::Asked::Install { path }))
        }
        PluginCommand::Installed => Under::Dispatched(Command::Plugins(plugins::Asked::Installed)),
        PluginCommand::Remove { plugin } => {
            Under::Dispatched(Command::Plugins(plugins::Asked::Remove { plugin }))
        }
        PluginCommand::Update { path } => {
            Under::Dispatched(Command::Plugins(plugins::Asked::Update { path }))
        }
        PluginCommand::Authoring(read) => Under::Published(read),
    }
}

#[cfg(test)]
mod tests;
