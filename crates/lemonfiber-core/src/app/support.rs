//! What a support request comes to: a description of a bundle, or a bundle.
//!
//! The gathering and the redaction are [`super::bundle`]'s. What is here is the
//! errand: a bare run collects, redacts, scans and says what a bundle would hold,
//! and writes nothing. Producing one is a second, deliberate run over the same
//! collection and the same scan — a description that checked differently from the
//! write would be a description of something else.
//!
//! The file is written where it is produced and sent nowhere. Where that is is the
//! only question a caller with no filesystem in front of it cannot answer for
//! itself, so [`Destination`] carries the two answers rather than leaving a path to
//! be supplied by whoever is asking.

use std::path::PathBuf;

use serde::Serialize;

use crate::bundle::Contents;
use crate::error::{Code, Problem, Remedy, Severity, State};

use super::bundle::{collect, measure, unconfirmed, without_marks, write, Wanted};
use super::Ctx;

/// Raised when this run has nowhere it knows to keep a bundle.
pub const NOWHERE_TO_KEEP: Code = Code::new("BUNDLE-6");

/// Where a bundle is written.
///
/// Two answers because two surfaces can answer. An operator at a shell names a
/// path, or takes the one beside them; a browser has no filesystem in front of it
/// and no path it could name that would mean anything, so it takes the directory
/// lemonfiber keeps its own files in — which is an answer to *which path*, not a
/// reason a browser cannot ask for a bundle.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Destination {
    /// At the path the operator named.
    At(PathBuf),
    /// Beside the operator, under a name carrying the moment it was taken.
    Beside,
    /// With lemonfiber's own files, under a name carrying the moment it was taken.
    Kept,
}

/// What a support request said: what a bundle holds, and where it is if it exists.
///
/// One record with an absent path rather than two shapes, because the two answers
/// are the same answer at two moments: both list what goes in the file and say how
/// large it is, and only one of them has a file to point at. A caller reads whether
/// there is a path to know which it has.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, schemars::JsonSchema)]
pub struct Bundle {
    /// Everything it holds, gathered, redacted and read back.
    pub contents: Contents,
    /// How large the file is, or would be.
    pub bytes: u64,
    /// Where it was written, or nothing where a run that writes nothing described it.
    pub path: Option<PathBuf>,
}

/// Describe a bundle, or produce one.
///
/// # Errors
///
/// Returns a [`Problem`] where a setting was named to be shown as it is without
/// that being confirmed, where the machine would provide no randomness to derive
/// stand-ins from, where the assembled bundle still holds something reading as a
/// credential, where this run has nowhere it keeps its own files, or where the
/// archive could not be written.
pub async fn run(
    ctx: &Ctx,
    wanted: &Wanted,
    write_it: bool,
    dest: &Destination,
) -> Result<Bundle, Box<Problem>> {
    if !wanted.reveal.is_empty() && !wanted.confirmed {
        return Err(Box::new(unconfirmed(&wanted.reveal)));
    }
    let contents = collect(ctx, env!("CARGO_PKG_VERSION"), wanted)
        .await
        .ok_or_else(|| Box::new(without_marks()))?;

    if !write_it {
        let bytes = measure(&contents)?;
        return Ok(Bundle {
            contents,
            bytes,
            path: None,
        });
    }
    // Asked for once for both halves of writing: the adapter that packs the file,
    // and — where the caller named no path — the directory it goes in. Asked twice
    // it could be answered twice, and a bundle written by one run's adapter into
    // another run's directory is a file in a place nothing looks.
    let archives = ctx
        .archives
        .as_ref()
        .ok_or_else(|| Box::new(nowhere_to_keep()))?;
    let at = match dest {
        Destination::At(path) => path.clone(),
        Destination::Beside => PathBuf::from(named_for_the_moment(&contents)),
        Destination::Kept => archives
            .paths
            .bundles()
            .join(named_for_the_moment(&contents)),
    };
    let written = write(archives.vault.as_ref(), &contents, &at).await?;
    Ok(Bundle {
        contents,
        bytes: written.bytes,
        path: Some(written.path),
    })
}

/// The file a bundle is written as.
///
/// Named for the moment because a bundle is refused rather than written over one
/// already there, and somebody asking for help twice in an afternoon should not
/// have to think about why the second attempt failed.
fn named_for_the_moment(contents: &Contents) -> String {
    format!(
        "{}-support-{}.tar.gz",
        crate::PRODUCT,
        contents.taken.at.replace(':', "-")
    )
}

/// The refusal for a run that cannot say where its own files go.
///
/// Resolving the configuration home is the surface's half of a run, and a machine
/// that will not answer leaves it with neither a directory of its own to write into
/// nor the adapter that packs an archive — so a bundle asked for at a named path is
/// refused here too.
fn nowhere_to_keep() -> Problem {
    Problem::new(
        NOWHERE_TO_KEEP,
        Severity::Error,
        "This run cannot write an archive",
        "A bundle is one archive, and this run holds neither anywhere of its own to keep one \
         nor anything to pack one with — which is what a machine that will not say where its \
         own files go leaves behind. Nothing was written.",
        Remedy::new("Set a home directory for this user and run it again"),
    )
    .in_state(State::Guided)
}
