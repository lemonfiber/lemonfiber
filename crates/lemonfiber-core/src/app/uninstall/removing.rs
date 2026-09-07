//! Carrying out what the manifest said, and reporting everything it could not.
//!
//! Two rules shape all of it.
//!
//! **Nothing is skipped quietly.** A container an engine would not remove, a path
//! the platform refused, an image something else took a hold of between the reading
//! and the removal — each is named with what the machine said and with the command
//! that finishes it. An operator who ran an uninstall and was told nothing believes
//! the machine is clean.
//!
//! **Nothing becomes somebody else.** Where a path cannot be removed for want of
//! permission, the path and the permission are reported and the run stops there. No
//! step here escalates, and none of the instructions handed back asks the operator to
//! escalate in order to remove lemonfiber's own files — a directory in your own home
//! that needs administrative rights to delete is a fault to understand rather than
//! one to overrule.

use std::path::Path;

use crate::app::{Ctx, Outcome, Waiting};
use crate::platform::Environment;
use crate::stack::compose::Action;
use crate::uninstall::{Left, Manifest, Removal, Sort, Tier};

/// What one removal accumulated as it went.
#[derive(Default)]
struct Went {
    gone: Vec<String>,
    left: Vec<Left>,
}

impl Went {
    /// Record a step that worked, or the reason it did not with the way to finish it.
    fn step(&mut self, name: &str, by_hand: String, outcome: Result<(), String>) {
        match outcome {
            Ok(()) => self.gone.push(name.to_owned()),
            Err(why) => self.left.push(Left {
                name: name.to_owned(),
                why,
                by_hand,
            }),
        }
    }
}

/// Carry out the removal this manifest describes.
pub(super) async fn remove(
    ctx: &Ctx,
    tier: Tier,
    manifest: &Manifest,
    waiting: Waiting,
) -> Removal {
    if waiting == Waiting::ForTheDownloads {
        crate::app::engine::drained(ctx, &[]).await;
    }

    let mut went = Went::default();
    stopping(ctx, tier, &mut went).await;
    match tier {
        Tier::Stop => {}
        Tier::Services => pulled(ctx, manifest, &mut went).await,
        Tier::Configuration | Tier::Media => paths(ctx, manifest, &mut went).await,
    }

    let credentials = destroyed(manifest);
    if went.left.is_empty() {
        return Removal::Complete {
            gone: went.gone,
            credentials,
        };
    }
    Removal::Partial {
        gone: went.gone,
        credentials,
        left: went.left,
    }
}

/// The credentials this removal destroyed, named rather than left to be inferred.
///
/// Read off the manifest's own lines, so what is said to have been destroyed is what
/// was said would be. A secret that was kept rather than taken is not one that has
/// gone, and saying it had would be the reassurance this exists to avoid.
fn destroyed(manifest: &Manifest) -> Vec<String> {
    manifest
        .items
        .iter()
        .filter(|item| item.secret && item.goes())
        .map(|item| item.name.clone())
        .collect()
}

/// Stop the services, whatever else this tier does.
///
/// Every tier begins here. A service still writing its database while its directory
/// goes is exactly the corruption a backup exists to undo, and one still holding a
/// container while its image is removed is a removal the engine refuses — so the
/// stop is part of the removal rather than something the operator is told to do
/// first.
///
/// The tier that removes the containers stops them by removing them, which is the
/// same Compose invocation and one step rather than two.
async fn stopping(ctx: &Ctx, tier: Tier, went: &mut Went) {
    let action = if tier == Tier::Services {
        Action::Down
    } else {
        Action::Stop(Vec::new())
    };
    let name = if tier == Tier::Services {
        "the containers and the network they were on"
    } else {
        "the running services"
    };

    // Read off what Compose exited with rather than off the call having returned.
    // A lifecycle command answers with a report whichever way the process went, so a
    // stop that failed and one that worked are the same `Ok` here — and a removal
    // that read the first as the second would carry on removing a running service's
    // files.
    let outcome = match crate::app::engine::lifecycle(ctx, &[], &action).await {
        Err(problem) => Err(problem.summary.clone()),
        Ok(Outcome::Lifecycle(report)) if report.status.unwrap_or_default() == 0 => Ok(()),
        Ok(_) => Err(format!(
            "`docker compose {}` did not succeed",
            action.name()
        )),
    };
    went.step(
        name,
        format!(
            "docker compose --project-name {} {}",
            ctx.settings.project,
            action.name()
        ),
        outcome,
    );
}

