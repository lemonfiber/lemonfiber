//! Installing one plugin from a directory this run can read: what it settles, the
//! offer that is its yes, and the writes, proofs and checks that make it installed.
//!
//! Apart from the dispatch beside it because every source comes here in the end: a
//! name is resolved to a git source, a git source is fetched into a directory, and a
//! directory is read.

use std::path::{Path, PathBuf};

use crate::doctor::BUNDLED_CHECKS;
use crate::error::codes::plugin::UNCONFORMING;
use crate::error::{Problem, Remedy, State};
use crate::instant::Instant;
use crate::plugin::{CaseVerdict, Install, Installed, Installs, Register};

use super::super::Ctx;
use super::fetching::Fetched;
use super::offering::{self, Consent};
use super::twice::already;
use super::writing::{carry_out, nowhere_to_write};
use super::{
    kept_at, proving, refused, reversing, standing, unreadable_source, unrecordable, verifying,
    writing,
};

/// Settle what installing this source decides, write the plugin's wiring, and record
/// it.
///
/// Everything a rehearsal holds back is one branch wide, so what a rehearsal reports
/// is what the real run reports — settled by the same code, refused for the same
/// reasons, and stating the same three lists before stopping short of carrying them
/// out.
///
/// **A rehearsal is refused wherever the install would be, including for want of a
/// stack.** The account it gives is the one the install then follows, so a rehearsal
/// that answered on a machine the install could not run on would be describing an
/// operation that cannot happen there — and the operator would find that out on the
/// run they thought they had already checked. What answers with no machine at all is
/// `plugin claims`, which is the author's read and needs neither a stack nor a
/// record.
///
/// **The register is the last thing written, and it is written only once the proofs
/// have held.** That is what makes *registered* and *proved* the same fact rather than
/// two that agree on a good day: the wiring goes down, the plugin's own services come
/// up, every proof it declared is asked of them, and only then is the plugin recorded
/// as installed. Every failure after the first write puts the install back, so what
/// an operator is left with is the machine they had. And a run that dies outright
/// still leaves only files nothing reads — inert, on the change record, and
/// removable — because the register is what layers a plugin's document into the
/// stack. The other order would leave a plugin the machine reports as installed and
/// never proved.
///
/// **A proof that does not hold puts the whole install back.** The container comes off
/// first, because nothing on disk records that it is running and a document removed
/// out from under one leaves something Compose will never be asked about again; then
/// the files go back through the rollback layer, over the journal entries the writing
/// already made. Nothing here undoes anything itself.
///
/// **The yes is the offer.** Asked with none, this is the reading: everything above is
/// What the stack's own checks make of `would` once its proofs held: the reading taken
/// before it, `before`, against one taken again now.
async fn verified(
    ctx: &Ctx,
    (standing, before): (&crate::app::engine::Stack, &[crate::doctor::Finding]),
    would: &Installed,
) -> crate::plugin::Verification {
    let after = verifying::again(ctx, standing).await;
    crate::plugin::against(before, &after, &would.service_ids())
}

