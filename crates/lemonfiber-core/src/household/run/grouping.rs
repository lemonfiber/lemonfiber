//! The household's requests, gathered under whoever made each one.

use std::collections::BTreeMap;

use super::allowance;
use super::naming::{titled, Naming};
use crate::collected::Reference;
use crate::household::State;
use crate::model::MemberRequest;
use crate::ports::media::{Kind, TitleKind};
use crate::ports::service::{HouseholdRequest, Member};

/// Whether this account is the one the caller named.
///
/// Two ways of naming one person, because two callers name them differently. A person
/// types a name and means whoever they were thinking of, so it is looked for inside one
/// — the same courtesy the trace extends to a title. A session carries the id the media
/// server assigned and means that account and no other, so it is compared whole.
///
/// **An id names one account and nothing else.** Where `named` is an account's id it is
/// compared whole against every account and never looked for inside a name: this
/// narrowing decides whose requests somebody is shown, and the one reading it must never
/// produce is a member handed another member's row because that row's name happened to
/// contain their id.
///
/// `named` arrives lower-cased, because the caller lower-cases it once rather than this
/// doing it per account.
pub(super) fn names(account: &Member, named: &str, by_id: bool) -> bool {
    if by_id {
        return account.id.to_lowercase() == named;
    }
    account.name.to_lowercase().contains(named)
}

/// The id of the account a request is filed under, lower-cased, or the name it was
/// asked under where no account answers to it.
///
/// By the media server's id wherever the record carries one, because that is the one
/// thing about the requester they cannot change: the name the request service shows is
/// theirs to set, and joined on it a member could rename themselves after somebody else
/// and have their requests, and their count, filed under that person. Only a record
/// carrying no id at all is filed by name, which is all it has.
///
/// `ids` is every account's lower-cased name against its lower-cased id.
fn whose(request: &HouseholdRequest, ids: &BTreeMap<String, String>) -> String {
    if let Some(id) = &request.member_id {
        return id.to_lowercase();
    }
    let named = request.member.to_lowercase();
    ids.get(&named).cloned().unwrap_or(named)
}

/// The requests, gathered under whoever made each one.
///
/// Keyed by the lower-cased id of the account a request is filed under, or by the name it
/// was asked under where no account answers to it — see [`whose`].
pub(super) fn grouped(
    requests: Vec<HouseholdRequest>,
    accounts: &[Member],
    naming: &Naming<'_>,
) -> BTreeMap<String, Theirs> {
    // Lower-cased either way: the media server treats two names differing only in case
    // as the same person, so a join on the exact string would file a member's own
    // requests under nobody.
    let ids: BTreeMap<String, String> = accounts
        .iter()
        .map(|account| (account.name.to_lowercase(), account.id.to_lowercase()))
        .collect();
    let mut by_whom: BTreeMap<String, Theirs> = BTreeMap::new();
    for request in requests {
        let state = State::of(request.request_status, request.media_status);
        let made = request.made;
        let theirs = by_whom.entry(whose(&request, &ids)).or_default();
        if theirs.shown_as.is_empty() {
            theirs.shown_as = request.member.to_lowercase();
        }
        // Kept beside the requests rather than read back off them: what a period counts
        // is when something was asked for, and a request already fetched is still inside
        // the window that counted it.
        //
        // A request that was turned down is not, which is the service's own arithmetic
        // and not a choice made here: it excludes a declined one from the count, so
        // counting its date would name a day the window is not waiting on.
        if let Some(made) = made.filter(|_| state != Some(State::Declined)) {
            theirs.made.push(made);
        }
        let named = titled(&request, naming.titles).or_else(|| naming.unheld.get(&request.id));
        theirs.requests.push(MemberRequest {
            id: request.id.to_string(),
            asked: request.made,
            title: named.map(|named| named.title.clone()),
            year: named.and_then(|named| named.year),
            arrived: request.arrived,
            shelf_id: request.shelf_id.clone(),
            media: request.kind.map(Kind::noun).map(str::to_owned),
            kind: request.kind.map(TitleKind::from),
            unit: None,
            state,
            waiting_days: allowance::waiting(state, made, naming.now),
            estimate: allowance::estimated(request.kind, naming.quality),
            refused: naming.reasons.of(Reference::Filed(request.id)).cloned(),
        });
    }
    by_whom
}

/// One member's requests, and when each was asked for.
///
/// The dates are gathered beside the requests rather than carried on them, because they
/// answer a different question: a report says how long something has been *waiting*,
/// and a period counts when it was *asked for* — which includes everything already
/// fetched.
#[derive(Default)]
pub(super) struct Theirs {
    /// What they asked for, as the report carries it.
    pub(super) requests: Vec<MemberRequest>,
    /// When each was asked for, as the request service says.
    pub(super) made: Vec<crate::instant::Instant>,
    /// The name the request service showed them by, lower-cased, for saying who asked
    /// where nobody in the household did.
    pub(super) shown_as: String,
}
