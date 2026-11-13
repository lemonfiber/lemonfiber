//! What the engine is asked before a start or an apply writes a project holding a fence.
//!
//! First whether its containers can hold the rules a fence installs: the fence image's
//! `fence nftables`, run once in a container of its own on no network, holding no
//! capability but `NET_ADMIN`, with a read-only root filesystem and no mount, and removed
//! once it ends. Then, where they can, the addresses of the engine's host: `fence
//! addresses`, run once on the host's network with every capability dropped, a read-only
//! root filesystem and no mount, and removed once it ends. Both are asked again every
//! time, so an engine that changed since the last start, or an address the host took
//! since, is what the next fence is written for.

use std::path::Path;
use std::time::Duration;

use lemonfiber_manifest::Service;

use super::super::Ctx;
use crate::config::{store, REACH_REGISTRY_KEY};
use crate::fence::{GRANTED, HELD, HOLDS, NFTABLES_HELD};
use crate::plugin::Installed;
use crate::ports::program::{Bound, Ended, Joined, Ran, Run};
use crate::project::entry::{DROPPED, PROCESSES, SECURITY};

/// The argument the fence image answers its host's addresses under.
pub(crate) const ASKED: &str = "addresses";

/// The argument the fence image answers under whether the container it runs in can hold
/// the rules a fence installs.
pub(crate) const PROBED: &str = "nftables";

/// What the engine is asked when it is asked whether its containers hold the rules.
const PROBING: &str = "whether its containers can hold nftables rules";

/// The exit the fence image answers `fence nftables` with where the container it ran in
/// could not hold the rules, having written why.
const UNHELD: i64 = 1;

/// How long the engine is given to run it.
const WITHIN: Duration = Duration::from_secs(60);

/// The most of what it writes that is read.
const READ_AT_MOST: usize = 256 * 1024;

/// The memory it is held to, in bytes.
const MEMORY: i64 = 64 * 1024 * 1024;

/// Ask the engine whether its containers hold the rules a fence installs and, where they
/// do, for its host's addresses, where a service of `stack`'s or of `installed`'s is
/// fenced and `image` is the fence this build pins, and keep both answers where the
/// project reads them; say why either is not kept, where it is not. A rehearsal runs and
/// keeps nothing.
pub(crate) async fn readied(
    ctx: &Ctx,
    stack: &[Service],
    installed: &[Installed],
    image: Option<&str>,
) -> Option<String> {
    if ctx.dry_run || !fenced(stack, installed) {
        return None;
    }
    let image = image?;
    let directory = super::directory(ctx)?;
    let probed = probed(ctx, image).await;
    let held = if probed.is_ok() { HOLDS } else { "" };
    let unkept = kept(&directory.join(NFTABLES_HELD), held).err();
    let answered = match probed {
        Ok(()) => asked(ctx, image).await,
        Err(why) => Err(why),
    };
    let text = answered
        .as_ref()
        .map(|held| {
            held.iter()
                .flat_map(|address| [address.to_string(), "\n".to_owned()])
                .collect::<String>()
        })
        .unwrap_or_default();
    let why = kept(&directory.join(HELD), &text).err();
    answered.err().or(unkept).or(why)
}

/// Whether a service of `stack`'s or of `installed`'s declares itself fenced.
fn fenced(stack: &[Service], installed: &[Installed]) -> bool {
    stack.iter().any(Service::fenced)
        || installed.iter().any(|one| {
            one.services.iter().any(|placed| {
                placed
                    .fields
                    .network
                    .as_ref()
                    .is_some_and(|network| network.fenced)
            })
        })
}

/// What `image` answers, run as `fence addresses` on the host's network.
async fn asked(ctx: &Ctx, image: &str) -> Result<Vec<std::net::IpAddr>, String> {
    let said = answered(ctx, image, ASKED, (Joined::Host, None), "its addresses").await?;
    let held = crate::fence::kept(&said);
    if held.is_empty() {
        return Err("the engine's host answered no address that can be read".to_owned());
    }
    Ok(held)
}

