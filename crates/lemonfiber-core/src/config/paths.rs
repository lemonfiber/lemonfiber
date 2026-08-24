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

/// The change journal's file name, named once so a caller placing it beside the
/// environment file — as a repair recording what it changed does — cannot place it
/// somewhere the reversal will not look.
pub const JOURNAL: &str = "journal.jsonl";

impl Paths {
    /// The layout beneath a configuration base and a data base.
    ///
    /// The two are separate because they mean different things to the operating
    /// system and to the operator: configuration is small, hand-editable and
    /// worth backing up, and the data directory holds things that can be
    /// regenerated.
    #[must_use]
    pub fn rooted(config: &Path, data: &Path) -> Self {
        Self::at(&config.join(crate::PRODUCT), &data.join(crate::PRODUCT))
    }

    /// The layout beneath the two directories themselves, rather than the bases
    /// they sit in.
    ///
    /// What [`Self::rooted`] resolves to, and the way back for a caller that
    /// already holds the resolved pair — a running command knows where its
    /// environment file and its stack are, and both name a directory this layout
    /// is spelled beneath.
    #[must_use]
    pub fn at(config: &Path, data: &Path) -> Self {
        Self {
            config: config.to_path_buf(),
            data: data.to_path_buf(),
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
        self.config.join(JOURNAL)
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

    /// Where the words this operator has gone and found out about are kept.
    ///
    /// Beside the settings rather than beside the stack, for the same reason the
    /// baseline is: two projects sharing one stack directory are two installations,
    /// and what one operator has been told is not what the other has.
    #[must_use]
    pub fn acknowledged(&self) -> PathBuf {
        self.config.join("acknowledged.json")
    }

    /// The operator's quality choice: which preset is in force, globally and per
    /// media type. Kept with configuration so a backup carries it, and so a later
    /// run applies the same choice rather than falling back to the default.
    #[must_use]
    pub fn quality(&self) -> PathBuf {
        self.config.join("quality.json")
    }

    /// The operator's notification choice: which appetite is in force, and the
    /// individual events they set apart from it. Kept with configuration for the
    /// same reasons the quality choice is — a backup carries it, and a later run
    /// respects the answer rather than falling back to the quiet default.
    #[must_use]
    pub fn notifications(&self) -> PathBuf {
        self.config.join("notifications.json")
    }

    /// The choices the operator answered whose cost was stated to them once —
    /// running with no VPN, or with a provider that forwards no port. Kept with
    /// configuration because it records a decision, and a backup that restored the
    /// stack without it would put every settled question again.
    #[must_use]
    pub fn accepted(&self) -> PathBuf {
        self.config.join("accepted.json")
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
            paths.acknowledged(),
            paths.materialised(),
            paths.quality(),
            paths.notifications(),
            paths.accepted(),
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
    fn the_answered_choices_sit_beside_the_env_file() {
        // The doctor derives where it keeps them from the environment file it is
        // handed — `env_file.with_file_name("accepted.json")` — while a backup
        // captures the configuration directory whole. The two derivations must land
        // on the same file, or a restore would put every settled question again.
        let paths = paths();
        assert_eq!(
            paths.env_file().with_file_name("accepted.json"),
            paths.accepted()
        );
    }

    #[test]
    fn the_quality_choice_sits_beside_the_env_file() {
        // The quality command derives where it keeps the choice from the environment
        // file it is handed — `env_file.with_file_name("quality.json")` — the same way
        // seeding derives the baseline's. This ties that formula to the layout's own
        // `quality()`, so a backup that captures the configuration directory carries it.
        let paths = paths();
        assert_eq!(
            paths.env_file().with_file_name("quality.json"),
            paths.quality()
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
            paths.quality(),
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
    /// Beside the settings, for the same reason the baseline is: what one operator
    /// has been told is not what another has, even sharing a stack directory.
    #[test]
    fn what_has_been_acknowledged_sits_with_the_configuration() {
        let paths = paths();

        let held = paths.acknowledged();

        let where_it_is = held.display().to_string();
        assert_eq!(held.parent(), paths.baseline().parent());
        assert!(held.ends_with("acknowledged.json"), "{where_it_is}");
    }
}
