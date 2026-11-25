//! What the log viewer shows, redacted as a support bundle is and written beside where
//! it was run.

use std::path::PathBuf;

use serde::Serialize;

use crate::bundle::{prose, residual, Marks, Terms};
use crate::error::codes::bundle::{BUNDLE_LEAK, BUNDLE_NO_MARKS, BUNDLE_UNWRITTEN};
use crate::error::{Problem, Remedy, State};

use super::{Ctx, Exporting};

/// What the export is called where a residual is found in it.
const EXPORT: &str = "the export";

/// Where an export went, or would have gone.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, schemars::JsonSchema)]
#[schemars(rename = "LogsExported")]
pub struct Exported {
    /// The file, named relative to the directory the viewer was run in.
    pub path: String,
    /// Whether it only said where it would have written.
    pub rehearsed: bool,
}

pub(super) async fn exported(ctx: &Ctx, asked: Exporting) -> Result<Exported, Box<Problem>> {
    let marks = Marks::new(ctx.seams.random.as_ref()).ok_or_else(|| Box::new(markless()))?;
    let terms = Terms::default();
    let mut said = prose(&asked.text, &marks, &terms);
    said.push('\n');
    let said = cleared(said, &terms)?;
    let path = named(ctx, asked.written);
    let shown = path.display().to_string();
    if ctx.dry_run {
        return Ok(Exported {
            path: shown,
            rehearsed: true,
        });
    }
    ctx.seams.filesystem.write(&path, &said).await;
    if ctx.seams.filesystem.read(&path).await.is_none() {
        return Err(Box::new(unwritten(&shown)));
    }
    Ok(Exported {
        path: shown,
        rehearsed: false,
    })
}

fn cleared(said: String, terms: &Terms) -> Result<String, Box<Problem>> {
    match residual(&[(EXPORT.to_owned(), said.clone())], terms) {
        Some(found) => Err(Box::new(leaking(found.line))),
        None => Ok(said),
    }
}

fn named(ctx: &Ctx, written: usize) -> PathBuf {
    let stamp = ctx
        .seams
        .clock
        .now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|since| since.as_secs())
        .unwrap_or_default();
    PathBuf::from(format!("lemonfiber-logs-{stamp}-{written}.txt"))
}

fn markless() -> Problem {
    Problem::new(
        BUNDLE_NO_MARKS,
        "This machine would not provide the randomness an export needs",
        "Every replaced value carries a stand-in derived with randomness, and a stand-in \
         anyone can reproduce is a way back to the value it stands for. Nothing was written.",
        Remedy::new(
            "Report this: a machine that cannot produce random bytes is a fault in its own right",
        ),
    )
    .in_state(State::Guided)
}

fn leaking(line: usize) -> Problem {
    Problem::new(
        BUNDLE_LEAK,
        format!("Not written: line {line} still reads like a credential after redaction"),
        "An export is redacted as a support bundle is, and anything in one that still reads \
         like a key is treated as one. Nothing was written.",
        Remedy::new(
            "Report which line this names, so what it holds can be added to what is replaced",
        ),
    )
    .in_state(State::Guided)
}

fn unwritten(path: &str) -> Problem {
    Problem::new(
        BUNDLE_UNWRITTEN,
        format!("Could not write {path}"),
        "The export was written and could not be read back, so it is not said to be there.",
        Remedy::new("Run the viewer from a directory you can write to, then export again"),
    )
    .in_state(State::Guided)
}

#[cfg(test)]
mod tests;
