//! Constructing the Compose invocation, and nothing else.
//!
//! [`build`] is a pure function over a plan, the settings and the environment.
//! It does not spawn anything, which is what lets every form on every platform
//! be covered by golden files with no daemon present — and it is why a rehearsal
//! and a real run cannot disagree, because they are the same function and the
//! rehearsal simply declines to hand the result to a process.
//!
//! Profiles arrive already resolved and narrowed. Nothing here knows what a
//! service is.

use std::path::Path;

use crate::config::Settings;
use crate::platform::Environment;

use super::closure::Plan;

/// What to do with the profiles in a plan.
///
/// Every variant changes something. Reading what is running — state, stats,
/// logs — goes through the Engine API instead, because observation is streamed
/// and cheap there and spawning a process once a second to watch nineteen
/// services would be both wasteful and visibly jittery.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Action {
    /// Start, detached.
    Up,
    /// Stop and remove.
    Down,
    /// Stop without removing.
    Stop,
    /// Restart named services, leaving the rest alone.
    Restart(Vec<String>),
    /// Fetch newer images without applying them.
    Pull,
    /// Resolve the project and print it, changing nothing.
    Config,
}

impl Action {
    /// What this action is called, for reporting it back.
    #[must_use]
    pub const fn name(&self) -> &'static str {
        match self {
            Self::Up => "up",
            Self::Down => "down",
            Self::Stop => "stop",
            Self::Restart(_) => "restart",
            Self::Pull => "pull",
            Self::Config => "config",
        }
    }

    /// The Compose subcommand and its own arguments.
    pub(crate) fn argv(&self) -> Vec<String> {
        match self {
            Self::Up => vec!["up".to_owned(), "--detach".to_owned()],
            Self::Down => vec!["down".to_owned()],
            Self::Stop => vec!["stop".to_owned()],
            Self::Restart(services) => {
                let mut argv = vec!["restart".to_owned()];
                argv.extend(services.iter().cloned());
                argv
            }
            Self::Pull => vec!["pull".to_owned()],
            Self::Config => vec!["config".to_owned()],
        }
    }
}

/// Build the argument vector for a Compose invocation.
///
/// `stack` is the directory holding `compose.yml`. It is passed as the project
/// directory as well as the compose file's location, because the stack's own
/// fragments resolve their relative paths against the project root — pointing
/// Compose at the file without also naming the directory turns every relative
/// path in every fragment into a path relative to somewhere else.
///
/// `environment` does not currently change the result. It is an argument rather
/// than something to add later because the first thing that depends on it
/// should not also be the change that threads it through every call site.
#[must_use]
pub fn build(
    plan: &Plan,
    settings: &Settings,
    stack: &Path,
    action: &Action,
    environment: Environment,
) -> Vec<String> {
    let _ = environment;

    let mut argv = vec!["docker".to_owned(), "compose".to_owned()];

    argv.push("--project-name".to_owned());
    argv.push(settings.project.clone());

    argv.push("--project-directory".to_owned());
    argv.push(stack.display().to_string());

    if let Some(env_file) = &settings.env_file {
        argv.push("--env-file".to_owned());
        argv.push(env_file.display().to_string());
    }

    argv.push("--file".to_owned());
    argv.push(stack.join("compose.yml").display().to_string());

    for overlay in &settings.overlays {
        argv.push("--file".to_owned());
        argv.push(overlay.display().to_string());
    }

    // Sorted, because the plan holds an ordered set: the same request must
    // produce the same command, or golden files test nothing and a cached
    // Compose project churns for no reason.
    for profile in &plan.profiles {
        argv.push("--profile".to_owned());
        argv.push(profile.clone());
    }

    argv.extend(action.argv());
    argv
}

#[cfg(test)]
mod tests {
    use std::path::{Path, PathBuf};

    use lemonfiber_manifest::Manifest;

    use super::{build, Action, Plan, Settings};
    use crate::config::Protocols;
    use crate::platform::Environment;
    use crate::stack::closure::resolve;

    const STACK: &str = include_str!("../../../../assets/media-stack/stack.toml");

