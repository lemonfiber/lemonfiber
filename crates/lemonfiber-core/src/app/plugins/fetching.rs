//! A plugin installed from a git source: the revision resolved to one commit, that
//! commit fetched as data, and the install run over it as over any directory.
//!
//! **One commit, whatever was named.** A branch or a tag is a name its repository can
//! move, so it is resolved before anything is fetched and the commit it resolved to is
//! what is installed and recorded; the next look at that source is a question about
//! whether the name has moved, not an assumption that it has not.
//!
//! **Fetched as data.** One commit, shallow, no tags, no submodules and no hooks, into a
//! directory made for that one install and removed when it is done — a rehearsal
//! included, so asking what an install would do leaves nothing behind, and two installs
//! of one commit at once never share a directory. Nothing from
//! the repository is run: the install reads its manifest and recordings and holds them
//! to everything a directory's are held to.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use futures_util::StreamExt as _;

use crate::app::Ctx;
use crate::config::REACH_PLUGIN_SOURCE_KEY;
use crate::error::codes::plugin::{NO_REVISION, SOURCE_OFF, UNFETCHED};
use crate::error::{Diagnose, Problem, Remedy, State};
use crate::plugin::{Fetchable, Installed, Installs, Register, Source, Sourced};

/// Where an install came from, where that is not the directory it read.
pub(super) struct Fetched<'a> {
    /// The repository, as the operator named it.
    pub(super) url: &'a str,
    /// The one commit the named revision resolved to.
    pub(super) commit: &'a str,
    /// What the catalogue vouched for, where it was resolved through one: the manifest
    /// the install reads is held to it, in the one read the install makes.
    pub(super) vouched: Option<&'a Vouched<'a>>,
}

/// What the catalogue vouched for, which the fetched commit is held to.
pub(super) struct Vouched<'a> {
    /// The entry its verified index holds.
    pub(super) entry: &'a crate::plugin::catalogue::Entry,
    /// What signed that index.
    pub(super) signed: &'a str,
}

/// Where a git source is, and what vouched for it where anything did.
pub(super) struct Fetching<'a> {
    /// The repository, as the operator named it or the catalogue's index named it.
    pub(super) url: &'a str,
    /// The branch, tag or commit named, or nothing for what it serves by default.
    pub(super) revision: Option<&'a str>,
    /// What the catalogue vouched for, which the fetched commit is held to.
    pub(super) vouched: Option<&'a Vouched<'a>>,
}

/// Carry an errand out over the plugin a git source holds at the revision named, or
/// at what it serves by default.
///
/// Where the catalogue vouched for it, the manifest the commit holds has to be the one
/// the catalogue reviewed, which the install holds it to in the one read it makes.
///
/// # Errors
///
/// Where fetching from a git source is switched off, where the source cannot be
/// reached or holds nothing by the revision named, where the commit cannot be fetched,
/// where it holds a manifest other than the one the catalogue reviewed, and every
/// refusal the errand makes over a directory.
pub(super) async fn fetched(
    ctx: &Ctx,
    held: Register,
    source: &Fetching<'_>,
    errand: super::Errand<'_>,
    consent: &super::Consent,
) -> Result<Installs, Box<Problem>> {
    let url = source.url;
    if let Some(scheme) = crate::plugin::unspoken(url) {
        return Err(Box::new(super::reach::scheme_refused(url, scheme)));
    }
    if !ctx.settings.reaching.allows(REACH_PLUGIN_SOURCE_KEY) {
        return Err(Box::new(switched_off(url)));
    }
    let reached = super::reach::reached(ctx, url).await?;
    let commit = resolved(ctx, url, source.revision, &reached).await?;
    let Some(into) = checkout(ctx, &commit) else {
        return Err(Box::new(crate::config::store::Failure::Nowhere.problem()));
    };
    if let Err(why) = made_fresh(&into) {
        return Err(Box::new(unfetched(url, &why.to_string())));
    }
    let result = match fetched_into(ctx, url, &commit, &into, &reached).await {
        Ok(()) => {
            let from = Fetched {
                url,
                commit: &commit,
                vouched: source.vouched,
            };
            super::carried(ctx, held, &into, Some(&from), errand, consent).await
        }
        Err(problem) => Err(problem),
    };
    let _ = tokio::fs::remove_dir_all(&into).await;
    result
}