/// worked out and stated, with the name it goes by, and nothing is written.
pub(super) async fn install(
    ctx: &Ctx,
    held: Register,
    path: &Path,
    from: Option<&Fetched<'_>>,
    consent: &Consent,
) -> Result<Installs, Box<Problem>> {
    let read = accepted(path, from).await?;

    // One stamp for the run, taken before anything is decided, so the record says it
    // was installed at the moment its changes are journalled under. A plugin fetched
    // from a git source is recorded as coming from that source, at the one commit that
    // was fetched, rather than from the checkout it was read out of.
    let stamp = ctx.stamp();
    let stack_manifest = writing::stack_manifest(ctx)?;
    let (mut would, named) = settled(&stack_manifest, held.installed(), &read, path, from, &stamp);
    let trusted = crate::plugin::first_party::EMBEDDED;
    let granting = super::granting::asked(ctx, &stack_manifest, held.installed(), &would, trusted);
    would.granted = super::granting::approved(&granting, &stamp);
    let mut after = held.clone();
    after
        .record(would.clone())
        .map_err(|there| Box::new(already(&there, &named)))?;
    super::colliding::apart(&would, held.installed())?;
    super::pinning::as_trusted(&would, trusted)?;
    super::granting::aimed(&would, trusted)?;

    // Where the writes land, asked for before the branch rather than inside it. What
    // a rehearsal has to state is where every change goes, and a path is a fact about
    // this machine — so a machine with nowhere to put them has nothing for a
    // rehearsal to state and nothing for an install to do.
    let stack = &super::super::project::directory(ctx)
        .ok_or_else(|| Box::new(nowhere_to_write(&would.plugin)))?;
    let served = crate::seed::run::served(ctx).await;
    super::occupied::unoccupied(ctx, &would, held.installed(), &served)?;
    let planned = super::planned::writes(ctx, &would, stack, Some(&crate::plugin::root(path)))?;
    let wiring = standing::standing(ctx, &stack_manifest, &held, &would);
    let changes = crate::plugin::changes(&planned);
    let offer = offering::installing(&read.digest, &would, &changes, &wiring);
    let (acting, taking) = offering::answered(ctx, consent, &would, &offer, &offering::INSTALLING)?;

    let mut stated = crate::plugin::proofs(&read.manifest);
    let (mut against, mut checked, mut secured) = (None, None, None);
    let mut put_back = None;
    let mut recorded = false;
    let mut recipes_ran = Vec::new();

    if acting {
        // Every value a recipe asks the operator for is given, and nothing else is,
        // before anything is written.
        let consent = &super::following::consented(ctx, &would.plugin, &read.manifest, consent)?;

        // An install starts containers, so it owes the pre-flight every start does,
        // and owes it before anything is written: a machine that would resolve the
        // plugin's mounts somewhere else is refused with nothing to put back.
        super::super::engine::verified(ctx).await?;

        // Read before a byte of it is written, and that order is the whole of what
        // makes the second reading mean anything. What this has to tell apart is a
        // check the install broke from one that was already failing, and after the
        // fact there is nothing left to ask.
        let (standing, before) = verifying::looked(ctx).await?;

        let asked = super::planned::asked(ctx, &would).await;
        super::planned::fenced(ctx, &would, &asked)?;
        let planned = super::planned::writes(ctx, &would, stack, Some(&crate::plugin::root(path)))?;
        writing::unlinked(ctx, &would, stack)?;
        super::minting::minted(ctx, &would, &read.manifest, &stamp, trusted)?;
        carry_out(ctx, &would.plugin, &stamp, &planned)?;
        super::granting::journalled(ctx, &would.plugin, &[], &would.granted, &stamp)?;

        // Started before it is registered, which is why the invocation carries this
        // plugin rather than reading it back: the register is what layers a plugin's
        // document into the stack, and it is deliberately not written yet.
        proving::started(ctx, &would, stack, &stamp).await?;
        let source = crate::plugin::root(path);
        secured = proving::asked(ctx, &read.manifest, &would, stack, &source, &mut stated).await;
        against = Some(proving::AGAINST);

        // The stack is asked only where the plugin's own proofs held. A run that has
        // already failed is a run being put back, and asking a machine mid-reversal
        // what it makes of itself would produce an account of neither state.
        if proving::held(&stated) {
            checked = Some(verified(ctx, (&standing, &before), &would).await);
        }

        // Recorded where both halves held, and put back where either did not. One
        // question answers for both: a verification nobody took is a run whose proofs
        // did not hold, because that is the only way this gets here without one.
        if checked
            .as_ref()
            .is_some_and(crate::plugin::Verification::held)
        {
            // The recipes run once the install holds and before it is recorded, so a
            // recipe that does not hold puts back an install nothing has recorded.
            let coming = Following::of(&read.manifest, &would, &stack_manifest, stack, &stamp);
            recipes_ran = followed(ctx, &coming, consent).await?;
            // Answered for here rather than passed on. The record writer is shared and
            // says *your settings could not be saved, your existing settings are
            // untouched* — which after the lines above is false twice over: the file
            // is not the settings, and the machine has been written to.
            //
            // And it goes back, rather than being left for somebody to find. The
            // proofs held, so the only thing between here and an install is the one
            // file that could not be written — and a plugin whose container is up
            // with nothing recording it is the state this verb exists to not leave.
            if let Err(why) = super::super::record::keep(kept_at(ctx).as_deref(), &after) {
                let back = reversing(ctx, &would, stack, &stamp).await;
                return Err(Box::new(unrecordable(&would.plugin, *why, &back)));
            }
            recorded = true;
            super::guarding::journalled(ctx, &stack_manifest, &would)?;
            super::kept::kept(ctx, &would, &crate::plugin::root(path));
            let _ = super::conformance::cleared(ctx, &would.plugin);
            crate::seed::run::reinstalled(ctx).await;
        } else {
            put_back = Some(reversing(ctx, &would, stack, &stamp).await);
        }
    }

    // What the record holds, which after a rehearsal or a reversal is what it held
    // before. A listing that counted the entry nobody wrote would report an install
    // that did not happen, in the same breath as saying nothing was written — and a
    // reader who believes the count over the sentence is the one this is written for.
    let standing = if recorded { after } else { held };

    Ok(Installs {
        installed: standing.installed().to_vec(),
        install: Some(Box::new(Install {
            granting: crate::plugin::granting(&would),
            sharing: crate::plugin::sharing(&would),
            waiting: standing::waiting(ctx, &stack_manifest, standing.installed(), &would),
            secured: secured.or_else(|| proving::rehearsed(ctx, &would, acting)),
            would,
            recorded,
            changes,
            proofs: stated,
            against,
            verified: checked,
            contests: wiring.contests,
            asks: wiring.asks,
            overrides: crate::plugin::overrides(&read.manifest),
            reversed: put_back,
            recipes_ran,
            taking,
        })),
        agreement: Some(offer),
        ..Installs::default()
    })
}

