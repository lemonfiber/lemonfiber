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
use crate::error::{Problem, Remedy, Severity, State};
use crate::plugin::{Fetchable, Installed, Installs, Register, Source, Sourced};

/// Where an install came from, where that is not the directory it read.
pub(super) struct Fetched<'a> {
    /// The repository, as the operator named it.
    pub(super) url: &'a str,
    /// The one commit the named revision resolved to.
    pub(super) commit: &'a str,
}

/// Install the plugin a git source holds at the revision named, or at what it serves
/// by default.
///
/// # Errors
///
/// Where fetching from a git source is switched off, where the source cannot be
/// reached or holds nothing by the revision named, where the commit cannot be fetched,
/// and every refusal an install from a directory makes.
pub(super) async fn installed(
    ctx: &Ctx,
    held: Register,
    url: &str,
    revision: Option<&str>,
) -> Result<Installs, Box<Problem>> {
    if !ctx.settings.reaching.allows(REACH_PLUGIN_SOURCE_KEY) {
        return Err(Box::new(switched_off(url)));
    }
    let commit = resolved(ctx, url, revision).await?;
    let into = checkout(&commit);
    let _ = tokio::fs::remove_dir_all(&into).await;
    let result = match fetched(ctx, url, &commit, &into).await {
        Ok(()) => {
            let from = Fetched {
                url,
                commit: &commit,
            };
            super::install(ctx, held, &into, Some(&from)).await
        }
        Err(problem) => Err(problem),
    };
    let _ = tokio::fs::remove_dir_all(&into).await;
    result
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
        Source::Path(path) if path.exists() => Fetchable::Reachable,
        Source::Path(_) => Fetchable::Unreachable {
            why: format!("{from} is no longer there"),
        },
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

/// Where one commit is checked out while it is installed, and removed from after.
///
/// Named for the commit and this process, so two runs never share one, and one this run
/// finds already there is removed before anything is fetched into it rather than read.
pub(super) fn checkout(commit: &str) -> PathBuf {
    std::env::temp_dir().join(format!("lemonfiber-plugin-{commit}-{}", std::process::id()))
}
