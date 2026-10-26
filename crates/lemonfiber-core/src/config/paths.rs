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
pub(crate) const JOURNAL: &str = "journal.jsonl";

/// The file name of the last entry of the request gate's record a diagnosis read,
/// named once because a running check reaches it beside the environment file rather
/// than through this layout.
pub const GATE_READ: &str = "gate-read.json";

/// The install record's file name, named once for the reason the journal's is: the
/// command that writes it reaches it beside the environment file rather than through
/// this layout, so the two spellings have to be one.
pub const PLUGINS: &str = "plugins.json";

/// The file name of the record of every answer an installed plugin's adapter gave
/// outside its contract, named once for the reason the install record's is.
pub(crate) const NONCONFORMING: &str = "nonconforming.json";

/// The directory kept for each installed plugin: its manifest and the starting files of
/// its run proofs.
pub(crate) const STARTING_KEPT: &str = "plugin-starting";

/// The file name of the verdicts the doctor keeps for installed plugins' run proofs.
pub(crate) const RUN_VERDICTS: &str = "run-proofs.json";

/// The file name of the record of the newest catalogue index this machine verified,
/// named once for the reason the install record's is.
pub(crate) const CATALOGUE: &str = "catalogue.json";

/// The file name of the settings the operator recorded by name.
pub(crate) const SETTINGS: &str = "settings.json";

/// The file name of the secrets lemonfiber holds by name.
pub const SECRETS: &str = "secrets.json";

/// The directory each service's own secrets file is written in.
pub(crate) const SECRETS_FILES: &str = "env";