/// Everything running an install's recipes reads, gathered once.
struct Following<'a> {
    /// The manifest the recipes are declared in.
    manifest: &'a lemonfiber_plugin::Manifest,
    /// What the install settles.
    would: &'a Installed,
    /// The stack's services, which a recipe may call.
    services: &'a [lemonfiber_manifest::Service],
    /// Where the stack is.
    stack: &'a Path,
    /// The stamp the install is journalled under.
    stamp: &'a str,
}

impl<'a> Following<'a> {
    /// What an install of this manifest into this stack reads to run its recipes.
    const fn of(
        manifest: &'a lemonfiber_plugin::Manifest,
        would: &'a Installed,
        stack_manifest: &'a lemonfiber_manifest::Manifest,
        stack: &'a Path,
        stamp: &'a str,
    ) -> Self {
        Self {
            manifest,
            would,
            services: stack_manifest.services.as_slice(),
            stack,
            stamp,
        }
    }
}

/// Run the install's recipes, putting the install back where one does not hold.
///
/// # Errors
///
/// Where a recipe does not hold or what it captured could not be kept, saying what
/// putting the install back left on the machine.
async fn followed(
    ctx: &Ctx,
    coming: &Following<'_>,
    consent: &Consent,
) -> Result<Vec<crate::plugin::running::Ran>, Box<Problem>> {
    let ran = super::following::followed(
        ctx,
        coming.manifest,
        coming.would,
        coming.services,
        consent,
        coming.stamp,
    )
    .await;
    match ran {
        Ok(ran) => Ok(ran),
        Err(unfollowed) => {
            let back = reversing(ctx, coming.would, coming.stack, coming.stamp).await;
            Err(Box::new(
                unfollowed.problem(&coming.would.plugin, super::left_behind(&back)),
            ))
        }
    }
}

/// What installing this manifest from this source would record, and the source to
/// name in a refusal.
///
/// A plugin fetched from a git source is recorded as coming from that source, at the
/// one commit that was fetched, rather than from the checkout it was read out of, and
/// every recipe call to one of the stack's services is given the adapter it reaches
/// through, which only the stack and the plugins `installed_beside` it can say.
pub(super) fn settled(
    _stack: &lemonfiber_manifest::Manifest,
    installed_beside: &[Installed],
    read: &Accepted,
    path: &Path,
    from: Option<&Fetched<'_>>,
    stamp: Instant,
) -> (Installed, PathBuf) {
    let source = crate::plugin::root(path);
    let mut installed = Installed::of(&read.manifest)
        .installed(path, stamp)
        .starting(&|service| {
            crate::plugin::program::starting_digest(Some(crate::plugin::program::Starting {
                root: &source,
                service,
            }))
            .ok()
        });
    installed.manifest.clone_from(&read.digest);
    installed.tree.clone_from(&read.tree);
    let settled = installed.reaching(installed_beside);
    match from {
        Some(fetched) => {
            let fetched_at = settled.fetched(fetched.url, fetched.commit);
            (
                match fetched.vouched {
                    Some(vouched) => fetched_at.vouched(vouched.signed),
                    None => fetched_at,
                },
                PathBuf::from(fetched.url),
            )
        }
        None => (settled, path.to_path_buf()),
    }
}