/// Remove the images the manifest said would go.
///
/// Only those. An image another project is standing on is on the list as kept, and
/// asking the engine to remove it would either fail or take that project's stack
/// down — which is why the decision is made where it can be read rather than here.
async fn pulled(ctx: &Ctx, manifest: &Manifest, went: &mut Went) {
    for item in manifest
        .items
        .iter()
        .filter(|item| item.sort == Sort::Image && item.goes())
    {
        let argv = vec![
            "docker".to_owned(),
            "image".to_owned(),
            "rm".to_owned(),
            item.name.clone(),
        ];
        let outcome = ran(ctx, &argv).await;
        went.step(&item.name, argv.join(" "), outcome);
    }
}

/// Run one program and turn what it said into an outcome.
///
/// A non-zero exit keeps whatever the program wrote, because that sentence is what
/// the operator needs in order to finish the step by hand.
async fn ran(ctx: &Ctx, argv: &[String]) -> Result<(), String> {
    match ctx.runner.run(argv).await {
        Ok(output) if output.status == Some(0) => Ok(()),
        Ok(output) => Err(said(&output)),
        Err(failure) => Err(failure.to_string()),
    }
}

/// What a program that failed said, or the fact that it said nothing.
fn said(output: &crate::ports::process::Output) -> String {
    let spoken = output.stderr.trim();
    if spoken.is_empty() {
        return format!("it exited {:?} and said nothing", output.status);
    }
    spoken.to_owned()
}

/// Remove the paths the manifest said would go.
///
/// One attempt per tree. A manifest names the directory that is removed *and* each
/// thing inside it, because an operator is owed both — and asking the platform to
/// remove a file inside a directory that has just gone would report nineteen
/// failures about one removal that worked. So anything beneath a path already
/// attempted shares that path's outcome and is not asked about again.
///
/// The data location itself is on the list only where everything beneath it is the
/// stack's, so a tree with anything of the operator's in it is removed directory by
/// directory or not at all.
async fn paths(ctx: &Ctx, manifest: &Manifest, went: &mut Went) {
    let mut attempted: Vec<std::path::PathBuf> = Vec::new();
    for item in manifest
        .items
        .iter()
        .filter(|item| item.sort == Sort::Path && item.goes())
    {
        let at = Path::new(&item.name);
        if attempted.iter().any(|tree| at.starts_with(tree)) {
            continue;
        }
        attempted.push(at.to_path_buf());
        let outcome = ctx
            .eraser
            .erase(at)
            .await
            .map_err(|fault| fault.message.clone());
        went.step(&item.name, by_hand(ctx.environment, at), outcome);
    }
}

