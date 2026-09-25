//! Handing a long-running command to this machine, and taking it back.
//!
//! Three of this product's guarantees are made by a command the terminal that
//! started it would otherwise take with it. Two of them keep running for weeks; the
//! third is a start that has to happen again at every login, which is the same
//! promise reached from the other direction. This is what makes all three survive:
//! the operating system's own service manager is asked to run the very command the
//! operator would have typed.
//!
//! Nothing here reports success from having written a file. What a reading says
//! is what the manager answered, and where the manager would not answer, that is
//! its own state and says so — because the failure this exists to remove is an
//! operator believing a guarantee is in force while nothing is keeping it.
//!
//! Installing is asked for and never arrived at. No other command reaches this,
//! and running one of the long commands does not offer it: an operator who
//! asked to guard a volume this afternoon has not asked for something on their
//! machine that starts at every login.

use crate::error::{Diagnose, Problem, Remedy, Severity, State};
use crate::model::{Changed, HostedCommand, Hosting, HostingReport};
use crate::ports::hosting::{Held, Hosted, Manager, Standing};

use super::command::{Hostable, Keeping, HOSTABLE};
use super::Ctx;
use crate::error::codes::host::{NOTHING_NAMED_TO_GUARD, NOWHERE_TO_WRITE, NO_PROGRAM};

/// What a systemd user session does not do, said before it is relied on.
const UNTIL_LOGOUT: &str = "A user service runs while you are logged in. Surviving a logout \
                            needs lingering turned on for your account, which is a setting on \
                            the account rather than on this service — lemonfiber does not turn \
                            it on for you.";

/// What to do where lemonfiber configures nothing on this platform.
const INSTEAD: &str = "lemonfiber does not configure this platform's way of starting things. \
                       Arrange it with whatever this system uses to run a program at login, \
                       naming the command exactly as it reads above.";

/// Say what is hosted, or change it and then say.
///
/// The reading is taken after the change rather than assembled from it, so what
/// is reported is what the manager holds now and never what a write claimed.
///
/// # Errors
///
/// Returns a [`Problem`] where an install or a removal could not be carried out:
/// a platform with no manager, a manager that refused, a machine that will not
/// say where it keeps its files, or a guard named against no forms.
pub(crate) async fn hosting(ctx: &Ctx, asked: Keeping) -> Result<HostingReport, Box<Problem>> {
    let changed = match asked {
        Keeping::Read => None,
        Keeping::Install { what, forms } => Some(install(ctx, what, &forms).await?),
        Keeping::Remove { what } => Some(remove(ctx, what).await?),
    };
    Ok(reading(ctx, changed).await)
}

/// What this machine keeps running, as it stands now.
async fn reading(ctx: &Ctx, changed: Option<Changed>) -> HostingReport {
    let manager = ctx.seams.hosting.manager();
    let mut commands = Vec::with_capacity(HOSTABLE.len());
    for what in HOSTABLE {
        commands.push(described(ctx, what).await);
    }
    HostingReport {
        manager,
        commands,
        changed,
        instruction: (!manager.configurable()).then(|| INSTEAD.to_owned()),
        caveat: matches!(manager, Manager::Systemd).then(|| UNTIL_LOGOUT.to_owned()),
    }
}

/// One command, and what stands between it and the machine.
async fn described(ctx: &Ctx, what: Hostable) -> HostedCommand {
    // The one thing a manager refuses to answer about is a platform it is not on,
    // which is a state of the machine rather than of this command.
    let (standing, held) = match ctx.seams.hosting.standing(what.name()).await {
        Err(_) => (Hosting::Unsupported, Held::absent()),
        Ok(held) => (settled(&held), held),
    };
    HostedCommand {
        name: what.name().to_owned(),
        guarantees: what.guarantees().to_owned(),
        command: typed(what, &[]),
        standing,
        definition: held.definition,
        runs: held.runs,
        output: held.output,
        missing: held
            .program
            .filter(|program| !program.present)
            .map(|program| program.at),
    }
}

/// Whether this machine is running one of them, rather than whether one is installed.
///
/// The two are different facts everywhere in this module, and they are different
/// here for the reason that matters most: what reads this is a promise made to a
/// household, and a definition sitting on disk that the manager is not running
/// keeps none of it. A manager that will not say is not one to promise on either,
/// so anything short of a confirmed run reads as nothing running it.
pub(crate) async fn keeping(ctx: &Ctx, what: Hostable) -> bool {
    ctx.seams
        .hosting
        .standing(what.name())
        .await
        .is_ok_and(|held| held.standing == Standing::Running)
}

/// What the manager's answer amounts to.
fn settled(held: &Held) -> Hosting {
    match held.standing {
        Standing::Absent => Hosting::NotHosted,
        _ if held.orphaned() => Hosting::Orphaned,
        Standing::Running => Hosting::Hosted,
        Standing::Stopped => Hosting::Stopped,
        Standing::Unsaid => Hosting::InstalledUnverified,
    }
}

