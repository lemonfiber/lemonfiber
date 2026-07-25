//! Where lemonfiber keeps things.
//!
//! The *layout* is here — which directory holds the environment file, the
//! materialised stack, the per-service configuration, the backups. Where the
//! two base directories are is the surface's problem, because finding them means
//! asking the operating system and there is nothing to get wrong about it that a
//! test could catch.
//!
//! Splitting it there keeps the part that can be wrong pure: [`Paths::rooted`]
//! takes the bases and is a function over them, so the whole layout is
//! exercisable against a temporary directory on any platform.

use std::path::{Path, PathBuf};

/// Every location lemonfiber reads or writes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Paths {
    config: PathBuf,
    data: PathBuf,
}

impl Paths {
    /// The layout beneath a configuration base and a data base.
    ///
    /// The two are separate because they mean different things to the operating
    /// system and to the operator: configuration is small, hand-editable and
    /// worth backing up, and the data directory holds things that can be
    /// regenerated.
    #[must_use]
    pub fn rooted(config: &Path, data: &Path) -> Self {
        Self {
            config: config.join(crate::PRODUCT),
            data: data.join(crate::PRODUCT),
        }
    }

    /// The directory holding configuration.
    #[must_use]
    pub fn config_dir(&self) -> &Path {
        &self.config
    }

    /// The directory holding everything that can be regenerated.
    #[must_use]
    pub fn data_dir(&self) -> &Path {
        &self.data
    }

    /// The environment file handed to Compose.
    #[must_use]
    pub fn env_file(&self) -> PathBuf {
        self.config.join(".env")
    }

    /// The change journal, so a write can be undone.
    #[must_use]
    pub fn journal(&self) -> PathBuf {
        self.config.join("journal.jsonl")
    }

    /// The materialised stack — compose files written where Compose can read
    /// them.
    #[must_use]
    pub fn stack(&self) -> PathBuf {
        self.data.join("stack")
    }

    /// The per-service configuration directories the containers mount.
    #[must_use]
    pub fn service_config(&self) -> PathBuf {
        self.data.join("config")
    }

    /// Where backup archives are written.
    #[must_use]
    pub fn backups(&self) -> PathBuf {
        self.data.join("backups")
    }
}

#[cfg(test)]
mod tests {
    use std::path::{Path, PathBuf};

    use super::Paths;

    fn paths() -> Paths {
        Paths::rooted(
            Path::new("/home/op/.config"),
            Path::new("/home/op/.local/share"),
        )
    }

    #[test]
    fn everything_lives_under_a_directory_named_for_the_product() {
        let paths = paths();
        assert_eq!(paths.config_dir(), Path::new("/home/op/.config/lemonfiber"));
        assert_eq!(
            paths.data_dir(),
            Path::new("/home/op/.local/share/lemonfiber")
        );
    }

    #[test]
    fn configuration_and_regenerable_data_are_kept_apart() {
        let paths = paths();
        let config: Vec<PathBuf> = vec![paths.env_file(), paths.journal()];
        let data: Vec<PathBuf> = vec![paths.stack(), paths.service_config(), paths.backups()];

        // The failure message uses an inline capture rather than a call such as
        // `path.display()`: an argument expression only evaluates when the
        // assertion fails, so it is a line no passing test can cover.
        for path in &config {
            assert!(
                path.starts_with(paths.config_dir()),
                "{path:?} is not configuration"
            );
        }
        for path in &data {
            assert!(
                path.starts_with(paths.data_dir()),
                "{path:?} is not regenerable"
            );
        }
    }

    #[test]
    fn every_location_is_distinct() {
        let paths = paths();
        let all = [
            paths.env_file(),
            paths.journal(),
            paths.stack(),
            paths.service_config(),
            paths.backups(),
        ];
        for (index, path) in all.iter().enumerate() {
            for other in all.iter().skip(index + 1) {
                assert_ne!(path, other, "two things share a location");
            }
        }
    }

    #[test]
    fn the_layout_does_not_depend_on_where_the_bases_are() {
        let elsewhere = Paths::rooted(Path::new("/tmp/a"), Path::new("/tmp/b"));
        assert_eq!(elsewhere.env_file(), Path::new("/tmp/a/lemonfiber/.env"));
        assert_eq!(elsewhere.stack(), Path::new("/tmp/b/lemonfiber/stack"));
    }
}
