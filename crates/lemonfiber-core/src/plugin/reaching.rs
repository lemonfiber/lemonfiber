//! Reaching a plugin's adapter service over the contracts it speaks.
//!
//! The core runs on the host, so it reaches an adapter on the host's loopback interface,
//! at the host port the engine gave the port the adapter listens on, under the key the
//! install wrote beside the adapter's configuration. Each of the three is read where it
//! is held, and nothing here composes an address from anything else.

use std::path::Path;
use std::sync::Arc;

use lemonfiber_contract::adapter::KEY_FILE;
use lemonfiber_contract::Contracted;

use super::container::LOOPBACK;
use super::{key_file, Placed};
use crate::app::Ctx;
use crate::ports::filesystem::Beneath;
use crate::wiring::Filler;

/// The adapter `placed` is, asked over its contracts, or why it cannot be reached.
///
/// # Errors
///
/// A sentence saying what is missing: the port it speaks on, its key, or the engine's
/// word on where that port is published.
pub(crate) async fn reached(
    ctx: &Ctx,
    stack: &Path,
    placed: &Placed,
) -> Result<Contracted, String> {
    asked(
        ctx,
        &placed.service,
        placed.listens,
        &key_file(stack, &placed.service),
    )
    .await
}

/// The service filling a capability, asked over the contracts it speaks, or why it
/// cannot be reached.
///
/// # Errors
///
/// As [`reached`], and where the stack has not been written to this machine, so there
/// is no key beside the service to read.
pub(crate) async fn filling(ctx: &Ctx, filler: &Filler) -> Result<Contracted, String> {
    let service = &filler.id;
    let key = filler
        .confined_to
        .as_ref()
        .map(|configuration| configuration.join(KEY_FILE))
        .ok_or_else(|| format!("{service} has no configuration on this machine to hold its key"))?;
    let listens = filler.address.as_ref().map(|address| address.port);
    asked(ctx, service, listens, &key).await
}

/// `service`, listening on `listens` with its key at `key`, asked over its contracts.
async fn asked(
    ctx: &Ctx,
    service: &str,
    listens: Option<u16>,
    key: &Path,
) -> Result<Contracted, String> {
    let listens =
        listens.ok_or_else(|| format!("{service} says no port it speaks its contracts on"))?;
    let key = keyed(ctx, key, service)?;
    let port = published(ctx, service, listens).await?;
    Ok(Contracted::new(
        Arc::clone(&ctx.seams.http),
        format!("http://{LOOPBACK}:{port}"),
        service,
        key,
    ))
}

/// The key written beside `service`'s configuration, read as a file its container may
/// have touched.
fn keyed(ctx: &Ctx, at: &Path, service: &str) -> Result<String, String> {
    let within = at.parent().unwrap_or(at);
    match ctx.seams.confined.read(at, within) {
        Beneath::Read(written) if !written.trim().is_empty() => Ok(written.trim().to_owned()),
        Beneath::Read(_) | Beneath::Absent => {
            Err(format!("{service} holds no key to be asked with"))
        }
        Beneath::Escaped => Err(format!(
            "{service}'s key is not a plain file of its own, so it was not read"
        )),
    }
}

/// The host port the engine published `listens` on for `service`, on loopback.
async fn published(ctx: &Ctx, service: &str, listens: u16) -> Result<u16, String> {
    let containers = ctx
        .seams
        .engine
        .list(&ctx.settings.project)
        .await
        .map_err(|_| format!("the engine would not say where {service} is published"))?;
    containers
        .iter()
        .filter(|container| container.service == service)
        .flat_map(|container| &container.published)
        .find(|published| published.private == listens && published.address.is_loopback())
        .map(|published| published.port)
        .ok_or_else(|| {
            format!("{service} publishes no port on this machine's loopback for {listens}")
        })
}

#[cfg(test)]
mod tests;
