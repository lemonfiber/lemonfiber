//! What an operator is told after handing somebody's device the way onto the stack.

use serde::Serialize;

/// Where one person's hand-off stands.
///
/// Read from the media server each time rather than remembered, apart from when a code
/// was first issued: whether a device is signed in is the server's to say, and a copy
/// kept here would go on saying it after the person signed out.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, schemars::JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum HandoffState {
    /// Nobody by that name holds an account yet, so there is nothing to sign in to.
    ///
    /// No account is made here. Making one is an invitation, which is where what the
    /// person may watch is chosen; an account made on the way to handing over a phone
    /// would be one nobody chose anything for.
    Unprovisioned,
    /// The account is there and the code was issued by this run.
    Ready,
    /// The code went out on an earlier run and no device of theirs is signed in.
    ///
    /// Told apart from [`Failed`](Self::Failed) because nothing has gone wrong: the next
    /// step is on the person's device, and until they take it there is nothing to prove.
    Pending,
    /// The media server lists at least one device signed in to their account now.
    Connected,
    /// The hand-off could not go ahead, for the reason given beside it.
    Failed,
}

/// One device the media server lists as signed in to the account.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, schemars::JsonSchema)]
#[schemars(rename = "HandoffSession")]
pub struct HandedSession {
    /// What the device calls itself.
    pub device: String,
    /// The app it signed in with.
    pub client: String,
    /// When the media server last heard from it, where it says.
    pub last_seen: Option<String>,
}

/// One app a device can be pointed at the stack with, and the code that points it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, schemars::JsonSchema)]
#[schemars(rename = "HandoffClient")]
pub struct HandedClient {
    /// What somebody would call the device they are holding.
    pub device: String,
    /// What to use on it.
    pub client: String,
    /// Whether that app is open source. A closed one is never the recommended path.
    pub open_source: bool,
    /// What the code for this app carries: a link that opens the app at this server where
    /// the app takes one, and the server's address otherwise.
    pub code: String,
    /// Whether [`code`](Self::code) is such a link rather than the address alone.
    pub deep_link: bool,
}

/// Where one person's hand-off stands, and what to hand them.
///
/// **The code is an address and nothing more.** Whoever holds a copy of it can find the
/// server and still has to sign in as somebody, so a code sent to the wrong phone, or
/// photographed over a shoulder, gives away where the server is and nothing else.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, schemars::JsonSchema)]
#[schemars(rename = "HandoffReport")]
pub struct Handoff {
    /// Who it is for, as the media server spells their account where it holds one, and as
    /// it was asked for otherwise.
    pub name: String,
    /// Where it stands.
    pub state: HandoffState,
    /// Why it stands there, where that is not the state itself: what to do for an account
    /// that is not there, and what stopped one that failed.
    pub reason: Option<String>,
    /// The address the code carries. Absent where there is no address to carry, which is
    /// one of the ways a hand-off fails.
    pub address: Option<String>,
    /// What is worth knowing about that address, where anything is: most often that it
    /// answers only on the home network.
    pub caution: Option<String>,
    /// When a code was first issued for them, as an instant. Absent until one is.
    pub issued: Option<String>,
    /// Whether the media server offers the sign-in by short code, where one account already
    /// signed in approves another device.
    pub quick_connect: bool,
    /// How the person signs in on the new device, one step at a time.
    ///
    /// **Guidance and never an approval.** Where the sign-in asks for a short code to be
    /// approved from a device they are already signed in on, that approval is theirs: it
    /// is the step that proves the person holding the new device is the person the
    /// account is for, and a program that took it for them would have proved nothing.
    pub steps: Vec<String>,
    /// The apps to point a device at the stack with, each with its code.
    pub clients: Vec<HandedClient>,
    /// The devices signed in to the account now, as the media server lists them.
    pub sessions: Vec<HandedSession>,
    /// Whether this was a rehearsal, in which no issue was written down.
    pub rehearsed: bool,
}
