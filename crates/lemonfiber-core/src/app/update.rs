//! Where this copy of lemonfiber stands, and what moving it would come to.
//!
//! Nothing here can fail, and that is the requirement rather than a convenience: an
//! update check that could refuse would be one every other command had to be ready
//! for. A machine that will not say where its own binary is, a release list that
//! answers nothing, and an operator who switched the check off all arrive at the same
//! place — a report saying what could not be told and why — so nothing waits on this
//! and nothing stops without it.
//!
//! **Nothing here replaces the binary.** What an operator is handed is the exact
//! command for whichever tool owns the copy they are running, which is the difference
//! between updating and fighting the thing that installed it. The copy already in
//! memory is untouched by whatever they then run, and the stack goes on running
//! throughout, because containers do not run inside this program.

mod checking;
mod reading;

use crate::model::UpdateReport;
use crate::update::{
    availability, carries, command, configuration, stands, why_not, Installed, AFTERWARDS,
};

use super::Ctx;

/// Where this copy stands, and what moving it to `named` — or to whatever is newest,
/// where nothing was named — would come to.
pub(super) async fn standing(ctx: &Ctx, named: Option<&str>) -> UpdateReport {
    let running = env!("CARGO_PKG_VERSION");
    let files = ctx.filesystem.as_ref();
    let at = reading::at(files, ctx.settings.program.as_ref()).await;
    let installed = reading::installed(files, at.as_deref(), ctx.settings.home.as_ref()).await;
    let read = checking::read(ctx).await;
    let toward = toward(installed, named, read.offered.as_deref());
    UpdateReport {
        standing: stands(
            read.offered
                .as_deref()
                .map(|offered| availability(running, offered))
                .as_ref(),
            installed,
        ),
        running: running.to_owned(),
        at: at.as_ref().map(|path| path.display().to_string()),
        installed,
        owner: installed.owner().map(str::to_owned),
        offered: read.offered.clone(),
        asked: named.map(str::to_owned),
        command: command(installed, toward),
        instead: why_not(installed, toward).map(str::to_owned),
        replaceable: replaceable(ctx, installed, at.as_deref()).await,
        configuration: named.map(|named| configuration(named, running)),
        afterwards: AFTERWARDS.to_owned(),
        carries: carries(lemonfiber_manifest::SUPPORTED_SCHEMA_VERSIONS),
        untold: read.untold.map(|quiet| quiet.why().to_owned()),
    }
}

/// The version to name to whatever would carry out the move.
///
/// What was asked for where the operator asked for one, and the newest that was read
/// where they did not — except for a tool that finds the newest for itself, which is
/// asked to upgrade rather than told a version its index may not carry. A tool asked
/// for one particular version is told it either way, because a request for a version
/// it cannot name has to be refused rather than quietly answered with a different one.
fn toward<'a>(
    installed: Installed,
    named: Option<&'a str>,
    offered: Option<&'a str>,
) -> Option<&'a str> {
    if named.is_none() && installed.resolves_newest() {
        return None;
    }
    named.or(offered)
}

/// Whether this copy could be replaced, where that is lemonfiber's question at all.
///
/// Not asked of a copy a package manager owns. Whether that file is writable is
/// beside the point — it is that tool's to replace, and probing beside it would be
/// asking a question whose answer must not be acted on.
async fn replaceable(
    ctx: &Ctx,
    installed: Installed,
    at: Option<&std::path::Path>,
) -> Option<bool> {
    if installed.defers() {
        return None;
    }
    reading::replaceable(ctx.filesystem.as_ref(), at).await
}
