//! What setup asks for, and what an answer to it is.
//!
//! The vocabulary of the conversation: one type per question, so an answer that does
//! not make sense cannot be represented rather than being caught later.

use super::{PathBuf, Protocols};
use crate::alert::Appetite;
use serde::{Deserialize, Serialize};

/// How the operator wants their existing and downloaded media served, where they
/// want it served at all.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Library {
    /// No media server; an existing player, or downloads only.
    None,
    /// Jellyfin in a container — the portable default that works everywhere.
    JellyfinDocker,
    /// Jellyfin on the host, for a hardware encoder the container cannot reach.
    ///
    /// Only a valid answer where the platform offers it; the wizard rejects it
    /// elsewhere rather than letting a surface record a choice that buys nothing.
    JellyfinNative,
}

impl Library {
    /// The `JELLYFIN_MODE` this choice records, where it runs Jellyfin at all.
    ///
    /// `None` means no media server, which writes no mode rather than a third one.
    pub(crate) const fn mode(self) -> Option<&'static str> {
        match self {
            Self::JellyfinDocker => Some("docker"),
            Self::JellyfinNative => Some("native"),
            Self::None => None,
        }
    }

    /// The choice a recorded `JELLYFIN_MODE` stands for — the inverse of
    /// [`Self::mode`] — or `None` for a value that is neither mode, so a later
    /// reader draws no conclusion from a setting it does not recognise.
    #[must_use]
    pub fn from_mode(mode: &str) -> Option<Self> {
        [Self::JellyfinDocker, Self::JellyfinNative]
            .into_iter()
            .find(|library| library.mode() == Some(mode))
    }
}

/// An indexer credential the operator supplied, and whether it was proven.
///
/// `validated` records whether the live test passed before this was kept — an
/// operator may proceed with one that could not be proven, and a later diagnosis
/// is owed the knowledge that it went in unverified rather than treating it as
/// good.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Indexer {
    /// The indexer's API base URL.
    pub url: String,
    /// The API key it authenticates with.
    pub key: String,
    /// Whether a live test proved the key before it was kept.
    ///
    /// Defaulted when it is absent, so a surface submitting an answer states only
    /// what was entered; what a test decided is not a caller's to assert.
    #[serde(default)]
    pub validated: bool,
}

/// Whether a VPN carries the torrent traffic, and where none does, that the
/// operator was told what that costs and chose to go on.
///
/// Two states rather than a bool, because the second is not merely "no". Torrents
/// expose the home address to every peer, so going without is a decision the
/// operator has to make knowingly — and one a later run must not quietly re-ask,
/// nor a diagnosis mistake for an oversight. Recording the acceptance is what
/// makes it theirs.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Vpn {
    /// A VPN carries it.
    Carrying,
    /// Nothing does, and the operator confirmed they understood that.
    Absent,
}

/// Where the credentials step stands: not yet reached, reached and left empty, or
/// answered with an indexer.
///
/// A three-state value of its own rather than a nested option, so the "answered
/// but empty" case survives being written and read back — a plain
/// `Option<Option<_>>` collapses it to the same `null` as "not answered", and a
/// resumed run would ask the skipped step again.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Credentials {
    /// The step has not been reached yet.
    #[default]
    Unanswered,
    /// Reached, and left without an indexer — a supported end, not a gap.
    Empty,
    /// Answered with an indexer, proven or not.
    Given(Indexer),
}

/// A Usenet provider login the operator supplied, and whether it was proven.
///
/// Its password rides here the way the stack holds its other secrets; `validated`
/// records whether the live login took before it was kept, so a later diagnosis
/// knows an unproven one went in unverified.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Provider {
    /// The provider's hostname.
    pub host: String,
    /// The port it answers NNTP on.
    pub port: u16,
    /// The account username.
    pub user: String,
    /// The account password.
    pub pass: String,
    /// Whether to connect over TLS, as it must be to carry the password.
    pub tls: bool,
    /// Whether a live login proved the account before it was kept.
    ///
    /// Defaulted when it is absent, for the same reason the indexer's is.
    #[serde(default)]
    pub validated: bool,
}

