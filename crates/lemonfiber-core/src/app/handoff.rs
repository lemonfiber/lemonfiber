//! Handing somebody's device the way onto the stack, and proving it arrived.
//!
//! **An invitation makes the account; this points a device at it.** The two are kept
//! apart because only one of them decides anything: what somebody may watch is chosen
//! when the account is made, and a hand-off that made accounts on the way would make
//! ones nobody chose anything for. So a name with no account behind it is answered as
//! not provisioned, with inviting them as the remedy, and nothing is made.
//!
//! **Whether a device arrived is asked of the media server every time.** What is written
//! down here is when a code was first handed over and which devices were signed in to
//! the account at that moment, neither of which the server keeps. The first is what
//! tells somebody who has yet to sign in from somebody never handed anything; the second
//! is what tells the device just handed over from a phone they already had, which is the
//! only one whose arrival proves anything. A device that signed in and then out is read
//! as it is now, not as it was once.
//!
//! **The code is the server's address.** It is not a credential: it says where the
//! server is, and whoever scans it still signs in as somebody.
//!
//! **What it says, every surface can show.** A reason and a step are words about the
//! server and the device, and name no command; what to do next is a [`HandoffRemedy`],
//! which a terminal answers with a command and an app with a control.

use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};

use crate::app::Ctx;
use crate::error::{Problem, Remedy, Severity};
use crate::model::{HandedClient, HandedSession, Handoff, HandoffRemedy, HandoffState};
use crate::ports::service::{Household as _, Member};

/// What the record is called, beside the environment file.
///
/// Named once and used from both sides: a reader and a writer disagreeing about the file
/// name would read as nobody ever having been handed a code.
const NAME: &str = "handoffs.json";

/// Each account's first code, by the identifier the media server gave the account.
type Issued = BTreeMap<String, Kept>;

/// One account's first code: when it was given, and which devices were signed in then.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct Given {
    /// When the code was first given, as an instant.
    at: String,
    /// The devices signed in to the account at that moment, by the identifier the media
    /// server tells each apart by. None of them is the device being handed over.
    #[serde(default)]
    signed_in: BTreeSet<String>,
}

/// One account's entry as it was written.
///
/// A record written before the devices were kept holds the moment alone, and reads as a
/// code given while nothing was signed in: every device signed in now counts as having
/// arrived, which is what that record always claimed.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(untagged)]
enum Kept {
    /// The moment and the devices signed in then.
    Given(Given),
    /// The moment alone.
    Dated(String),
}

impl Kept {
    /// What was kept, with no devices where none were written down.
    fn given(self) -> Given {
        match self {
            Self::Given(given) => given,
            Self::Dated(at) => Given {
                at,
                signed_in: BTreeSet::new(),
            },
        }
    }
}

