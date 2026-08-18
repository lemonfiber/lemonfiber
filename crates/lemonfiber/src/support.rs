//! Producing a support bundle, and saying what one would hold before it exists.
//!
//! The gathering and the redaction belong to the core. What is here is the surface: which
//! of the two things a run does, and how each of them reads.
//!
//! A bare run describes and writes nothing; writing is a second, deliberate run. The
//! decision to make a file worth attaching to a public thread is one to take after seeing
//! what goes in it, and an operator who is already asking for help is not in a good
//! position to be careful on their own behalf. Both runs collect and scan identically — a
//! preview that ran a different check from the write would be a preview of something else.

use std::path::PathBuf;
use std::process::ExitCode;

use lemonfiber_core::app::bundle::{collect, measure, unconfirmed, without_marks, write, Wanted};
use lemonfiber_core::app::Ctx;
use lemonfiber_core::bundle::{Contents, Filenames};
use lemonfiber_core::error::Problem;

use crate::cli::Asked;
use crate::render::support::{render_preview, render_written};
use crate::render::Lines;

/// Describe a bundle, or produce one.
pub(crate) async fn run(ctx: Ctx, asked: Asked, json: bool) -> ExitCode {
    if !asked.reveal.is_empty() && !asked.confirm {
        return crate::complain(&unconfirmed(&asked.reveal));
    }

    let wanted = Wanted {
        lines: asked.logs,
        filenames: if asked.filenames {
            Filenames::Shown
        } else {
            Filenames::Replaced
        },
        reveal: asked.reveal,
    };

    let Some(contents) = collect(&ctx, env!("CARGO_PKG_VERSION"), &wanted).await else {
        return crate::complain(&without_marks());
    };

    // Both answers are built the same way and refused the same way, so there is one place
    // that turns a refusal into an exit code rather than one per answer — two would be two
    // chances for a bundle to be refused quietly in one of them.
    let answer = if asked.write {
        produce(&contents, asked.out, json).await
    } else {
        describe(&contents, json)
    };
    match answer {
        Ok(lines) => {
            lines.print();
            ExitCode::SUCCESS
        }
        Err(problem) => crate::complain(&problem),
    }
}

/// Say what a bundle would hold, having done everything that producing one does except
/// produce it — the scan included, so a bundle that would be refused is refused here
/// rather than after the operator has been told to run the command again.
///
/// # Errors
///
/// Returns the [`Problem`] describing a bundle that still holds something reading as a
/// credential, which is a refusal rather than a warning.
fn describe(contents: &Contents, json: bool) -> Result<Lines, Box<Problem>> {
    Ok(render_preview(contents, measure(contents)?, json))
}

/// Produce the file, and say where it went.
///
/// # Errors
///
/// Returns the [`Problem`] for a bundle that would leak, would not fit, or could not be
/// written — in all three of which nothing is left behind.
async fn produce(
    contents: &Contents,
    out: Option<PathBuf>,
    json: bool,
) -> Result<Lines, Box<Problem>> {
    let dest = destination(out, contents);
    let written = write(&crate::archive::Tar, contents, &dest)
        .await
        .map_err(Box::new)?;
    Ok(render_written(&written, json))
}