/// Where the Usenet-provider step stands: the same three states the credentials
/// step has, kept distinct on the wire for the same reason.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Usenet {
    /// The step has not been reached yet.
    #[default]
    Unanswered,
    /// Reached, and left without a provider — a supported end.
    Empty,
    /// Answered with a provider, proven or not.
    Given(Provider),
}

/// What the operator has answered so far.
///
/// Every field is optional because a wizard mid-flight is partly answered, and a
/// resumed one is restored to exactly that partial state. This is also the review
/// payload: the complete set of values that will be written, shown before any of
/// them is.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct Answers {
    /// Which download protocols the operator wants.
    pub protocols: Option<Protocols>,
    /// Where downloads and the library live.
    pub data_location: Option<PathBuf>,
    /// The indexer credential, where a download protocol was chosen. Its own
    /// three-state value so "answered, none given" survives a resume, which a
    /// nested option would lose. Absent from an older progress file, so defaulted.
    #[serde(default)]
    pub credentials: Credentials,
    /// The Usenet provider, where Usenet was chosen. Its own three-state value for
    /// the same reason the indexer's is. Absent from an older progress file, so
    /// defaulted.
    #[serde(default)]
    pub usenet: Usenet,
    /// The container user and group, where the platform makes it meaningful.
    ///
    /// Distinguishes "answered: no id needed here" (`Some(None)`) from "not yet
    /// answered" (`None`), because the wizard must know the step is done.
    pub service_user: Option<Option<(u32, u32)>>,
    /// How the library is served.
    pub library: Option<Library>,
    /// Whether the household beyond the operator will use it.
    pub household: Option<bool>,
    /// How much the operator wants to be told about. Absent from a progress file
    /// written before the question existed, so defaulted rather than failing the
    /// resume.
    #[serde(default)]
    pub notifications: Option<Appetite>,
    /// Whether a VPN carries the torrents, where torrents were chosen. Absent from
    /// a progress file written before the question existed, so defaulted rather
    /// than failing the resume.
    #[serde(default)]
    pub vpn: Option<Vpn>,
    /// Whether the stack starts on boot.
    pub autostart: Option<bool>,
}

/// One answer, tagged by the step it belongs to.
///
/// Read back as well as built, because a surface that is not in this process
/// submits one: the tag is the step, so an answer names the question it belongs
/// to rather than arriving as a field a reader has to guess the meaning of.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Answer {
    /// The protocol choice.
    Protocols(Protocols),
    /// Whether a VPN carries the torrents, and where none does, that the operator
    /// accepted what that means.
    Vpn(Vpn),
    /// The data location.
    DataLocation(PathBuf),
    /// The indexer credential the operator settled on, or none where they entered
    /// nothing.
    Credentials(Option<Indexer>),
    /// The Usenet provider the operator settled on, or none where they entered
    /// nothing.
    Provider(Option<Provider>),
    /// The container user and group, or none where it is not needed.
    ServiceUser(Option<(u32, u32)>),
    /// How the library is served.
    Library(Library),
    /// How much the operator wants to be told about.
    Notifications(Appetite),
    /// Whether the household uses it.
    Household(bool),
    /// Whether to start on boot.
    Autostart(bool),
}

/// Why an answer could not be accepted.
///
/// Not every value a surface could submit is meaningful on every platform, and
/// the wizard rejects the ones that are not rather than recording a choice that
/// would silently do nothing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Rejected {
    /// Native Jellyfin was chosen where the platform cannot reach a hardware
    /// encoder, so the choice would buy nothing.
    NativeJellyfinUnavailable,
    /// A container user was given where ownership is mapped away, so it would have
    /// no observable effect.
    ServiceUserNotApplicable,
}
