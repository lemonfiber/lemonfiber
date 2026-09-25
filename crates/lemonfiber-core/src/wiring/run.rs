//! Reading what this stack wires to what, and changing which service fills a thing.
//!
//! Two operations on one subject. The listing is a read of the manifest and of the
//! one setting that records a choice; the substitution writes that setting and
//! journals it, having first worked out what it would cost.
//!
//! Worked out *first* and in full, which is the rule the reconfiguration path keeps
//! for the same reason: an operator told afterwards that something stopped being
//! filled has been told about a thing they can no longer choose.

use crate::error::{Problem, Remedy, Severity};

use super::Refused;
use crate::app::Ctx;
use crate::error::codes::wire::{CANNOT_FILL, CHOICE_UNWRITABLE, NOTHING_ASKS, NO_SUCH_FILLER};
use crate::error::Diagnose;
use crate::model::{SubstitutionReport, WiringReport};

/// Read what reaches what, or change one of those links.
///
/// The two arrive as one command because they are two things asked of one subject,
/// and a dispatcher that split them would put the listing and the change in
/// different parts of the same list.
///
/// # Errors
///
/// Whatever the operation asked for returns.
pub(crate) fn wiring(
    ctx: &Ctx,
    asked: &crate::app::Linking,
) -> Result<crate::app::Outcome, Box<Problem>> {
    match asked {
        crate::app::Linking::Read => listing(ctx).map(crate::app::Outcome::Wiring),
        crate::app::Linking::Fill(filling) => {
            substituting(ctx, filling).map(crate::app::Outcome::Substitution)
        }
    }
}

/// What this stack wires to what, and what nothing fills.
///
/// # Errors
///
/// Returns the [`Problem`] a surface should render when the stack cannot be read, or
/// when what it declares does not hold together. Boxed as the listings beside it are.
fn listing(ctx: &Ctx) -> Result<WiringReport, Box<Problem>> {
    let manifest = ctx
        .stack
        .checked_manifest(ctx.today())
        .map_err(|err| Box::new(err.problem()))?;

    // What is installed is read with the stack, because a plugin's service that claims
    // what the stack asks for is a candidate like any other. A record that is there and
    // cannot be read refuses the listing, for the reason it refuses a diagnosis: an
    // answer that quietly left a stranger's service out would settle a contest nobody
    // was told about.
    let installed = crate::app::plugins::read(ctx)?;
    let wired = super::settle(
        &manifest,
        installed.installed(),
        &crate::app::targets::chosen_fillers(ctx),
    );
    Ok(WiringReport {
        unfilled: super::unfilled(&wired),
        wired,
    })
}

/// Choose which service fills a capability, and say what that costs.
///
/// A rehearsal works the whole thing out and writes nothing, which is the same
/// answer with `applied` false: what a substitution would leave unfilled is exactly
/// what somebody wants to know before agreeing to it.
///
/// # Errors
///
/// Returns the [`Problem`] for a stack that cannot be read, a service this stack does
/// not have, one that cannot do the thing, a capability nothing asks for, one the
/// named service already fills, or a settings file that could not be written.
fn substituting(
    ctx: &Ctx,
    filling: &crate::app::Filling,
) -> Result<SubstitutionReport, Box<Problem>> {
    let manifest = ctx
        .stack
        .checked_manifest(ctx.today())
        .map_err(|err| Box::new(err.problem()))?;

    let held = crate::app::targets::chosen_fillers(ctx);
    let installed = crate::app::plugins::read(ctx)?;
    let substitution = super::substitute(
        &manifest,
        installed.installed(),
        &held,
        &filling.capability,
        &filling.service,
    )
    .map_err(|refused| Box::new(problem(&refused)))?;

    if ctx.dry_run {
        return Ok(SubstitutionReport {
            substitution,
            applied: false,
        });
    }

    // The record of the change goes down before the change does, so a run stopped
    // between the two leaves a journal entry for a setting that still holds its old
    // value — which unwinds to the value it already has. The other order leaves a
    // changed setting nothing can put back.
    let previous = held.setting();
    let (Some(path), Some(paths)) = (
        ctx.settings.env_file.as_deref(),
        crate::app::targets::layout(ctx),
    ) else {
        return Err(Box::new(nowhere_to_record()));
    };
    crate::app::recover::journalled(
        &paths.journal(),
        &[super::recorded(
            &substitution,
            previous.as_deref(),
            &ctx.stamp(),
        )],
        ctx.seams.random.as_ref(),
    );
    if let Err(err) = crate::config::store::set(path, super::FILLS_KEY, &substitution.setting) {
        return Err(Box::new(err.problem()));
    }

    Ok(SubstitutionReport {
        substitution,
        applied: true,
    })
}

/// A refusal in the form an operator can act on.
///
/// Each of the four is a different mistake with a different next step, so each gets
/// its own code and its own remedy rather than one "that did not work".
fn problem(refused: &Refused) -> Problem {
    let listing = Remedy::new("See what this stack wires to what").with_detail("lemonfiber wiring");
    match refused {
        Refused::NoSuchService(service) => Problem::new(
            NO_SUCH_FILLER,
            Severity::Error,
            format!("there is no service called {service} in this stack"),
            "Nothing was changed. A capability is filled by a service this stack \
             declares, and that name is not one of them.",
            Remedy::new("List the services this stack has").with_detail("lemonfiber catalogue"),
        )
        .or_try(listing),
        Refused::DoesNotProvide {
            service,
            capability,
        } => Problem::new(
            CANNOT_FILL,
            Severity::Error,
            format!("{service} does not provide {capability}"),
            "Nothing was changed. Filling a capability with a service that does not \
             declare it would point everything that asked at something that cannot \
             answer.",
            Remedy::new("See which services declare it")
                .with_detail("lemonfiber plugin capabilities"),
        )
        .or_try(listing),
        Refused::NothingAsks(capability) => Problem::new(
            NOTHING_ASKS,
            Severity::Warning,
            format!("nothing in this stack asks for {capability}"),
            "Nothing was changed. Choosing who fills a capability nothing asks for \
             would record a setting no wiring reads.",
            listing,
        ),
        Refused::AlreadyFills {
            service,
            capability,
        } => Problem::new(
            CANNOT_FILL,
            Severity::Advisory,
            format!("{service} already fills {capability}"),
            "Nothing was changed, and nothing needed to be.",
            listing,
        )
        .in_state(crate::error::State::Guided),
    }
}

/// There is nowhere to record the choice, so the choice cannot be made.
fn nowhere_to_record() -> Problem {
    Problem::new(
        CHOICE_UNWRITABLE,
        Severity::Error,
        "there is no settings file to record the choice in",
        "Nothing was changed. Which service fills a capability is a setting, and \
         this run has no settings file to put it in.",
        Remedy::new("Set this machine up first").with_detail("lemonfiber setup"),
    )
}

#[cfg(test)]
mod tests;