/// The one commit a revision names on a source, or the one it serves by default.
async fn resolved(
    ctx: &Ctx,
    url: &str,
    revision: Option<&str>,
    reached: &super::reach::Reached,
) -> Result<String, Box<Problem>> {
    if let Some(commit) = revision.filter(|named| is_commit(named)) {
        return Ok(commit.to_ascii_lowercase());
    }
    let asked = revision.unwrap_or("HEAD");
    let listing = ["ls-remote", "--", url, asked];
    let listed = super::git::run_pinned(ctx, reached.pin(), &listing, super::git::ASKING)
        .await
        .map_err(|why| Box::new(unfetched(url, &why)))?;
    listed_commit(&listed, asked).ok_or_else(|| Box::new(no_revision(url, asked)))
}

/// The commit `git ls-remote` gave for a name: the one a tag points at where it
/// answered with that too, and the first it gave otherwise.
pub(super) fn listed_commit(listed: &str, asked: &str) -> Option<String> {
    let rows: Vec<(&str, &str)> = listed
        .lines()
        .filter_map(|line| line.split_once('\t'))
        .filter(|(commit, _)| is_commit(commit))
        .collect();
    let peeled = rows
        .iter()
        .find(|(_, name)| name.ends_with("^{}") && name.contains(asked));
    peeled
        .or_else(|| rows.first())
        .map(|(commit, _)| (*commit).to_owned())
}

/// Whether a name is a whole commit, which needs no asking.
fn is_commit(named: &str) -> bool {
    named.len() == 40 && named.bytes().all(|byte| byte.is_ascii_hexdigit())
}

/// Fetch one commit into `into`, as data.
async fn fetched_into(
    ctx: &Ctx,
    url: &str,
    commit: &str,
    into: &Path,
    reached: &super::reach::Reached,
) -> Result<(), Box<Problem>> {
    let at = into.display().to_string();
    let steps: [&[&str]; 4] = [
        &["init", "--quiet", &at],
        &["-C", &at, "remote", "add", "origin", url],
        &[
            "-C",
            &at,
            "fetch",
            "--quiet",
            "--depth",
            "1",
            "--no-tags",
            "--no-recurse-submodules",
            "origin",
            commit,
        ],
        &["-C", &at, "checkout", "--quiet", "FETCH_HEAD"],
    ];
    for step in steps {
        super::git::run_pinned(ctx, reached.pin(), step, super::git::FETCHING)
            .await
            .map_err(|why| Box::new(unfetched(url, &why)))?;
    }
    Ok(())
}

/// Said where fetching from a git source is switched off.
fn switched_off(url: &str) -> Problem {
    Problem::new(
        SOURCE_OFF,
        format!("fetching a plugin from a git source is switched off, so {url} was not asked"),
        "Nothing was fetched and nothing was installed. This machine's settings keep lemonfiber \
         from fetching from a git source you name.",
        Remedy::new(format!(
            "Install it from a directory instead, or allow it with `lemonfiber config set \
             {REACH_PLUGIN_SOURCE_KEY} on`"
        )),
    )
    .in_state(State::Guided)
}

/// Said where a git source could not be reached or would not hand a revision over.
pub(super) fn unfetched(url: &str, why: &str) -> Problem {
    Problem::new(
        UNFETCHED,
        format!("{url} could not be fetched"),
        "Nothing was installed. The source did not answer, or would not hand over the \
         revision asked for.",
        Remedy::new("Check the address, and that this machine can reach it, then install it again"),
    )
    .in_state(State::Guided)
    .with_detail(why.to_owned())
}

/// Said where a git source holds nothing by the name given.
fn no_revision(url: &str, asked: &str) -> Problem {
    Problem::new(
        NO_REVISION,
        format!("{url} holds no branch, tag or commit called {asked}"),
        "Nothing was fetched and nothing was installed.",
        Remedy::new(
            "Name a branch, a tag or a whole commit after the last `@`, or leave it off for what \
             the source serves by default",
        ),
    )
    .in_state(State::Guided)
}