/// How to remove a path by hand on this platform.
///
/// Never with elevated rights. A directory lemonfiber wrote is one the account that
/// ran lemonfiber owns, and a permission error on it means something else is true —
/// a different account created it, or the volume is mounted read-only — which
/// escalating would paper over rather than fix.
fn by_hand(environment: Environment, at: &Path) -> String {
    match environment {
        Environment::Windows => format!(
            "Remove-Item -Recurse -Force '{}' — as the account that owns it",
            at.display()
        ),
        Environment::MacOs
        | Environment::LinuxNative
        | Environment::LinuxDesktop
        | Environment::Unsupported => {
            format!("rm -rf '{}' — as the account that owns it", at.display())
        }
    }
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::{by_hand, destroyed, said, Went};
    use crate::platform::Environment;
    use crate::ports::process::Output;
    use crate::uninstall::{Item, Manifest, Sort, Tier};

    /// A manifest holding exactly these lines, since nothing else here is read.
    fn holding(items: Vec<Item>) -> Manifest {
        Manifest {
            tier: Tier::Configuration,
            removes: String::new(),
            keeps: String::new(),
            items,
            bytes: 0,
            foreign: Vec::new(),
            volume: None,
            coming: Vec::new(),
            outside: Vec::new(),
            backup: None,
            confidence: crate::uninstall::Confidence::whole(),
            agreement: String::new(),
        }
    }

    /// A line naming a path, with whether it holds a credential and whether it goes.
    fn path(name: &str, secret: bool, kept: Option<&str>) -> Item {
        Item {
            name: name.to_owned(),
            sort: Sort::Path,
            what: "a path".to_owned(),
            bytes: Some(1),
            kept: kept.map(str::to_owned),
            secret,
        }
    }

    #[test]
    fn what_is_said_destroyed_is_what_held_a_credential_and_went() {
        let manifest = holding(vec![
            path("/cfg/.env", true, None),
            path("/cfg/journal.jsonl", false, None),
            path("/cfg/admission.json", true, Some("could not be confirmed")),
        ]);

        assert_eq!(destroyed(&manifest), vec!["/cfg/.env".to_owned()]);
    }

    /// A run that destroyed no credential says so by naming none, rather than by
    /// naming everything it touched.
    #[test]
    fn a_removal_that_took_no_credential_names_none() {
        assert!(destroyed(&holding(vec![path("/cfg/journal.jsonl", false, None)])).is_empty());
    }

    #[test]
    fn a_step_that_worked_is_recorded_apart_from_one_that_did_not() {
        let mut went = Went::default();
        went.step("/cfg", "rm -rf /cfg".to_owned(), Ok(()));
        went.step(
            "/data",
            "rm -rf /data".to_owned(),
            Err("permission denied".to_owned()),
        );

        assert_eq!(went.gone, vec!["/cfg".to_owned()]);
        assert_eq!(went.left.len(), 1);
        let left = went.left.first().cloned();
        assert_eq!(
            left.as_ref().map(|left| left.why.clone()),
            Some("permission denied".to_owned())
        );
        assert_eq!(
            left.map(|left| left.by_hand),
            Some("rm -rf /data".to_owned())
        );
    }

    #[test]
    fn a_program_that_failed_keeps_its_own_words() {
        assert_eq!(
            said(&Output {
                status: Some(1),
                stdout: String::new(),
                stderr: "  image is in use by a container\n".to_owned(),
            }),
            "image is in use by a container"
        );
    }

    /// A program that failed silently is still reported, because "it did not work
    /// and said nothing" is what the operator is owed rather than an empty string.
    #[test]
    fn a_program_that_failed_silently_still_reports_that_it_failed() {
        let quiet = said(&Output {
            status: Some(137),
            stdout: String::new(),
            stderr: "   ".to_owned(),
        });

        assert!(quiet.contains("137"), "{quiet}");
        assert!(quiet.contains("said nothing"), "{quiet}");
    }

    /// The instruction is per-platform, and on no platform does it escalate.
    #[test]
    fn no_instruction_for_lemonfibers_own_files_asks_for_administrative_rights() {
        let platforms = [
            Environment::MacOs,
            Environment::LinuxNative,
            Environment::LinuxDesktop,
            Environment::Windows,
            Environment::Unsupported,
        ];
        assert_eq!(platforms.len(), 5);

        for environment in platforms {
            let said = by_hand(environment, Path::new("/home/op/.config/lemonfiber"));
            assert!(
                !said.contains("sudo") && !said.contains("Administrator"),
                "{environment:?} is told to escalate: {said}"
            );
            assert!(
                said.contains("owns it"),
                "{environment:?} is not told whose account to use: {said}"
            );
            assert!(said.contains("/home/op/.config/lemonfiber"), "{said}");
        }
    }

    #[test]
    fn windows_is_given_a_windows_command_and_the_rest_are_not() {
        assert!(by_hand(Environment::Windows, Path::new("/a")).starts_with("Remove-Item"));
        assert!(by_hand(Environment::MacOs, Path::new("/a")).starts_with("rm -rf"));
    }
}
