//! A lemonfiber version, read off a file and compared with the running build.
//!
//! Two things lemonfiber writes carry the version that wrote them — a backup
//! archive's manifest and the settings file — and both are read back by a build
//! that may be older than the one that wrote them. The question each asks is the
//! same one, so it is answered once here rather than twice with two chances to
//! disagree about what `0.14.0-rc.1` comes to.

/// A three-part version, compared to decide whether what was written here can be
/// read by the build that is running.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) struct Version {
    pub(crate) major: u32,
    pub(crate) minor: u32,
    pub(crate) patch: u32,
}

impl Version {
    /// Parse a `major.minor.patch` string, ignoring any pre-release suffix, and
    /// return nothing for anything that is not three numbers.
    ///
    /// Lenient about a trailing `-rc.1` because a release candidate reads like the
    /// release it precedes; strict about the three numbers because a version that
    /// cannot be read is not a guess to make.
    pub(crate) fn parse(text: &str) -> Option<Self> {
        // `split` always yields at least the whole string, so a version with no
        // `-` suffix is its own first segment — `unwrap_or(text)` says that plainly.
        let core = text.split('-').next().unwrap_or(text);
        let mut parts = core.split('.');
        let major = parts.next()?.parse().ok()?;
        let minor = parts.next()?.parse().ok()?;
        let patch = parts.next()?.parse().ok()?;
        if parts.next().is_some() {
            return None;
        }
        Some(Self {
            major,
            minor,
            patch,
        })
    }

    /// Whether `written` names a build strictly newer than `running`, where both
    /// can be read at all.
    ///
    /// Anything unreadable on either side answers `false`. A version this cannot
    /// parse is one nothing can be concluded from, and concluding "newer" from it
    /// would refuse work over a string an operator is free to have typed by hand.
    pub(crate) fn is_newer(written: &str, running: &str) -> bool {
        matches!(
            (Self::parse(written), Self::parse(running)),
            (Some(written), Some(running)) if written > running
        )
    }
}
