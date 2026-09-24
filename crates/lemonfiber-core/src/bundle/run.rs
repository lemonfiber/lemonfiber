//! Gathering what a support bundle holds.
//!
//! What may be shared is [`crate::bundle`]'s decision, and pure. This is the gathering
//! that feeds it: all reads, and every one of them allowed to fail. A bundle is wanted
//! precisely when a machine is not working, so a collector that refused to produce
//! anything without a complete picture would refuse exactly when it is needed — each
//! source that will not answer is named instead, and the rest is collected.
//!
//! Everything gathered is redacted on the way in, before it is a piece of the bundle at
//! all. The scan that reads it all back is the second line rather than the first, because
//! a check that is the only line is a check that has to be perfect.

use std::path::{Path, PathBuf};

use crate::archive::{Archive, Fault, Space};
use crate::bundle::{self, Contents, Filenames, Marks, Piece, Residual, Taken, Terms};
use crate::bytes::humanize;
use crate::doctor::Verdict;
use crate::error::{Code, Problem, Remedy, Severity, State};
use crate::instant;
use crate::ports::docker::LogQuery;

use crate::app::Ctx;

/// What a bundle says for a version it could not read, rather than leaving a blank a
/// reader would take for a version of nothing.
const UNKNOWN: &str = "unknown";

/// How many log lines each service contributes when nothing else is asked for.
///
/// A window rather than everything: a bundle is a thing somebody attaches to a forum post,
/// and a stack's whole log is gigabytes nobody will read. Two hundred lines a service is
/// about what a fault that is still happening looks like.
pub const LINES: u32 = 200;

/// What the operator asked for, before it becomes what the bundle says was done.
///
/// The command carries choices and the bundle carries their consequences. Keeping the two
/// apart means the stated window is worked out in one place, rather than once by whoever
/// collects and again by whoever prints.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Wanted {
    /// How many log lines to take from each service.
    pub lines: u32,
    /// Whether media filenames are shown.
    pub filenames: Filenames,
    /// The settings to show as they are, named as the bundle names them.
    pub reveal: Vec<String>,
    /// Whether showing those settings was agreed to on the same request.
    ///
    /// Beside what it agrees to rather than apart from it: a flag that publishes a
    /// credential and the agreement to publish it are one decision, and two values
    /// a caller carries separately are two that eventually arrive apart.
    pub confirmed: bool,
}

impl Default for Wanted {
    /// What somebody who asked for nothing in particular gets: the whole window, filenames
    /// replaced, nothing revealed. Every default here is the careful one.
    fn default() -> Self {
        Self {
            lines: LINES,
            filenames: Filenames::Replaced,
            reveal: Vec::new(),
            confirmed: false,
        }
    }
}

impl Wanted {
    /// What a caller asked for, as one value rather than four.
    ///
    /// Gathered here so a surface hands over what was wanted rather than spelling
    /// out the same four fields, and so a field added to the four is added to the
    /// surfaces that carry it rather than silently defaulted at one of them.
    #[must_use]
    pub fn asked(lines: u32, filenames: Filenames, reveal: Vec<String>, confirmed: bool) -> Self {
        Self {
            lines,
            filenames,
            reveal,
            confirmed,
        }
    }

    /// The terms a bundle made this way carries.
    fn terms(&self) -> Terms {
        Terms {
            window: format!("the last {} lines of each service", self.lines),
            filenames: self.filenames,
            revealed: self.reveal.clone(),
        }
    }
}

