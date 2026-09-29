//! Handing somebody's device the way onto the stack, and proving it arrived.
//!
//! **An invitation makes the account; this points a device at it.** The two are kept
//! apart because only one of them decides anything: what somebody may watch is chosen
//! when the account is made, and a hand-off that made accounts on the way would make
//! ones nobody chose anything for. So a name with no account behind it is answered as
//! not provisioned, with the invitation to run, and nothing is made.
//!
//! **Whether a device arrived is asked of the media server every time.** The one thing
//! written down here is when a code was first handed over, which the server has no
//! record of and which is what tells somebody who has yet to sign in from somebody never
//! handed anything. A device that signed in and then out is read as it is now, not as it
//! was once.
//!
//! **The code is the server's address.** It is not a credential: it says where the
//! server is, and whoever scans it still signs in as somebody.

use std::collections::BTreeMap;

use crate::app::Ctx;
use crate::error::{Problem, Remedy, Severity};
use crate::model::{HandedClient, HandedSession, Handoff, HandoffState};
use crate::ports::service::{Household as _, Member};

/// What the record is called, beside the environment file.
///
/// Named once and used from both sides: a reader and a writer disagreeing about the file
/// name would read as nobody ever having been handed a code.
const NAME: &str = "handoffs.json";

/// When each account was first handed a code, by the identifier the media server gave it.
type Issued = BTreeMap<String, String>;

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
    let Some(jellyfin) = super::seed::identity::jellyfin_service(&manifest.services) else {
        return Err(Box::new(no_media_server()));
    };
    let Some(password) = super::seed::identity::recorded_jellyfin_password(ctx) else {
        return Err(Box::new(not_set_up()));
    };
    let server = crate::jellyfin::Jellyfin::authenticated(
        ctx.seams.http.clone(),
        &jellyfin.loopback,
        "jellyfin",
        crate::config::JELLYFIN_ADMIN_USER,
        &password,
    );
    let reachable = super::invite::household_address(ctx, jellyfin.port).await;
    let mut report = Handoff {
        name,
        state: HandoffState::Failed,
        reason: None,
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
        return Ok(failed(report, SERVER_SILENT));
    };
    let asked = report.name.to_lowercase();
    let Some(member) = household
        .into_iter()
        .find(|member| member.name.to_lowercase() == asked)
    else {
        report.state = HandoffState::Unprovisioned;
        report.reason = Some(unprovisioned(&report.name));
        return Ok(report);
    };
    // Refused for the reason an invitation refuses it: this is the account the program
    // signs in as, and a phone handed it would hold the key to everybody else's.
    if member.access.administrator {
        return Err(Box::new(runs_the_server(&member.name)));
    }
    report.name.clone_from(&member.name);
    let Some(address) = report.address.clone() else {
        return Ok(failed(report, NOWHERE_TO_REACH));
    };

    report.quick_connect = server.quick_connect().await.unwrap_or(false);
    report.steps = steps(&member, &address, report.quick_connect);
    report.clients = clients(&address);

    let Ok(sessions) = server.sessions(&member.id).await else {
        return Ok(failed(report, SESSIONS_UNREAD));
    };
    report.sessions = sessions
        .into_iter()
        .map(|session| HandedSession {
            device: session.device,
            client: session.client,
            last_seen: session.last_seen,
        })
        .collect();

    let mut issued: Issued = super::record::beside(ctx, NAME);
    let before = issued.get(&member.id).cloned();
    report.state = match (report.sessions.is_empty(), before.is_some()) {
        (false, _) => HandoffState::Connected,
        (true, true) => HandoffState::Pending,
        (true, false) => HandoffState::Ready,
    };
    report.issued = match before {
        Some(at) => Some(at),
        None if ctx.dry_run => None,
        None => {
            let now = ctx.hours_ago(0);
            issued.insert(member.id.clone(), now.clone());
            // Best effort: a record that would not write costs the next run the
            // difference between ready and pending, and never claims a device arrived.
            super::record::keep_beside(ctx, NAME, &issued);
            Some(now)
        }
    };
    if report.state == HandoffState::Pending {
        report.reason = Some(STILL_THEIRS.to_owned());
    }
    Ok(report)
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
            code: device
                .deep_link
                .map_or_else(|| address.to_owned(), |link| link.replace(ADDRESS, address)),
            deep_link: device.deep_link.is_some(),
        })
        .collect();
    handed.sort_by_key(|client| !client.open_source);
    handed
}

/// Where the server's address goes in a client's link.
const ADDRESS: &str = "{address}";

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
    steps.push(
        "Run this again once they have: it asks the media server which of their devices \
         are signed in."
            .to_owned(),
    );
    steps
}

/// A failed hand-off, with why.
fn failed(mut report: Handoff, reason: &str) -> Handoff {
    report.state = HandoffState::Failed;
    report.reason = Some(reason.to_owned());
    report
}

/// Why nothing could be handed over to a name with no account behind it.
fn unprovisioned(name: &str) -> String {
    format!(
        "Nobody called {name} has an account yet, so there is nothing for a device to sign \
         in to. Invite them first, which is where what they may watch is chosen: \
         `lemonfiber invite {name} --confirm`."
    )
}

/// Said where the media server does not answer at all.
const SERVER_SILENT: &str = "The media server did not answer, so nothing could be checked or \
    handed over. That is the server, not anybody's device or app: `lemonfiber status` says \
    whether it is running.";

/// Said where this machine has no address another device could reach it at.
const NOWHERE_TO_REACH: &str = "This machine has no address another device could reach it \
    at, so there is nothing to put in a code. That is a question of reaching the server rather \
    than of any app: record the address the household uses with `lemonfiber config set \
    HOUSEHOLD_HOST <address>`.";

/// Said where the media server answers but will not list who is signed in.
const SESSIONS_UNREAD: &str = "The media server would not say which devices are signed in, so \
    whether theirs arrived is not known. Nothing on their device is at fault for that; run \
    this again once the server answers.";

/// Said where a code went out and no device of theirs is signed in yet.
const STILL_THEIRS: &str = "No device of theirs is signed in yet, and the next step is theirs, \
    on the device. If the app cannot find the server, that is the way to the server rather \
    than the app: the address opens on the home network, so the device has to be on it.";

/// Said where the hand-off is for nobody: the name is blank, or only spaces.
fn nobody_named() -> Problem {
    Problem::new(
        crate::error::codes::handoff::NOBODY_NAMED,
        Severity::Error,
        "a hand-off needs somebody to be for",
        "The name is the account their device signs in to, so a blank one leads nowhere",
        Remedy::new("Give the name they sign in as")
            .with_detail("lemonfiber household handoff ana"),
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
        Remedy::new("Run setup so the media server's account is made and recorded")
            .with_detail("lemonfiber setup"),
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
