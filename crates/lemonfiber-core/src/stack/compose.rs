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
    /// Start named services, detached, leaving the rest alone.
    Start(Vec<String>),
    /// Stop and remove.
    Down,
    /// Stop named services without removing them, leaving the rest alone.
    Stop(Vec<String>),
    /// Stop named services and remove their containers, leaving the rest alone.
    ///
    /// Apart from [`Self::Down`], which takes the whole project with it, and apart
    /// from [`Self::Stop`], which leaves a stopped container where it was. This is
    /// what an install has to reach for when it puts itself back: the document that
    /// declared the service is about to be removed, and a container Compose no longer
    /// knows about is one nothing will ever take down.
    Remove(Vec<String>),
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
            Self::Up | Self::Start(_) => "up",
            Self::Down => "down",
            Self::Stop(_) => "stop",
            Self::Remove(_) => "rm",
            Self::Restart(_) => "restart",
            Self::Pull => "pull",
            Self::Config => "config",
        }
    }

    /// The Compose subcommand and its own arguments.
    ///
    /// `registry` is whether this operator allows images to be fetched. Where they
    /// do not, a start is told never to pull rather than being left to Compose's
    /// default of fetching whatever is missing — the setting has to reach the one
    /// command that would otherwise do it silently, or it is a setting that means
    /// nothing.
    pub(crate) fn argv(&self, registry: bool) -> Vec<String> {
        let starting = || {
            let mut argv = vec!["up".to_owned(), "--detach".to_owned()];
            if !registry {
                argv.push("--pull".to_owned());
                argv.push("never".to_owned());
            }
            argv
        };
        match self {
            Self::Up => starting(),
            Self::Start(services) => fenced(starting(), services),
            Self::Down => vec!["down".to_owned()],
            Self::Stop(services) => fenced(vec!["stop".to_owned()], services),
            // `--stop` because a running container cannot be removed, and `--force`
            // because the question it would otherwise ask is put to a terminal nobody
            // is watching — this runs inside an install that is already failing.
            Self::Remove(services) => fenced(
                vec!["rm".to_owned(), "--force".to_owned(), "--stop".to_owned()],
                services,
            ),
            Self::Restart(services) => fenced(vec!["restart".to_owned()], services),
            Self::Pull => vec!["pull".to_owned()],
            Self::Config => vec!["config".to_owned()],
        }
    }
}

/// A Compose invocation aimed at named services, or at the whole project where none
/// are named.
///
/// Starting, stopping and restarting differ in the words in front and in nothing
/// else, and the part that is easy to get wrong is shared: a `--` fences the service
/// names off from option parsing, so one that begins with a dash is treated as a name
/// and not as a flag.
fn fenced(mut argv: Vec<String>, services: &[String]) -> Vec<String> {
    if !services.is_empty() {
        argv.push("--".to_owned());
        argv.extend(services.iter().cloned());
    }
    argv
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

    let mut argv = vec!["docker".to_owned()];

    // The endpoint is named on the invocation rather than left to whatever the shell
    // exported. Compose is a subprocess and would inherit the environment, which is
    // how it came to obey a remote context that the Engine API client silently
    // ignored — the writes went to the server and the reads came from the laptop.
    // Naming it here means both halves read the same field of the same settings, so
    // there is no arrangement of environment and configuration that can separate
    // them. Nothing is added where nothing was chosen, because a guessed endpoint
    // would override the machine's own conventions with this build's idea of them.
    if let Some(endpoint) = settings.docker.endpoint() {
        argv.push("--host".to_owned());
        argv.push(endpoint.to_owned());
    }

    argv.push("compose".to_owned());

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

    // Then the installed plugins' own documents, joined here against the very
    // directory this invocation names as the project root. Resolved per invocation
    // rather than carried resolved, because an operator's own stack and the embedded
    // one are different roots and a path settled anywhere else would be right for one
    // of them and silently wrong for the other.
    //
    // After the operator's overlay, because Compose takes the later file as the one
    // that wins and a stranger's plugin is not entitled to override a choice the
    // operator made.
    for document in crate::plugin::documents(&settings.plugins, stack) {
        argv.push("--file".to_owned());
        argv.push(document.display().to_string());
    }

    // Sorted, because the plan holds an ordered set: the same request must
    // produce the same command, or golden files test nothing and a cached
    // Compose project churns for no reason.
    for profile in &plan.profiles {
        argv.push("--profile".to_owned());
        argv.push(profile.clone());
    }

    argv.extend(action.argv(settings.reaching.allows(crate::config::REACH_REGISTRY_KEY)));
    argv
}

#[cfg(test)]
mod tests;