/// Where the bundle goes: where the operator said, or beside them under a name carrying
/// the moment it was taken.
///
/// Named for the moment because a bundle is refused rather than written over one already
/// there, and somebody asking for help twice in an afternoon should not have to think
/// about why the second attempt failed.
fn destination(out: Option<PathBuf>, contents: &Contents) -> PathBuf {
    out.unwrap_or_else(|| {
        let at = contents.taken.at.replace(':', "-");
        PathBuf::from(format!("lemonfiber-support-{at}.tar.gz"))
    })
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use async_trait::async_trait;
    use lemonfiber_core::adapters;
    use lemonfiber_core::app::bundle::Written;
    use lemonfiber_core::bundle::{Contents, Piece, Taken, Terms};
    use lemonfiber_core::config::Settings;
    use lemonfiber_core::platform::Environment;
    use lemonfiber_core::ports::docker::{
        Container, Engine, ExecOutput, Failure, Health, Lifecycle, LogLine, LogQuery, Stats, Stream,
    };
    use lemonfiber_core::ports::random::Random;
    use lemonfiber_core::stack::Source;
    use tokio::sync::mpsc::Receiver;

    use crate::exit::shown;
    use crate::render::support::{render_preview, render_written};

    use super::{destination, run, Asked, Ctx, ExitCode, PathBuf};

    /// An engine running one service, which says one thing when its logs are read.
    ///
    /// A bundle asks an engine exactly two things — what is running, and what it has been
    /// saying — so those are the two this answers and the rest state plainly that a bundle
    /// never reaches for them.
    struct Talking;

    #[async_trait]
    impl Engine for Talking {
        async fn list(&self, _project: &str) -> Result<Vec<Container>, Failure> {
            Ok(vec![Container {
                id: "abc".to_owned(),
                project: "media-stack".to_owned(),
                service: "sonarr".to_owned(),
                lifecycle: Lifecycle::Running,
                health: Health::Healthy,
                exit: None,
            }])
        }
        async fn logs(
            &self,
            _project: &str,
            _services: &[String],
            _query: LogQuery,
        ) -> Result<Receiver<LogLine>, Failure> {
            let (sending, receiving) = tokio::sync::mpsc::channel(4);
            let _ = sending
                .send(LogLine {
                    service: "sonarr".to_owned(),
                    stream: Stream::Stdout,
                    at: None,
                    line: "grabbed something".to_owned(),
                })
                .await;
            Ok(receiving)
        }
        async fn exec(&self, _container: &str, _argv: &[String]) -> Result<ExecOutput, Failure> {
            Err(unused())
        }
        async fn stats(&self, _project: &str) -> Result<Receiver<(String, Stats)>, Failure> {
            Err(unused())
        }
    }

    /// What the capabilities a bundle never reaches for answer with.
    fn unused() -> Failure {
        Failure::Unreachable {
            reason: "a bundle never asks this".to_owned(),
        }
    }

    fn ctx() -> Ctx {
        Ctx::new(
            Arc::new(adapters::Local),
            Arc::new(Talking),
            Arc::new(adapters::System),
            Arc::new(adapters::Disk),
            Source::Embedded(&crate::cli::STACK),
            Settings::default(),
            Environment::MacOs,
        )
    }

    /// What a run with no flags asks for.
    fn asked() -> Asked {
        Asked {
            write: false,
            out: None,
            logs: 5,
            filenames: false,
            reveal: Vec::new(),
            confirm: false,
        }
    }

    fn contents() -> Contents {
        Contents {
            pieces: vec![Piece {
                name: "platform.txt".to_owned(),
                body: "lemonfiber 0.7.0".to_owned(),
            }],
            missing: vec!["the container engine could not be reached".to_owned()],
            taken: Taken {
                lemonfiber: "0.7.0".to_owned(),
                stack: "1.2.0".to_owned(),
                at: "2026-08-18T00:00:00Z".to_owned(),
            },
            terms: Terms::default(),
        }
    }

    /// The rule that makes the consent mean anything: a flag that publishes a credential
    /// is not honoured because it appeared on a command line somebody copied.
    #[tokio::test]
    async fn a_reveal_without_a_confirmation_is_refused() {
        let code = run(
            ctx(),
            Asked {
                reveal: vec!["SONARR_API_KEY".to_owned()],
                ..asked()
            },
            false,
        )
        .await;
        assert_ne!(shown(code), shown(ExitCode::SUCCESS));
    }

    /// A machine that cannot produce random bytes gets no bundle at all, rather than one
    /// whose stand-ins anybody can reproduce — which would be a way back to every value
    /// they stand for.
    #[tokio::test]
    async fn a_machine_without_randomness_gets_no_bundle() {
        struct Nothing;

        impl Random for Nothing {
            fn bytes(&self, _n: usize) -> Option<Vec<u8>> {
                None
            }
        }

        let code = run(ctx().with_random(Arc::new(Nothing)), asked(), false).await;
        assert_ne!(shown(code), shown(ExitCode::SUCCESS));
    }

    /// A bare run says what a bundle would hold and writes nothing at all — with the
    /// filenames in it replaced, or kept where the operator asked for them.
    #[tokio::test]
    async fn a_bare_run_describes_a_bundle_and_writes_nothing() {
        assert_eq!(
            shown(run(ctx(), asked(), false).await),
            shown(ExitCode::SUCCESS)
        );
        assert_eq!(
            shown(
                run(
                    ctx(),
                    Asked {
                        filenames: true,
                        ..asked()
                    },
                    false
                )
                .await
            ),
            shown(ExitCode::SUCCESS)
        );
    }

    /// A bundle is a read-only errand: it asks an engine what is running and what it has
    /// been saying, and never asks it to run anything or to measure anything. The two
    /// capabilities it does not use say so rather than answering something plausible.
    #[tokio::test]
    async fn a_bundle_never_asks_an_engine_to_run_or_to_measure_anything() {
        assert!(Talking.exec("abc", &[]).await.is_err());
        assert!(Talking.stats("media-stack").await.is_err());
    }

    /// The second, deliberate run: it produces the file, at the path it was told.
    #[tokio::test]
    async fn asking_for_it_in_writing_produces_the_file() {
        let dest = std::env::temp_dir()
            .join("lemonfiber-support-tests")
            .join("bundle.tar.gz");
        let _ = std::fs::remove_file(&dest);

        let code = run(
            ctx(),
            Asked {
                write: true,
                out: Some(dest.clone()),
                ..asked()
            },
            false,
        )
        .await;

        assert_eq!(shown(code), shown(ExitCode::SUCCESS));
        assert!(dest.exists(), "the bundle is where it was asked for");
        let _ = std::fs::remove_file(&dest);
    }

    /// Refused twice over: the second run finds the first still there, and a bundle is
    /// never written over one already written.
    #[tokio::test]
    async fn a_bundle_is_not_written_over_one_already_there() {
        let dest = std::env::temp_dir()
            .join("lemonfiber-support-tests")
            .join("twice.tar.gz");
        let _ = std::fs::remove_file(&dest);
        let asking = || Asked {
            write: true,
            out: Some(dest.clone()),
            ..asked()
        };

        assert_eq!(
            shown(run(ctx(), asking(), true).await),
            shown(ExitCode::SUCCESS)
        );
        assert_ne!(
            shown(run(ctx(), asking(), false).await),
            shown(ExitCode::SUCCESS)
        );
        let _ = std::fs::remove_file(&dest);
    }

    /// Somebody asking for help twice in an afternoon should not have to work out why the
    /// second attempt failed, so an unnamed destination carries the moment it was taken.
    #[test]
    fn an_unnamed_destination_carries_the_moment_it_was_taken() {
        let named = destination(Some(PathBuf::from("/tmp/mine.tar.gz")), &contents());
        assert_eq!(named, PathBuf::from("/tmp/mine.tar.gz"));

        let generated = destination(None, &contents());
        assert_eq!(
            generated,
            PathBuf::from("lemonfiber-support-2026-08-18T00-00-00Z.tar.gz")
        );
    }

    /// What the operator reads before deciding: what it would hold, how large, what could
    /// not be read, and that nothing has been written.
    #[test]
    fn a_preview_says_what_would_be_held_and_that_nothing_was_written() {
        let shown = render_preview(&contents(), 41_000, false).text();
        assert!(shown.contains("README.txt"), "{shown}");
        assert!(shown.contains("platform.txt"), "{shown}");
        assert!(shown.contains("40.0 KiB in all."), "{shown}");
        assert!(shown.contains("Could not be read:"), "{shown}");
        assert!(shown.contains("Nothing has been written"), "{shown}");
        assert!(!shown.contains("because you asked"), "nothing was revealed");

        let asked = Contents {
            terms: Terms {
                revealed: vec!["SONARR_API_KEY".to_owned()],
                ..Terms::default()
            },
            ..contents()
        };
        let one = render_preview(&asked, 1, false).text();
        assert!(
            one.contains("SONARR_API_KEY as it is, because you asked"),
            "{one}"
        );

        let two = Contents {
            terms: Terms {
                revealed: vec!["A".to_owned(), "B".to_owned()],
                ..Terms::default()
            },
            ..contents()
        };
        let both = render_preview(&two, 1, false).text();
        assert!(both.contains("A, B as they are"), "{both}");

        // Nothing missing prints no heading: an empty "Could not be read:" reads as a
        // list somebody forgot to fill in, which is the same mistake the bundle's own
        // first page takes care not to make.
        let whole = Contents {
            missing: Vec::new(),
            ..contents()
        };
        let complete = render_preview(&whole, 1, false).text();
        assert!(!complete.contains("Could not be read"), "{complete}");

        assert!(render_preview(&contents(), 1, true)
            .text()
            .contains("platform.txt"));
    }

    /// And afterwards: where it went, and that sending it is theirs to do.
    #[test]
    fn what_was_written_says_where_it_went_and_that_it_stayed() {
        let written = Written {
            path: PathBuf::from("/home/you/support.tar.gz"),
            bytes: 2048,
            holds: vec!["README.txt".to_owned()],
        };
        let shown = render_written(&written, false).text();
        assert!(shown.contains("/home/you/support.tar.gz"), "{shown}");
        assert!(shown.contains("2.0 KiB"), "{shown}");
        assert!(shown.contains("README.txt"), "{shown}");
        assert!(shown.contains("Nothing has left this machine"), "{shown}");

        assert!(render_written(&written, true)
            .text()
            .contains("support.tar.gz"));
    }
}
