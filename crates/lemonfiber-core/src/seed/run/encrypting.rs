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
fn connection(server: &MediaServer) -> String {
    format!(
        "{} serving its internal encrypted port with the certificate lemonfiber made for it",
        server.name()
    )
}

/// Make the media server's internal certificate before the stack starts, where the stack
/// has a media server and has been written to disk.
pub(crate) fn made_before_start(ctx: &Ctx, manifest: &lemonfiber_manifest::Manifest) {
    let Some(server) = crate::app::targets::hosted(ctx, manifest) else {
        return;
    };
    if let Some(config) = server.served.confined_to.as_deref() {
        let _ = internal::kept_or_made(config, &server.served.id);
    }
}

/// The pins every holder of a credential of `server` is handed: the certificate kept for
/// its internal TLS port. None where none is kept, which reaches nothing.
pub(crate) fn pins(server: &MediaServer) -> Vec<String> {
    server
        .served
        .confined_to
        .as_deref()
        .and_then(|config| internal::kept(config).ok().flatten())
        .map(|kept| vec![kept.fingerprint])
        .unwrap_or_default()
}

/// Where the media server's container holds its internal certificate, where the server
/// is a plugin's service whose manifest says where its configuration is mounted.
fn inside(server: &MediaServer) -> Option<String> {
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
    settled(served(ctx, server, client.as_ref(), &path).await)
}

/// What telling `client` to serve the certificate at `path` comes to, waiting until it
/// answers again where it had to start again.
async fn served(ctx: &Ctx, server: &MediaServer, client: &dyn serve::Fills, path: &str) -> State {
    match client.serve_tls(path).await {
        Ok(Applied::Now) => State::AlreadyWired,
        Ok(Applied::OnRestart) => match restarted(ctx, server).await {
            Ok(()) => {
                let _ = crate::patience::RESTART
                    .until(|| client.declared(), Result::is_ok)
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
    let manifest = ctx
        .stack
        .checked_manifest(ctx.today())
        .map_err(|why| why.to_string())?;
    let register = crate::app::plugins::read(ctx).map_err(|why| why.summary)?;
    let profile = crate::project::profile_of(&manifest, register.installed(), id)
        .ok_or_else(|| format!("{id} sits in no profile of the stack"))?;
    let stack =
        crate::app::targets::project_directory(&ctx.stack, ctx.settings.stack_dir.as_deref())
            .ok_or_else(|| "the stack has not been written to disk".to_owned())?;
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
