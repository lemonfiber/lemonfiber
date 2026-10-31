//! The addresses of the engine's host, asked of the engine before a start or an apply
//! writes a project holding a fence: the fence image's `fence addresses`, run once on the
//! host's network with every capability dropped, a read-only root filesystem and no
//! mount, and removed once it ends. Asked again every time, so an address the host took
//! since the last start is the one the next fence refuses.

use std::path::Path;
use std::time::Duration;

use lemonfiber_manifest::Manifest;

use super::super::Ctx;
use crate::config::{store, REACH_REGISTRY_KEY};
use crate::fence::HELD;
use crate::ports::program::{Ended, Joined, Run};
use crate::project::entry::{DROPPED, PROCESSES, SECURITY};

/// The argument the fence image answers its host's addresses under.
pub(crate) const ASKED: &str = "addresses";

/// How long the engine is given to run it.
const WITHIN: Duration = Duration::from_secs(60);

/// The most of what it writes that is read.
const READ_AT_MOST: usize = 256 * 1024;

/// The memory it is held to, in bytes.
const MEMORY: i64 = 64 * 1024 * 1024;

/// Ask the engine for its host's addresses where a service of `manifest`'s or an
/// installed plugin's is fenced and `image` is the fence this build pins, and keep the
/// answer where the project reads it; say why none is kept, where none is. A rehearsal
/// runs and keeps nothing.
pub(crate) async fn readied(ctx: &Ctx, manifest: &Manifest, image: Option<&str>) -> Option<String> {
    if ctx.dry_run || !fenced(ctx, manifest) {
        return None;
    }
    let image = image?;
    let directory = super::directory(ctx)?;
    let answered = asked(ctx, image).await;
    let text = answered
        .as_ref()
        .map(|held| {
            held.iter()
                .flat_map(|address| [address.to_string(), "\n".to_owned()])
                .collect::<String>()
        })
        .unwrap_or_default();
    let why = kept(&directory.join(HELD), &text).err();
    answered.err().or(why)
}

/// Whether a service of `manifest`'s, or of a plugin installed here, declares itself
/// fenced.
fn fenced(ctx: &Ctx, manifest: &Manifest) -> bool {
    manifest
        .services
        .iter()
        .any(lemonfiber_manifest::Service::fenced)
        || super::super::plugins::read(ctx).is_ok_and(|register| {
            register.installed().iter().any(|installed| {
                installed.services.iter().any(|placed| {
                    placed
                        .fields
                        .network
                        .as_ref()
                        .is_some_and(|network| network.fenced)
                })
            })
        })
}

/// What `image` answers, run as `fence addresses` on the host's network.
async fn asked(ctx: &Ctx, image: &str) -> Result<Vec<std::net::IpAddr>, String> {
    let run = Run {
        image: image.to_owned(),
        args: vec![ASKED.to_owned()],
        env: Vec::new(),
        user: None,
        security_opt: SECURITY.map(str::to_owned).to_vec(),
        cap_drop: DROPPED.map(str::to_owned).to_vec(),
        cap_add: Vec::new(),
        pids_limit: i64::from(PROCESSES),
        read_only: true,
        memory: Some(MEMORY),
        configuration: None,
        within: WITHIN,
        read_at_most: READ_AT_MOST,
        network: Joined::Host,
        pull: ctx.settings.reaching.allows(REACH_REGISTRY_KEY),
    };
    let ran = ctx
        .seams
        .program
        .run(&run)
        .await
        .map_err(|unran| format!("the engine's host was not asked its addresses: {unran}"))?;
    match ran.ended {
        Ended::Exited(0) => {}
        Ended::Exited(status) => {
            return Err(format!(
                "asked its addresses, the engine's host exited {status} rather than answering"
            ))
        }
        Ended::Overran => {
            return Err(format!(
                "asked its addresses, the engine's host had not answered within {} seconds",
                WITHIN.as_secs()
            ))
        }
    }
    if ran.stdout.len() >= READ_AT_MOST {
        return Err(format!(
            "the engine's host answered more than the {READ_AT_MOST} bytes read, so the \
             addresses past them are not known"
        ));
    }
    let said = String::from_utf8(ran.stdout)
        .map_err(|_| "the engine's host answered what is not text".to_owned())?;
    let held = crate::fence::kept(&said);
    if held.is_empty() {
        return Err("the engine's host answered no address that can be read".to_owned());
    }
    Ok(held)
}

/// Write `text` to `path` owner-only, or say why it was not written.
fn kept(path: &Path, text: &str) -> Result<(), String> {
    store::write(path, text).map_err(|failure| failure.to_string())
}

#[cfg(test)]
mod tests;
