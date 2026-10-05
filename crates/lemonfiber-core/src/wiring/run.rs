//! Reading what this stack wires to what, and changing which service fills a thing.
//!
//! Two operations on one subject. The listing is a read of the manifest and of the
//! one setting that records a choice; the substitution writes that setting and
//! journals it, having first worked out what it would cost.
//!
//! Worked out *first* and in full, which is the rule the reconfiguration path keeps
//! for the same reason: an operator told afterwards that something stopped being
//! filled has been told about a thing they can no longer choose.

use crate::error::{Amiss, Problem, Remedy, Severity};

use super::Refused;
use crate::app::Ctx;
use crate::error::codes::wire::{
    CANNOT_FILL, CHOICE_UNWRITABLE, NOTHING_ASKS, NO_SUCH_FILLER, UNREASONABLE, WIRING_MOVED,
};
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
pub fn listing(ctx: &Ctx) -> Result<WiringReport, Box<Problem>> {
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

/// The parts a choice's offer is named over, in the order it is built, as a refusal
/// names them.
///
/// The choice itself is one of them: an answer carried over to a different service or
/// capability is not an answer to this reading, however much else it shares.
const OFFERED: [&str; 4] = [
    "the choice itself",
    "what fills it now",
    "what asks for it",
    "what it would leave unfilled",
];

/// Choose which service fills a capability, and say what that costs.
///
/// Answered with no agreement, it is the reading: the whole change worked out, with
/// the name it goes by, and nothing written. Answered with that name, the reading is
/// worked out again from the wiring as it stands and the change is made only where
/// the two agree. A rehearsal checks the name the same way and writes nothing either.
///
/// # Errors
///
/// Returns the [`Problem`] for a reason that cannot be recorded, a stack that cannot be
/// read, a service this stack does not have, one that cannot do the thing, a capability
/// nothing asks for, one the named service already fills, an agreement naming a
/// reading that has since moved, or a settings file that could not be written.
fn substituting(
    ctx: &Ctx,
    filling: &crate::app::Filling,
) -> Result<SubstitutionReport, Box<Problem>> {
    let reason = filling.reason.as_deref().filter(|said| !said.is_empty());
    if let Some(said) = reason {
        reasonable(said)?;
    }
    let manifest = ctx
        .stack
        .checked_manifest(ctx.today())
        .map_err(|err| Box::new(err.problem()))?;

    let held = crate::app::targets::chosen_fillers(ctx);
    let installed = crate::app::plugins::read(ctx)?;
    let mut substitution = super::substitute(
        &manifest,
        installed.installed(),
        &held,
        &filling.capability,
        &filling.service,
    )
    .map_err(|refused| Box::new(problem(&refused)))?;
    substitution.why = reason.map(str::to_owned);

    let standing = offer(&substitution);
    let reading = |applied| SubstitutionReport {
        rehearsed: false,
        substitution: substitution.clone(),
        applied,
        agreement: standing.clone(),
    };
    let Some(answered) = filling.agreement.as_deref() else {
        return Ok(reading(false));
    };
    let moved = crate::agreement::differs(answered, &standing, &OFFERED);
    if !moved.is_empty() {
        return Err(Box::new(offer_moved(
            &filling.capability,
            &moved,
            &standing,
        )));
    }
    if ctx.dry_run {
        return Ok(reading(false));
    }

    // The record of the change goes down before the change does, so a run stopped
    // between the two leaves a journal entry for a setting that still holds its old
    // value — which unwinds to the value it already has. The other order leaves a
    // changed setting nothing can put back.
    let (Some(path), Some(paths)) = (
        ctx.settings.env_file.as_deref(),
        crate::app::targets::layout(ctx),
    ) else {
        return Err(Box::new(nowhere_to_record()));
    };
    // The reasons are written only where they change, so a choice made with nothing
    // said, over choices that had nothing said either, leaves no trace of a setting
    // that holds nothing.
    let reasons = held.reasons_with(&filling.capability, reason);
    let before = held.reasons();
    let stamp = ctx.stamp();
    let mut changes = vec![super::recorded(
        &substitution,
        held.setting().as_deref(),
        &stamp,
    )];
    let mut writes = vec![(super::FILLS_KEY, substitution.setting.as_str())];
    if reasons != before {
        changes.push(super::recorded_why(
            &substitution,
            before.as_deref(),
            reasons.as_deref(),
            &stamp,
        ));
        writes.push((super::FILLS_WHY_KEY, reasons.as_deref().unwrap_or_default()));
    }
    // What the choice moves goes with it: each plugin whose service it settles in for a
    // stack service, or out of one, joins what that settles, so its document and the
    // record it is written from are rewritten in the same journalled change.
    let overwrites = crate::app::plugins::writing::rejoined(
        ctx,
        &manifest,
        &installed,
        &crate::wiring::Chosen::read(Some(&substitution.setting)),
    );
    changes.extend(
        overwrites
            .iter()
            .map(|overwrite| super::overwritten(overwrite, &stamp)),
    );
    crate::app::recover::journalled(&paths.journal(), &changes, ctx.seams.random.as_ref())
        .map_err(|failure| Box::new(failure.problem()))?;
    for (key, value) in writes {
        kept(path, key, value)?;
    }
    for overwrite in &overwrites {
        crate::config::store::write(&overwrite.path, &overwrite.text)
            .map_err(|err| Box::new(err.problem()))?;
    }

    Ok(reading(true))
}

/// One setting written, or the settings file's own refusal.
fn kept(path: &std::path::Path, key: &str, value: &str) -> Result<(), Box<Problem>> {
    crate::config::store::set(path, key, value).map_err(|err| Box::new(err.problem()))
}

/// What a reading of a choice names itself, part by part.
fn offer(substitution: &crate::wiring::Substitution) -> String {
    let choice = [substitution.capability.as_str(), substitution.now.as_str()];
    let was = [substitution.was.as_deref().unwrap_or_default()];
    let asked: Vec<&str> = substitution.asked_by.iter().map(String::as_str).collect();
    let left: Vec<String> = substitution
        .leaves_unfilled
        .iter()
        .map(ToString::to_string)
        .collect();
    let left: Vec<&str> = left.iter().map(String::as_str).collect();
    crate::agreement::parted(&[&choice, &was, &asked, &left])
}

/// A reason that can be recorded beside a choice: one line of printable text, and no
/// longer than a reason may be.
///
/// A control character is refused with the line breaks, because the reason is printed
/// back to a terminal, and one that carried an escape sequence would be rewriting the
/// screen it is shown on.
fn reasonable(said: &str) -> Result<(), Box<Problem>> {
    let long = said.chars().count() > crate::wiring::REASON_MOST;
    if long || said.chars().any(char::is_control) {
        return Err(Box::new(unreasonable(long)));
    }
    Ok(())
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
        .or_try(listing)
        .lies_in(Amiss::Naming),
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
        .or_try(listing)
        .lies_in(Amiss::Asking),
        Refused::NothingAsks(capability) => Problem::new(
            NOTHING_ASKS,
            Severity::Warning,
            format!("nothing in this stack asks for {capability}"),
            "Nothing was changed. Choosing who fills a capability nothing asks for \
             would record a setting no wiring reads.",
            listing,
        )
        .lies_in(Amiss::Asking),
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
        .in_state(crate::error::State::Guided)
        .lies_in(Amiss::Asking),
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

/// A choice answering a reading of the wiring that has since moved.
///
/// Every part that moved is named, because an operator told only that something
/// changed has to read the whole choice again to find out what.
fn offer_moved(capability: &str, moved: &[&str], standing: &str) -> Problem {
    crate::agreement::moved(
        Problem::new(
            WIRING_MOVED,
            Severity::Error,
            format!("That choice was agreed to against a different reading of {capability}"),
            format!(
                "Since it was read, {} changed, so nothing was changed. Agreeing to it now \
                 would be agreeing to something nobody saw.",
                moved.join(" and ")
            ),
            Remedy::new("Read the choice again, and answer the name it prints")
                .with_detail(format!("the offer standing now is {standing}")),
        )
        .in_state(crate::error::State::Guided),
    )
}

/// A reason that cannot be recorded beside a choice.
fn unreasonable(long: bool) -> Problem {
    let why = if long {
        format!(
            "A reason is read on one line beside the choice it explains, and may be at most \
             {} characters.",
            crate::wiring::REASON_MOST
        )
    } else {
        "A reason is read on one line beside the choice it explains, so it may hold no \
         line break or other control character."
            .to_owned()
    };
    Problem::new(
        UNREASONABLE,
        Severity::Error,
        "That reason cannot be recorded with the choice",
        format!("{why} Nothing was changed."),
        Remedy::new("Say why in one shorter line, or make the choice with no reason"),
    )
    .lies_in(Amiss::Asking)
}

#[cfg(test)]
mod tests;
