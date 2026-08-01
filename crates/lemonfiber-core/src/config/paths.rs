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

    /// The setup wizard's saved progress, so quitting mid-setup resumes rather
    /// than restarts. The one thing setup writes before the operator confirms.
    #[must_use]
    pub fn setup_progress(&self) -> PathBuf {
        self.config.join("setup-progress.json")
    }

    /// The expected-state baseline: what seeding last wrote into each service, so a
    /// later run can tell an operator's edit from lemonfiber's own value. Unlike the
    /// journal it persists across runs — it is the only memory of what was written.
    #[must_use]
    pub fn baseline(&self) -> PathBuf {
        self.config.join("baseline.json")
    }

    /// The materialised stack — compose files written where Compose can read
    /// them.
    #[must_use]
    pub fn stack(&self) -> PathBuf {
        self.data.join("stack")
    }

    /// The record of what lemonfiber last wrote to the stack directory — a checksum
    /// per file, so a later run tells an operator's edit from a version it has not
    /// upgraded yet. Kept with configuration, not beside the stack it describes: it
    /// is the memory of what was written, which losing would let an edit be
    /// overwritten, so it belongs with what a backup keeps.
    #[must_use]
    pub fn materialised(&self) -> PathBuf {
        self.config.join("materialised.json")
    }

    /// The baseline the storage check records, to notice when hardlinks stop
    /// working. Regenerable — losing it costs one run's history, nothing more.
    #[must_use]
    pub fn storage_state(&self) -> PathBuf {
        self.data.join("storage-state.json")
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
        let config: Vec<PathBuf> = vec![
            paths.env_file(),
            paths.journal(),
            paths.setup_progress(),
            paths.baseline(),
            paths.materialised(),
        ];
        let data: Vec<PathBuf> = vec![
            paths.stack(),
            paths.service_config(),
            paths.backups(),
            paths.storage_state(),
        ];

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
    fn the_baseline_sits_beside_the_env_file() {
        // Seeding derives where it writes the baseline from the environment file it is
        // handed — `env_file.with_file_name("baseline.json")` — while a backup captures
        // the configuration directory whole, and its restore-survival test checks
        // against `baseline()`. The two derivations must land on the same file, or an
        // adopted baseline would be written somewhere a restore does not carry. This
        // ties them: the layout's own `baseline()` is exactly what the seed's formula
        // produces from `env_file()`.
        let paths = paths();
        assert_eq!(
            paths.env_file().with_file_name("baseline.json"),
            paths.baseline()
        );
    }

    #[test]
    fn the_materialised_record_sits_beside_the_env_file() {
        // A lifecycle command derives where it keeps the record from the environment
        // file it is handed — `env_file.with_file_name("materialised.json")` — the same
        // way seeding derives the baseline's. This ties that formula to the layout's
        // own `materialised()`, so the two cannot drift onto different files.
        let paths = paths();
        assert_eq!(
            paths.env_file().with_file_name("materialised.json"),
            paths.materialised()
        );
    }

    #[test]
    fn every_location_is_distinct() {
        let paths = paths();
        let all = [
            paths.env_file(),
            paths.journal(),
            paths.setup_progress(),
            paths.baseline(),
            paths.materialised(),
            paths.stack(),
            paths.service_config(),
            paths.backups(),
            paths.storage_state(),
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