/// What the stack looked like when lemonfiber last wrote it.
pub(crate) const BASELINE: &str = "baseline.json";
/// What lemonfiber last materialised.
pub(crate) const MATERIALISED: &str = "materialised.json";
/// The operator's quality choice.
pub(crate) const QUALITY: &str = "quality.json";
/// The operator's notification choice.
pub(crate) const NOTIFICATIONS: &str = "notifications.json";
/// The operator's autostart answer.
pub(crate) const AUTOSTART: &str = "autostart.json";
/// Which restart of this machine lemonfiber last acted on.
pub(crate) const BOOT: &str = "boot.json";
/// What the household declared about its line.
pub(crate) const BANDWIDTH: &str = "bandwidth.json";
/// Why each request was turned down.
pub(crate) const REFUSALS: &str = "refusals.json";
/// When each person was first handed a code.
pub(crate) const HANDOFFS: &str = "handoffs.json";
/// The choices whose cost was stated once.
pub(crate) const ACCEPTED: &str = "accepted.json";
/// The port the egress guard last forwarded to every torrent client.
pub(crate) const FORWARDED: &str = "forwarded.json";
/// What the update check remembers between runs.
pub(crate) const UPDATES: &str = "updates.json";
/// When each invitation was offered.
pub(crate) const INVITATIONS: &str = "invitations.json";
/// What the household agreed about the requests nobody rules on.
pub(crate) const EXPIRING: &str = "expiring.json";
/// What the operator has been told, and what is still owed them.
pub(crate) const OUTBOX: &str = "outbox.json";
/// Which containers lemonfiber stopped itself.
pub(crate) const HALTED: &str = "halted.json";
/// The public halves of the keys a phone proves the operator with.
pub const DEVICE_KEYS: &str = "device-keys.json";
/// The keys other programs reach the web surface with, as digests.
pub const KEYS: &str = "keys.json";
/// When each of those keys was last used.
pub const KEYS_USED: &str = "keys-used.json";
/// Whose asking was taken away while the disk had no room.
pub(crate) const HELD_BACK: &str = "held-back.json";
/// Every repair lemonfiber carried out.
pub(crate) const REPAIRS: &str = "repairs.json";
/// The last day each member's client spoke to the media server.
pub(crate) const GRANTS: &str = "grants.json";
/// What is wrong, and since when.
pub(crate) const CONDITIONS: &str = "conditions.json";

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

    /// The key the journal's credentials are sealed under, beside the journal itself.
    ///
    /// Named here as well as derived there, for the reason the update check's memory is:
    /// both readers of the journal are handed a path to it rather than this layout, so
    /// the formula lives beside the file it opens — and a restore that took the records
    /// and left this behind is a restore that can no longer read them.
    #[must_use]
    pub fn journal_key(&self) -> PathBuf {
        self.journal().with_file_name(crate::journal::KEY_FILE)
    }

    /// What left this machine, so the operator can check what was sent rather than
    /// only what the enumeration says would be.
    #[must_use]
    pub fn outbound(&self) -> PathBuf {
        self.config.join("outbound.log")
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
        self.config.join(BASELINE)
    }

    /// The last entry of the request gate's record a diagnosis read, so the next one
    /// reports only what came after. Kept with configuration because it records what
    /// this operator has been shown: a restore that lost it would show them every
    /// refusal and removal the record still holds again, as though each were new.
    #[must_use]
    pub fn gate_read(&self) -> PathBuf {
        self.config.join(GATE_READ)
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
        self.config.join(QUALITY)
    }

    /// The operator's notification choice: which appetite is in force, and the
    /// individual events they set apart from it. Kept with configuration for the
    /// same reasons the quality choice is — a backup carries it, and a later run
    /// respects the answer rather than falling back to the quiet default.
    #[must_use]
    pub fn notifications(&self) -> PathBuf {
        self.config.join(NOTIFICATIONS)
    }

    /// The operator's autostart answer: whether they asked for the stack to come
    /// back after this machine restarts. Kept with configuration rather than as a
    /// setting in the environment file, because the environment file is handed to
    /// Compose as it stands and this is not a thing Compose has any use for — and
    /// kept at all because an answer gathered at setup and then discarded leaves
    /// the operator believing they chose something they did not.
    #[must_use]
    pub fn autostart(&self) -> PathBuf {
        self.config.join(AUTOSTART)
    }

    /// Which restart of this machine lemonfiber last acted on.
    ///
    /// The observation half of autostart, kept apart from the answer beside it on
    /// purpose: one records what the operator asked for and the other records what
    /// this machine did, and a single file holding both is the `enabled-unverified`
    /// trap committed to disk — a record that cannot tell wanting from working.
    ///
    /// Kept with configuration rather than beside the stack because losing it is not
    /// free: a run that forgot which restart it had already acted on would verify the
    /// same boot again, and report a failure the operator was told about a week ago as
    /// though it had just happened.
    #[must_use]
    pub fn boot(&self) -> PathBuf {
        self.config.join(BOOT)
    }

    /// What the household declared about its line: the limits, the hours, the cap,
    /// and what the line has been seen to carry. Kept with configuration for the
    /// same reasons the quality choice is, and for one of its own — the measured
    /// capacity is learned by watching over weeks, so a restore that lost it would
    /// leave every share resolving to nothing until the stack had watched again.
    #[must_use]
    pub fn bandwidth(&self) -> PathBuf {
        self.config.join(BANDWIDTH)
    }

    /// Why a request was turned down, by the number the request service files it
    /// under. Kept with configuration because it is a record of something the operator
    /// decided and the request service has nowhere to hold — a restore that lost it
    /// would leave every refusal bare again, which is the state this record exists to
    /// get a household out of.
    #[must_use]
    pub fn refusals(&self) -> PathBuf {
        self.config.join(REFUSALS)
    }

    /// When each person was first handed a code to sign a device in with. Kept with
    /// configuration because the media server records no such thing, and it is what tells
    /// a code still waiting on somebody's phone from one never handed over.
    #[must_use]
    pub fn handoffs(&self) -> PathBuf {
        self.config.join(HANDOFFS)
    }

    /// The choices the operator answered whose cost was stated to them once —
    /// running with no VPN, or with a provider that forwards no port. Kept with
    /// configuration because it records a decision, and a backup that restored the
    /// stack without it would put every settled question again.
    #[must_use]
    pub fn accepted(&self) -> PathBuf {
        self.config.join(ACCEPTED)
    }

    /// The port the egress guard last forwarded, which every torrent client was set to.
    #[must_use]
    pub fn forwarded(&self) -> PathBuf {
        self.config.join(FORWARDED)
    }

    /// When each invitation was offered, which the media server may lose.
    #[must_use]
    pub fn invitations(&self) -> PathBuf {
        self.config.join(INVITATIONS)
    }

    /// What the household agreed about the requests nobody rules on.
    #[must_use]
    pub fn expiring(&self) -> PathBuf {
        self.config.join(EXPIRING)
    }

    /// What the operator has been told, and what is still owed them.
    #[must_use]
    pub fn outbox(&self) -> PathBuf {
        self.config.join(OUTBOX)
    }

    /// Which containers lemonfiber stopped itself.
    #[must_use]
    pub fn halted(&self) -> PathBuf {
        self.config.join(HALTED)
    }

    /// The public halves of the keys a phone proves the operator with.
    #[must_use]
    pub fn device_keys(&self) -> PathBuf {
        self.config.join(DEVICE_KEYS)
    }

    /// The keys other programs reach the web surface with, as digests.
    #[must_use]
    pub fn keys(&self) -> PathBuf {
        self.config.join(KEYS)
    }

    /// When each of those keys was last used.
    #[must_use]
    pub fn keys_used(&self) -> PathBuf {
        self.config.join(KEYS_USED)
    }

    /// Whose asking was taken away while the disk had no room.
    #[must_use]
    pub fn held_back(&self) -> PathBuf {
        self.config.join(HELD_BACK)
    }

    /// Every repair lemonfiber carried out, and what each came to.
    #[must_use]
    pub fn repairs(&self) -> PathBuf {
        self.config.join(REPAIRS)
    }

    /// The last day each member's client spoke to the media server.
    #[must_use]
    pub fn grants(&self) -> PathBuf {
        self.config.join(GRANTS)
    }

    /// What is wrong, since when, and whether the operator was told.
    #[must_use]
    pub fn conditions(&self) -> PathBuf {
        self.config.join(CONDITIONS)
    }

    /// Every answer an installed plugin's adapter gave outside its contract.
    #[must_use]
    pub fn nonconforming(&self) -> PathBuf {
        self.config.join(NONCONFORMING)
    }

    /// What each installed plugin is run with again: its manifest and its run proofs' starting files.
    #[must_use]
    pub fn plugin_starting(&self) -> PathBuf {
        self.config.join(STARTING_KEPT)
    }

    /// What each installed plugin's run proofs last came to.
    #[must_use]
    pub fn run_verdicts(&self) -> PathBuf {
        self.config.join(RUN_VERDICTS)
    }

    /// The newest plugin catalogue index this machine verified.
    #[must_use]
    pub fn catalogue(&self) -> PathBuf {
        self.config.join(CATALOGUE)
    }

    /// When the release list was last asked which version of lemonfiber is newest,
    /// how the last few of those went, and what the last one read.
    ///
    /// Kept with configuration rather than beside the stack, and it is the one record
    /// here whose worth is in *not* being regenerated: what it holds is a machine
    /// having tried and got nothing back, several times over, and a restore that
    /// threw that away would put a laptop with no route out straight back to asking.
    #[must_use]
    pub fn updates(&self) -> PathBuf {
        self.config.join(UPDATES)
    }

    /// The password the web surface asks for, as the verifier that proves it.
    ///
    /// Kept with configuration rather than beside the stack, for the reason the
    /// baseline is: two projects sharing one stack directory are two installations,
    /// and who may open one of them is not who may open the other. A backup carries
    /// it, which is what an operator restoring onto a new machine wants — a restore
    /// that silently dropped the password would put a surface back with nothing in
    /// front of it.
    #[must_use]
    pub fn admission(&self) -> PathBuf {
        self.config.join("admission.json")
    }

    /// What pairing a phone with this stack rests on: the certificate the web surface
    /// presents, the stack's own identifier, and how the surface was last served.
    ///
    /// Kept with configuration rather than beside the stack, for the reason the password
    /// is: a backup carries it, and a restore that dropped the certificate or the
    /// identifier would leave every paired phone refusing a machine it had been told was
    /// this one.
    #[must_use]
    pub fn companion(&self) -> PathBuf {
        self.config.join("companion")
    }

    /// What is installed of somebody else's, and what installing each decided.
    ///
    /// Kept with configuration rather than beside the stack, and it is the record
    /// here with the least tolerance for being lost: every other one holds an answer
    /// that can be given again, and this one is the only memory that a stranger's
    /// service is on this machine at all. A restore that dropped it would leave every
    /// value a plugin set orphaned with nothing to name the owner, and a reading of
    /// what is installed confidently answering *nothing*.
    #[must_use]
    pub fn plugins(&self) -> PathBuf {
        self.config.join(PLUGINS)
    }

    /// The settings the operator recorded, by name.
    #[must_use]
    pub fn settings(&self) -> PathBuf {
        self.config.join(SETTINGS)
    }

    /// The secrets lemonfiber holds, by name.
    #[must_use]
    pub fn secrets(&self) -> PathBuf {
        self.config.join(SECRETS)
    }

    /// The directory each service's own secrets file is written in.
    #[must_use]
    pub fn secrets_files(&self) -> PathBuf {
        self.config.join(SECRETS_FILES)
    }

    /// The materialised stack: the project and every service's starting files, written
    /// where Compose reads them.
    #[must_use]
    pub fn stack(&self) -> PathBuf {
        self.data.join("stack")
    }

    /// Where a command this machine keeps running writes what it would otherwise
    /// have said on a terminal.
    ///
    /// Under the data directory rather than beside the settings, because it is
    /// regenerated output rather than something a backup should carry — and a
    /// month of a clock's words is not a thing to restore onto another machine.
    #[must_use]
    pub fn hosted(&self) -> PathBuf {
        self.data.join("hosted")
    }

    /// The record of what lemonfiber last wrote to the stack directory — a checksum
    /// per file, so a later run tells an operator's edit from a version it has not
    /// upgraded yet. Kept with configuration, not beside the stack it describes: it
    /// is the memory of what was written, which losing would let an edit be
    /// overwritten, so it belongs with what a backup keeps.
    #[must_use]
    pub fn materialised(&self) -> PathBuf {
        self.config.join(MATERIALISED)
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

    /// Where support bundles are written for a caller that named no directory.
    ///
    /// Not the backups directory, though both hold a `.tar.gz`: retention prunes
    /// what it finds beside the backups, and a bundle left there would be deleted
    /// by a rule about how many backups to keep. Regenerable — a bundle is produced
    /// again by asking again.
    #[must_use]
    pub fn bundles(&self) -> PathBuf {
        self.data.join("support")
    }
}

#[cfg(test)]
mod tests;
