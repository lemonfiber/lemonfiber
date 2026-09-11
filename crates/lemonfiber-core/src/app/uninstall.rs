//! Taking lemonfiber off this machine.
//!
//! Two errands over one answer, the shape a forget and a reclaim both take: asked
//! without an agreement it enumerates and touches nothing, asked with one it removes
//! exactly what it enumerated. Both answer with the same value, because what is
//! agreed to has to be what was read — a removal that summarised a manifest in
//! different words would be a second account of the same thing, and the operator
//! would be agreeing to the summary.
//!
//! **Nothing here refuses over a broken machine.** A wedged stack is a common reason
//! to uninstall. An unreadable configuration, an unreachable daemon and a stack
//! description that will not parse are each recorded as a gap in the manifest and
//! everything else goes on — because the alternative is a product that is hardest to
//! remove exactly when somebody most wants it gone.
//!
//! **The one tier that reaches the library takes an agreement of its own.** Not a
//! flag: the name of the reading, built over the list and the size in the words they
//! were shown in, so a yes given against one disk cannot be spent on another.

mod gathering;
mod lines;
mod removing;

use crate::error::{Problem, Remedy, Severity, State};
use crate::uninstall::{
    against, beside, naming, reclaimable, Manifest, Removal, Tier, Uninstall, ANOTHER_READING,
    BESIDE, NEEDS_AGREEING,
};

use super::command::Removing;
use super::{Ctx, Outcome};

/// What a removal would come to, or what it came to.
///
/// # Errors
///
/// Returns a [`Problem`] where the tier that takes the library was confirmed without
/// an agreement, or where an agreement names a reading that is not the one standing
/// now. Nothing else here refuses: a machine too broken to read is a machine this
/// still has to be able to leave.
pub(super) async fn uninstalled(ctx: &Ctx, asked: Removing) -> Result<Outcome, Box<Problem>> {
    let manifest = survey(ctx, asked.tier).await;

    if !asked.goes_ahead() {
        return Ok(answered(manifest, Removal::Surveyed));
    }
    held(&asked, &manifest)?;

    // A rehearsal ends where a confirmed run begins, which is what `--dry-run` means
    // everywhere else in this product: the agreement was given and taken, and nothing
    // was touched.
    if ctx.dry_run {
        return Ok(answered(manifest, Removal::Confirmed));
    }

    let removal = removing::remove(ctx, asked.tier, &manifest, asked.waiting).await?;
    Ok(answered(manifest, removal))
}

/// The manifest and what became of it, as one answer.
fn answered(manifest: Manifest, removal: Removal) -> Outcome {
    Outcome::Uninstall(Uninstall { manifest, removal })
}

/// Whether this run holds what it takes to act.
///
/// Two rules, checked in the order they are read. The tier that takes the library
/// needs an agreement at all; and any agreement given must name the reading that
/// stands now, whichever tier it was given for — a yes carried over from a run that
/// listed different things is a yes to something nobody saw.
fn held(asked: &Removing, manifest: &Manifest) -> Result<(), Box<Problem>> {
    match asked.agreement.as_deref() {
        None if asked.tier.needs_its_own_agreement() => Err(Box::new(needs_agreeing(
            &manifest.agreement,
            manifest.bytes,
        ))),
        Some(given) if given != manifest.agreement => {
            Err(Box::new(another_reading(&manifest.agreement)))
        }
        None | Some(_) => Ok(()),
    }
}

/// Everything one tier reaches, read and judged.
async fn survey(ctx: &Ctx, tier: Tier) -> Manifest {
    let gathered = gathering::gather(ctx, tier).await;

    let types: Vec<String> = gathered
        .services
        .iter()
        .flat_map(|service| service.media_types.clone())
        .collect();
    let foreign = gathered
        .root
        .as_ref()
        .map(|root| beside(root, &gathered.walked, &types))
        .unwrap_or_default();

    let items = lines::items(tier, &ctx.settings.project, &gathered, &foreign);
    let bytes = reclaimable(&items);
    let agreement = naming(tier, &items, &foreign, gathered.volume.as_deref());

    Manifest {
        tier,
        removes: tier.removes().to_owned(),
        keeps: tier.keeps().to_owned(),
        bytes,
        outside: BESIDE
            .iter()
            .zip(gathered.found.iter())
            .map(|(entry, found)| against(entry, ctx.environment, *found))
            .collect(),
        backup: tier.touches_configuration().then(|| BACKUP.to_owned()),
        items,
        foreign,
        volume: gathered.volume,
        coming: gathered.coming,
        confidence: gathered.confidence,
        agreement,
    }
}

/// What is said about the backup taken before anything that cannot be made again goes.
///
/// A statement rather than the advice it used to be. Telling somebody to run `lemonfiber
/// backup` first put the one step that makes a removal survivable on the far side of a
/// sentence they had to read, agree with and act on — and the run went ahead either way.
/// It is taken now, after the stop and before the destruction, and a capture that fails
/// stops the removal.
const BACKUP: &str = "A backup is taken before any of this goes — after the services stop \
     and before anything is removed — so this machine can be set up like it is now again. \
     `lemonfiber restore` puts it back, which is a great deal less work than answering \
     setup again. If that backup cannot be taken, nothing is removed.";

/// The tier that takes the library was confirmed without an agreement.
fn needs_agreeing(agreement: &str, bytes: u64) -> Problem {
    let size = crate::bytes::humanize(bytes);
    Problem::new(
        NEEDS_AGREEING,
        Severity::Error,
        format!("Removing your library would destroy {size}, and that takes its own answer"),
        format!(
            "Every other removal here leaves your library where it is, and this one \
             does not. {size} of content you may have spent years building is not \
             something a yes meant for the containers should be able to take, so this \
             tier is answered by the name the reading printed rather than by a flag."
        ),
        Remedy::new("Read what would go, then answer that reading by its own name")
            .with_detail(format!("lemonfiber uninstall media --agreed {agreement}")),
    )
    .in_state(State::Guided)
}

/// The agreement names a reading that is not the one standing now.
fn another_reading(agreement: &str) -> Problem {
    Problem::new(
        ANOTHER_READING,
        Severity::Error,
        "That agreement was given for a different reading of this machine",
        "Everything you read before answering is in the name a reading goes by — the \
         list, what each line occupies, what was found beside the library, and what \
         sort of drive it is on. Something has changed since, so acting on this answer \
         would be acting on something nobody saw.",
        Remedy::new("Read it again, and answer the name it prints")
            .with_detail(format!("the reading standing now is {agreement}")),
    )
    .in_state(State::Guided)
}

#[cfg(test)]
mod tests;
