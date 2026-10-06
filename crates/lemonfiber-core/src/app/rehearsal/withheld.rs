//! What a rehearsal carries out in place of what was asked.
//!
//! Apart from the verdict beside it because it is the mechanism rather than the
//! decision: the verdict says what a rehearsal of each command means, and this takes
//! the operator's go-ahead back from the commands that already answer twice.

use super::super::command::{Gathering, MigrateAction, Restoring};
use super::super::{repair, restore, update, Command, Ctx};

/// The command as this run should carry it out.
///
/// A real run carries what was asked. A rehearsal carries the same thing with the
/// operator's go-ahead taken back — see [`unconfirmed`] for why that is the whole of
/// what a rehearsal of those commands needs to be.
#[must_use]
pub fn carried(command: Command, ctx: &Ctx) -> Command {
    if ctx.dry_run {
        unconfirmed(command)
    } else {
        command
    }
}

/// The same command with the operator's go-ahead withheld.
///
/// Eight commands here already answer twice: unconfirmed they say what they would do,
/// confirmed they do it. The unconfirmed answer *is* the rehearsal — the same report,
/// in the same words, filled in by the same code path — so a rehearsal takes the yes
/// back rather than eight handlers each learning a second way to say what they already
/// say. A second way is a second thing to keep true, and the one nobody exercises is
/// the one that stops being true.
///
/// Not exhaustive, and that is deliberate: this is the mechanism, not the decision.
/// What a rehearsal of a command *means* is decided in [`super::asked`], which the compiler
/// checks, and a command that claims to report while its go-ahead is not taken back
/// here fails `a_rehearsal_changes_nothing` against a real disk.
#[must_use]
fn unconfirmed(command: Command) -> Command {
    match command {
        Command::Reset { .. } => Command::Reset { confirm: false },
        Command::Remove { name, .. } => Command::Remove {
            name,
            confirm: false,
        },
        Command::QualityUpgrade { .. } => Command::QualityUpgrade { confirm: false },
        Command::Migrate(MigrateAction::Act { mode, .. }) => Command::Migrate(MigrateAction::Act {
            mode,
            confirmed: false,
        }),
        Command::Migrate(MigrateAction::Replace { .. }) => {
            Command::Migrate(MigrateAction::Replace { offer: None })
        }
        Command::Update(asked) => Command::Update(update::Asked {
            confirm: false,
            ..asked
        }),
        Command::Restore(Restoring {
            archive, repoint, ..
        }) => Command::Restore(Restoring {
            archive,
            repoint,
            consent: restore::Consent::List,
        }),
        Command::Repair { disruptive, .. } => Command::Repair {
            consent: repair::Consent::Offer,
            disruptive,
        },
        // A support bundle is asked for twice by the same word: without `--write` it
        // says what one would hold and where it would land, with it there is a file.
        // The description is the rehearsal, and it is the better command to be asked
        // for it — an operator deciding whether to produce the archive wants the size
        // and the path at the one moment the answer can still change what they do.
        Command::Support(Gathering { wanted, dest, .. }) => Command::Support(Gathering {
            write: false,
            wanted,
            dest,
        }),
        // Everything else either changes nothing, reports for itself, or refuses the
        // flag outright — none of which a withheld confirmation would change.
        other => other,
    }
}
