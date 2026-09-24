//! Whether the stack would actually come back, rather than whether it was asked to.
//!
//! "Start on boot" is three different arrangements wearing one name. On Linux with
//! Docker Engine the distribution has already enabled the daemon at boot, so the
//! containers' own restart policies are the whole of it. On macOS and Windows the
//! daemon is Docker Desktop, which **does not open at login by default** — so the
//! machine restarts after an overnight update and nothing comes back, with no error
//! and nothing in any log the operator would think to look at.
//!
//! That setting belongs to Docker Desktop. lemonfiber can read it and cannot write
//! it, and the whole point of this check is that the difference is stated rather than
//! papered over: where the prerequisite can be confirmed it is confirmed, where it is
//! confirmed absent it is named as the single most common cause of this failure, and
//! where it can be neither it is reported as `enabled-unverified` — configured on our
//! side, unproven on the other.
//!
//! **The unverified verdict is the load-bearing one.** An operator who believes they
//! have autostart and does not is worse off than one who knows they have none: the
//! first finds out weeks later, from a household asking why nothing has downloaded
//! since Tuesday. A "could not check" that rendered as a pass would produce exactly
//! that belief, which is why it is a verdict of its own here rather than a severity.
//!
//! Reading the setting is a read of a file Docker Desktop wrote, in the operator's
//! own home directory, and every candidate location is tried rather than one chosen
//! from the build target — the setting has moved between Docker Desktop versions and
//! sits in a different place on each platform, and a check that knew only one of them
//! would report `enabled-unverified` on a machine where the answer was sitting on
//! disk.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use async_trait::async_trait;

use super::{Category, Check, Finding, Verdict};
use crate::error::{Problem, Remedy, Severity, State};
use crate::platform::Environment;
use crate::ports::{FileSystem, Runner};

pub(crate) use crate::error::codes::env::ENGINE_NOT_AT_BOOT;

/// The check this reports under, named once so a finding and an answer to it cannot
/// drift apart on a rename.
///
/// Public because one caller has to be able to *leave it out*. Setup's preflight asks
/// this whole family and stops before a single question where the answer is broken or
/// undetermined — which is right about an engine that cannot be reached and wrong
/// about a Docker Desktop setting that could not be read, since the second says
/// nothing about whether this machine can run the stack today. A name it can match on
/// is the honest way to say that; the alternative is this check quietly never
/// answering `enabled-unverified`, which is the one answer it exists to give.
pub const CHECK: &str = "environment.autostart";

/// What the operator reads on the line above the verdict.
const TITLE: &str = "The stack comes back after a restart";

/// What the operator asked for, weighed against what the platform can confirm.
///
/// Three of the five states the feature names. `degraded` and `failed-boot` are
/// about a boot that has already happened and are not this check's to give: nothing
/// here watches a start, so claiming either would be claiming to have seen something
/// this never looked at.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Standing {
    /// Nothing starts automatically, because nobody asked for it to.
    Disabled,
    /// Asked for, and the platform prerequisite is confirmed present.
    Enabled,
    /// Asked for, and a platform prerequisite is not confirmed.
    EnabledUnverified,
}

impl Standing {
    /// The word the feature names this state by.
    ///
    /// Hyphenated because these are the words an operator reads and the requirement
    /// writes, and a second spelling would be a second vocabulary for one set of
    /// facts.
    #[must_use]
    pub const fn named(self) -> &'static str {
        match self {
            Self::Disabled => "disabled",
            Self::Enabled => "enabled",
            Self::EnabledUnverified => "enabled-unverified",
        }
    }
}

/// What was found out about the prerequisite this platform's autostart rests on.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Prerequisite {
    /// It is in place, and that was established rather than assumed.
    InPlace {
        /// What was read, in the words the operator can go and check themselves.
        evidence: String,
    },
    /// It is confirmed absent, which is the common cause of this whole failure.
    Absent {
        /// What is not set, and where it is set.
        what: String,
        /// How to set it, since this is not something lemonfiber can write.
        how: String,
    },
    /// It could not be established either way.
    Unconfirmed {
        /// Why not, in terms the operator can act on.
        why: String,
        /// What to do about the uncertainty.
        how: String,
    },
}

/// Whether the stack would actually come back after a restart of this machine.
pub struct AutostartCheck {
    filesystem: Arc<dyn FileSystem>,
    runner: Arc<dyn Runner>,
    environment: Environment,
    wanted: bool,
    home: Option<PathBuf>,
}

