//! The pass service's place in the project, made before the project that runs it is
//! written: its image pinned by digest to this release's own, the key it confirms passes
//! with, and the file of live-stream paths it reads.

use std::path::{Path, PathBuf};
use std::sync::Mutex;

use lemonfiber_contract::capabilities::stream::guard;
use lemonfiber_manifest::Manifest;
use lemonfiber_ports::registry::Tagged;

use super::super::Ctx;
use crate::config::store;
use crate::config::REACH_REGISTRY_KEY;
use crate::pass::{renewed_as, File, Key, Paths, Renewals, Renewed, KEY_BYTES, PINNED, SERVICE};
use crate::project::entry::CONFIGURATION_TREE;
use crate::stack::RUNNING;

/// Make the pass service's files and pin its image, where a guard is filled, the pin last
/// so no project runs the pass service without its key; and say why the pass service is
/// left out of the project, where it is.
pub(crate) async fn readied(ctx: &Ctx, manifest: &Manifest) -> Option<String> {
    let register = super::super::plugins::read(ctx).ok()?;
    crate::project::placed_at(
        manifest,
        register.installed(),
        &super::super::targets::chosen_fillers(ctx),
        guard::CAPABILITY,
    )?;
    if crate::project::pass_taken(manifest, register.installed()) {
        return Some(format!(
            "a service the stack or a plugin brings is named {SERVICE}, the id lemonfiber runs \
             its pass service under"
        ));
    }
    let directory = super::directory(ctx)?;
    let pin = match super::pinned(ctx) {
        Some(_) => None,
        None => match pinning(ctx).await {
            Ok(pin) => Some(pin),
            Err(why) => return Some(why),
        },
    };
    if let Err(why) = made(ctx, &directory) {
        return Some(why);
    }
    kept(&directory.join(PINNED), &format!("{}\n", pin?)).err()
}

/// The pin of this release's own image, asked of the registry it is published to.
async fn pinning(ctx: &Ctx) -> Result<String, String> {
    let product = crate::PRODUCT;
    if !ctx.settings.reaching.allows(REACH_REGISTRY_KEY) {
        return Err(format!(
            "fetching images is switched off, so the image of {product} {RUNNING} the pass \
             service runs is not pinned"
        ));
    }
    let tagged = Tagged::new(crate::IMAGE, RUNNING);
    match ctx.seams.registry.digest(&tagged).await {
        Ok(Some(digest)) => {
            let pin = crate::pass::pin(RUNNING, &digest);
            crate::pass::pinned(&pin, RUNNING).ok_or_else(|| {
                format!("the registry named {digest} for {product} {RUNNING}, which is no digest")
            })
        }
        Ok(None) => Err(format!(
            "no image of {product} {RUNNING} is published, so the pass service has no image to run"
        )),
        Err(why) => Err(format!(
            "the image of {product} {RUNNING} the pass service runs could not be pinned: {why}"
        )),
    }
}

/// Make the pass service's key where it holds none that reads, and its file of
/// live-stream paths where there is none yet.
fn made(ctx: &Ctx, directory: &Path) -> Result<(), String> {
    let configuration = configuration(directory);
    let reading = |file: File| {
        crate::within::read_unlinked(
            ctx.seams.confined.as_ref(),
            &configuration.join(file.name()),
            directory,
        )
        .ok()
        .flatten()
    };
    if reading(File::Key).is_none_or(|text| Key::read(&text).is_err()) {
        let key = ctx
            .seams
            .random
            .bytes(KEY_BYTES)
            .and_then(|bytes| <[u8; KEY_BYTES]>::try_from(bytes).ok())
            .map(Key::of)
            .ok_or("no randomness could be drawn for the pass service's key")?;
        kept(&configuration.join(File::Key.name()), &key.written())?;
    }
    if reading(File::Paths).is_none() {
        kept(
            &configuration.join(File::Paths.name()),
            &Paths::of(Vec::new(), Vec::new()).written(),
        )?;
    }
    Ok(())
}

/// The key the pass service confirms passes with, as the project keeps it for it;
/// nothing where there is no project or no key there that reads.
pub(crate) fn key(ctx: &Ctx) -> Option<Key> {
    let directory = super::directory(ctx)?;
    crate::within::read_unlinked(
        ctx.seams.confined.as_ref(),
        &configuration(&directory).join(File::Key.name()),
        &directory,
    )
    .ok()
    .flatten()
    .and_then(|text| Key::read(&text).ok())
}

/// Hold each of `passes`, issued to `member`, good until `lapses`, forgetting every
/// renewal that lapsed by `now`; whether the pass service was told.
pub(crate) fn renewed(ctx: &Ctx, passes: &[String], member: &str, lapses: u64) -> bool {
    let now = ctx.seconds();
    changed(ctx, |renewals| {
        renewals.passes.retain(|_, renewed| renewed.lapses > now);
        for name in passes.iter().filter_map(|pass| renewed_as(pass)) {
            renewals.passes.insert(
                name,
                Renewed {
                    member: member.to_owned(),
                    lapses,
                },
            );
        }
    })
}

/// Forget every renewal of a pass issued to `member`, as they leave the household.
pub(crate) fn unrenewed(ctx: &Ctx, member: &str) {
    let _ = changed(ctx, |renewals| {
        renewals
            .passes
            .retain(|_, renewed| renewed.member != member);
    });
}

/// Held over every change to the renewals, so two in this process never write over each
/// other.
static RENEWING: Mutex<()> = Mutex::new(());

/// Change the renewals the pass service reads, holding every other change in this
/// process off until it is written; whether it was written.
fn changed(ctx: &Ctx, change: impl FnOnce(&mut Renewals)) -> bool {
    let Some(directory) = super::directory(ctx) else {
        return false;
    };
    let at = configuration(&directory).join(File::Renewals.name());
    let _held = RENEWING
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let mut renewals = crate::within::read_unlinked(ctx.seams.confined.as_ref(), &at, &directory)
        .ok()
        .flatten()
        .and_then(|text| Renewals::read(&text).ok())
        .unwrap_or_else(Renewals::none);
    change(&mut renewals);
    kept(&at, &renewals.written()).is_ok()
}

/// The pass service's configuration directory beneath the project directory `directory`.
fn configuration(directory: &Path) -> PathBuf {
    directory.join(CONFIGURATION_TREE).join(SERVICE)
}

/// Write `text` to `path` owner-only, or say why it was not written.
fn kept(path: &Path, text: &str) -> Result<(), String> {
    store::write(path, text).map_err(|failure| failure.to_string())
}

#[cfg(test)]
mod tests;
