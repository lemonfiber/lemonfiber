//! Which of the core's own commands each name reaches.
//!
//! The whole of this surface's authority. A command is what the command line
//! produces too, so an action reaching a command cannot be something only a browser
//! can do — and a name outside the table is refused rather than invented, which is
//! what keeps the two surfaces the same size.
//!
//! Only the actions that *change* something are here. Asking what the stack is
//! doing is a read and has an endpoint of its own; a write that also happens to
//! report is still a write.

use lemonfiber_core::app::bundle::{Wanted, LINES};
use lemonfiber_core::app::restore::Kept;
use lemonfiber_core::app::support::Destination;
use lemonfiber_core::app::{Command, QualityAction};
use lemonfiber_core::audio::Format;
use lemonfiber_core::quality::Preset;
use lemonfiber_core::recyclarr::Kind;

use super::asked::{unwanted, Arguments};
use super::Refused;

/// Every action this surface offers, in the order they are worth reading.
///
/// Held as a list so that what the surface offers can be counted and checked
/// against what it translates, rather than being knowable only by reading a
/// match arm at a time.
pub const OFFERED: &[&str] = &[
    "up",
    "down",
    "switch",
    "restart",
    "pull",
    "config-set",
    "quality-set",
    "quality-reapply",
    "quality-upgrade",
    "seed",
    "adopt",
    "reset",
    "backup",
    "support",
    "restore",
];

/// The actions that must be told what to act on.
const NAMES_ITS_FORMS: [&str; 3] = ["switch", "restart", "pull"];

/// The command an action names, or why it names none.
///
/// # Errors
///
/// Returns the [`Refused`] a caller should be answered with.
pub fn named(action: &str, given: Arguments) -> Result<Command, Refused> {
    let needs = |argument: &str| Refused::Missing {
        action: action.to_owned(),
        argument: argument.to_owned(),
    };
    // Naming nothing means everything for the actions that can mean it, and is a
    // mistake for the three that cannot: switching to nothing, restarting nothing
    // and fetching nothing are each a request that has lost its subject, and the
    // command line refuses all three the same way.
    if NAMES_ITS_FORMS.contains(&action) && given.forms.is_empty() {
        return Err(needs("forms"));
    }
    if let Some(refused) = unwanted(action, &given, OFFERED) {
        return Err(refused);
    }
    let Arguments {
        forms,
        services,
        service,
        key,
        value,
        preset,
        media_type,
        archive,
        repoint,
        write,
        logs,
        filenames,
        reveal,
        confirm,
    } = given;
    match action {
        "up" => Ok(Command::Up { forms }),
        // Stopping named services and tearing a form down are different requests
        // rather than one request with an argument, exactly as the command line
        // reads them, and Compose spells them differently too.
        "down" if services.is_empty() => Ok(Command::Down { forms }),
        "down" => Ok(Command::Halt { forms, services }),
        "switch" => Ok(Command::Switch { forms }),
        "restart" => Ok(Command::Restart { forms, services }),
        "pull" => Ok(Command::Pull { forms }),
        "config-set" => match (key, value) {
            (Some(key), Some(value)) => Ok(Command::ConfigSet { key, value }),
            (None, _) => Err(needs("key")),
            (_, None) => Err(needs("value")),
        },
        "quality-set" => match preset {
            Some(preset) => quality(&preset, media_type, confirm),
            None => Err(needs("preset")),
        },
        "quality-reapply" => Ok(Command::Quality(QualityAction::Reapply)),
        "quality-upgrade" => Ok(Command::QualityUpgrade { confirm }),
        "seed" => Ok(Command::Seed),
        "adopt" => Ok(Command::Adopt),
        "reset" => Ok(Command::Reset { confirm }),
        "backup" => Ok(Command::Backup { service }),
        // The bundle goes where lemonfiber keeps its own files. A browser has no
        // filesystem in front of it and no path it could name that would mean
        // anything here, so the destination is settled rather than asked for —
        // which answers *which path*, the only web-specific question a bundle has.
        "support" => Ok(Command::Support {
            write,
            wanted: Wanted::asked(logs.unwrap_or(LINES), filenames, reveal, confirm),
            dest: Destination::Kept,
        }),
        // By the name it was written under, never by a path. The server runs as the
        // operator, so a path it accepted would be a path it could read; a name is
        // resolved beneath the backups directory by the core and nowhere else.
        "restore" => match archive {
            Some(name) => Ok(Command::Restore {
                archive: Kept::Named(name),
                repoint,
                confirm,
            }),
            None => Err(needs("archive")),
        },
        _ => Err(Refused::Unknown {
            name: action.to_owned(),
        }),
    }
}

/// A quality choice, which is two commands depending on what it is about.
///
/// Music has no resolution: choosing for it picks an audio format rather than a
/// resolution preset, and reaches the service rather than only recording, so it
/// routes to its own command — the same fork the command line takes.
fn quality(preset: &str, media_type: Option<String>, confirm: bool) -> Result<Command, Refused> {
    if media_type.as_deref() == Some("music") {
        let Some(format) = Format::from_label(preset) else {
            return Err(Refused::Unrecognised {
                argument: "preset".to_owned(),
                offered: "try compact, lossless, or hi-res".to_owned(),
            });
        };
        return Ok(Command::QualityMusic { format });
    }
    let Some(preset) = Preset::from_label(preset) else {
        return Err(Refused::Unrecognised {
            argument: "preset".to_owned(),
            offered: "try space-saving, balanced, high-quality, or maximum".to_owned(),
        });
    };
    // Asked as one value and answered with one expression rather than through an
    // early return: a block that only ever ends by leaving leaves its own closing
    // brace with nothing to run it.
    let known = media_type
        .as_deref()
        .is_none_or(|named| Kind::ALL.iter().any(|kind| kind.media_type() == named));
    if known {
        Ok(Command::Quality(QualityAction::Set {
            preset,
            media_type,
            confirm,
        }))
    } else {
        Err(Refused::Unrecognised {
            argument: "media_type".to_owned(),
            offered: "try tv, movies, or music".to_owned(),
        })
    }
}
