//! Taking somebody out of the household, and saying what that costs first.
//!
//! A household member exists on two services: the media server holds the account they
//! watch with, and the request service holds one they ask through. Removing somebody has
//! to reach both, or they keep half of what they were given.
//!
//! **The media server goes first.** The request service authenticates *through* it, so
//! once the first account is gone the second cannot be signed into either way — a
//! failure after that leaves something tidy-up-able rather than somebody who can still
//! watch. The other order has the opposite failure, and it is the one that matters.
//!
//! **Nothing happens until it is confirmed**, because none of it can be undone. What an
//! unconfirmed run says is the whole answer: every figure in it is knowable without
//! removing anybody, which is what makes the confirmation meaningful rather than
//! ceremonial.

use crate::app::Ctx;
use crate::model::{HouseholdRemoval, Revoked};
use crate::ports::service::{Household as _, Member, Requests as _};

/// The same, as the answer a surface is handed.
///
/// Here rather than in the dispatcher so that what a removal *is* and what it is called
/// stay together: the dispatcher's job is to route, and a route that also builds the
/// answer is two jobs in one line.
///
/// # Errors
///
/// Returns whatever [`remove`] returns.
pub(super) async fn dispatched(
    ctx: &Ctx,
    name: String,
    confirm: bool,
) -> Result<super::Outcome, Box<crate::error::Problem>> {
    remove(ctx, name, confirm)
        .await
        .map(super::Outcome::Removed)
}

/// Remove somebody from the household, or — until `confirm` — say what that would cost.
///
/// # Errors
///
/// Returns a [`Problem`](crate::error::Problem) where the stack has no media server,
/// where it will not answer, where nobody is named, where nobody by that name is here,
/// or where the account named administers the server.
pub(super) async fn remove(
    ctx: &Ctx,
    name: String,
    confirm: bool,
) -> Result<HouseholdRemoval, Box<crate::error::Problem>> {
    // Trimmed for the same reason an invitation is: the media server keeps the spaces
    // and treats the result as a different person, so `ana ` would find nobody while an
    // account plainly called `ana` sits in the list.
    let name = name.trim().to_owned();
    if name.is_empty() {
        return Err(Box::new(nobody_named()));
    }
    let manifest = ctx
        .stack
        .checked_manifest(ctx.today())
        .map_err(|err| Box::new(crate::error::Diagnose::problem(&err)))?;
    let Some(server) = super::targets::jellyfin_reader(ctx, &manifest.services) else {
        return Err(Box::new(no_media_server()));
    };
    let Ok(household) = server.household().await else {
        return Err(Box::new(unreadable()));
    };

    let Some(member) = here(&household, &name) else {
        return Err(Box::new(nobody_here(&name)));
    };
    // Refused here rather than left to the media server. It does refuse — but with a
    // `400` where another administrator remains and a `500` where this is the last
    // account, and a stack trace is not a refusal anybody can act on. It is also the
    // account this program signs in as, so removing it would take away the credential
    // every other command depends on.
    if member.access.administrator {
        return Err(Box::new(runs_the_server(&member.name)));
    }

    let asking = super::targets::seerr_reader(ctx, &manifest.services);
    let mut cost = what_it_costs(asking.as_ref(), &member).await;

    if !confirm {
        return Ok(cost.into_report(member.name, false, Revoked::Nothing));
    }

    if server.withdraw(&member.id).await.is_err() {
        return Err(Box::new(would_not_remove(&member.name)));
    }
    let revoked =
        also_from_the_request_service(asking.as_ref(), &cost.held, &mut cost.findings).await;

    Ok(cost.into_report(member.name, true, revoked))
}

/// What removing this member takes, read before anything is written.
///
/// Every figure is knowable without removing anybody, which is what the unconfirmed run
/// exists to say — and what makes the confirmation it asks for worth asking.
struct Cost {
    /// What the request service holds for them, if it holds anything or would say.
    held: Held,
    /// How many requests go with them.
    requests: usize,
    /// What could not be read, and anything else worth the operator's attention.
    findings: Vec<String>,
}

impl Cost {
    /// The same cost as the answer a surface renders, once its fate is known.
    fn into_report(self, name: String, confirmed: bool, revoked: Revoked) -> HouseholdRemoval {
        HouseholdRemoval {
            name,
            confirmed,
            requests: self.requests,
            asks_through_the_request_service: matches!(self.held, Held::Account(_)),
            revoked,
            findings: self.findings,
        }
    }
}

/// Read what removing them takes, touching nothing.
async fn what_it_costs(asking: Option<&super::targets::HouseholdAccess>, member: &Member) -> Cost {
    let mut findings = Vec::new();
    let held = match asking {
        Some(access) => their_account(access, member).await,
        None => Held::NoService,
    };
    if matches!(held, Held::Unreadable) {
        findings.push(
            "the request service could not be asked what it holds for them, so what it \
             holds is not counted and not removed"
                .to_owned(),
        );
    }
    let requests = match asking {
        Some(access) => theirs(access, &member.name).await,
        None => 0,
    };
    Cost {
        held,
        requests,
        findings,
    }
}