/// The command as it would be typed at a terminal.
fn typed(what: Hostable, forms: &[String]) -> String {
    format!("lemonfiber {}", what.arguments(forms).join(" "))
}

/// Hand one to the machine.
async fn install(ctx: &Ctx, what: Hostable, forms: &[String]) -> Result<Changed, Box<Problem>> {
    let wanted = wanted(ctx, what, forms)?;
    if ctx.dry_run {
        return Ok(Changed {
            name: what.name().to_owned(),
            installed: true,
            touched: Vec::new(),
            started: false,
            rehearsed: true,
        });
    }
    let placed = ctx
        .seams
        .hosting
        .place(&wanted)
        .await
        .map_err(|failure| Box::new(failure.problem()))?;
    answered(ctx, what, true);
    Ok(Changed {
        name: what.name().to_owned(),
        installed: true,
        touched: vec![placed.definition],
        started: placed.started,
        rehearsed: false,
    })
}

/// Take one back off the machine.
async fn remove(ctx: &Ctx, what: Hostable) -> Result<Changed, Box<Problem>> {
    if ctx.dry_run {
        let held = ctx
            .seams
            .hosting
            .standing(what.name())
            .await
            .unwrap_or_else(|_| Held::absent());
        return Ok(Changed {
            name: what.name().to_owned(),
            installed: false,
            touched: held.definition.into_iter().collect(),
            started: false,
            rehearsed: true,
        });
    }
    let taken = ctx
        .seams
        .hosting
        .withdraw(what.name())
        .await
        .map_err(|failure| Box::new(failure.problem()))?;
    answered(ctx, what, false);
    Ok(Changed {
        name: what.name().to_owned(),
        installed: false,
        touched: taken,
        started: false,
        rehearsed: false,
    })
}

/// Keep the autostart answer in step with what was just installed or withdrawn.
///
/// Only the boot start, and only because for that one the act *is* the answer: an
/// operator who asks this machine to bring the stack back after a restart has said
/// yes, and one who takes it back off has said no. Leaving the recorded answer at yes
/// with nothing installed would be the `enabled-unverified` trap in its purest form —
/// a record saying autostart is wanted, on a machine where nothing would do it.
///
/// The other two say nothing about it. A guard on the data location and a clock on
/// requests are neither of them a statement about what should happen at a restart.
///
/// Best effort, like every other record written beside a command rather than by one:
/// the thing the operator asked for has happened either way, and a record that could
/// not be written costs the next run its knowledge of the answer rather than leaving
/// a claim that is wrong.
fn answered(ctx: &Ctx, what: Hostable, wanted: bool) {
    if !matches!(what, Hostable::Boot) {
        return;
    }
    let returning = super::autostart::load(ctx).answering(wanted);
    super::autostart::save(ctx, &returning);
}

/// What is to be installed, or why it cannot be described.
fn wanted(ctx: &Ctx, what: Hostable, forms: &[String]) -> Result<Hosted, Box<Problem>> {
    if what.takes_forms() && forms.is_empty() {
        return Err(Box::new(nothing_to_guard()));
    }
    let Some(program) = ctx.settings.program.clone() else {
        return Err(Box::new(no_program()));
    };
    let Some(hosted) = ctx.settings.hosted.clone() else {
        return Err(Box::new(nowhere_to_write()));
    };
    Ok(Hosted {
        name: what.name().to_owned(),
        program,
        arguments: what.arguments(forms),
        output: hosted.join(format!("{}.log", what.name())),
        about: what.guarantees().to_owned(),
    })
}

/// The problem for a guard to be hosted against nothing.
fn nothing_to_guard() -> Problem {
    Problem::new(
        NOTHING_NAMED_TO_GUARD,
        Severity::Error,
        "The guard was not told what to guard",
        "A guard stops the forms it was started against, so one started against none would \
         watch the data location and then have nothing to stop.",
        Remedy::new("Name the forms to guard, as you would when running the guard yourself"),
    )
    .in_state(State::Guided)
}

/// The problem for a run that cannot say where its own program is.
fn no_program() -> Problem {
    Problem::new(
        NO_PROGRAM,
        Severity::Error,
        "This machine would not say where lemonfiber itself is",
        "A service names the program it runs, and one naming a guessed path would fail at \
         every login with nothing to say why.",
        Remedy::new("Run this again from an installed copy of lemonfiber rather than a piped one"),
    )
    .in_state(State::Guided)
}

/// The problem for a run with nowhere to put a hosted command's words.
fn nowhere_to_write() -> Problem {
    Problem::new(
        NOWHERE_TO_WRITE,
        Severity::Error,
        "This machine would not say where lemonfiber keeps its files",
        "A hosted command has no terminal to speak in, so without somewhere to write what it \
         says, nothing it did would be readable afterwards.",
        Remedy::new("Set a home directory for this account, then install it again"),
    )
    .in_state(State::Guided)
}

#[cfg(test)]
mod tests;
