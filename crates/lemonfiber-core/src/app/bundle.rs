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

use crate::bundle::{self, Contents, Marks, Piece, Taken};
use crate::doctor::Verdict;
use crate::instant;

use super::Ctx;

/// What a bundle says for a version it could not read, rather than leaving a blank a
/// reader would take for a version of nothing.
const UNKNOWN: &str = "unknown";

/// Everything a bundle would hold, gathered and redacted, with whatever could not be read
/// named rather than passed over — or nothing at all where the machine could not provide
/// the randomness the stand-ins are derived from.
///
/// Nothing at all, rather than a bundle with a predictable salt: a stand-in anyone can
/// reproduce is a way back to the value it stands for, and a bundle is a thing people
/// post in public.
pub async fn collect(ctx: &Ctx, lemonfiber: &str) -> Option<Contents> {
    let marks = &Marks::new(ctx.random.as_ref())?;
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

    match super::engine::diagnose(ctx, None, false).await {
        Err(problem) => missing.push(format!("the diagnosis could not run — {}", problem.summary)),
        Ok(report) => pieces.push(Piece {
            name: "diagnosis.txt".to_owned(),
            body: bundle::settings(&findings(&report), marks),
        }),
    }

    match ctx.engine.list(&project(ctx)).await {
        Err(_) => missing.push("the container engine could not be reached".to_owned()),
        Ok(containers) => pieces.push(Piece {
            name: "services.txt".to_owned(),
            body: services(&containers),
        }),
    }

    pieces.push(Piece {
        name: "platform.txt".to_owned(),
        body: platform(ctx, lemonfiber),
    });

    match configuration(ctx).await {
        None => missing.push("no configuration has been written yet".to_owned()),
        Some(body) => pieces.push(Piece {
            name: "configuration.env".to_owned(),
            body: bundle::settings(&body, marks),
        }),
    }

    Some(Contents {
        pieces,
        missing,
        taken: Taken {
            lemonfiber: lemonfiber.to_owned(),
            stack,
            at: instant::written(ctx.clock.now()).unwrap_or_default(),
        },
    })
}

/// The Compose project the containers belong to, as every other read of them names it.
fn project(ctx: &Ctx) -> String {
    ctx.settings
        .stack_dir
        .as_deref()
        .and_then(|dir| dir.file_name())
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_default()
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

#[cfg(test)]
mod tests {
    use super::reading;
    use crate::doctor::Verdict;
    use crate::error::{Code, Problem, Remedy, Severity};

    /// A problem as a check reports one.
    fn problem() -> Problem {
        Problem::new(
            Code::new("BUNDLE-0"),
            Severity::Warning,
            "something is wrong",
            "why it matters",
            Remedy::new("do something"),
        )
    }

    /// Every verdict a check can reach, in the words the check chose. A bundle that
    /// paraphrased any of them would leave the operator and the person helping comparing
    /// two accounts of one finding.
    #[test]
    fn a_verdict_reads_in_the_words_the_check_used() {
        assert_eq!(
            reading(&Verdict::Pass {
                note: Some("340 GiB left".to_owned())
            }),
            "340 GiB left"
        );
        // A pass with nothing to add says nothing rather than inventing a reassurance.
        assert_eq!(reading(&Verdict::Pass { note: None }), "");
        assert_eq!(reading(&Verdict::Warn(problem())), "something is wrong");
        assert_eq!(reading(&Verdict::Fail(problem())), "something is wrong");
        assert_eq!(
            reading(&Verdict::Unverified {
                reason: "nothing answered".to_owned(),
                remedy: Remedy::new("try again"),
            }),
            "nothing answered"
        );
        assert_eq!(
            reading(&Verdict::Skipped {
                reason: "nothing to read".to_owned()
            }),
            "nothing to read"
        );
    }
}
