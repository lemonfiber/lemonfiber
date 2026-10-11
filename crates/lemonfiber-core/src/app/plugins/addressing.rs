//! The addresses a plugin's variables are written with, from the asks the wiring settles.

use std::path::Path;

use lemonfiber_manifest::container::UrlOf;
use lemonfiber_manifest::Manifest;

use super::super::Ctx;
use crate::plugin::{Installed, Reading, Write};
use crate::wiring::Wired;

/// What installing `installed` writes, each variable asking for an address written with
/// the one its ask settles on beside everything else installed here.
///
/// Every address is left out where the stack's manifest or the record of what is
/// installed cannot be read, as it is where nothing fills the ask.
pub(crate) fn writes(ctx: &Ctx, installed: &Installed, stack: &Path) -> Vec<Write> {
    let manifest = super::writing::stack_manifest(ctx).ok();
    let beside: Vec<Installed> = super::read(ctx)
        .map(|register| {
            register
                .installed()
                .iter()
                .filter(|one| one.plugin != installed.plugin)
                .cloned()
                .collect::<Vec<_>>()
        })
        .unwrap_or_default()
        .into_iter()
        .chain(std::iter::once(installed.clone()))
        .collect();
    let settled = manifest.as_ref().map(|manifest| {
        crate::wiring::settle(manifest, &beside, &crate::app::targets::chosen_fillers(ctx))
    });
    let address = |by: &str, url: &UrlOf| {
        manifest
            .as_ref()
            .zip(settled.as_deref())
            .and_then(|(manifest, settled)| {
                crate::wiring::address(settled, manifest, &beside, by, url)
            })
    };
    let reading = Reading {
        address: &address,
        ..Reading::bare()
    };
    crate::plugin::writes(installed, stack, &reading)
}

/// The Compose document of `one`, each variable asking for an address written with the
/// one its ask settled on in `settled`.
pub(crate) fn written(
    settled: &[Wired],
    manifest: &Manifest,
    installed: &[Installed],
    one: &Installed,
) -> String {
    let address =
        |by: &str, url: &UrlOf| crate::wiring::address(settled, manifest, installed, by, url);
    crate::plugin::written(
        one,
        &Reading {
            address: &address,
            ..Reading::bare()
        },
    )
}