/// Hand somebody's device the way onto the stack, or say where doing so stands.
///
/// Run once, it issues the code; run again, it says whether a device of theirs has
/// signed in since. Each run is the whole of a step, so every step can be taken from a
/// script: an invitation provisions, and this issues and proves.
///
/// A rehearsal reads everything a real run reads and writes nothing, so it says what
/// would be handed over without recording that anything was.
///
/// # Errors
///
/// Returns a [`Problem`] where nobody is named, where the stack has no media server,
/// where the media server's own account was never recorded, or where the name is the
/// account that administers it. Everything else is an answer: a state, with the reason
/// where the state is not the whole of it.
pub(crate) async fn handoff(ctx: &Ctx, name: String) -> Result<Handoff, Box<Problem>> {
    let name = name.trim().to_owned();
    if name.is_empty() {
        return Err(Box::new(nobody_named()));
    }
    let manifest = ctx
        .stack
        .checked_manifest(ctx.today())
        .map_err(|err| Box::new(crate::error::Diagnose::problem(&err)))?;
    // Only the client and the port are carried on: the server as the lookup resolved
    // it is not held across what follows.
    let (server, port) = {
        let Some(media) = super::targets::hosted(ctx, &manifest) else {
            return Err(Box::new(no_media_server()));
        };
        let Some(server) = media.administered(ctx) else {
            return Err(Box::new(not_set_up()));
        };
        (server, media.port)
    };
    let reachable = super::invite::household_address(ctx, port).await;
    let mut report = Handoff {
        name,
        state: HandoffState::Failed,
        reason: None,
        remedy: None,
        address: reachable.as_ref().map(|at| at.url.clone()),
        caution: reachable.and_then(|at| at.caution),
        issued: None,
        quick_connect: false,
        steps: Vec::new(),
        clients: Vec::new(),
        sessions: Vec::new(),
        rehearsed: ctx.dry_run,
    };

    let Ok(household) = server.household().await else {
        return Ok(failed(report, SERVER_SILENT, HandoffRemedy::StartServer));
    };
    let asked = report.name.to_lowercase();
    let Some(member) = household
        .into_iter()
        .find(|member| member.name.to_lowercase() == asked)
    else {
        report.state = HandoffState::Unprovisioned;
        report.reason = Some(unprovisioned(&report.name));
        report.remedy = Some(HandoffRemedy::Invite);
        return Ok(report);
    };
    // Refused for the reason an invitation refuses it: this is the account the program
    // signs in as, and a phone handed it would hold the key to everybody else's.
    if member.access.administrator {
        return Err(Box::new(runs_the_server(&member.name)));
    }
    report.name.clone_from(&member.name);
    let Some(address) = report.address.clone() else {
        return Ok(failed(
            report,
            NOWHERE_TO_REACH,
            HandoffRemedy::RecordAddress,
        ));
    };

    report.quick_connect = server.quick_connect().await.unwrap_or(false);
    report.steps = steps(&member, &address, report.quick_connect);
    report.clients = clients(&address);

    let Ok(sessions) = server.sessions(&member.id).await else {
        return Ok(failed(report, SESSIONS_UNREAD, HandoffRemedy::AskAgain));
    };
    let now_signed_in: BTreeSet<String> = sessions
        .iter()
        .map(|session| session.device_id.clone())
        .collect();
    report.sessions = sessions
        .into_iter()
        .map(|session| HandedSession {
            device: session.device,
            client: session.client,
            last_seen: session.last_seen,
        })
        .collect();

    (report.state, report.issued) = proved(ctx, &member.id, &now_signed_in);
    if report.state == HandoffState::Pending {
        report.reason = Some(STILL_THEIRS.to_owned());
    }
    // Until a device of theirs is signed in, the next thing anybody does here is ask
    // again once they have.
    if report.state != HandoffState::Connected {
        report.remedy = Some(HandoffRemedy::AskAgain);
    }
    Ok(report)
}

/// Where the hand-off to one account stands, and when its code was first given.
///
/// The first code is written down here, with the devices signed in at that moment, and
/// a rehearsal writes nothing. A device arrived where one is signed in now that was not
/// then; a code given by this run is given while these devices are signed in, so none
/// of them is the one it hands over.
fn proved(
    ctx: &Ctx,
    account: &str,
    now_signed_in: &BTreeSet<String>,
) -> (HandoffState, Option<String>) {
    let mut issued: Issued = super::record::beside(ctx, NAME);
    let before = issued.get(account).cloned().map(Kept::given);
    let already = before
        .as_ref()
        .map_or_else(|| now_signed_in.clone(), |given| given.signed_in.clone());
    let state = match (!now_signed_in.is_subset(&already), before.is_some()) {
        (true, _) => HandoffState::Connected,
        (false, true) => HandoffState::Pending,
        (false, false) => HandoffState::Ready,
    };
    let at = match before {
        Some(given) => Some(given.at),
        None if ctx.dry_run => None,
        None => {
            let now = ctx.hours_ago(0);
            issued.insert(
                account.to_owned(),
                Kept::Given(Given {
                    at: now.clone(),
                    signed_in: already,
                }),
            );
            // Best effort: a record that would not write costs the next run the
            // difference between ready and pending, and never claims a device arrived.
            super::record::keep_beside(ctx, NAME, &issued);
            Some(now)
        }
    };
    (state, at)
}

/// Every app the guidance names, each with the code that points it at this server.
///
/// Open-source apps first, and a closed one only after them: one may be named, and is
/// never the recommended path.
fn clients(address: &str) -> Vec<HandedClient> {
    let mut handed: Vec<HandedClient> = crate::clients::DEVICES
        .iter()
        .map(|device| HandedClient {
            device: device.device.to_owned(),
            client: device.client.to_owned(),
            open_source: device.open_source,
            code: code(device.deep_link, address),
            deep_link: device.deep_link.is_some(),
        })
        .collect();
    handed.sort_by_key(|client| !client.open_source);
    handed
}