/// Take the request service's account too, where it holds one.
///
/// Second, and best-effort by design: the account left here cannot be signed into now
/// that the one it authenticates through is gone, so a failure is untidiness rather than
/// access. Reported so the next run takes it.
async fn also_from_the_request_service(
    asking: Option<&super::targets::HouseholdAccess>,
    held: &Held,
    findings: &mut Vec<String>,
) -> Revoked {
    let (Some(access), Held::Account(id)) = (asking, held) else {
        // Nothing there to revoke, so nothing is outstanding.
        return Revoked::Everywhere;
    };
    if access.seerr.remove_member(id).await.is_ok() {
        return Revoked::Everywhere;
    }
    findings.push(
        "the request service still holds an account for them. They cannot sign in to it \
         — it authenticates through the media server, which is gone — but run this again \
         to take it"
            .to_owned(),
    );
    Revoked::MediaServerOnly
}

/// What the request service holds for somebody.
enum Held {
    /// An account, by the identifier this service knows it as.
    Account(String),
    /// None. They have never signed in there, so there is nothing to take away.
    None,
    /// There is no request service on this stack at all.
    NoService,
    /// It would not say, which is neither of the above and must not read as either.
    Unreadable,
}

/// The account the request service holds for this member, where it holds one.
async fn their_account(access: &super::targets::HouseholdAccess, member: &Member) -> Held {
    if access
        .seerr
        .sign_in(crate::config::JELLYFIN_ADMIN_USER, &access.password)
        .await
        .is_err()
    {
        return Held::Unreadable;
    }
    match access.seerr.member_for(&member.id).await {
        Ok(Some(id)) => Held::Account(id),
        Ok(None) => Held::None,
        Err(_) => Held::Unreadable,
    }
}

/// How many requests this member has made, as the request service records them.
///
/// Counted by name rather than by identifier because that is what the record carries,
/// and matched without regard to case for the same reason the account was.
async fn theirs(access: &super::targets::HouseholdAccess, name: &str) -> usize {
    let asked = name.to_lowercase();
    access.seerr.requests().await.map_or(0, |requests| {
        requests
            .iter()
            .filter(|request| request.member.to_lowercase() == asked)
            .count()
    })
}

/// The account this is about, matched the way the media server matches names.
///
/// Without regard to case, because the server treats two names differing only in case as
/// one person — so an exact match would miss the account and report nobody by that name
/// while it sits plainly in the list.
fn here(household: &[Member], name: &str) -> Option<Member> {
    let asked = name.to_lowercase();
    household
        .iter()
        .find(|member| member.name.to_lowercase() == asked)
        .cloned()
}

/// Said where the removal is for nobody: the name is blank, or only spaces.
fn nobody_named() -> crate::error::Problem {
    crate::error::Problem::new(
        crate::error::Code::new("REMOVE-1"),
        crate::error::Severity::Error,
        "no name was given, so there is nobody to remove",
        "Removing somebody takes the name their account is held under",
        crate::error::Remedy::new("Name the person, as they appear in `lemonfiber household`"),
    )
}

/// Said where the stack holds no media server: there is no account to remove.
fn no_media_server() -> crate::error::Problem {
    crate::error::Problem::new(
        crate::error::Code::new("REMOVE-2"),
        crate::error::Severity::Error,
        "this stack has no media server, so there is no household to remove anybody from",
        "A household member is an account on the media server; without one there is \
         nobody to take away",
        crate::error::Remedy::new("Add a media server to the stack and run setup"),
    )
}

/// Said where the media server will not say who it holds.
fn unreadable() -> crate::error::Problem {
    crate::error::Problem::new(
        crate::error::Code::new("REMOVE-3"),
        crate::error::Severity::Error,
        "the media server would not say who holds an account, so nobody was removed",
        "Removing somebody starts by finding their account, and that read did not answer",
        crate::error::Remedy::new("Check the media server is running, then run this again"),
    )
}

/// Said where nobody by that name is in the household.
fn nobody_here(name: &str) -> crate::error::Problem {
    crate::error::Problem::new(
        crate::error::Code::new("REMOVE-4"),
        crate::error::Severity::Error,
        format!("nobody called {name} is in this household"),
        "Nothing was removed. The name has to match an account the media server holds, \
         though not its capitalisation",
        crate::error::Remedy::new("Run `lemonfiber household` to see who is here"),
    )
}

/// Said where the account named administers the server.
fn runs_the_server(name: &str) -> crate::error::Problem {
    crate::error::Problem::new(
        crate::error::Code::new("REMOVE-5"),
        crate::error::Severity::Error,
        format!("{name} administers the media server, so it is not an account to remove"),
        "The media server refuses to be left without an administrator, and this is also \
         the account lemonfiber signs in as — removing it would take away what every \
         other command depends on",
        crate::error::Remedy::new(
            "Remove a household member instead; to hand the server to somebody else, do \
             it in the media server's own settings first",
        ),
    )
}

/// Said where the media server refused to remove the account.
fn would_not_remove(name: &str) -> crate::error::Problem {
    crate::error::Problem::new(
        crate::error::Code::new("REMOVE-6"),
        crate::error::Severity::Error,
        format!("the media server would not remove {name}, so nothing was removed"),
        "Nothing else was touched: the request service is only asked once the media \
         server's account is gone",
        crate::error::Remedy::new("Check the media server is running, then run this again"),
    )
}
