//! What the household asked for, and where each request stands.
//!
//! The trace answers "where is my show?" for whoever runs the stack. This answers the
//! same question for whoever asked for the show — in the words they would use, and
//! grouped by who they are, so an operator can see at a glance who is still waiting.
//!
//! The request service authenticates its household against the media server, and the one
//! account lemonfiber holds a credential for is the owner's. A member has no way to run
//! this themselves, so the owner's session — which sees every member's requests — asks on
//! their behalf. Nothing new is stored to make that work: the sign-in uses the media-server
//! password seeding already minted and recorded.

mod allowance;
mod handing_over;
mod holding;
mod naming;
mod notices;

use std::collections::BTreeMap;

use crate::app::targets::{jellyfin_reader, seerr_reader};
use crate::app::{Ctx, Hostable};
use naming::{library_titles, named_access, named_by_the_server, title_of, Naming};

use crate::asking::Policy;
use crate::error::{Diagnose, Problem};
use crate::household::State;
use crate::model::{HouseholdMember, HouseholdReport, MemberRequest, Restriction};
use crate::ports::service::{Household as _, HouseholdRequest, Member, Requests};
use crate::quality::Selection;
use crate::recyclarr::Kind;

/// Read the household's requests, grouped by the member who made each one.
///
/// `member` narrows to one person, named the way the caller knows them: a person types a
/// name and gets the forgiving match a typed name deserves, and a session carries the id
/// the media server files its accounts under and gets an exact one.
///
/// **Both, because both callers name a person and only one of them types.** The web
/// surface narrows a member's own read by rewriting the command with their account id,
/// which is the only name a session has for them; an account id is not a substring of
/// anybody's name, so a narrowing that read names alone finds nobody. What that costs is
/// the whole of why it is worth saying here: a member filtered out of their own
/// household comes back as a house holding nobody, which is not an error anywhere, and
/// the app above draws it as *there is nothing to tell you*.
pub(crate) async fn household(
    ctx: &Ctx,
    member: Option<&str>,
) -> Result<HouseholdReport, Box<Problem>> {
    let manifest = ctx
        .stack
        .checked_manifest(ctx.today())
        .map_err(|err| Box::new(err.problem()))?;

    // The household is the set of accounts the media server holds. Reading it out of
    // the *requests* instead makes a list of requesters wearing the name of a list of
    // members: somebody with an account who has never asked for anything does not
    // appear at all, and neither does an invitation nobody has taken up.
    let Some(server) = jellyfin_reader(ctx, &manifest.services) else {
        return Ok(unavailable(
            "there is no media server to ask who is in the household, or no recorded \
             password to sign in with — so who is here cannot be read",
        ));
    };

    let Ok(accounts) = server.household().await else {
        return Ok(unavailable(
            "the media server would not say who holds an account, so who is in the \
             household could not be read — reported as unavailable, not as nobody",
        ));
    };

    let mut findings = Vec::new();
    let (libraries, certificates) = named_by_the_server(&server, &mut findings).await;

    // A request service that will not answer costs the requests, not the household.
    // Who is here is the media server's fact, and reporting nobody because a second
    // service is down would be this same defect one service along.
    //
    // Reached once for both questions it is asked — what the household requested, and
    // what each member may request — because a second reach would be a second chance
    // to disagree about whether it answered at all.
    //
    // Kept rather than consumed, because the same session is what hangs the house's
    // notices further down — and hanging them is the only half of this reading the
    // household itself ever sees.
    let reached = match reaching(ctx, &manifest.services).await {
        Ok(access) => Some(access),
        Err(reason) => {
            findings.push(reason);
            None
        }
    };
    let (requests, asked) = match &reached {
        Some(access) => {
            let asked = access.seerr.requests().await.map_err(|_| {
                "the request service's own record could not be read, so what the \
                 household has asked for is not shown"
                    .to_owned()
            });
            let requests = match asked {
                Ok(requests) => requests,
                Err(reason) => {
                    findings.push(reason);
                    Vec::new()
                }
            };
            (
                requests,
                allowance::gathered(&access.seerr, &accounts).await,
            )
        }
        None => (
            Vec::new(),
            allowance::Asked {
                household: None,
                members: BTreeMap::new(),
            },
        ),
    };
    if asked.household.is_none() {
        findings.push(
            "what the household may ask for could not be read, so no policy and no \
             limit are shown — reported as unread rather than as unlimited"
                .to_owned(),
        );
    }

    // One library read per service names every request that has been handed over, rather
    // than a lookup per request: the same read either way, made once.
    let (titles, named) = library_titles(ctx, &manifest.services).await;
    if !named {
        findings.push(
            "a library could not be read, so some requests are named by what they are \
             rather than by their title"
                .to_owned(),
        );
    }

    // The disk is asked once for the whole household, and asked at all because a member
    // deciding what to ask for is owed the same refusal an approval would meet — in the
    // disk's own words, so nobody reads a full disk as their own limit and waits for a
    // period to roll over instead of freeing some room.
    let no_room = crate::space::run::admits(ctx).await.is_err();
    let quality = crate::quality::run::recorded_selection(ctx);

    if let Some(access) = &reached {
        findings.extend(shown_to_the_household(ctx, access, &asked, &quality, no_room).await);
    }

    let mut report = assemble(
        accounts,
        requests,
        &Naming {
            libraries: &libraries,
            titles: &titles,
            certificates: &certificates,
            asked: &asked,
            quality: &quality,
            now: ctx.seams.clock.now(),
            reasons: &crate::app::refusals::load(ctx),
            expiring: crate::app::arrangement::load(ctx).after(),
            no_room,
            hosted: crate::app::hosting::keeping(ctx, Hostable::Expiring).await,
        },
        member,
    );
    report.findings.append(&mut findings);
    Ok(report)
}

