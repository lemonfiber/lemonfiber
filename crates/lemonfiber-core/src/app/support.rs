//! What a support request comes to: a description of a bundle, or a bundle.
//!
//! The gathering and the redaction are [`super::bundle`]'s. What is here is the
//! errand: a bare run collects, redacts, scans and says what a bundle would hold,
//! and writes nothing. Producing one is a second, deliberate run over the same
//! collection and the same scan — a description that checked differently from the
//! write would be a description of something else.
//!
//! lemonfiber sends the file nowhere. Where it is *written* is the one question a
//! caller with no filesystem in front of it cannot answer for itself, so
//! [`Destination`] carries the answers rather than leaving a path to be supplied by
//! whoever is asking — and [`held`] is the other half of that for the caller with no
//! filesystem: a bundle it asked for, handed back to it whole, because being handed
//! the file is what a browser has instead of a path to keep it at.

use std::path::PathBuf;

use serde::Serialize;

use crate::error::Amiss;

use crate::bundle::Contents;
use crate::error::{Problem, Remedy, Severity, State};

use super::bundle::{collect, measure, unconfirmed, without_marks, write, Wanted};
use super::Ctx;

pub use crate::error::codes::bundle::NOWHERE_TO_KEEP;

pub(crate) use crate::error::codes::bundle::NOWHERE_HELD;

pub(crate) use crate::error::codes::bundle::NOT_HELD;

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
    /// Where it would be written, on the run that only describes one.
    ///
    /// The other half of what a description is for. What goes in the file and how
    /// large it is answer *whether* to make it; where it lands answers *where to find
    /// it*, and an operator deciding at a shell needs both at the one moment the
    /// answer can still change what they do. Resolved by the same function the run
    /// that writes resolves it with, so the path shown and the path written are one.
    ///
    /// Absent on a run that wrote one — `path` is then where it went — and absent on a
    /// machine that would not say where lemonfiber keeps its own files, which is the
    /// one destination of the three that needs that answer.
    pub would_go: Option<PathBuf>,
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

    // Worked out before the branch, because both halves of this command need the same
    // answer: it is what a run that writes nothing reports, and it is where the run
    // that writes one puts the file. Resolved twice it could resolve differently, and
    // a description of somewhere other than where the file lands is the shape of
    // report this module exists not to produce.
    let at = landing(
        ctx.archives
            .as_ref()
            .map(|archives| archives.paths.bundles()),
        &contents,
        dest,
    );

    if !write_it {
        let bytes = measure(&contents)?;
        return Ok(Bundle {
            contents,
            bytes,
            path: None,
            would_go: at,
        });
    }
    // Both halves of writing, asked for once: the adapter that packs the file, and the
    // directory it goes in where the caller named no path. A bundle written by one
    // run's adapter into a path this run could not resolve is a file in a place
    // nothing looks, so a run holding neither is refused here rather than half of it.
    let (Some(archives), Some(at)) = (ctx.archives.as_ref(), at) else {
        return Err(Box::new(nowhere_to_keep()));
    };
    let written = write(archives.vault.as_ref(), &contents, &at).await?;
    Ok(Bundle {
        contents,
        bytes: written.bytes,
        path: Some(written.path),
        would_go: None,
    })
}

/// Where a bundle goes: the path the operator named, one beside them, or one with
/// lemonfiber's own files.
///
/// `bundles` is the directory this run keeps its own archives in, absent on a machine
/// that would not say where those go — which is the one destination of the three that
/// needs the answer. A named path and one written beside the operator are knowable
/// either way, and a description that withheld them because of a directory it was not
/// going to use would be withholding the answer it was asked for.
fn landing(bundles: Option<PathBuf>, contents: &Contents, dest: &Destination) -> Option<PathBuf> {
    match dest {
        Destination::At(path) => Some(path.clone()),
        Destination::Beside => Some(PathBuf::from(named_for_the_moment(contents))),
        Destination::Kept => Some(bundles?.join(named_for_the_moment(contents))),
    }
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

/// One bundle this run kept, whole.
///
/// The name travels with the bytes because it is what the file is called wherever
/// it lands next, and a caller handed bytes alone would have to be told separately.
#[derive(Debug, PartialEq, Eq)]
pub struct Held {
    /// What the file is called, derived from the name that was asked for rather
    /// than echoed back from it.
    pub name: String,
    /// The archive itself.
    pub bytes: Vec<u8>,
}

/// One of the bundles this run kept, read back for a caller with no filesystem.
///
/// The answer to `--out` for a surface that has no path to name: a bundle is
/// written with lemonfiber's own files and then handed over, so where it ends up is
/// answered on both surfaces rather than on one. Nothing is sent anywhere — the
/// bytes go to the caller that asked, over the connection it already holds.
///
/// The name is a name and never a path. It is resolved to a single file in the
/// bundles directory by [`crate::within::one_file`], so a name carrying a path, or
/// climbing out of that directory, names nothing rather than reaching what it
/// climbed to — the rule a restore names an archive under, and for the same reason:
/// the server runs as the operator. The directory itself is lemonfiber's own and
/// not the caller's, which is what keeps this from being a way to read a file the
/// endpoints beside it would refuse.
///
/// # Errors
///
/// Returns a [`Problem`] where this run has nowhere it keeps its own files, or
/// where the name names none of the bundles kept there.
pub fn held(ctx: &Ctx, name: &str) -> Result<Held, Box<Problem>> {
    let archives = ctx
        .archives
        .as_ref()
        .ok_or_else(|| Box::new(nowhere_held()))?;
    let file = crate::within::one_file(name).ok_or_else(|| Box::new(not_held(name)))?;
    let bytes = std::fs::read(archives.paths.bundles().join(&file))
        .map_err(|_| Box::new(not_held(name)))?;
    Ok(Held {
        name: file.to_string_lossy().into_owned(),
        bytes,
    })
}

/// The refusal for a run that cannot say where its own files are.
fn nowhere_held() -> Problem {
    Problem::new(
        NOWHERE_HELD,
        Severity::Error,
        "This run has nowhere it knows to look for a bundle",
        "Bundles asked for here are kept with lemonfiber's own files, and this machine would \
         not say where those are — so there is nowhere to read one back from.",
        Remedy::new("Set a home directory for this user and run it again"),
    )
    .in_state(State::Guided)
}

/// The refusal for a name that is not one of the bundles kept here.
///
/// The name is quoted back because the caller chose it and a caller that mistyped
/// one needs to see which. What it is not is followed: a name carrying a path is a
/// request to read somewhere lemonfiber does not keep bundles, and the server runs
/// as the operator.
fn not_held(name: &str) -> Problem {
    Problem::new(
        NOT_HELD,
        Severity::Error,
        format!("`{name}` is not one of the bundles kept here"),
        "A bundle asked for by name is one of the files this run wrote into lemonfiber's own \
         directory. A name holding a path, or climbing out of that directory, is refused \
         rather than followed.",
        Remedy::new("Ask for a bundle by the name the run that produced it reported"),
    )
    .lies_in(Amiss::Naming)
    .in_state(State::Guided)
}

#[cfg(test)]
mod tests;