/// A manifest read and held to everything this build refuses.
pub(super) struct Accepted {
    /// The manifest.
    pub(super) manifest: lemonfiber_plugin::Manifest,
    /// The SHA-256 of the bytes it was read from, in lower-case hexadecimal.
    pub(super) digest: String,
    /// The digest of the plugin's tree read with those bytes.
    pub(super) tree: String,
}

/// The manifest at this path, read and held to everything this build refuses and every
/// case of the contracts its adapters speak to the recordings it ships, and the SHA-256
/// of the bytes it was read from.
///
/// One gate for an install and an update, so a version an update brings on is refused
/// for exactly what an install of it would be. A refusal is total: none of a refused
/// manifest is acted on. Where the catalogue vouched for the source, the bytes read
/// here are the ones held to what it reviewed, so what is checked and what is installed
/// are one read and not two.
///
/// # Errors
///
/// Where the path holds no manifest this build can read, where the catalogue vouched
/// for a different one, where this build refuses it, or where a case of a contract it
/// speaks is not answered by a recording it ships.
pub(super) async fn accepted(
    path: &Path,
    from: Option<&Fetched<'_>>,
) -> Result<Accepted, Box<Problem>> {
    let crate::plugin::Digested {
        manifest,
        digest,
        tree,
    } = crate::plugin::read_digested(path)
        .map_err(|unreadable| Box::new(unreadable_source(&unreadable)))?;
    if let Some(vouched) = from.and_then(|fetched| fetched.vouched) {
        if !vouched.entry.reviewed(&digest) {
            return Err(Box::new(super::cataloguing::not_as_reviewed(vouched.entry)));
        }
    }
    let mut refusals = lemonfiber_plugin::refusals(&manifest, BUNDLED_CHECKS);
    refusals.extend(lemonfiber_plugin::uninstallable(&manifest));
    if !crate::plugin::first_party::named(
        crate::plugin::first_party::EMBEDDED,
        &manifest.plugin.id,
        &digest,
        &tree,
    ) {
        refusals.extend(lemonfiber_plugin::claiming::awaited(&manifest));
    }
    match crate::plugin::seeding::shipped(
        manifest.services.iter().map(|service| service.id.as_str()),
        &crate::plugin::root(path),
    ) {
        Ok(shipped) => refusals.extend(crate::plugin::seeding::refused(&manifest, &shipped)),
        Err(message) => refusals.push(lemonfiber_plugin::Violation {
            location: crate::project::entry::CONFIGURATION_TREE.to_owned(),
            message,
        }),
    }
    refusals.extend(crate::plugin::seeding::strays(
        manifest.services.iter().map(|service| service.id.as_str()),
        &crate::plugin::root(path),
    ));
    if !refusals.is_empty() {
        return Err(Box::new(refused(&manifest, &refusals)));
    }
    let unpassed: Vec<String> = lemonfiber_conformance::judged(
        &crate::plugin::root(path),
        &crate::plugin::conforming::speakers(&manifest),
        &lemonfiber_contract::capabilities::all(),
    )
    .await
    .iter()
    .filter(|judged| judged.verdict != CaseVerdict::Passed)
    .map(ToString::to_string)
    .collect();
    if !unpassed.is_empty() {
        return Err(Box::new(unconforming(&manifest, &unpassed)));
    }
    Ok(Accepted {
        manifest,
        digest,
        tree,
    })
}

/// A plugin a case of whose contracts is not answered by a recording it ships, each such
/// case named in `unpassed`.
fn unconforming(manifest: &lemonfiber_plugin::Manifest, unpassed: &[String]) -> Problem {
    Problem::new(
        UNCONFORMING,
        format!(
            "{}'s adapters do not answer every case of the contracts they speak",
            manifest.plugin.id
        ),
        "Nothing was installed and nothing was written. A plugin fills a capability only where \
         a recording it ships answers each case of the contract, and these do not.",
        Remedy::new("Take it up with whoever published the plugin, naming each case")
            .with_detail("lemonfiber plugin conform shows what every case came to"),
    )
    .in_state(State::Guided)
    .with_detail(unpassed.join("; "))
}