/// The half of this reading the household itself sees, and the block that goes with it.
///
/// Everything else here is written for the operator. This is the only part anybody in the
/// house ever meets, and it reaches them on the request service's own page because they
/// have no account here on purpose — what a thing costs, whether the disk has room, and
/// whether anything is counted over a period at all, which is the one moment all three are
/// in hand at once.
///
/// **The notices and the block are one pass deliberately.** The sentence saying why nothing
/// can be fetched and the permission that stops anything being asked for go up and come
/// down together; in two passes they would be two chances for one to outlive the other,
/// which is a house told the disk is full and still able to ask, or one refused with
/// nothing on the page to say why.
async fn shown_to_the_household(
    ctx: &Ctx,
    access: &crate::app::targets::HouseholdAccess,
    asked: &allowance::Asked,
    quality: &Selection,
    no_room: bool,
) -> Vec<String> {
    let mut findings = Vec::new();
    if let Some(finding) = notices::put_where_they_ask(
        &access.seerr,
        quality,
        no_room,
        asked.under_a_limit(),
        ctx.dry_run,
    )
    .await
    {
        findings.push(finding);
    }
    findings.extend(
        holding::as_the_disk_stands(ctx, &access.seerr, &asked.known(), no_room, ctx.dry_run).await,
    );
    findings
}

/// The request service, signed in — or in plain words why it could not be asked.
///
/// A reason rather than an error: none of these stops the household being listed, and
/// each is something the operator can act on.
pub(crate) async fn reaching(
    ctx: &Ctx,
    services: &[lemonfiber_manifest::Service],
) -> Result<crate::app::targets::HouseholdAccess, String> {
    let Some(access) = seerr_reader(ctx, services) else {
        return Err(
            "there is no request service to ask, or no recorded media-server \
                    password to sign in with, so what the household has asked for is \
                    not shown"
                .to_owned(),
        );
    };

    if access
        .seerr
        .sign_in(crate::config::JELLYFIN_ADMIN_USER, &access.password)
        .await
        .is_err()
    {
        return Err(
            "the request service would not accept the household's sign-in, so \
                    what it has been asked for is not shown"
                .to_owned(),
        );
    }

    Ok(access)
}

/// Whether this account is the one the caller named.
///
/// Two ways of naming one person, because two callers name them differently. A person
/// types a name and means whoever they were thinking of, so it is looked for inside one
/// — the same courtesy the trace extends to a title. A session carries the id the media
/// server assigned and means that account and no other, so it is compared whole.
///
/// **The id is tried first, and exactly.** An identifier matched the forgiving way would
/// be one that could find a different person whose name happened to contain it, and this
/// narrowing decides whose requests somebody is shown — so the one reading it must never
/// produce is a member handed another member's row.
///
/// `named` arrives lower-cased, because the caller lower-cases it once rather than this
/// doing it per account.
fn names(account: &Member, named: &str) -> bool {
    account.id.to_lowercase() == named || account.name.to_lowercase().contains(named)
}

