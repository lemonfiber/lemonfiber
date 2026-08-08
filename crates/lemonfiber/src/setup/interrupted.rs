//! The ways out of a setup that stopped part-way.
//!
//! An interrupted apply is the one state setup must never guess about: it wrote
//! something, and what it wrote is the operator's to keep, undo, or forget. Kept
//! apart from the gathering so the three ways out read as the whole of the choice.

use std::process::ExitCode;

use lemonfiber_core::app::{recover, setup as core_setup, Ctx};
use lemonfiber_core::config::paths::Paths;
use lemonfiber_core::journal::{Change, Kind};
use lemonfiber_core::wizard::{Choice, Progress, Recovery, Resolution, Wizard};
use lemonfiber_core::PRODUCT;

use super::boot::start;
use super::{fresh_setup, stamp, Surface};
use crate::context::read_settings;
use crate::exit::{complain, USAGE};
use crate::prompt::SetupFlags;

/// Offer the operator a way out of a setup whose apply stopped part-way.
///
/// It is shown what the interrupted run wrote and given the three ways forward the
/// wizard keeps recoverable: finish it, undo and redo it, or undo and forget it.
/// Deciding is not done for a piped run that cannot answer — the state is left as
/// it is, still recoverable, rather than acted on unasked.
pub(super) async fn recover_setup(
    ctx: Ctx,
    paths: &Paths,
    surface: &dyn Surface,
    progress: Option<Progress>,
) -> ExitCode {
    // A stopped apply always leaves its answers; if they are somehow gone there is
    // nothing to resume from, so a fresh run is the honest fallback — interactive,
    // since recovery carries no flags.
    let Some(progress) = progress else {
        return fresh_setup(ctx, paths, surface, SetupFlags::none()).await;
    };

    let journal = recover::journal_at(&paths.journal());
    let recovery = Recovery::of(&journal);

    println!("A previous setup was interrupted part-way through applying.");
    let written = recovery.written();
    if written.is_empty() {
        println!("It had not written anything yet.");
    } else {
        println!("It had written:");
        for change in written {
            println!("  · {}", describe(change));
        }
    }

    if !surface.interactive() {
        eprintln!("\nerror: recovering an interrupted setup needs a terminal to choose.");
        eprintln!("Run `{PRODUCT} setup` interactively to resume, roll back, or start over.");
        return ExitCode::from(USAGE);
    }

    let env = paths.env_file();
    match recovery.resolve(ask_recovery_choice(surface)) {
        Resolution::Resume => {
            println!("\nResuming.");
            resume_and_start(ctx, paths, progress).await
        }
        Resolution::RollBack(undos) => {
            if let Err(problem) = recover::undo(&undos, &env) {
                return complain(&problem);
            }
            println!("\nRolled back. Applying again.");
            resume_and_start(ctx, paths, progress).await
        }
        Resolution::StartOver(undos) => {
            if let Err(problem) = recover::undo(&undos, &env) {
                return complain(&problem);
            }
            discard(paths);
            println!("\nStarted over — nothing of the interrupted setup remains.");
            println!("Run `{PRODUCT} setup` to begin again.");
            ExitCode::SUCCESS
        }
    }
}

/// Re-apply the answers a stopped setup recorded, then bring the stack up.
pub(super) async fn resume_and_start(mut ctx: Ctx, paths: &Paths, progress: Progress) -> ExitCode {
    let mut wizard = Wizard::resume(ctx.environment, progress);
    match core_setup::resume(&mut wizard, paths, ctx.stack, &stamp()) {
        Ok(()) => {
            ctx.settings = read_settings();
            println!("\nSetup is done — bringing your stack up.");
            start(&ctx).await
        }
        Err(problem) => complain(&problem),
    }
}

/// Which way out of an interrupted setup the operator chooses.
pub(super) fn ask_recovery_choice(surface: &dyn Surface) -> Choice {
    println!("\nWhat would you like to do?");
    println!("  1) Resume — finish applying from where it stopped");
    println!("  2) Roll back — undo what was written, then apply again");
    println!("  3) Start over — undo it and forget the answers");
    match surface.line("Choose [1]:").as_str() {
        "2" => Choice::RollBack,
        "3" => Choice::StartOver,
        _ => Choice::Resume,
    }
}

/// A written change, said plainly enough for the operator to recognise.
pub(super) fn describe(change: &Change) -> String {
    match &change.kind {
        Kind::Set { key, .. } => format!("the setting {key}"),
        Kind::Made { path } => format!("the directory {path}"),
        Kind::Created { resource, .. } => format!("a {resource}"),
    }
}

/// Remove what an interrupted setup left, so starting over leaves nothing behind.
pub(super) fn discard(paths: &Paths) {
    let _ = std::fs::remove_file(paths.setup_progress());
    let _ = std::fs::remove_file(paths.journal());
}

#[cfg(test)]
mod tests {
    use lemonfiber_core::config::paths::Paths;
    use lemonfiber_core::journal::{Change, Kind};
    use lemonfiber_core::wizard::Choice;

    use super::{ask_recovery_choice, describe, discard};
    use crate::setup::tests::Scripted;

    /// A scratch install unique to this test.
    fn scratch(name: &str) -> Paths {
        let root =
            std::env::temp_dir().join(format!("lemonfiber-recover-{}-{name}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        Paths::rooted(&root.join("config"), &root.join("data"))
    }

    #[test]
    fn each_way_out_of_an_interrupted_setup_can_be_chosen() {
        assert!(matches!(
            ask_recovery_choice(&Scripted::saying(true, &["2"])),
            Choice::RollBack
        ));
        assert!(matches!(
            ask_recovery_choice(&Scripted::saying(true, &["3"])),
            Choice::StartOver
        ));
        // Anything else resumes, which is the safe default: it finishes what was
        // started rather than undoing work nobody asked to lose.
        for answer in ["1", "", "what"] {
            assert!(matches!(
                ask_recovery_choice(&Scripted::saying(true, &[answer])),
                Choice::Resume
            ));
        }
    }

    #[test]
    fn a_written_change_is_said_plainly_enough_to_recognise() {
        // What an operator is shown of an interrupted run: the point is that they
        // recognise it, not that it round-trips.
        let change = |kind| Change {
            at: String::new(),
            operation: "setup".to_owned(),
            target: "the environment file".to_owned(),
            kind,
        };
        assert_eq!(
            describe(&change(Kind::Set {
                key: "DATA_ROOT".to_owned(),
                previous: None,
                current: "/srv".to_owned(),
            })),
            "the setting DATA_ROOT"
        );
        assert_eq!(
            describe(&change(Kind::Made {
                path: "/srv/media".to_owned()
            })),
            "the directory /srv/media"
        );
        assert_eq!(
            describe(&change(Kind::Created {
                resource: "root folder".to_owned(),
                id: "1".to_owned(),
            })),
            "a root folder"
        );
    }

    #[test]
    fn starting_over_leaves_nothing_of_the_interrupted_run() {
        let paths = scratch("discard");
        for path in [paths.setup_progress(), paths.journal()] {
            let _ = path.parent().map(std::fs::create_dir_all);
            let _ = std::fs::write(&path, "something");
            assert!(path.exists());
        }
        discard(&paths);
        assert!(!paths.setup_progress().exists());
        assert!(!paths.journal().exists());
        // Discarding what is already gone is not a failure.
        discard(&paths);
    }
}