    fn stack_dir() -> &'static Path {
        Path::new("/opt/lemonfiber/stack")
    }

    fn plan(forms: &[&str]) -> Option<Plan> {
        let named: Vec<String> = forms.iter().map(|form| (*form).to_owned()).collect();
        Manifest::from_toml(STACK)
            .ok()
            .and_then(|manifest| resolve(&manifest, &named, Protocols::both()).ok())
    }

    fn line(forms: &[&str], action: &Action, settings: &Settings) -> Option<String> {
        plan(forms)
            .map(|plan| build(&plan, settings, stack_dir(), action, Environment::MacOs).join(" "))
    }

    #[test]
    fn starts_a_form_detached_with_its_profiles() {
        assert_eq!(
            line(&["library"], &Action::Up, &Settings::default()).as_deref(),
            Some(concat!(
                "docker compose --project-name lemonfiber ",
                "--project-directory /opt/lemonfiber/stack ",
                "--file /opt/lemonfiber/stack/compose.yml ",
                "--profile media up --detach"
            ))
        );
    }

    #[test]
    fn the_same_request_always_produces_the_same_command() {
        let first = line(&["tv"], &Action::Up, &Settings::default());
        let second = line(&["tv"], &Action::Up, &Settings::default());
        assert_eq!(first, second);
        assert_eq!(
            first.as_deref().map(|command| command.contains(
                "--profile search --profile subs --profile torrent --profile tv --profile usenet"
            )),
            Some(true),
            "profiles are sorted: {first:?}"
        );
    }

    #[test]
    fn naming_forms_in_a_different_order_produces_the_same_command() {
        assert_eq!(
            line(&["search", "library"], &Action::Up, &Settings::default()),
            line(&["library", "search"], &Action::Up, &Settings::default())
        );
    }

    #[test]
    fn an_environment_file_is_passed_when_one_has_been_written() {
        let settings = Settings {
            env_file: Some(PathBuf::from("/home/op/.config/lemonfiber/.env")),
            ..Settings::default()
        };
        assert_eq!(
            line(&["library"], &Action::Up, &settings)
                .as_deref()
                .map(|command| command
                    .contains("--env-file /home/op/.config/lemonfiber/.env --file")),
            Some(true)
        );
    }

    #[test]
    fn overlays_are_layered_after_the_stack_s_own_file() {
        let settings = Settings {
            overlays: vec![PathBuf::from(
                "/opt/lemonfiber/stack/stacks/compose.storage.nas.yml",
            )],
            ..Settings::default()
        };
        assert_eq!(
            line(&["library"], &Action::Up, &settings)
                .as_deref()
                .map(|command| command.contains(concat!(
                    "--file /opt/lemonfiber/stack/compose.yml ",
                    "--file /opt/lemonfiber/stack/stacks/compose.storage.nas.yml"
                ))),
            Some(true)
        );
    }

    #[test]
    fn each_action_becomes_its_own_subcommand() {
        let settings = Settings::default();
        let ending = |action: &Action| {
            line(&["library"], action, &settings).and_then(|command| {
                command
                    .split("--profile media ")
                    .nth(1)
                    .map(ToOwned::to_owned)
            })
        };

        assert_eq!(ending(&Action::Up).as_deref(), Some("up --detach"));
        assert_eq!(ending(&Action::Down).as_deref(), Some("down"));
        assert_eq!(ending(&Action::Stop).as_deref(), Some("stop"));
        assert_eq!(ending(&Action::Pull).as_deref(), Some("pull"));
        assert_eq!(ending(&Action::Config).as_deref(), Some("config"));
        assert_eq!(
            ending(&Action::Restart(vec!["sonarr".to_owned()])).as_deref(),
            Some("restart sonarr")
        );
        assert_eq!(
            ending(&Action::Restart(Vec::new())).as_deref(),
            Some("restart"),
            "restarting nothing in particular restarts the form"
        );
    }

    #[test]
    fn every_action_reports_the_name_it_runs_under() {
        for (action, name) in [
            (Action::Up, "up"),
            (Action::Down, "down"),
            (Action::Stop, "stop"),
            (Action::Restart(Vec::new()), "restart"),
            (Action::Pull, "pull"),
            (Action::Config, "config"),
        ] {
            assert_eq!(action.name(), name);
            assert_eq!(
                action.argv().first().map(String::as_str),
                Some(name),
                "the name and the subcommand must not drift apart"
            );
        }
    }

    #[test]
    fn the_project_name_is_what_correlates_containers_back_to_services() {
        let settings = Settings {
            project: "housemedia".to_owned(),
            ..Settings::default()
        };
        assert_eq!(
            line(&["library"], &Action::Up, &settings)
                .as_deref()
                .map(|command| command.contains("--project-name housemedia")),
            Some(true)
        );
    }
}
