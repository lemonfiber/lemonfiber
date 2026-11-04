//! The group owning the render devices of the engine's host, asked of the engine before a
//! start writes a project holding a service taking `render`: the fence image's
//! `fence render-group`, run once with the host's render devices mounted read-only and no
//! other mount, every capability dropped and a read-only root filesystem, and removed once
//! it ends. Asked again every time, so the group written is the one the host holds now.

use lemonfiber_manifest::container::{Shape, RENDER_DEVICE};
use lemonfiber_manifest::Manifest;

use super::super::Ctx;
use super::fencing::{answered, kept};
use crate::fence::RENDER_HELD;
use crate::ports::program::{Bound, Joined};

/// The argument the fence image answers its host's render group under.
pub(crate) const ASKED: &str = "render-group";

/// Ask the engine for the group owning its host's render devices where a service of
/// `manifest`'s or an installed plugin's takes `render`, with `image` the fence this build
/// pins, and keep the answer where the project reads it, keeping none where it answers
/// none; say why none is kept, where it answered nothing that can be read. A rehearsal
/// runs and keeps nothing.
pub(crate) async fn readied(ctx: &Ctx, manifest: &Manifest, image: Option<&str>) -> Option<String> {
    if ctx.dry_run || !renders(ctx, manifest) {
        return None;
    }
    let directory = super::directory(ctx)?;
    let answered = match image {
        Some(image) => asked(ctx, image).await,
        None => Err("this build's bundle pins no fence to ask the engine's host with".to_owned()),
    };
    let text = answered
        .as_ref()
        .ok()
        .copied()
        .flatten()
        .map(|group| group.to_string())
        .unwrap_or_default();
    let why = kept(&directory.join(RENDER_HELD), &text).err();
    answered.err().or(why)
}

/// Each service of `project` written without the render devices its shape takes, in words
/// naming it.
pub(crate) fn unrendered(project: &crate::project::Project) -> Vec<String> {
    project
        .unrendered
        .iter()
        .map(|service| format!("{service} runs without hardware transcoding"))
        .collect()
}

/// Whether a service of `manifest`'s, or of a plugin installed here, takes `render`.
fn renders(ctx: &Ctx, manifest: &Manifest) -> bool {
    manifest
        .services
        .iter()
        .any(|service| service.shape == Some(Shape::Render))
        || super::super::plugins::read(ctx).is_ok_and(|register| {
            register.installed().iter().any(|installed| {
                installed
                    .services
                    .iter()
                    .any(|placed| placed.shape == Some(Shape::Render))
            })
        })
}

/// What `image` answers, run as `fence render-group` with the host's render devices
/// mounted read-only: the group owning them, or none where the host holds none.
async fn asked(ctx: &Ctx, image: &str) -> Result<Option<u32>, String> {
    let devices = Bound {
        source: RENDER_DEVICE.into(),
        target: RENDER_DEVICE.to_owned(),
        read_only: true,
    };
    let said = answered(
        ctx,
        image,
        ASKED,
        (Joined::Nothing, Some(devices)),
        "its render group",
    )
    .await?;
    if said.trim().is_empty() {
        return Ok(None);
    }
    crate::fence::group(&said)
        .map(Some)
        .ok_or_else(|| "the engine's host answered a render group that is not one".to_owned())
}

#[cfg(test)]
mod tests;