/// Everything a bundle would hold, gathered and redacted, with whatever could not be read
/// named rather than passed over — or nothing at all where the machine could not provide
/// the randomness the stand-ins are derived from.
///
/// Nothing at all, rather than a bundle with a predictable salt: a stand-in anyone can
/// reproduce is a way back to the value it stands for, and a bundle is a thing people
/// post in public.
pub async fn collect(ctx: &Ctx, lemonfiber: &str, wanted: &Wanted) -> Option<Contents> {
    let marks = &Marks::new(ctx.random.as_ref())?;
    let terms = wanted.terms();
    let mut pieces = Vec::new();
    let mut missing = Vec::new();
    // The first thing read and the first thing that can be missing: a machine whose stack
    // will not read is exactly the machine somebody needs a bundle from.
    let stack = if let Ok(manifest) = ctx.stack.checked_manifest(ctx.today()) {
        manifest.stack_version
    } else {
        missing.push("the stack description could not be read".to_owned());
        UNKNOWN.to_owned()
    };

    match crate::app::engine::diagnose(ctx, &crate::doctor::Narrowing::Suite, false).await {
        Err(problem) => missing.push(format!("the diagnosis could not run — {}", problem.summary)),
        // A finding is a sentence, not a setting: the provider checks quote a download
        // client's own words back, and those arrive with the provider's hostname in them
        // and could arrive with a key. The settings rule leaves a line with no `=` in it
        // exactly as it found it, so free text goes through the free-text rule.
        Ok(report) => pieces.push(Piece {
            name: "diagnosis.txt".to_owned(),
            body: bundle::prose(&findings(&report), marks, &terms),
        }),
    }

    match ctx.engine.list(&ctx.settings.project).await {
        Err(_) => missing.push("the container engine could not be reached".to_owned()),
        Ok(containers) => pieces.push(Piece {
            name: "services.txt".to_owned(),
            body: bundle::prose(&services(&containers), marks, &terms),
        }),
    }

    pieces.push(Piece {
        name: "platform.txt".to_owned(),
        body: bundle::prose(&platform(ctx, lemonfiber), marks, &terms),
    });

    match configuration(ctx).await {
        None => missing.push("no configuration has been written yet".to_owned()),
        Some(body) => pieces.push(Piece {
            name: "configuration.env".to_owned(),
            body: bundle::settings(&body, marks, &terms),
        }),
    }

    match logs(ctx, wanted.lines).await {
        Err(problem) => missing.push(format!("the logs could not be read — {}", problem.summary)),
        Ok(body) => pieces.push(Piece {
            name: "logs.txt".to_owned(),
            body: bundle::prose(&body, marks, &terms),
        }),
    }

    Some(Contents {
        pieces,
        // The gaps carry other people's words too — each is built from the summary of
        // whatever refused to answer, and a service that fails while authenticating says
        // so with the credential in hand. A line on the bundle's first page is as public
        // as any other line in it.
        missing: missing
            .iter()
            .map(|gap| bundle::prose(gap, marks, &terms))
            .collect(),
        taken: Taken {
            lemonfiber: lemonfiber.to_owned(),
            stack,
            at: instant::written(ctx.clock.now()).unwrap_or_default(),
        },
        terms,
    })
}

/// The logs, bounded and stated.
///
/// Bounded because a bundle is something somebody attaches to a post, and stated because
/// an extract that does not say what it is an extract of reads as the whole story. Allowed
/// to fail like every other source: a machine whose engine will not answer still has a
/// diagnosis and a configuration worth reading, and it is the engine not answering that
/// the bundle is most likely being asked for.
async fn logs(ctx: &Ctx, lines: u32) -> Result<String, Box<Problem>> {
    let mut arriving = crate::app::engine::logs(ctx, &[], &[], LogQuery::recent(lines)).await?;
    let mut held = Vec::new();
    while let Some(line) = arriving.recv().await {
        held.push(format!("{} | {}", line.service, line.line));
    }
    Ok(held.join("\n"))
}

/// The diagnosis as a person reads it: one line per finding, worst first, which is the
/// order the report already puts them in.
fn findings(report: &crate::model::DoctorReport) -> String {
    report
        .findings
        .iter()
        .map(|finding| format!("{}: {}", finding.title, reading(&finding.verdict)))
        .collect::<Vec<_>>()
        .join("\n")
}

