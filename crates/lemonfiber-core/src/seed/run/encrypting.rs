//! The media server's internal TLS port: the certificate the core makes for it before the
//! stack starts, the media server told to serve it, and the pins every holder of a media
//! server credential is handed with it.

use std::path::Path;

use super::Ctx;
use crate::app::targets::MediaServer;
use crate::certificate::internal;
use lemonfiber_contract::capabilities::media::serve;

use crate::ports::service::Applied;
use crate::seed::{State, Wiring};
use crate::stack::closure::Plan;
use crate::stack::compose::{build, Action};

/// What the report calls this connection.
pub(super) fn connection(server: &MediaServer) -> String {
    format!(
        "{} serving its internal encrypted port with the certificate lemonfiber made for it",
        server.name()
    )
}

/// Make the media server's internal certificate before the stack starts, where the stack
/// has a media server and has been written to disk, and begin replacing the one kept
/// where it is not made out to the service the media server runs as: the seed hands every
/// holder the replacement beside it, then serves it.
pub(crate) fn made_before_start(ctx: &Ctx, manifest: &lemonfiber_manifest::Manifest) {
    let Some(server) = crate::app::targets::hosted(ctx, manifest) else {
        return;
    };
    let Some(config) = server.served.confined_to.as_deref() else {
        return;
    };
    let confined = ctx.seams.confined.as_ref();
    let service = &server.served.id;
    if let Ok(kept) = internal::kept_or_made(confined, config, service) {
        if !kept.made_out_to(service) {
            let _ = internal::staged(confined, config, service);
        }
    }
}

/// The pins every holder of a credential of `server` is handed: every certificate kept for
/// its internal TLS port, the one in force and any replacing it or replaced by it. None
/// where none is kept, which reaches nothing.
///
/// # Errors
///
/// Where one that is kept cannot be read, so no holder is handed fewer than it holds.
pub(crate) fn pins(ctx: &Ctx, server: &MediaServer) -> Result<Vec<String>, String> {
    let Some(config) = server.served.confined_to.as_deref() else {
        return Ok(Vec::new());
    };
    internal::replacing(ctx.seams.confined.as_ref(), config)
        .map(|kept| kept.pins())
        .map_err(|unkept| {
            format!(
                "the certificates kept for {} could not be read, so every holder keeps what it \
                 holds: {unkept}",
                server.name()
            )
        })
}

/// Where the media server's container holds its internal certificate, where the server
/// is a plugin's service whose manifest says where its configuration is mounted.
pub(super) fn inside(server: &MediaServer) -> Option<String> {
    server.served.config_at.as_deref().map(|config| {
        Path::new(config)
            .join(internal::INTERNAL)
            .display()
            .to_string()
    })
}

/// Have the media server serve its internal TLS port with the certificate kept for it,
/// starting it again where it serves it only once it restarts.
pub(super) async fn seed_internal_tls(ctx: &Ctx, server: Option<&MediaServer>) -> Option<Wiring> {
    let server = server?;
    let path = inside(server)?;
    let settled = |state| Some(Wiring::settled(connection(server), state));
    let Some(client) = server.administering(ctx).await else {
        return ctx.dry_run.then(|| {
            Wiring::settled(
                connection(server),
                State::WouldWire {
                    yours: None,
                    ours: Some(path.clone()),
                },
            )
        });
    };
    if ctx.dry_run {
        return settled(State::WouldWire {
            yours: None,
            ours: Some(path),
        });
    }
    match in_force(ctx, server) {
        Ok(pin) => settled(served(ctx, server, client.as_ref(), (&path, &pin)).await),
        Err(detail) => settled(State::Failed { detail }),
    }
}

/// The SHA-256 of the certificate kept for `server`, made first where none is; why not
/// where it has no configuration directory lemonfiber writes, or none could be kept.
fn in_force(ctx: &Ctx, server: &MediaServer) -> Result<String, String> {
    let config = server.served.confined_to.as_deref().ok_or_else(|| {
        format!(
            "{} has no configuration directory lemonfiber writes its certificate into",
            server.name()
        )
    })?;
    internal::kept_or_made(ctx.seams.confined.as_ref(), config, &server.served.id)
        .map(|kept| kept.fingerprint)
        .map_err(|unkept| unkept.to_string())
}

/// What telling `client` to serve the certificate at `path` with the SHA-256 `pin` comes to,
/// asking the media server nothing else until it says it is ready again where it had to
/// start again.
pub(super) async fn served(
    ctx: &Ctx,
    server: &MediaServer,
    client: &dyn serve::Fills,
    (path, pin): (&str, &str),
) -> State {
    match client.serve_tls(path, pin).await {
        Ok(Applied::Now) => State::AlreadyWired,
        Ok(Applied::OnRestart) => match restarted(ctx, server).await {
            Ok(()) => {
                let _ = crate::patience::RESTART
                    .until(|| server.ready(ctx), |ready| *ready)
                    .await;
                State::Wired
            }
            Err(why) => State::Failed {
                detail: format!(
                    "{} serves its internal encrypted port only once it starts again, and \
                     starting it again failed: {why}",
                    server.name()
                ),
            },
        },
        Err(failure) => super::super::report::unreached(&failure),
    }
}

/// Start the media server's own service again, so it serves what it was told to.
async fn restarted(ctx: &Ctx, server: &MediaServer) -> Result<(), String> {
    let id = server.id();
    let (profile, stack) = ctx
        .stack
        .checked_manifest(ctx.today())
        .ok()
        .zip(crate::app::plugins::read(ctx).ok())
        .and_then(|(manifest, register)| {
            crate::project::profile_of(&manifest, register.installed(), id)
        })
        .zip(crate::app::targets::project_directory(
            &ctx.stack,
            ctx.settings.stack_dir.as_deref(),
        ))
        .ok_or_else(|| format!("{id} is not a service of the stack written to disk"))?;
    crate::app::project::unlinked(ctx).map_err(|refused| refused.summary)?;
    let command = build(
        &Plan::one(id, &profile),
        &ctx.settings,
        &stack,
        &Action::Restart(vec![id.to_owned()]),
        ctx.environment,
    );
    let ran = ctx
        .seams
        .runner
        .run(&command)
        .await
        .map_err(|failure| failure.to_string())?;
    if ran.succeeded() {
        Ok(())
    } else {
        Err(crate::text::plain(ran.stderr.trim()))
    }
}

#[cfg(test)]
mod tests;
