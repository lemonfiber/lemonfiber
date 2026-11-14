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

/// The name Docker Desktop gives the context it installs on Linux.
const DESKTOP_CONTEXT: &str = "desktop-linux";

/// Where under a home directory Docker Desktop's own socket lives on Linux.
const DESKTOP_SOCKET: &str = "/.docker/desktop/";

/// The name `OrbStack` gives the context it installs.
const ORBSTACK_CONTEXT: &str = "orbstack";

/// Where under a home directory `OrbStack`'s own socket lives.
const ORBSTACK_SOCKET: &str = "/.orbstack/";

/// The operating system this build targets.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
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
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
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
    /// Decide the environment from the build target and the engine it is pointed at.
    ///
    /// `desktop` is whether that engine is Docker Desktop's, as
    /// [`Self::engine_is_desktop`] answers it. It is an argument rather than something
    /// read here, so a test can reach all four environments without four machines.
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

    /// Whether the engine a run is pointed at is Docker Desktop's, from the context it
    /// was found through and the socket it is reached on, where either is one.
    ///
    /// Read off where the engine was found rather than asked of the daemon: Docker
    /// Desktop on Linux installs a context of its own, named `desktop-linux`, whose
    /// endpoint is a socket under `~/.docker/desktop/`, and a run resolves its engine
    /// from those before it does anything. Either one said is Desktop. Anything else —
    /// the machine's own daemon, a socket of Docker Engine's, a remote engine — is not.
    #[must_use]
    pub fn engine_is_desktop(context: Option<&str>, socket: Option<&str>) -> bool {
        context == Some(DESKTOP_CONTEXT) || socket.is_some_and(|at| at.contains(DESKTOP_SOCKET))
    }

    /// Whether the engine a run is pointed at is `OrbStack`'s, read off where the engine was
    /// found as [`Self::engine_is_desktop`] reads Docker Desktop: the context `OrbStack`
    /// installs, named `orbstack`, or a socket under `~/.orbstack/`.
    #[must_use]
    pub fn engine_is_orbstack(context: Option<&str>, socket: Option<&str>) -> bool {
        context == Some(ORBSTACK_CONTEXT) || socket.is_some_and(|at| at.contains(ORBSTACK_SOCKET))
    }

    /// Whether file ownership on the data volume is real rather than mapped.
    ///
    /// True on Linux with Docker Engine and nowhere else. Asking an operator for
    /// a user id where it changes nothing observable is a question that cannot
    /// be answered meaningfully, which is worse than not asking.
    ///
    /// This is the gate the setup wizard uses to decide whether to ask for
    /// `PUID`/`PGID` at all: they are requested only where ownership is
    /// user-visible.
    #[must_use]
    pub const fn ownership_is_real(self) -> bool {
        matches!(self, Self::LinuxNative)
    }

    /// Whether the setup wizard should offer to run the media server natively rather than
    /// in a container.
    ///
    /// Native mode exists to reach a hardware encoder the Docker VM cannot, so it
    /// is offered only where the container cannot transcode and the platform is
    /// one lemonfiber supports — macOS and Windows. On Linux, Docker reaches the
    /// encoder directly, so native mode buys nothing and must not be offered;
    /// offering it there would trade away deployment uniformity for no gain.
    #[must_use]
    pub const fn offers_native_media_server(self) -> bool {
        !self.can_transcode_in_docker() && !matches!(self, Self::Unsupported)
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
    /// host-run media server fails with nothing in any log explaining why.
    #[must_use]
    pub const fn resolves_host_gateway(self) -> bool {
        !matches!(self, Self::LinuxNative | Self::Unsupported)
    }
}

#[cfg(test)]
mod tests;