/// One verdict as a line of a report reads it.
///
/// The words the check itself chose, every time. A bundle that paraphrased them would
/// leave the operator and the person helping comparing two accounts of one finding.
fn reading(verdict: &Verdict) -> String {
    match verdict {
        Verdict::Pass { note } => note.clone().unwrap_or_default(),
        Verdict::Warn(problem) | Verdict::Fail(problem) => problem.summary.clone(),
        Verdict::Unverified { reason, .. } | Verdict::Skipped { reason } => reason.clone(),
    }
}

/// What each container is doing, which is the half of a fault the diagnosis cannot see.
fn services(containers: &[crate::ports::docker::Container]) -> String {
    containers
        .iter()
        .map(|container| {
            format!(
                "{}: {:?}, health {:?}",
                container.service, container.lifecycle, container.health
            )
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// What the machine is, and what is running on it.
fn platform(ctx: &Ctx, lemonfiber: &str) -> String {
    format!("lemonfiber {lemonfiber}\nplatform {:?}", ctx.environment)
}

/// The operator's own configuration, where one has been written.
async fn configuration(ctx: &Ctx) -> Option<String> {
    let path = ctx.settings.env_file.as_deref()?;
    ctx.filesystem.read(path).await
}

/// Bytes kept free beyond the bundle itself, so writing one never spends the last of the
/// disk the operator is already asking for help about.
const HEADROOM: u64 = 64 * 1024 * 1024;

pub use crate::error::codes::bundle::BUNDLE_LEAK;

pub use crate::error::codes::bundle::BUNDLE_NO_ROOM;

pub use crate::error::codes::bundle::BUNDLE_UNWRITTEN;

pub use crate::error::codes::bundle::BUNDLE_UNCONFIRMED;

pub use crate::error::codes::bundle::BUNDLE_NO_MARKS;

/// Refuse to show a setting nobody confirmed showing.
///
/// Naming a field and agreeing to publish it are two acts, deliberately. A flag that puts
/// a credential in a file people post is not one to honour because it turned up on a
/// command line somebody copied out of a thread — and the refusal names the settings, so
/// what gets confirmed is those rather than a policy.
#[must_use]
pub fn unconfirmed(fields: &[String]) -> Problem {
    Problem::new(
        BUNDLE_UNCONFIRMED,
        Severity::Error,
        "Showing a setting as it is has to be confirmed",
        "A bundle is a thing people post in public. Showing one of its settings as it is puts that value in the file, so it takes saying twice.",
        Remedy::new("Run it again with --confirm if you meant it"),
    )
    .in_state(State::Guided)
    .with_detail(format!("would have shown: {}", fields.join(", ")))
}

/// Refuse a bundle on a machine that will not provide randomness.
///
/// Not something to paper over with a fixed salt: every replaced value carries a stand-in
/// derived from it, and one anybody can reproduce is a way back to the value it stands for.
#[must_use]
pub(crate) fn without_marks() -> Problem {
    Problem::new(
        BUNDLE_NO_MARKS,
        Severity::Error,
        "A bundle could not be made on this machine",
        "Every replaced value carries a stand-in derived with randomness this machine would not provide, and a stand-in anyone can reproduce is a way back to the value it stands for. Nothing has been written.",
        Remedy::new("Report this: a machine that cannot produce random bytes is a fault in its own right"),
    )
    .in_state(State::Guided)
}

/// What was written, and what a reader will find in it.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct Written {
    /// Where it is. It is only ever here: nothing sends it anywhere.
    pub path: PathBuf,
    /// How large it is, so the operator knows what they are about to attach.
    pub bytes: u64,
    /// What it holds, in order, so they can read it before anyone else does.
    pub holds: Vec<String>,
}

/// How large a bundle would be, having read it back and found nothing left in it that
/// reads as a credential.
///
/// The question a preview asks, and the first thing writing one asks — the same question,
/// asked the same way, because a preview that scanned differently from the write would be
/// a preview of something else. An operator is told the size while there is still nothing
/// to attach, which is the only point at which the answer can change what they do.
///
/// # Errors
///
/// Returns a [`Problem`] where the assembled bundle still holds something that reads as a
/// credential, naming the file it came from. Boxed as a capture's refusals are, because a
/// refusal carries a good deal more than the number it refuses to give.
pub fn measure(contents: &Contents) -> Result<u64, Box<Problem>> {
    let files = contents.files();
    if let Some(residual) = bundle::residual(&files, &contents.terms) {
        return Err(Box::new(leaking(&residual)));
    }
    Ok(files
        .iter()
        .map(|(name, body)| (name.len() + body.len()) as u64)
        .sum())
}

/// Write `contents` as one archive at `dest`, or refuse and say why.
///
/// Read back before written, always. The allow-list decides what may be shared and this
/// asks a different question of the result — does anything in here still read as a
/// credential — because two checks that fail the same way are one check. A hit is not a
/// warning: nothing is written, and the file that produced it is named, since the operator
/// cannot fix what nobody points at.
///
/// Then room, before rather than after: an operator asking for help about a machine is not
/// helped by filling its disk, and a bundle that failed halfway leaves them with a file
/// that looks like a bundle and is not.
///
/// # Errors
///
/// Returns a [`Problem`] where the assembled bundle still holds something that reads as a
/// credential, where there is not enough room to write it, or where the archive itself
/// could not be written. Nothing is left behind in any of the three.
pub async fn write(
    archive: &dyn Archive,
    contents: &Contents,
    dest: &Path,
) -> Result<Written, Box<Problem>> {
    let bytes = measure(contents)?;
    let files = contents.files();
    let dir = dest.parent().unwrap_or(dest);
    if let Ok(space) = archive.space(dir, &[]).await {
        let room = Space {
            needed: bytes,
            available: space.available,
        };
        if !room.fits(HEADROOM) {
            return Err(Box::new(no_room(&room)));
        }
    }

    archive
        .write_files(dest, &files)
        .await
        .map_err(|fault| unwritten(dest, &fault))?;

    Ok(Written {
        path: dest.to_path_buf(),
        bytes,
        holds: files.into_iter().map(|(name, _)| name).collect(),
    })
}

/// A bundle that would have carried a credential out. Refused rather than written, and the
/// source named — the one failure this whole feature exists to prevent is a bundle that
/// looked fine and was not.
fn leaking(residual: &Residual) -> Problem {
    Problem::new(
        BUNDLE_LEAK,
        Severity::Critical,
        "The bundle still held something that reads as a credential",
        "Nothing has been written. A bundle is a thing people post in public, so anything in one that still reads like a key is treated as one — even where it turns out not to be.",
        Remedy::new(
            "Report which file this names, so the value it holds can be added to what a bundle knows how to replace",
        ),
    )
    .in_state(State::Guided)
    .with_detail(format!(
        "{} line {} — nothing was written",
        residual.source, residual.line
    ))
}

/// Not enough room. Reported before collecting anything rather than partway through
/// writing it, because a machine an operator is already asking about is not helped by
/// having its disk filled.
fn no_room(space: &Space) -> Problem {
    Problem::new(
        BUNDLE_NO_ROOM,
        Severity::Error,
        "There is not enough room to write the bundle",
        "The bundle would not fit where it was to be written, with room left over for the machine to keep working in.",
        Remedy::new("Free some space, or write the bundle somewhere with more room"),
    )
    .in_state(State::Guided)
    .with_detail(format!(
        "{} needed, {} free",
        humanize(space.needed),
        humanize(space.available)
    ))
}

/// The archive itself would not be written.
fn unwritten(dest: &Path, fault: &Fault) -> Problem {
    Problem::new(
        BUNDLE_UNWRITTEN,
        Severity::Error,
        "The bundle could not be written",
        "Nothing was left behind: a bundle is written whole or not at all, so there is no half-file to mistake for one.",
        Remedy::new("Check the path is writable, then ask for the bundle again"),
    )
    .in_state(State::Guided)
    .with_detail(format!("{}: {}", dest.display(), fault.message))
}

#[cfg(test)]
mod tests;
