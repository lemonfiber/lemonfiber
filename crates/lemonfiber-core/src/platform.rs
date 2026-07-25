//! Which of the four environments this is, decided in one place.
//!
//! Three operating systems but four environments, because Linux has two Docker
//! deployments that behave differently: file ownership is real under Engine and
//! mapped under Desktop, so the same setting is load-bearing in one and
//! cosmetic in the other.
//!
//! Everything else asks this module rather than testing the platform itself.
//! Conditional compilation sprinkled through call sites is how a codebase
//! becomes impossible to exercise on any single machine — here, all four
//! environments are reachable from one laptop because [`Environment::resolve`]
//! is a pure function over inputs a test can supply.

use serde::Serialize;

/// The operating system this build targets.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum HostOs {
    /// macOS.
    MacOs,
    /// Any Linux distribution.
    Linux,
    /// Windows.
    Windows,
    /// Something else, which lemonfiber does not support.
    Other,
}

/// The operating system this binary was built for.
///
/// A constant rather than a runtime test, so the arms that cannot apply are not
/// compiled at all.
#[cfg(target_os = "macos")]
pub const HOST_OS: HostOs = HostOs::MacOs;

/// The operating system this binary was built for.
#[cfg(target_os = "linux")]
pub const HOST_OS: HostOs = HostOs::Linux;

/// The operating system this binary was built for.
#[cfg(target_os = "windows")]
pub const HOST_OS: HostOs = HostOs::Windows;

/// The operating system this binary was built for.
#[cfg(not(any(target_os = "macos", target_os = "linux", target_os = "windows")))]
pub const HOST_OS: HostOs = HostOs::Other;

/// One of the four environments lemonfiber supports.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Environment {
    /// macOS with Docker Desktop.
    MacOs,
    /// Linux running Docker Engine directly.
    LinuxNative,
    /// Linux running Docker Desktop.
    LinuxDesktop,
    /// Windows with Docker Desktop over WSL2.
    Windows,
    /// A platform lemonfiber does not support.
    Unsupported,
}

impl Environment {
    /// Decide the environment from the build target and what the daemon says.
    ///
    /// `desktop` is whether the engine reports itself as Docker Desktop. It is
    /// an argument rather than something read here, so a test can reach all four
    /// environments without four machines.
    #[must_use]
    pub const fn resolve(host: HostOs, desktop: bool) -> Self {
        match host {
            HostOs::MacOs => Self::MacOs,
            HostOs::Windows => Self::Windows,
            HostOs::Linux if desktop => Self::LinuxDesktop,
            HostOs::Linux => Self::LinuxNative,
            HostOs::Other => Self::Unsupported,
        }
    }

    /// Whether file ownership on the data volume is real rather than mapped.
    ///
    /// True on Linux with Docker Engine and nowhere else. Asking an operator for
    /// a user id where it changes nothing observable is a question that cannot
    /// be answered meaningfully, which is worse than not asking.
    #[must_use]
    pub const fn ownership_is_real(self) -> bool {
        matches!(self, Self::LinuxNative)
    }

    /// Whether hardware transcoding is reachable from inside a container.
    ///
    /// True on Linux, where device passthrough works, and false where the
    /// engine runs in a virtual machine that cannot reach the encoder.
    #[must_use]
    pub const fn can_transcode_in_docker(self) -> bool {
        matches!(self, Self::LinuxNative | Self::LinuxDesktop)
    }

    /// Whether the engine resolves `host.docker.internal` without being told to.
    ///
    /// It is a Docker Desktop convenience. Without it, a service reaching a
    /// host-run Jellyfin fails with nothing in any log explaining why.
    #[must_use]
    pub const fn resolves_host_gateway(self) -> bool {
        !matches!(self, Self::LinuxNative | Self::Unsupported)
    }
}

#[cfg(test)]
mod tests {
    use super::{Environment, HostOs, HOST_OS};

    #[test]
    fn linux_splits_on_whether_the_daemon_is_desktop() {
        assert_eq!(
            Environment::resolve(HostOs::Linux, false),
            Environment::LinuxNative
        );
        assert_eq!(
            Environment::resolve(HostOs::Linux, true),
            Environment::LinuxDesktop
        );
    }

    #[test]
    fn the_other_platforms_do_not_depend_on_the_daemon() {
        for desktop in [true, false] {
            assert_eq!(
                Environment::resolve(HostOs::MacOs, desktop),
                Environment::MacOs
            );
            assert_eq!(
                Environment::resolve(HostOs::Windows, desktop),
                Environment::Windows
            );
            assert_eq!(
                Environment::resolve(HostOs::Other, desktop),
                Environment::Unsupported
            );
        }
    }

    #[test]
    fn ownership_is_real_only_where_docker_runs_without_a_virtual_machine() {
        assert!(Environment::LinuxNative.ownership_is_real());
        for environment in [
            Environment::MacOs,
            Environment::LinuxDesktop,
            Environment::Windows,
            Environment::Unsupported,
        ] {
            assert!(!environment.ownership_is_real());
        }
    }

    #[test]
    fn transcoding_in_docker_is_a_linux_capability() {
        assert!(Environment::LinuxNative.can_transcode_in_docker());
        assert!(Environment::LinuxDesktop.can_transcode_in_docker());
        for environment in [
            Environment::MacOs,
            Environment::Windows,
            Environment::Unsupported,
        ] {
            assert!(!environment.can_transcode_in_docker());
        }
    }

    #[test]
    fn only_native_linux_has_to_be_told_about_the_host_gateway() {
        assert!(!Environment::LinuxNative.resolves_host_gateway());
        assert!(Environment::MacOs.resolves_host_gateway());
        assert!(Environment::LinuxDesktop.resolves_host_gateway());
        assert!(Environment::Windows.resolves_host_gateway());
        assert!(!Environment::Unsupported.resolves_host_gateway());
    }

    #[test]
    fn this_build_targets_a_platform_lemonfiber_supports() {
        assert_ne!(HOST_OS, HostOs::Other);
    }
}
