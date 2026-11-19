//! What setup's walk asks of whoever answers it: a person at a terminal, or the
//! command line's flags.

use std::path::{Path, PathBuf};

use lemonfiber_core::alert::Appetite;
use lemonfiber_core::app::setup::Asking;
use lemonfiber_core::config::Protocols;
use lemonfiber_core::model::{SettingReport, StorageFinding};
use lemonfiber_core::prerequisites::PrerequisiteMap;
use lemonfiber_core::validate::Validation;
use lemonfiber_core::wizard::{House, Library};

/// How the operator is asked the questions setup's walk puts to them.
///
/// One method per question the wizard asks, and one per finding the core comes back
/// with that is the operator's to weigh.
pub trait Prompt {
    /// Which download protocols to use: Usenet, torrents, both, or neither.
    fn protocols(&self) -> Protocols;
    /// Show the accounts their protocol choice needs, before any is asked for.
    fn prerequisites(&self, map: &PrerequisiteMap);
    /// Whether a VPN will carry the torrent traffic.
    fn vpn(&self) -> bool;
    /// One question of the egress guard a VPN carries the torrents through: one of the
    /// values offered, a setting's value, or a secret read without echo; nothing to
    /// leave it unanswered.
    fn guard(&self, asking: &Asking<'_>) -> Option<String>;
    /// Torrents were chosen and nothing will carry them: state what that exposes and
    /// ask whether to go on anyway (`true`) or reconsider (`false`).
    fn unprotected(&self) -> bool;
    /// Where the library and downloads are kept.
    fn data_location(&self) -> PathBuf;
    /// The chosen location was tested and hardlinks; `inferred_from` is the parent the
    /// test ran on where the location did not exist yet.
    fn hardlinks(&self, path: &Path, inferred_from: Option<&Path>);
    /// The chosen location copies or could not be tested: say what was found and ask
    /// whether to use it anyway (`true`) or name another (`false`).
    fn storage_warning(&self, path: &Path, found: &StorageFinding) -> bool;
    /// The indexer's URL and API key, or nothing where the operator has none to give.
    fn credential(&self) -> Option<(String, String)>;
    /// The credential was proven: report what the service showed while proving it.
    fn credential_valid(&self, observed: &str);
    /// The credential could not be proven: show what the test came to and ask whether
    /// to try again, keep it unverified, or leave it unset.
    fn credential_failed(&self, outcome: &Validation) -> CredentialChoice;
    /// The Usenet provider's login, or nothing where the operator has none to give.
    fn usenet_provider(&self) -> Option<ProviderEntry>;
    /// The user and group the containers run as; `None` keeps the image's default.
    fn service_user(&self) -> Option<(u32, u32)>;
    /// Whether to run the media server, and how.
    fn library(&self) -> Library;
    /// Whether others in the home will use it.
    fn household(&self) -> bool;
    /// How much the operator wants to be told about.
    fn notifications(&self) -> Appetite;
    /// Whether the stack starts on boot.
    fn autostart(&self) -> bool;
    /// The house's zone and locale, offered `proposed` to confirm or change.
    fn house(&self, proposed: &House) -> House;
    /// Whether the operator, shown what applying will write, confirms it.
    fn confirm(&self, plan: &[SettingReport]) -> bool;
}

/// A Usenet provider login as the operator enters it, before it is proven.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProviderEntry {
    /// The provider's hostname.
    pub host: String,
    /// The port it answers NNTP on.
    pub port: u16,
    /// The account username.
    pub user: String,
    /// The account password.
    pub pass: String,
    /// Whether to connect over TLS.
    pub tls: bool,
}

/// What the operator does with a credential the live test could not prove.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CredentialChoice {
    /// Enter it again and test it afresh.
    Retry,
    /// Keep it as it is, unverified, and go on.
    Proceed,
    /// Leave it unset for now.
    Skip,
}