/// Where the server's address goes in a client's link.
const ADDRESS: &str = "{address}";

/// What one app's code carries: its link with the address in it, or the address alone.
fn code(deep_link: Option<&str>, address: &str) -> String {
    deep_link.map_or_else(|| address.to_owned(), |link| link.replace(ADDRESS, address))
}

/// How the person signs in on the new device, one step at a time.
fn steps(member: &Member, address: &str, quick_connect: bool) -> Vec<String> {
    let mut steps = vec![format!(
        "Open the app on their device and give it the server: scan the code, or type \
         {address}."
    )];
    steps.push(if member.claimed {
        format!("Sign in as {} with their own password.", member.name)
    } else {
        format!(
            "Sign in as {} with the password left empty; the account has none yet, and \
             they set their own.",
            member.name
        )
    });
    if quick_connect {
        steps.push(format!(
            "Where a password is a chore to type, as on a television, choose Quick Connect \
             in the app once another device of theirs is signed in. It shows a short code, \
             which they approve on that device under their own profile's Quick Connect, \
             signed in as {}. That approval is theirs to give, and lemonfiber does not give \
             it for them.",
            member.name
        ));
    }
    steps
}

/// A failed hand-off, with why and what to do about it.
fn failed(mut report: Handoff, reason: &str, remedy: HandoffRemedy) -> Handoff {
    report.state = HandoffState::Failed;
    report.reason = Some(reason.to_owned());
    report.remedy = Some(remedy);
    report
}

/// Why nothing could be handed over to a name with no account behind it.
fn unprovisioned(name: &str) -> String {
    format!(
        "Nobody called {name} has an account yet, so there is nothing for a device to sign \
         in to. Invite them first, which is where what they may watch is chosen."
    )
}

/// Said where the media server does not answer at all.
const SERVER_SILENT: &str = "The media server did not answer, so nothing could be checked or \
    handed over. That is the server, not anybody's device or app.";

/// Said where this machine has no address another device could reach it at.
const NOWHERE_TO_REACH: &str = "This machine has no address another device could reach it \
    at, so there is nothing to put in a code. That is a question of reaching the server rather \
    than of any app: the address the household uses has not been recorded.";

/// Said where the media server answers but will not list who is signed in.
const SESSIONS_UNREAD: &str = "The media server would not say which devices are signed in, so \
    whether theirs arrived is not known. Nothing on their device is at fault for that.";

/// Said where a code went out and no device of theirs has signed in since.
const STILL_THEIRS: &str = "No device of theirs has signed in since the code was given, and the \
    next step is theirs, on the device. If the app cannot find the server, that is the way to the server rather \
    than the app: the address opens on the home network, so the device has to be on it.";

/// Said where the hand-off is for nobody: the name is blank, or only spaces.
fn nobody_named() -> Problem {
    Problem::new(
        crate::error::codes::handoff::NOBODY_NAMED,
        Severity::Error,
        "a hand-off needs somebody to be for",
        "The name is the account their device signs in to, so a blank one leads nowhere",
        Remedy::new("Give the name they sign in as"),
    )
}

/// Said where the stack holds no media server.
fn no_media_server() -> Problem {
    Problem::new(
        crate::error::codes::handoff::NO_MEDIA_SERVER,
        Severity::Error,
        "this stack has no media server, so there is nothing for a device to sign in to",
        "A hand-off points somebody's device at the media server and proves it signed in",
        Remedy::new("Add a media server to the stack and run setup"),
    )
}

/// Said where the media server's own account was never recorded.
fn not_set_up() -> Problem {
    Problem::new(
        crate::error::codes::handoff::NOT_SET_UP,
        Severity::Error,
        "the media server's own account has not been set up yet",
        "Finding somebody's account and the devices signed in to it is done as the \
         administrator, and this machine has not recorded one",
        Remedy::new("Run setup so the media server's account is made and recorded"),
    )
}

/// Said where the account named administers the media server.
fn runs_the_server(name: &str) -> Problem {
    Problem::new(
        crate::error::codes::handoff::RUNS_THE_SERVER,
        Severity::Error,
        format!("{name} administers the media server, so it is not an account to hand over"),
        "This is the account lemonfiber signs in as, and a device handed it could change \
         what everybody else in the household may watch",
        Remedy::new("Invite the person under a name of their own, and hand that over"),
    )
}

#[cfg(test)]
mod tests;