impl AutostartCheck {
    /// A check over the operator's answer and this machine's own arrangements.
    ///
    /// `wanted` is the answer as it was given rather than anything derived from it:
    /// what this check adds is the other half, and a check that decided for itself
    /// whether autostart had been asked for would be deciding the thing it reports.
    #[must_use]
    pub fn new(
        filesystem: Arc<dyn FileSystem>,
        runner: Arc<dyn Runner>,
        environment: Environment,
        wanted: bool,
        home: Option<PathBuf>,
    ) -> Self {
        Self {
            filesystem,
            runner,
            environment,
            wanted,
            home,
        }
    }
}

#[async_trait]
impl Check for AutostartCheck {
    fn category(&self) -> Category {
        Category::Environment
    }

    async fn run(&self) -> Vec<Finding> {
        ran(self).await
    }
}

/// What this machine would do at its next restart, and what is known about it.
async fn ran(check: &AutostartCheck) -> Vec<Finding> {
    if !check.wanted {
        return vec![Finding::in_category(
            Category::Environment,
            CHECK,
            TITLE,
            Verdict::Skipped {
                reason: format!(
                    "{}: starting on boot was not asked for, so nothing here is expected to \
                     bring the stack back",
                    Standing::Disabled.named()
                ),
            },
        )];
    }
    vec![Finding::in_category(
        Category::Environment,
        CHECK,
        TITLE,
        verdict(prerequisite(check).await),
    )]
}

/// The verdict for what was found out about the prerequisite.
///
/// Three findings for three genuinely different situations, and the middle one is
/// the reason the check exists at all: a prerequisite nobody could read is not a
/// prerequisite that is in place, and reporting it as one would manufacture exactly
/// the belief this feature is written to prevent.
fn verdict(found: Prerequisite) -> Verdict {
    match found {
        Prerequisite::InPlace { evidence } => Verdict::Pass {
            note: Some(format!("{}: {evidence}", Standing::Enabled.named())),
        },
        Prerequisite::Absent { what, how } => Verdict::Warn(
            Problem::new(
                ENGINE_NOT_AT_BOOT,
                Severity::Warning,
                what,
                "The containers carry a restart policy, and a restart policy only brings a \
                 container back once the engine behind it is running. Nothing starts the engine, \
                 so after the next restart of this machine the stack is simply not there — no \
                 error, no notification, and nothing in any log you would think to look at.",
                Remedy::new(how),
            )
            .in_state(State::Guided),
        ),
        Prerequisite::Unconfirmed { why, how } => Verdict::Unverified {
            reason: format!(
                "{}: {why}, so whether the stack would come back is not established",
                Standing::EnabledUnverified.named()
            ),
            remedy: Remedy::new(how),
        },
    }
}

/// What this platform's autostart actually rests on, and whether it is there.
async fn prerequisite(check: &AutostartCheck) -> Prerequisite {
    match check.environment {
        // The distribution enabled the daemon at boot when it installed it, so the
        // only thing left to establish is that it is still enabled.
        Environment::LinuxNative => daemon_enabled(check).await,
        Environment::MacOs | Environment::LinuxDesktop | Environment::Windows => {
            desktop_at_login(check).await
        }
        Environment::Unsupported => Prerequisite::Unconfirmed {
            why: "lemonfiber does not know how this platform starts anything at login".to_owned(),
            how: "Arrange the container engine to start at login with whatever this system uses, \
                  then start the stack once by hand to confirm it comes back"
                .to_owned(),
        },
    }
}

/// The command that asks whether the service manager starts the daemon at boot.
///
/// The system service rather than a user one, because that is where a distribution
/// installs it — and asking does not need administrative rights, which is what makes
/// it a check rather than a prompt.
fn is_enabled() -> Vec<String> {
    ["systemctl", "is-enabled", "docker"]
        .map(str::to_owned)
        .to_vec()
}

