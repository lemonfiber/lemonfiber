//! A plugin installed from a git source: the revision resolved to one commit, that
//! commit fetched as data, and the install run over it as over any directory.
//!
//! **One commit, whatever was named.** A branch or a tag is a name its repository can
//! move, so it is resolved before anything is fetched and the commit it resolved to is
//! what is installed and recorded; the next look at that source is a question about
//! whether the name has moved, not an assumption that it has not.
//!
//! **Fetched as data.** One commit, shallow, no tags, no submodules and no hooks, into a
//! directory of its own that is removed when the install is done — a rehearsal
//! included, so asking what an install would do leaves nothing behind. Nothing from
//! the repository is run: the install reads its manifest and recordings and holds them
//! to everything a directory's are held to.

use std::path::{Path, PathBuf};

use crate::app::Ctx;
use crate::config::REACH_PLUGIN_SOURCE_KEY;
use crate::error::codes::plugin::{NO_REVISION, SOURCE_OFF, UNFETCHED};
use crate::error::{Diagnose, Problem, Remedy, Severity, State};
use crate::plugin::{Fetchable, Installed, Installs, Register, Source, Sourced};

/// Where an install came from, where that is not the directory it read.
pub(super) struct Fetched<'a> {
    /// The repository, as the operator named it.
    pub(super) url: &'a str,
    /// The one commit the named revision resolved to.
    pub(super) commit: &'a str,
    /// What signed the catalogue index it was resolved through, where it was.
    pub(super) signed: Option<&'a str>,
}

/// What the catalogue vouched for, which the fetched commit is held to.
pub(super) struct Vouched<'a> {
    /// The entry its verified index holds.
    pub(super) entry: &'a crate::plugin::catalogue::Entry,
    /// What signed that index.
    pub(super) signed: &'a str,
}

/// Install the plugin a git source holds at the revision named, or at what it serves
/// by default.
///
/// Where the catalogue vouched for it, the manifest the commit holds has to be the one
/// the catalogue reviewed before anything is installed.
///
/// # Errors
///
/// Where fetching from a git source is switched off, where the source cannot be
/// reached or holds nothing by the revision named, where the commit cannot be fetched,
/// where it holds a manifest other than the one the catalogue reviewed, and every
/// refusal an install from a directory makes.
pub(super) async fn installed(
    ctx: &Ctx,
    held: Register,
    url: &str,
    revision: Option<&str>,
    vouched: Option<&Vouched<'_>>,
) -> Result<Installs, Box<Problem>> {
    if !ctx.settings.reaching.allows(REACH_PLUGIN_SOURCE_KEY) {
        return Err(Box::new(switched_off(url)));
    }
    let commit = resolved(ctx, url, revision).await?;
    let Some(into) = checkout(ctx, &commit) else {
        return Err(Box::new(crate::config::store::Failure::Nowhere.problem()));
    };
    if let Err(why) = made_fresh(&into) {
        return Err(Box::new(unfetched(url, &why.to_string())));
    }
    let result = match fetched(ctx, url, &commit, &into).await {
        Ok(()) => match as_reviewed(&into, vouched).await {
            Ok(()) => {
                let from = Fetched {
                    url,
                    commit: &commit,
                    signed: vouched.map(|vouched| vouched.signed),
                };
                super::install(ctx, held, &into, Some(&from)).await
            }
            Err(problem) => Err(problem),
        },
        Err(problem) => Err(problem),
    };
    let _ = tokio::fs::remove_dir_all(&into).await;
    result
}

/// Whether the manifest a checkout holds is the one the catalogue reviewed, where the
/// catalogue vouched for it at all.
async fn as_reviewed(into: &Path, vouched: Option<&Vouched<'_>>) -> Result<(), Box<Problem>> {
    let Some(vouched) = vouched else {
        return Ok(());
    };
    let manifest = tokio::fs::read(into.join("plugin.toml"))
        .await
        .unwrap_or_default();
    if vouched.entry.holds(&manifest) {
        return Ok(());
    }
    Err(Box::new(super::cataloguing::not_as_reviewed(vouched.entry)))
}

/// The one commit a revision names on a source, or the one it serves by default.
async fn resolved(ctx: &Ctx, url: &str, revision: Option<&str>) -> Result<String, Box<Problem>> {
    if let Some(commit) = revision.filter(|named| is_commit(named)) {
        return Ok(commit.to_ascii_lowercase());
    }
    let asked = revision.unwrap_or("HEAD");
    let listed = git(ctx, &["ls-remote", "--", url, asked])
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
async fn fetched(ctx: &Ctx, url: &str, commit: &str, into: &Path) -> Result<(), Box<Problem>> {
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
        git(ctx, step)
            .await
            .map_err(|why| Box::new(unfetched(url, &why)))?;
    }
    Ok(())
}

/// Run git with the hooks of whatever it is working on switched off, and hand back
/// what it wrote, or why it did not finish.
async fn git(ctx: &Ctx, args: &[&str]) -> Result<String, String> {
    let command: Vec<String> = ["git", "-c", "core.hooksPath=/dev/null"]
        .iter()
        .chain(args)
        .map(|arg| (*arg).to_owned())
        .collect();
    let output = ctx
        .seams
        .runner
        .run(&command)
        .await
        .map_err(|failure| failure.to_string())?;
    if output.succeeded() {
        return Ok(output.stdout);
    }
    let said = output.stderr.trim();
    Err(said
        .lines()
        .last()
        .unwrap_or("git gave no reason")
        .to_owned())
}

/// Said where fetching from a git source is switched off.
fn switched_off(url: &str) -> Problem {
    Problem::new(
        SOURCE_OFF,
        Severity::Error,
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
fn unfetched(url: &str, why: &str) -> Problem {
    Problem::new(
        UNFETCHED,
        Severity::Error,
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
        Severity::Error,
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
    let mut standings = Vec::new();
    for one in installed {
        standings.push(Sourced {
            plugin: one.plugin.clone(),
            from: one.from.clone(),
            standing: standing(ctx, &one.from).await,
        });
    }
    standings
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
        Source::Git { url, .. } => match git(ctx, &["ls-remote", "--", &url, "HEAD"]).await {
            Ok(_) => Fetchable::Reachable,
            Err(why) => Fetchable::Unreachable { why },
        },
    }
}

/// The directory under lemonfiber's own data directory checkouts are made in.
const CHECKOUTS: &str = "checkouts";

/// Where one commit is checked out while it is installed, and removed from after:
/// nothing where this machine has not been set up and so has no data directory.
///
/// Under lemonfiber's own data directory rather than the shared temporary one, where any
/// other user of the machine could make the directory first and hold what is fetched
/// into it. Named for the commit and this process, so two runs never share one.
pub(super) fn checkout(ctx: &Ctx, commit: &str) -> Option<PathBuf> {
    let paths = crate::app::targets::layout(ctx)?;
    Some(
        paths
            .data_dir()
            .join(CHECKOUTS)
            .join(format!("{commit}-{}", std::process::id())),
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