/// Whether each installed plugin's source can still be fetched, asked now.
///
/// Asked only when somebody lists what is installed, never in the background: a git
/// source is asked for the commit it serves by default, which is the least a
/// repository answers, and a directory is looked for. A directory that is no longer
/// there cannot be fetched from, which is the same answer a repository that stopped
/// answering gets.
pub(super) async fn standings(ctx: &Ctx, installed: &[Installed]) -> Vec<Sourced> {
    // Asked a few at a time and answered in the record's order, so one host that is
    // slow to answer costs the listing its own deadline rather than everyone's in turn.
    let asking: Vec<_> = installed.iter().map(|one| sourced(ctx, one)).collect();
    futures_util::stream::iter(asking)
        .buffered(super::git::AT_ONCE)
        .collect()
        .await
}

/// One installed plugin's source, and what asking it came to.
async fn sourced(ctx: &Ctx, one: &Installed) -> Sourced {
    Sourced {
        plugin: one.plugin.clone(),
        from: one.from.clone(),
        standing: standing(ctx, &one.from).await,
    }
}

/// What asking one source comes to.
async fn standing(ctx: &Ctx, from: &str) -> Fetchable {
    if from.is_empty() {
        return Fetchable::Unasked {
            why: "the record names no source, so there is nowhere to ask".to_owned(),
        };
    }
    match Source::named(from) {
        // A record keeps where an install read its manifest, which is never a name, so a
        // `from` shaped as one is a directory written without a `./` in front of it.
        Source::Name(_) | Source::Path(_) => {
            let path = Path::new(from);
            if tokio::fs::try_exists(path).await.unwrap_or(false) {
                Fetchable::Reachable
            } else {
                Fetchable::Unreachable {
                    why: format!("{from} is no longer there"),
                }
            }
        }
        Source::Git { .. } if !ctx.settings.reaching.allows(REACH_PLUGIN_SOURCE_KEY) => {
            Fetchable::Unasked {
                why: format!(
                    "fetching from a git source is switched off by {REACH_PLUGIN_SOURCE_KEY}"
                ),
            }
        }
        Source::Git { url, .. } => {
            match super::git::run(ctx, &["ls-remote", "--", &url, "HEAD"], super::git::ASKING).await
            {
                Ok(_) => Fetchable::Reachable,
                Err(why) => Fetchable::Unreachable { why },
            }
        }
    }
}

/// The directory under lemonfiber's own data directory checkouts are made in.
const CHECKOUTS: &str = "checkouts";

/// How many checkouts this process has named, so each is named apart from every other.
static NAMED: AtomicU64 = AtomicU64::new(0);

/// Where one install checks a commit out, and removes it from after: nothing where this
/// machine has not been set up and so has no data directory.
///
/// Under lemonfiber's own data directory rather than the shared temporary one, where any
/// other user of the machine could make the directory first and hold what is fetched
/// into it. Named for the commit, this process and the checkouts it has named before,
/// so no two installs share one: not two runs, and not two installs of one commit that
/// one run is serving at once, where the first to finish would remove what the other is
/// still reading.
pub(super) fn checkout(ctx: &Ctx, commit: &str) -> Option<PathBuf> {
    let paths = crate::app::targets::layout(ctx)?;
    let named = NAMED.fetch_add(1, Ordering::Relaxed);
    Some(
        paths
            .data_dir()
            .join(CHECKOUTS)
            .join(format!("{commit}-{}-{named}", std::process::id())),
    )
}

/// Make `into` new and empty, readable by its owner alone where the platform tracks a
/// mode, or say why not.
///
/// One this run finds already there is removed first, and the directory is then created
/// rather than taken over: a directory somebody else made at the name between the two
/// is refused rather than fetched into.
fn made_fresh(into: &Path) -> std::io::Result<()> {
    let _ = std::fs::remove_dir_all(into);
    private_dir(into.parent().unwrap_or(into), true)?;
    private_dir(into, false)
}

/// Create the directory `at` owner-only, with its parents where `parents`; failing where
/// `at` is already there and `parents` is not asked for.
#[cfg(unix)]
fn private_dir(at: &Path, parents: bool) -> std::io::Result<()> {
    use std::os::unix::fs::DirBuilderExt as _;
    std::fs::DirBuilder::new()
        .recursive(parents)
        .mode(0o700)
        .create(at)
}

/// Where the platform tracks no mode, an ordinary create.
#[cfg(not(unix))]
fn private_dir(at: &Path, parents: bool) -> std::io::Result<()> {
    std::fs::DirBuilder::new().recursive(parents).create(at)
}