/// Whether the Docker service is enabled at boot, as the service manager says.
async fn daemon_enabled(check: &AutostartCheck) -> Prerequisite {
    let Ok(output) = check.runner.run(&is_enabled()).await else {
        return Prerequisite::Unconfirmed {
            why: "the service manager could not be asked whether the Docker service starts at \
                  boot"
                .to_owned(),
            how: "Run `systemctl is-enabled docker` yourself to see what it says".to_owned(),
        };
    };
    match output.stdout.trim() {
        // Enabled by a unit file, or by a runtime link that lasts until the next
        // boot. The second is not the same promise as the first, and is named rather
        // than folded in, because a link that disappears at the next boot is exactly
        // the arrangement somebody believes is permanent.
        "enabled" => Prerequisite::InPlace {
            evidence: "the Docker service is enabled at boot, so the containers' restart \
                       policies bring the stack back"
                .to_owned(),
        },
        "enabled-runtime" => Prerequisite::Absent {
            what: "The Docker service is enabled only until the next restart".to_owned(),
            how: "Run `sudo systemctl enable docker` so it survives a restart".to_owned(),
        },
        "disabled" | "masked" | "masked-runtime" => Prerequisite::Absent {
            what: "The Docker service is not enabled at boot".to_owned(),
            how: "Run `sudo systemctl enable docker`, then restart this machine to confirm the \
                  stack comes back"
                .to_owned(),
        },
        said => Prerequisite::Unconfirmed {
            why: format!(
                "the service manager answered {} when asked whether Docker starts at boot",
                quoted(said)
            ),
            how: "Run `systemctl is-enabled docker` yourself to see what it says".to_owned(),
        },
    }
}

/// What the service manager said, or a word for its having said nothing.
///
/// Written out rather than interpolated bare, because an empty answer rendered into
/// a sentence leaves a gap the operator reads as a missing word rather than as the
/// finding itself.
fn quoted(said: &str) -> String {
    if said.is_empty() {
        return "nothing at all".to_owned();
    }
    format!("`{said}`")
}

/// Where Docker Desktop keeps the setting, relative to the operator's home.
///
/// Every candidate rather than the one this build's platform uses, and all of them
/// tried: the file moved between Docker Desktop versions and sits somewhere
/// different on each platform, so a check that knew one location would answer
/// `enabled-unverified` on a machine where the answer was on disk the whole time.
const SETTINGS: [&str; 6] = [
    "Library/Group Containers/group.com.docker/settings-store.json",
    "Library/Group Containers/group.com.docker/settings.json",
    ".docker/desktop/settings-store.json",
    ".docker/desktop/settings.json",
    "AppData/Roaming/Docker/settings-store.json",
    "AppData/Roaming/Docker/settings.json",
];

/// Whether Docker Desktop is set to open at login, as its own settings file says.
async fn desktop_at_login(check: &AutostartCheck) -> Prerequisite {
    let Some(home) = check.home.as_deref() else {
        return Prerequisite::Unconfirmed {
            why: "this machine would not say where your home directory is, so Docker Desktop's \
                  own settings could not be found"
                .to_owned(),
            how: TURN_IT_ON.to_owned(),
        };
    };
    match read_setting(check, home).await {
        Some(true) => Prerequisite::InPlace {
            evidence: "Docker Desktop is set to open at login, so the engine is running by the \
                       time the containers' restart policies are read"
                .to_owned(),
        },
        Some(false) => Prerequisite::Absent {
            what: "Docker Desktop is not set to open at login".to_owned(),
            how: TURN_IT_ON.to_owned(),
        },
        None => Prerequisite::Unconfirmed {
            why: "Docker Desktop's own settings could not be read, and that setting is its own \
                  rather than something lemonfiber writes"
                .to_owned(),
            how: TURN_IT_ON.to_owned(),
        },
    }
}

/// How to set the one thing lemonfiber cannot set.
///
/// Written once, because it is said in all three of the answers that are not a
/// confirmed yes — and an operator who meets it twice should meet the same sentence.
const TURN_IT_ON: &str = "Open Docker Desktop, and under Settings → General turn on \
                          \"Start Docker Desktop when you sign in\"";

/// Read the setting out of whichever settings file this machine has.
async fn read_setting(check: &AutostartCheck, home: &Path) -> Option<bool> {
    for candidate in SETTINGS {
        let answer = check
            .filesystem
            .read(&home.join(candidate))
            .await
            .as_deref()
            .and_then(at_login);
        if answer.is_some() {
            return answer;
        }
    }
    None
}

/// Whether a Docker Desktop settings document says it opens at login.
///
/// Matched without regard to case, because the key is `autoStart` in the settings
/// file and `AutoStart` in the settings store that replaced it, and a reader that
/// knew one spelling would report the other as unreadable.
fn at_login(text: &str) -> Option<bool> {
    let document: serde_json::Value = serde_json::from_str(text).ok()?;
    document
        .as_object()?
        .iter()
        .find(|(key, _)| key.eq_ignore_ascii_case("autostart"))
        .and_then(|(_, value)| value.as_bool())
}

#[cfg(test)]
mod tests;