/// Whether the engine's containers hold the rules a fence installs, as `image` answers,
/// run as `fence nftables` on no network holding no capability but `NET_ADMIN`.
async fn probed(ctx: &Ctx, image: &str) -> Result<(), String> {
    let ran = ran(
        ctx,
        image,
        PROBED,
        (Joined::Nothing, None),
        &GRANTED,
        PROBING,
    )
    .await?;
    match ran.ended {
        Ended::Exited(0) => {
            let said = text(ran.stdout)?;
            if crate::fence::holds(&said) {
                return Ok(());
            }
            Err(format!(
                "asked {PROBING}, the engine answered {:?} rather than {HOLDS:?}",
                said.trim()
            ))
        }
        Ended::Exited(UNHELD) => Err(format!(
            "the engine's containers cannot hold the nftables rules a fence installs: {}",
            text(ran.stdout)?.trim()
        )),
        ended => Err(unanswered(ended, PROBING)),
    }
}

/// What the fence `image` writes, run once as `fence <command>` on `network` with `bound`
/// mounted where it is given, every capability dropped and a read-only root filesystem,
/// and removed once it ends; or why the engine's host, asked for `what`, answered nothing
/// that can be read.
pub(super) async fn answered(
    ctx: &Ctx,
    image: &str,
    command: &str,
    (network, bound): (Joined, Option<Bound>),
    what: &str,
) -> Result<String, String> {
    let ran = ran(ctx, image, command, (network, bound), &[], what).await?;
    match ran.ended {
        Ended::Exited(0) => text(ran.stdout),
        ended => Err(unanswered(ended, what)),
    }
}

/// How the fence `image` ended and what it wrote, run once as `fence <command>` on
/// `network` with `bound` mounted where it is given, every capability dropped but
/// `granted` and a read-only root filesystem, and removed once it ends; or why the engine,
/// asked for `what`, was not asked or answered more than is read.
async fn ran(
    ctx: &Ctx,
    image: &str,
    command: &str,
    (network, bound): (Joined, Option<Bound>),
    granted: &[&str],
    what: &str,
) -> Result<Ran, String> {
    let run = Run {
        image: image.to_owned(),
        args: vec![command.to_owned()],
        env: Vec::new(),
        user: None,
        security_opt: SECURITY.map(str::to_owned).to_vec(),
        cap_drop: DROPPED.map(str::to_owned).to_vec(),
        cap_add: granted.iter().map(|one| (*one).to_owned()).collect(),
        pids_limit: i64::from(PROCESSES),
        read_only: true,
        memory: Some(MEMORY),
        bound,
        within: WITHIN,
        read_at_most: READ_AT_MOST,
        network,
        pull: ctx.settings.reaching.allows(REACH_REGISTRY_KEY),
    };
    let ran = ctx
        .seams
        .program
        .run(&run)
        .await
        .map_err(|unran| format!("the engine's host was not asked {what}: {unran}"))?;
    if ran.stdout.len() >= READ_AT_MOST {
        return Err(format!(
            "the engine's host answered more than the {READ_AT_MOST} bytes read, so what \
             follows them is not known"
        ));
    }
    Ok(ran)
}

/// Why the engine, asked for `what`, answered nothing, as it `ended`.
fn unanswered(ended: Ended, what: &str) -> String {
    match ended {
        Ended::Exited(status) => {
            format!("asked {what}, the engine's host exited {status} rather than answering")
        }
        Ended::Overran => format!(
            "asked {what}, the engine's host had not answered within {} seconds",
            WITHIN.as_secs()
        ),
    }
}

/// `stdout` as text, or why it is not.
fn text(stdout: Vec<u8>) -> Result<String, String> {
    String::from_utf8(stdout).map_err(|_| "the engine's host answered what is not text".to_owned())
}

/// Write `text` to `path` owner-only, or say why it was not written.
pub(super) fn kept(path: &Path, text: &str) -> Result<(), String> {
    store::write(path, text).map_err(|failure| failure.to_string())
}

#[cfg(test)]
mod tests;
