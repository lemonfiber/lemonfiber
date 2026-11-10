//! What the read of where a message goes answers with: each back-end the stack delivers to,
//! how far each is proven, and what became of its deliveries.

use serde::{Deserialize, Serialize};

use crate::alert::Notification;

/// Where delivery as a whole stands.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "kebab-case")]
#[schemars(rename = "NotifyState")]
pub enum NotifyState {
    /// No back-end is configured, so nothing is delivered anywhere.
    Unconfigured,
    /// At least one back-end is proven, and none failed its last delivery or proof.
    Ready,
    /// A configured back-end failed its last delivery or its proof.
    Degraded,
    /// No configured back-end is accepting messages.
    Unreachable,
}

/// What a back-end speaks.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "kebab-case")]
#[schemars(rename = "NotifyBackendKind")]
pub enum BackendKind {
    /// An ntfy server's topic.
    Ntfy,
    /// A gotify server's application.
    Gotify,
    /// An address that is posted each message as JSON.
    Webhook,
    /// An Apprise API address.
    Apprise,
    /// A phone's `UnifiedPush` endpoint.
    Unifiedpush,
}

impl BackendKind {
    /// What it is called where it is named.
    #[must_use]
    pub const fn word(self) -> &'static str {
        match self {
            Self::Ntfy => "ntfy",
            Self::Gotify => "gotify",
            Self::Webhook => "webhook",
            Self::Apprise => "apprise",
            Self::Unifiedpush => "unifiedpush",
        }
    }
}

/// Who chose a back-end.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "kebab-case")]
#[schemars(rename = "NotifyChosenBy")]
pub enum ChosenBy {
    /// Whoever runs the house.
    Operator,
    /// A household member, by their account's id.
    Member(String),
}

/// How far a back-end is proven.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "kebab-case")]
#[schemars(rename = "NotifyProof")]
pub enum BackendProof {
    /// No test sent to it has been seen arriving.
    Unproven,
    /// A test was seen arriving, at this instant in seconds since the Unix epoch.
    Proven(u64),
    /// Nothing is delivered to it until it is proven again, and why.
    Held(String),
}

/// What became of one delivery.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "kebab-case")]
#[schemars(rename = "NotifyOutcome")]
pub enum DeliveryOutcome {
    /// Its arrival was observed.
    Confirmed,
    /// The back-end took it, and its arrival was not observed.
    Sent,
    /// The back-end refused it or did not answer, in its own words.
    Failed(String),
}

/// One delivery to one back-end.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[schemars(rename = "NotifyDelivery")]
pub struct Delivery {
    /// When it was sent, in seconds since the Unix epoch.
    pub at: u64,
    /// Which kind of alert it carried.
    pub kind: Notification,
    /// What became of it.
    pub outcome: DeliveryOutcome,
}

/// A delivery still to be tried again.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[schemars(rename = "NotifyRetry")]
pub struct DeliveryRetry {
    /// When it is tried next, in seconds since the Unix epoch.
    pub next: u64,
    /// Which attempt that will be, from one.
    pub attempt: u32,
    /// How many attempts it is given.
    pub of: u32,
}

/// A back-end on a host that is not the household's own.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[schemars(rename = "NotifyHosted")]
pub struct HostedBackend {
    /// The host it reaches.
    pub host: String,
    /// What leaves for it, in the operator's words.
    pub leaves: Vec<String>,
}

/// One back-end, as the read lists it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, schemars::JsonSchema)]
#[schemars(rename = "NotifyBackend")]
pub struct BackendReport {
    /// The stack's name for it, the same on every read.
    pub id: String,
    /// What it speaks.
    pub kind: BackendKind,
    /// The host and topic or path it reaches, with no credential.
    #[serde(rename = "where")]
    pub reaches: String,
    /// Who chose it.
    pub chosen_by: ChosenBy,
    /// Where it is hosted, absent where it is the household's own.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hosted: Option<HostedBackend>,
    /// How far it is proven.
    pub proof: BackendProof,
    /// Its newest delivery, absent where it has never delivered.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last: Option<Delivery>,
    /// Every failed delivery since its last confirmed one, newest first.
    pub failures: Vec<Delivery>,
    /// The delivery still to be tried again, absent where none is.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub retry: Option<DeliveryRetry>,
}

/// A back-end whose record could not be read.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, schemars::JsonSchema)]
#[schemars(rename = "NotifyUnread")]
pub struct UnreadBackend {
    /// The back-end, by its id.
    pub backend: String,
    /// Why.
    pub because: String,
}

/// Where a message goes: every back-end the stack delivers to.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, schemars::JsonSchema)]
pub struct NotifyReport {
    /// Where delivery as a whole stands.
    pub state: NotifyState,
    /// Each back-end, in the order it was added.
    pub backends: Vec<BackendReport>,
    /// Each back-end whose record could not be read, with why.
    pub unread: Vec<UnreadBackend>,
    /// Whether this was a rehearsal of a change, with nothing kept.
    pub rehearsed: bool,
}

/// One way to be told that the operator offered the household, as a member reads it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, schemars::JsonSchema)]
#[schemars(rename = "NotifyOffered")]
pub struct OfferedBackend {
    /// What it is chosen by.
    pub id: String,
    /// The name the household reads it by.
    pub name: String,
    /// Whether this member is told on it.
    pub receiving: bool,
}

/// The ways a member may be told that something they asked for is ready, and nothing
/// technical.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, schemars::JsonSchema)]
pub struct NotifyMineReport {
    /// Each way the operator offered the household, in the order it was offered.
    pub offered: Vec<OfferedBackend>,
    /// Whether this member's own phone is registered to be woken.
    pub phone: bool,
    /// Whether this was a rehearsal of a choice, with nothing kept.
    pub rehearsed: bool,
}
