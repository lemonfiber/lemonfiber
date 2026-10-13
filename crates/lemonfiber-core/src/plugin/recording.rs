//! Recording each proof that runs the image's own program, by running it.
//!
//! The same bounded run every proof gets, its result written beside the manifest with
//! the note the author wrote, and a recording the run no longer reproduces refused.

use std::path::Path;

use lemonfiber_plugin::{Manifest, Proof, Run};
use serde::Serialize;

use super::claimed::{sourced, Unreadable};
use super::installed::Installed;
use super::program::{performed, recorded, Came, Operator, Starting};
use super::recorded::{self, Of, Recording, RunAsked, RunResult};
use crate::doctor::BUNDLED_CHECKS;
use crate::ports::program::Program;

/// What recording one proof came to.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "outcome", rename_all = "kebab-case")]
pub enum Took {
    /// The recording was written.
    Written,
    /// The recording already held what the run wrote, byte for byte.
    Reproduced,
    /// The recording holds another result for the same image and arguments, so it is
    /// refused and left as it was.
    Unreproduced {
        /// What differs.
        why: String,
    },
    /// The run was still going at its limit, so there is nothing to record.
    Overran {
        /// The limit, in seconds.
        seconds: u32,
    },
    /// The run could not be made, or there is nowhere to write what it wrote.
    Unrecorded {
        /// Why.
        why: String,
    },
}

/// One run proof, and what recording it came to.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Taken {
    /// The proof.
    pub proof: String,
    /// The recording it names.
    pub fixture: Option<String>,
    /// What recording it came to.
    pub took: Took,
}

/// Every run proof of a plugin, recorded.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Recorded {
    /// The plugin's id.
    pub id: String,
    /// Everything this build refuses about the manifest; nothing is run where any is.
    pub refusals: Vec<lemonfiber_plugin::Violation>,
    /// Each run proof, in the order the manifest declares them.
    pub runs: Vec<Taken>,
    /// Whether every run proof was written or reproduced.
    pub holds: bool,
}

/// Run and record every proof of the plugin whose source is at `path` that runs the
/// image's own program.
///
/// # Errors
///
/// [`Unreadable`] where there is no manifest at the path, or this build cannot read it.
pub async fn record(path: &Path, program: &dyn Program) -> Result<Recorded, Unreadable> {
    let (root, manifest) = sourced(path)?;
    let refusals = lemonfiber_plugin::refusals(&manifest, BUNDLED_CHECKS);
    let mut runs = Vec::new();
    if refusals.is_empty() {
        for proof in &manifest.proofs {
            if let Some(run) = &proof.run {
                runs.push(taken(&manifest, proof, run, &root, program).await);
            }
        }
    }
    let holds = refusals.is_empty()
        && runs
            .iter()
            .all(|one| matches!(one.took, Took::Written | Took::Reproduced));
    Ok(Recorded {
        id: manifest.plugin.id.clone(),
        refusals,
        runs,
        holds,
    })
}

/// One run proof, run and recorded.
async fn taken(
    manifest: &Manifest,
    proof: &Proof,
    run: &Run,
    root: &Path,
    program: &dyn Program,
) -> Taken {
    let took = match (
        proof.fixture.as_deref(),
        placed(manifest, proof.service.as_deref()),
    ) {
        (Some(fixture), Some(placed)) => {
            let starting = Starting {
                root,
                service: &placed.service,
            };
            let came = performed(
                program,
                &placed,
                run,
                &Operator::recording(),
                Some(starting),
            )
            .await;
            let pinned = format!("{}@{}", placed.image, placed.digest);
            kept(root, fixture, &pinned, run, &came)
        }
        _ => Took::Unrecorded {
            why: "names no fixture, or no one service of this plugin, so there is nowhere to \
                  write what it ran"
                .to_owned(),
        },
    };
    Taken {
        proof: proof.id.clone(),
        fixture: proof.fixture.clone(),
        took,
    }
}

/// The service a proof runs, as installing the manifest would place it.
fn placed(manifest: &Manifest, named: Option<&str>) -> Option<super::Placed> {
    manifest.asks(named).and_then(|service| {
        Installed::of(manifest)
            .services
            .into_iter()
            .find(|one| one.service == service.id)
    })
}

/// What a run came to, written to `fixture` beneath `root` where it ended.
fn kept(root: &Path, fixture: &str, pinned: &str, run: &Run, came: &Came) -> Took {
    let result = match came {
        Came::Ended(result) => recorded(result),
        Came::Overran(seconds) => return Took::Overran { seconds: *seconds },
        Came::Unproven(why) => return Took::Unrecorded { why: why.clone() },
    };
    let Some(inside) = crate::within::beneath(fixture) else {
        return Took::Unrecorded {
            why: format!("{fixture} leads outside the plugin's own source"),
        };
    };
    let before = recorded::read(root, fixture).ok();
    if let Some(differs) = before
        .as_ref()
        .and_then(|before| unreproduced(before, pinned, run, &result))
    {
        return Took::Unreproduced { why: differs };
    }
    let reproduced = before.as_ref().is_some_and(|before| {
        before.recorded_from == pinned
            && matches!(&before.of, Of::Run { run: was, result: had } if was.args == run.args && *had == result)
    });
    if reproduced {
        return Took::Reproduced;
    }
    let recording = Recording {
        recorded_from: pinned.to_owned(),
        note: before.map(|before| before.note).unwrap_or_default(),
        of: Of::Run {
            run: RunAsked {
                args: run.args.clone(),
            },
            result,
        },
    };
    written(root, &root.join(inside), &recording)
}

/// How a recording of this same run of this same image differs from what the run wrote,
/// where it does.
fn unreproduced(before: &Recording, pinned: &str, run: &Run, result: &RunResult) -> Option<String> {
    let Of::Run {
        run: was,
        result: had,
    } = &before.of
    else {
        return None;
    };
    if before.recorded_from != pinned || was.args != run.args || had == result {
        return None;
    }
    Some(format!(
        "it recorded exit {} and output {:?}, and running it again came to exit {} and output \
         {:?}; a run that differs from one run to the next is evidence of nothing",
        had.exit,
        crate::text::plain(&had.stdout_starts_with),
        result.exit,
        crate::text::plain(&result.stdout_starts_with)
    ))
}

/// Write `recording` at `at` beneath `root`, as the committed artefacts are written,
/// through no link on the way and over none at its name.
fn written(root: &Path, at: &Path, recording: &Recording) -> Took {
    let mut here = root.to_path_buf();
    for step in at.strip_prefix(root).unwrap_or(at).components() {
        here.push(step);
        if std::fs::symlink_metadata(&here).is_ok_and(|kind| kind.file_type().is_symlink()) {
            return Took::Unrecorded {
                why: format!(
                    "{} is a link, and a recording is written through none",
                    here.display()
                ),
            };
        }
    }
    let made = super::rendered(recording)
        .ok_or(std::io::ErrorKind::InvalidData)
        .map_err(std::io::Error::from)
        .and_then(|text| {
            at.parent()
                .map_or(Ok(()), std::fs::create_dir_all)
                .and_then(|()| std::fs::write(at, text))
        });
    match made {
        Ok(()) => Took::Written,
        Err(why) => Took::Unrecorded {
            why: format!("{} could not be written: {why}", at.display()),
        },
    }
}

#[cfg(test)]
mod tests;