/// The household, member by member, with what each asked for joined onto them.
///
/// Members come out in name order, and each member's requests in the order the service
/// gave them — newest first, so the ones still worth asking about lead.
fn assemble(
    accounts: Vec<Member>,
    requests: Vec<HouseholdRequest>,
    naming: &Naming<'_>,
    member: Option<&str>,
) -> HouseholdReport {
    let wanted = member.map(str::to_lowercase);

    // Keyed by the lower-cased name: the media server treats two names differing only
    // in case as the same person, so a join on the exact string would file a member's
    // own requests under nobody.
    let mut by_name: BTreeMap<String, Theirs> = BTreeMap::new();
    for request in requests {
        let state = State::of(request.request_status, request.media_status);
        let made = request.made.clone();
        let theirs = by_name.entry(request.member.to_lowercase()).or_default();
        // Kept beside the requests rather than read back off them: what a period counts
        // is when something was asked for, and a request already fetched is still inside
        // the window that counted it.
        //
        // A request that was turned down is not, which is the service's own arithmetic
        // and not a choice made here: it excludes a declined one from the count, so
        // counting its date would name a day the window is not waiting on.
        if let Some(made) = made.clone().filter(|_| state != Some(State::Declined)) {
            theirs.made.push(made);
        }
        theirs.requests.push(MemberRequest {
            id: request.id,
            title: title_of(&request, naming.titles),
            media: request.kind.map(Kind::noun).map(str::to_owned),
            state,
            waiting_days: allowance::waiting(state, made.as_deref(), naming.now),
            estimate: allowance::estimated(request.kind, naming.quality),
            refused: naming.reasons.of(request.id).cloned(),
        });
    }

    let mut members: Vec<HouseholdMember> = Vec::new();
    for account in accounts {
        // Taken before the narrowing below, so asking about one person does not leave
        // everybody else's requests looking like requests belonging to nobody.
        let theirs = by_name
            .remove(&account.name.to_lowercase())
            .unwrap_or_default();
        // An id is compared whole and a name is looked for inside one. The id is tried
        // first and exactly, so a session naming an account reaches that account and
        // nothing else: an identifier matched the forgiving way would be one that could
        // find a *different* person whose name happened to contain it.
        if wanted.as_ref().is_some_and(|named| !names(&account, named)) {
            continue;
        }
        // An administrator is left out of the agreement: the request service treats
        // one as holding every permission, so an owner approving their own requests is
        // what an owner is rather than a household disagreeing with itself.
        let held = naming.asked.members.get(&account.id);
        let approves_own = if account.access.administrator {
            None
        } else {
            held.map(|held| held.approves_own)
        };
        let made: Vec<&str> = theirs.made.iter().map(String::as_str).collect();
        let mut member = HouseholdMember {
            access: named_access(&account.access, naming, approves_own),
            asking: held.map(|held| allowance::reported(held, &made, naming.now)),
            last_seen: account.last_seen,
            claimed: account.claimed,
            name: account.name,
            requests: theirs.requests,
            to_hand_over: Vec::new(),
        };
        // Composed from the finished member rather than from the parts, so the message
        // and the line above it cannot report different figures for one person.
        member.to_hand_over =
            handing_over::to_hand_over(&member, naming.quality, naming.expiring, naming.no_room);
        members.push(member);
    }
    members.sort_by(|one, two| one.name.cmp(&two.name));

    // Whatever is left was asked for by somebody the media server holds no account
    // under. Said rather than dropped: a request outliving the account that made it is
    // exactly the kind of thing an operator is looking at this list to find.
    let unclaimed: Vec<String> = by_name.into_keys().collect();
    let mut findings = Vec::new();
    if !unclaimed.is_empty() {
        findings.push(format!(
            "the media server holds no account under {}, so what they asked for is not \
             listed under anybody",
            unclaimed.join(", ")
        ));
    }

    // The disagreement this reading exists to find, named rather than left to be
    // spotted in a column: somebody who cannot watch something and can still ask for it
    // has been given half a limit, and half a limit looks like a whole one.
    for held in &members {
        if held.access.restriction.disagrees() {
            findings.push(format!(
                "{} is held to what they may watch and not to what they may ask for, so \
                 what they cannot watch they can still fetch",
                held.name
            ));
        }
    }

    // Said the moment anybody carries a limit, and not before: there is no claim to be
    // modest about on a household nobody has narrowed, and the reader who most needs
    // the sentence is the parent who has just set one.
    let filtering = members
        .iter()
        .any(|held| held.access.restriction != Restriction::Unrestricted)
        .then(|| crate::age_limit::A_FILTER_NOT_A_LOCK.to_owned());

    findings.extend(allowance::worth_saying(
        &members,
        naming.expiring,
        naming.hosted,
    ));

    HouseholdReport {
        policy: naming.asked.household.as_ref().map(Policy::of),
        allows: naming
            .asked
            .household
            .as_ref()
            .and_then(|held| held.quota)
            .map(crate::asking::limit),
        members,
        findings,
        available: true,
        filtering,
    }
}

/// One member's requests, and when each was asked for.
///
/// The dates are gathered beside the requests rather than carried on them, because they
/// answer a different question: a report says how long something has been *waiting*,
/// and a period counts when it was *asked for* — which includes everything already
/// fetched.
#[derive(Default)]
struct Theirs {
    /// What they asked for, as the report carries it.
    requests: Vec<MemberRequest>,
    /// When each was asked for, as the request service timestamps it.
    made: Vec<String>,
}

/// The household view where the requests could not be read at all — said plainly, so an
/// empty list is never mistaken for a household that has asked for nothing.
fn unavailable(reason: &str) -> HouseholdReport {
    HouseholdReport {
        members: Vec::new(),
        findings: vec![reason.to_owned()],
        available: false,
        // Nothing was read, so there is no policy to report and no limit to state. A
        // household shown as trusting everybody because nobody could be asked is the
        // same defect as one shown as having asked for nothing.
        policy: None,
        allows: None,
        // Nothing to be modest about: nobody was read, so nobody is limited as far as
        // this answer knows, and a caution beside an empty list is a claim about a
        // household nobody saw.
        filtering: None,
    }
}

#[cfg(test)]
mod tests;
