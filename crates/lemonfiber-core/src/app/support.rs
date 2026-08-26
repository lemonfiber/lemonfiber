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
use crate::error::{Code, Problem, Remedy, Severity, State};

use super::bundle::{collect, measure, unconfirmed, without_marks, write, Wanted};
use super::Ctx;

/// Raised when this run has nowhere it knows to keep a bundle.
pub const NOWHERE_TO_KEEP: Code = Code::new("BUNDLE-6");

/// Raised when this run has nowhere it knows to look for a bundle it kept.
pub const NOWHERE_HELD: Code = Code::new("BUNDLE-7");

/// Raised when a name does not name one of the bundles this run kept.
pub const NOT_HELD: Code = Code::new("BUNDLE-8");

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
mod tests {
    use std::path::Path;
    use std::sync::Arc;

    use super::{held, Held, NOT_HELD, NOWHERE_HELD};
    use crate::app::fixtures::{scratch, FakeArchive};
    use crate::app::Ctx;
    use crate::archive::Archiving;
    use crate::config::paths::Paths;
    use crate::test_support::a_context;

    /// A run keeping its own files under `dir`, which is a real directory.
    fn keeping_at(dir: &Path) -> Ctx {
        let vault: Arc<dyn crate::archive::Vault> = Arc::new(FakeArchive::roomy());
        a_context().build().keeping(Archiving {
            paths: Paths::at(dir, dir),
            vault,
        })
    }

    /// A bundles directory holding one file of the given name and contents.
    fn holding(test: &str, name: &str, contents: &str) -> Ctx {
        let dir = scratch(test);
        let bundles = dir.join("support");
        assert!(
            std::fs::create_dir_all(&bundles).is_ok(),
            "the scratch directory is writable"
        );
        assert!(std::fs::write(bundles.join(name), contents).is_ok());
        keeping_at(&dir)
    }

    #[test]
    fn a_bundle_this_run_kept_is_handed_back_whole() {
        let ctx = holding("held-whole", "lemonfiber-support-1.tar.gz", "an archive");
        assert_eq!(
            held(&ctx, "lemonfiber-support-1.tar.gz").ok(),
            Some(Held {
                name: "lemonfiber-support-1.tar.gz".to_owned(),
                bytes: b"an archive".to_vec(),
            })
        );
    }

    #[test]
    fn a_name_climbing_out_of_the_directory_reaches_nothing_it_climbed_to() {
        // The file is really there, one level above the bundles directory, and is
        // still not readable through this: the name is refused rather than resolved.
        let dir = scratch("held-climbing");
        let bundles = dir.join("support");
        assert!(std::fs::create_dir_all(&bundles).is_ok());
        assert!(std::fs::write(dir.join("secrets.env"), "INDEXER_KEY=live").is_ok());
        let ctx = keeping_at(&dir);
        assert_eq!(
            held(&ctx, "../secrets.env")
                .err()
                .map(|problem| problem.code),
            Some(NOT_HELD)
        );
    }

    #[test]
    fn a_name_that_names_no_bundle_is_refused_by_name() {
        let ctx = holding("held-missing", "lemonfiber-support-1.tar.gz", "an archive");
        let refused = held(&ctx, "lemonfiber-support-9.tar.gz").err();
        assert_eq!(refused.as_ref().map(|problem| problem.code), Some(NOT_HELD));
        assert!(
            refused.is_some_and(|problem| problem.summary.contains("lemonfiber-support-9.tar.gz")),
            "the name is quoted back so a mistyped one can be seen"
        );
    }

    #[test]
    fn a_run_with_nowhere_to_look_says_so_rather_than_answering_with_nothing() {
        let ctx = a_context().build();
        assert_eq!(
            held(&ctx, "lemonfiber-support-1.tar.gz")
                .err()
                .map(|problem| problem.code),
            Some(NOWHERE_HELD)
        );
    }
}
