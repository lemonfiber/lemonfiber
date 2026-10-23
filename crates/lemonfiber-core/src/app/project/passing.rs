//! The pass service's place in the project, made before the project that runs it is
//! written: its image pinned by digest to this release's own, the key it confirms passes
//! with, and the file of live-stream paths it reads.

use std::path::Path;

use lemonfiber_contract::capabilities::stream::guard;
use lemonfiber_manifest::Manifest;
use lemonfiber_ports::registry::Tagged;

use super::super::Ctx;
use crate::config::store;
use crate::config::REACH_REGISTRY_KEY;
use crate::pass::{File, Key, Live, KEY_BYTES, PINNED, SERVICE};
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
    store::write(&directory.join(PINNED), &format!("{}\n", pin?))
        .err()
        .map(|failure| failure.to_string())
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
    let configuration = directory.join(CONFIGURATION_TREE).join(SERVICE);
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
        store::write(&configuration.join(File::Key.name()), &key.written())
            .map_err(|failure| failure.to_string())?;
    }
    if reading(File::Live).is_none() {
        store::write(
            &configuration.join(File::Live.name()),
            &Live::of(Vec::new()).written(),
        )
        .map_err(|failure| failure.to_string())?;
    }
    Ok(())
}

#[cfg(test)]
mod tests;
