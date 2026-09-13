//! What the things in this report are called, and what it says where a name will not read.
//!
//! The tables a member's line is said in, and the two reads that fill them. One module
//! because they are one decision made in several places: each is read once for the whole
//! household rather than once per member, and each costs the report names and never
//! contents when it will not read. Apart, that second half is several places for the same
//! rule to be forgotten in.

use std::collections::BTreeMap;
use std::time::SystemTime;

use crate::app::targets::open_servarrs;
use crate::app::Ctx;
use crate::asking::Reasons;
use crate::model::{MemberAccess, Restriction};
use crate::ports::service::{Access, Certificate, Household as _, HouseholdRequest, Pipeline};
use crate::quality::Selection;

use super::allowance;

/// The two tables the media server names things in, read once for the whole household.
///
/// One read each rather than one per member: what a library is called and what a
/// certificate is called are the same answers for everybody in the house. Each failure
/// costs its own names and nothing else — a library list that will not read leaves access
/// named by the server's own identifiers, and a rating table that will not read leaves an
/// age limit named from this program's own mapping rather than from the certificates this
/// household already recognises — so each says so rather than quietly reading as absent.
pub(super) async fn named_by_the_server(
    server: &crate::jellyfin::Jellyfin,
    findings: &mut Vec<String>,
) -> (BTreeMap<String, String>, Vec<Certificate>) {
    let libraries = if let Ok(held) = server.libraries().await {
        held.into_iter()
            .map(|library| (library.id, library.name))
            .collect()
    } else {
        findings.push(
            "the media server's libraries could not be read, so access limited to \
             some of them names them by the server's own identifiers"
                .to_owned(),
        );
        BTreeMap::new()
    };
    let certificates = server.ratings().await.unwrap_or_default();
    if certificates.is_empty() {
        findings.push(
            "the media server's own ratings could not be read, so an age limit is named \
             from lemonfiber's own mapping rather than from this household's certificates"
                .to_owned(),
        );
    }
    (libraries, certificates)
}

/// The tables every member's line is said in, gathered so the assembly takes one of
/// them rather than four.
///
/// Read once for the whole household and used once per member: what a library is
/// called, what an item is called, what a certificate is called and what somebody may
/// ask for are the same four answers for everybody in the house.
pub(super) struct Naming<'a> {
    /// Library identifier to the name the operator gave it.
    pub(super) libraries: &'a BTreeMap<String, String>,
    /// The title each \*arr knows its items by.
    pub(super) titles: &'a BTreeMap<(&'static str, i64), String>,
    /// The media server's own certificates, in the operator's country.
    pub(super) certificates: &'a [Certificate],
    /// Whether each member's requests arrive unseen, how much of their period is
    /// spent, and what the household is under where nobody chose otherwise.
    pub(super) asked: &'a allowance::Asked,
    /// The quality in force, which is what an estimate of a request's cost turns on.
    pub(super) quality: &'a Selection,
    /// Now, against which a request that is waiting is measured.
    pub(super) now: SystemTime,
    /// Why each request that was turned down from here was turned down. The request
    /// service keeps none, so this is the only place the words survive.
    pub(super) reasons: &'a Reasons,
    /// How many days a request may wait here before it is closed unanswered, where the
    /// household has arranged that at all.
    ///
    /// Carried into the reading because it changes what is *true* of every request still
    /// waiting, and both halves of this say so: the reminder to the operator names the
    /// period and what runs it, and the message a member is handed stops promising that
    /// nothing ends their wait. Nothing where nobody arranged one, which is what every
    /// household is under until somebody says otherwise.
    pub(super) expiring: Option<u32>,
    /// Whether the disk has no room left, which refuses an acquisition in the disk's
    /// own words and is a different answer from anybody's limit.
    pub(super) no_room: bool,
    /// Whether this machine is running the clock that closes what nobody rules on.
    ///
    /// Carried for one sentence, and it is the sentence this reading would otherwise
    /// get wrong in whichever direction the machine happened to be in: a reminder
    /// naming a period and saying nothing runs it, on a machine that is running it, is
    /// as misleading as one implying a background that is not there.
    pub(super) hosted: bool,
}

/// The title each \*arr knows its items by, keyed by the service and the id the request
/// service hands over — the exact-id join a request is named through, and the second
/// value saying whether every library actually answered.
///
/// A library that will not read costs names, not the view: the requests still report where
/// they stand, and the gap is surfaced rather than left to look like unnamed items.
pub(super) async fn library_titles(
    ctx: &Ctx,
    services: &[lemonfiber_manifest::Service],
) -> (BTreeMap<(&'static str, i64), String>, bool) {
    let mut titles = BTreeMap::new();
    let mut named = true;
    for arr in open_servarrs(ctx, services).await {
        match arr.service.library(arr.kind).await {
            Ok(items) => titles.extend(
                items
                    .into_iter()
                    .map(|item| ((arr.kind.section(), item.id), item.title)),
            ),
            Err(_) => named = false,
        }
    }
    (titles, named)
}

/// The same access, with the libraries said in the words the operator gave them.
///
/// An identifier the library list did not name is kept as it is rather than dropped: a
/// library missing from the list is still a library this member can watch, and showing
/// nothing there would read as access they do not have.
pub(super) fn named_access(
    access: &Access,
    naming: &Naming<'_>,
    approves_own: Option<bool>,
) -> MemberAccess {
    let mut said = MemberAccess {
        every_library: access.every_library,
        libraries: access
            .libraries
            .iter()
            .map(|id| naming.libraries.get(id).unwrap_or(id).clone())
            .collect(),
        age_limit: access.age_limit,
        rated: access
            .age_limit
            .map(|age| crate::rating::rated(naming.certificates, age)),
        unrated: access.unrated,
        restriction: Restriction::Unrestricted,
        administrator: access.administrator,
        disabled: access.disabled,
    };
    // Settled last because it is read off the rest of the shape: what somebody is held
    // to is both halves of their access taken together with what a second service says
    // they may ask for.
    said.restriction = Restriction::of(&said, approves_own);
    said
}

/// What a request is called, where the \*arr filing it has been told about it and its
/// library could be read. Nothing otherwise — a request still awaiting approval has been
/// handed to no service, so there is no title to find and none is invented.
pub(super) fn title_of(
    request: &HouseholdRequest,
    titles: &BTreeMap<(&'static str, i64), String>,
) -> Option<String> {
    let kind = request.kind?;
    let item = request.item?;
    titles.get(&(kind.section(), item)).cloned()
}
