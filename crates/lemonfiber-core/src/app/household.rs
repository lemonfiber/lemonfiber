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

use std::collections::BTreeMap;

use super::targets::{jellyfin_reader, open_servarrs, seerr_reader};
use super::Ctx;
use crate::error::{Diagnose, Problem};
use crate::household::State;
use crate::model::{HouseholdMember, HouseholdReport, MemberAccess, MemberRequest};
use crate::ports::service::{Access, Household as _, HouseholdRequest, Member, Pipeline, Requests};
use crate::recyclarr::Kind;

/// Read the household's requests, grouped by the member who made each one.
///
/// `member` narrows to one person, matched the forgiving way a name is typed rather than
/// by an exact string — the same courtesy the trace extends to a title.
pub(super) async fn household(
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
    // One read for the whole household rather than one per member: what a library is
    // called is the same answer for everybody.
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

    // A request service that will not answer costs the requests, not the household.
    // Who is here is the media server's fact, and reporting nobody because a second
    // service is down would be this same defect one service along.
    let requests = match asked_for(ctx, &manifest.services).await {
        Ok(requests) => requests,
        Err(reason) => {
            findings.push(reason);
            Vec::new()
        }
    };

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

    let mut report = assemble(accounts, requests, &libraries, &titles, member);
    report.findings.append(&mut findings);
    Ok(report)
}

/// What the household has asked for, or in plain words why it could not be read.
///
/// A reason rather than an error: none of these stops the household being listed, and
/// each is something the operator can act on.
async fn asked_for(
    ctx: &Ctx,
    services: &[lemonfiber_manifest::Service],
) -> Result<Vec<HouseholdRequest>, String> {
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

    access.seerr.requests().await.map_err(|_| {
        "the request service's own record could not be read, so what the household has \
         asked for is not shown"
            .to_owned()
    })
}

/// The title each \*arr knows its items by, keyed by the service and the id the request
/// service hands over — the exact-id join a request is named through, and the second
/// value saying whether every library actually answered.
///
/// A library that will not read costs names, not the view: the requests still report where
/// they stand, and the gap is surfaced rather than left to look like unnamed items.
async fn library_titles(
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

/// The household, member by member, with what each asked for joined onto them.
///
/// Members come out in name order, and each member's requests in the order the service
/// gave them — newest first, so the ones still worth asking about lead.
fn assemble(
    accounts: Vec<Member>,
    requests: Vec<HouseholdRequest>,
    libraries: &BTreeMap<String, String>,
    titles: &BTreeMap<(&'static str, i64), String>,
    member: Option<&str>,
) -> HouseholdReport {
    let wanted = member.map(str::to_lowercase);

    // Keyed by the lower-cased name: the media server treats two names differing only
    // in case as the same person, so a join on the exact string would file a member's
    // own requests under nobody.
    let mut by_name: BTreeMap<String, Vec<MemberRequest>> = BTreeMap::new();
    for request in requests {
        by_name
            .entry(request.member.to_lowercase())
            .or_default()
            .push(MemberRequest {
                title: title_of(&request, titles),
                media: request.kind.map(Kind::noun).map(str::to_owned),
                state: State::of(request.request_status, request.media_status),
            });
    }

    let mut members: Vec<HouseholdMember> = Vec::new();
    for account in accounts {
        // Taken before the narrowing below, so asking about one person does not leave
        // everybody else's requests looking like requests belonging to nobody.
        let asked = by_name
            .remove(&account.name.to_lowercase())
            .unwrap_or_default();
        if wanted
            .as_ref()
            .is_some_and(|name| !account.name.to_lowercase().contains(name))
        {
            continue;
        }
        members.push(HouseholdMember {
            access: named_access(&account.access, libraries),
            last_seen: account.last_seen,
            claimed: account.claimed,
            name: account.name,
            requests: asked,
        });
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

    HouseholdReport {
        members,
        findings,
        available: true,
    }
}

/// The same access, with the libraries said in the words the operator gave them.
///
/// An identifier the library list did not name is kept as it is rather than dropped: a
/// library missing from the list is still a library this member can watch, and showing
/// nothing there would read as access they do not have.
fn named_access(access: &Access, libraries: &BTreeMap<String, String>) -> MemberAccess {
    MemberAccess {
        every_library: access.every_library,
        libraries: access
            .libraries
            .iter()
            .map(|id| libraries.get(id).unwrap_or(id).clone())
            .collect(),
        age_limit: access.age_limit,
        administrator: access.administrator,
        disabled: access.disabled,
    }
}

/// What a request is called, where the \*arr filing it has been told about it and its
/// library could be read. Nothing otherwise — a request still awaiting approval has been
/// handed to no service, so there is no title to find and none is invented.
fn title_of(
    request: &HouseholdRequest,
    titles: &BTreeMap<(&'static str, i64), String>,
) -> Option<String> {
    let kind = request.kind?;
    let item = request.item?;
    titles.get(&(kind.section(), item)).cloned()
}

/// The household view where the requests could not be read at all — said plainly, so an
/// empty list is never mistaken for a household that has asked for nothing.
fn unavailable(reason: &str) -> HouseholdReport {
    HouseholdReport {
        members: Vec::new(),
        findings: vec![reason.to_owned()],
        available: false,
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use lemonfiber_fixtures::http::{Answer, Fake as Transport};

    use super::{asked_for, assemble, household, title_of, Ctx};
    use crate::household::State;
    use crate::ports::service::{Access, HouseholdRequest, Member};
    use crate::recyclarr::Kind;
    use crate::test_support::{a_context, a_password, SeedFs};
    use std::collections::BTreeMap;

    /// A Servarr config that opens a target, carrying a readable key.
    const KEYED: &str = "<Config><ApiKey>the-key</ApiKey></Config>";

    /// A request as the service records it, for the grouping tests.
    fn request(
        member: &str,
        kind: Option<Kind>,
        item: Option<i64>,
        statuses: (u8, u8),
    ) -> HouseholdRequest {
        HouseholdRequest {
            member: member.to_owned(),
            kind,
            item,
            request_status: statuses.0,
            media_status: statuses.1,
        }
    }

    /// A title map holding one series and one film.
    fn titles() -> BTreeMap<(&'static str, i64), String> {
        let mut titles = BTreeMap::new();
        titles.insert((Kind::Sonarr.section(), 11), "The Expanse".to_owned());
        titles.insert((Kind::Radarr.section(), 7), "Dune".to_owned());
        titles
    }

    /// A transport answering the media server's accounts, the request service's
    /// sign-in and read, and the \*arr libraries, by the shape of the URL.
    struct Fake {
        accounts: &'static str,
        folders: &'static str,
        sign_in: &'static str,
        requests: &'static str,
        library: &'static str,
        refuse: bool,
    }

    impl Default for Fake {
        fn default() -> Self {
            Self {
                accounts: r#"[{"Id":"a1","Name":"Alex","HasPassword":true,
                    "Policy":{"EnableAllFolders":true},
                    "LastActivityDate":"2026-08-30T10:00:00Z"}]"#,
                folders: r#"{"Items":[{"Id":"lib-1","Name":"Films"}]}"#,
                sign_in: "",
                requests: "",
                library: "[]",
                refuse: false,
            }
        }
    }

    impl Fake {
        /// The scripted answers as a transport, routed by what each call asks for.
        fn transport(&self) -> Arc<Transport> {
            Transport::by_path(vec![
                // Ahead of `/Users`, whose text it contains: the media server signs
                // this program in before it will answer anything about accounts, and
                // a route matched by prefix would answer the sign-in with the list.
                (
                    "/Users/AuthenticateByName",
                    Answer::reply(200, r#"{"AccessToken":"token"}"#),
                ),
                ("/Library/MediaFolders", Answer::reply(200, self.folders)),
                ("/Users", Answer::reply(200, self.accounts)),
                (
                    "/auth/jellyfin",
                    Answer::reply(if self.refuse { 500 } else { 200 }, self.sign_in),
                ),
                ("/api/v1/request", Answer::reply(200, self.requests)),
                ("", Answer::reply(200, self.library)),
            ])
        }
    }

    /// An account the media server holds, for the joining tests.
    ///
    /// Somebody who has claimed theirs has been seen; an unclaimed invitation has not,
    /// which is the whole difference between the two.
    fn account(name: &str, claimed: bool) -> Member {
        Member {
            id: format!("id-{}", name.to_lowercase()),
            name: name.to_owned(),
            claimed,
            access: Access {
                every_library: true,
                ..Access::default()
            },
            last_seen: claimed.then(|| "2026-08-30T10:00:00Z".to_owned()),
        }
    }

    /// No library names read, which the joining tests do not depend on.
    fn unnamed() -> BTreeMap<String, String> {
        BTreeMap::new()
    }

    /// A context whose request service can be reached: the media-server password is
    /// recorded, so `seerr_reader` resolves a client. Tagged so each test keeps its own
    /// env file rather than racing on a shared one.
    fn ctx_with(fake: &Fake, tag: &str) -> Ctx {
        let dir =
            std::env::temp_dir().join(format!("lemonfiber-household-{tag}-{}", std::process::id()));
        let _ = std::fs::create_dir_all(&dir);
        let mut context = a_context()
            .build()
            .with_filesystem(Arc::new(SeedFs::keyed(Some(KEYED), None)))
            .with_http(fake.transport());
        context.settings.env_file = Some(dir.join(".env"));
        crate::app::targets::record_secret(
            &context,
            crate::config::JELLYFIN_ADMIN_PASSWORD_KEY,
            &a_password(),
        );
        context
    }

    #[test]
    fn requests_are_grouped_by_who_asked_in_name_order() {
        let report = assemble(
            vec![account("Sam", true), account("Alex", true)],
            vec![
                request("Sam", Some(Kind::Radarr), Some(7), (2, 5)),
                request("Alex", Some(Kind::Sonarr), Some(11), (2, 4)),
                request("Alex", Some(Kind::Radarr), None, (1, 1)),
            ],
            &unnamed(),
            &titles(),
            None,
        );
        let names: Vec<&str> = report
            .members
            .iter()
            .map(|member| member.name.as_str())
            .collect();
        assert_eq!(names, vec!["Alex", "Sam"]);
        let counts: Vec<usize> = report
            .members
            .iter()
            .map(|member| member.requests.len())
            .collect();
        assert_eq!(counts, vec![2, 1]);
        assert!(report.available);
    }

    /// The defect this read was rebuilt to fix.
    ///
    /// Sourced from the requests, somebody with an account who has never asked for
    /// anything did not appear at all — a list of *requesters* wearing the name of a
    /// list of members. Sourced from the accounts, they do.
    #[test]
    fn somebody_who_has_asked_for_nothing_is_still_in_the_household() {
        let report = assemble(
            vec![account("Alex", true), account("Sam", true)],
            vec![request("Alex", Some(Kind::Sonarr), Some(11), (2, 4))],
            &unnamed(),
            &titles(),
            None,
        );

        let listed: Vec<(&str, usize)> = report
            .members
            .iter()
            .map(|member| (member.name.as_str(), member.requests.len()))
            .collect();
        assert_eq!(
            listed,
            vec![("Alex", 1), ("Sam", 0)],
            "somebody who has asked for nothing is missing from their own household"
        );
    }

    /// An account nobody has set a password on is an invitation, and reads as one.
    ///
    /// Never seen, because being seen is signing in and setting the first password is
    /// how you do that — so the two facts always agree and neither is guessed.
    #[test]
    fn an_invitation_nobody_has_taken_up_is_listed_as_one() {
        let report = assemble(
            vec![account("Ana", false)],
            Vec::new(),
            &unnamed(),
            &titles(),
            None,
        );

        let waiting = report.members.first();
        assert_eq!(
            waiting.map(|member| member.claimed),
            Some(false),
            "{report:?}"
        );
        assert_eq!(
            waiting.and_then(|member| member.last_seen.clone()),
            None,
            "{report:?}"
        );
    }

    /// Access is said in the words the operator gave their libraries.
    ///
    /// An identifier the library list did not name is kept rather than dropped: a
    /// library missing from that list is still one this member can watch, and showing
    /// nothing would read as access they do not have.
    #[test]
    fn access_names_the_libraries_it_can_and_keeps_the_ones_it_cannot() {
        let mut named = BTreeMap::new();
        named.insert("lib-1".to_owned(), "Films".to_owned());
        let limited = Member {
            access: Access {
                every_library: false,
                libraries: vec!["lib-1".to_owned(), "lib-9".to_owned()],
                age_limit: Some(12),
                ..Access::default()
            },
            ..account("Ana", true)
        };

        let report = assemble(vec![limited], Vec::new(), &named, &titles(), None);

        let access = report.members.first().map(|member| &member.access);
        assert_eq!(
            access.map(|access| access.libraries.clone()),
            Some(vec!["Films".to_owned(), "lib-9".to_owned()]),
            "{report:?}"
        );
        assert_eq!(
            access.and_then(|access| access.age_limit),
            Some(12),
            "{report:?}"
        );
    }

    /// A request outliving the account that made it is said, not silently dropped.
    #[test]
    fn a_request_from_somebody_with_no_account_is_said_rather_than_dropped() {
        let report = assemble(
            vec![account("Alex", true)],
            vec![request("Gone", Some(Kind::Radarr), Some(7), (2, 5))],
            &unnamed(),
            &titles(),
            None,
        );

        assert!(
            report
                .findings
                .iter()
                .any(|finding| finding.contains("gone")),
            "a request belonging to nobody vanished without a word: {report:?}"
        );
    }

    /// Narrowing to one person does not make everybody else look accountless.
    ///
    /// Their requests are taken off the pile before the narrowing, so the finding
    /// above stays about requests that really belong to nobody.
    #[test]
    fn narrowing_does_not_turn_everybody_else_into_a_missing_account() {
        let report = assemble(
            vec![account("Alex", true), account("Sam", true)],
            vec![request("Sam", Some(Kind::Radarr), Some(7), (2, 5))],
            &unnamed(),
            &titles(),
            Some("alex"),
        );

        assert!(
            report.findings.is_empty(),
            "asking about one member reported everybody else's requests as orphans: {report:?}"
        );
    }

    #[test]
    fn a_request_is_named_by_the_library_the_service_handed_it_to() {
        let report = assemble(
            vec![account("Alex", true)],
            vec![request("Alex", Some(Kind::Sonarr), Some(11), (2, 4))],
            &unnamed(),
            &titles(),
            None,
        );
        let first = report.members.first().and_then(|m| m.requests.first());
        assert_eq!(
            first.and_then(|request| request.title.clone()),
            Some("The Expanse".to_owned())
        );
        assert_eq!(
            first.and_then(|request| request.state),
            Some(State::PartlyHere)
        );
    }

    #[test]
    fn a_request_no_service_holds_yet_is_named_by_what_it_is() {
        // Nothing has been handed over, so there is no title to find — and none is
        // invented. What it is still reads, so the line is not blank.
        let report = assemble(
            vec![account("Sam", true)],
            vec![request("Sam", Some(Kind::Radarr), None, (1, 1))],
            &unnamed(),
            &titles(),
            None,
        );
        let first = report.members.first().and_then(|m| m.requests.first());
        assert_eq!(first.and_then(|request| request.title.clone()), None);
        assert_eq!(
            first.and_then(|request| request.media.clone()),
            Some("film".to_owned())
        );
        assert_eq!(
            first.and_then(|request| request.state),
            Some(State::WaitingForApproval)
        );
    }

    #[test]
    fn an_item_the_library_does_not_hold_is_left_unnamed() {
        // Handed over, but the library has no such id — the join simply does not land,
        // and nothing is guessed from it.
        assert_eq!(
            title_of(
                &request("Alex", Some(Kind::Sonarr), Some(999), (2, 5)),
                &titles()
            ),
            None
        );
        // Nor is a film's id looked up against the television library.
        assert_eq!(
            title_of(
                &request("Alex", Some(Kind::Sonarr), Some(7), (2, 5)),
                &titles()
            ),
            None
        );
        // A request whose kind this build does not know is never joined at all.
        assert_eq!(
            title_of(&request("Alex", None, Some(11), (2, 5)), &titles()),
            None
        );
    }

    #[test]
    fn narrowing_to_one_member_is_forgiving_about_how_the_name_is_typed() {
        let requests = vec![
            request("Alex", Some(Kind::Sonarr), Some(11), (2, 4)),
            request("Sam", Some(Kind::Radarr), Some(7), (2, 5)),
        ];
        let report = assemble(
            vec![account("Alex", true), account("Sam", true)],
            requests,
            &unnamed(),
            &titles(),
            Some("alex"),
        );
        let names: Vec<&str> = report
            .members
            .iter()
            .map(|member| member.name.as_str())
            .collect();
        assert_eq!(names, vec!["Alex"]);
    }

    #[tokio::test]
    async fn the_household_view_reads_the_requests_and_names_them_from_the_library() {
        let context = ctx_with(
            &Fake {
                sign_in: "",
                requests: r#"{"pageInfo":{"results":1},"results":[
                    {"status":2,"type":"tv","media":{"status":5,"externalServiceId":1},
                     "requestedBy":{"displayName":"Alex"}}
                ]}"#,
                library: r#"[{"id":1,"title":"The Expanse","monitored":true}]"#,
                refuse: false,
                ..Fake::default()
            },
            "reads",
        );
        let report = household(&context, None).await.unwrap_or_default();
        assert!(report.available);
        let first = report.members.first().and_then(|m| m.requests.first());
        assert_eq!(
            first.and_then(|request| request.title.clone()),
            Some("The Expanse".to_owned())
        );
        assert_eq!(first.and_then(|request| request.state), Some(State::Here));
    }

    #[tokio::test]
    async fn a_service_still_starting_costs_names_without_being_called_a_failed_read() {
        // No key is readable yet, so no library opens. That is a service still coming up
        // rather than one that refused, so it is skipped — the requests still report
        // where they stand, and nothing claims a read failed that was never made.
        let mut context = ctx_with(
            &Fake {
                sign_in: "",
                requests: r#"{"pageInfo":{"results":1},"results":[
                    {"status":2,"type":"tv","media":{"status":5,"externalServiceId":1},
                     "requestedBy":{"displayName":"Alex"}}
                ]}"#,
                library: r#"[{"id":1,"title":"The Expanse","monitored":true}]"#,
                refuse: false,
                ..Fake::default()
            },
            "starting",
        );
        context = context.with_filesystem(Arc::new(SeedFs::keyed(None, None)));
        let report = household(&context, None).await.unwrap_or_default();
        assert!(report.available);
        let first = report.members.first().and_then(|m| m.requests.first());
        assert_eq!(first.and_then(|request| request.title.clone()), None);
        assert_eq!(first.and_then(|request| request.state), Some(State::Here));
        // Skipped, not failed: no unreadable-library finding is raised.
        assert!(report.findings.is_empty());
    }

    #[tokio::test]
    async fn a_refused_sign_in_costs_the_requests_and_not_the_household() {
        // Who is in the house is the media server's fact. Reporting nobody because the
        // *request* service refused would be the same defect this read was built to
        // fix, one service along — so the members still list and the refusal is said.
        let context = ctx_with(
            &Fake {
                sign_in: "no",
                refuse: true,
                ..Fake::default()
            },
            "refused",
        );
        let report = household(&context, None).await.unwrap_or_default();
        assert!(report.available);
        assert_eq!(
            report
                .members
                .iter()
                .map(|member| member.name.as_str())
                .collect::<Vec<&str>>(),
            vec!["Alex"],
            "a refused request service emptied the household: {report:?}"
        );
        assert!(
            report
                .findings
                .iter()
                .any(|finding| finding.contains("would not accept")),
            "{report:?}"
        );
    }

    #[tokio::test]
    async fn an_unreadable_request_record_costs_the_requests_and_not_the_household() {
        let context = ctx_with(
            &Fake {
                sign_in: "",
                requests: "not json",
                library: "[]",
                refuse: false,
                ..Fake::default()
            },
            "unreadable",
        );
        let report = household(&context, None).await.unwrap_or_default();
        assert!(report.available);
        assert_eq!(
            report
                .members
                .iter()
                .map(|member| member.name.as_str())
                .collect::<Vec<&str>>(),
            vec!["Alex"],
            "an unreadable request record emptied the household: {report:?}"
        );
        assert!(
            report
                .findings
                .iter()
                .any(|finding| finding.contains("could not be read")),
            "{report:?}"
        );
    }

    /// With nothing to sign in to the request service with, that is said and the
    /// household still reads.
    ///
    /// Driven at `asked_for` directly: reached through the whole command, the media
    /// server's own reader refuses first for the same missing password, so the branch
    /// this is about is never the one that answers.
    #[tokio::test]
    async fn with_nothing_to_ask_the_request_service_with_the_requests_are_what_is_lost() {
        let context = a_context().build();
        let services = context
            .stack
            .checked_manifest(context.today())
            .map(|manifest| manifest.services)
            .unwrap_or_default();
        assert!(
            !services.is_empty(),
            "the shipped stack declared no services, so this asserts nothing"
        );

        let asked = asked_for(&context, &services).await;

        assert!(
            asked.is_err_and(|reason| reason.contains("no request service")),
            "a stack with nothing to sign in with did not say so"
        );
    }

    /// A media server that will not say who holds an account is unavailable, not empty.
    ///
    /// The one refusal that *does* blank the list, because the accounts are where the
    /// household comes from — and it is said rather than shown as a house with nobody
    /// in it, which is the reading an empty list would invite.
    #[tokio::test]
    async fn a_media_server_that_will_not_say_who_is_here_is_unavailable_not_empty() {
        let context = ctx_with(
            &Fake {
                accounts: "not json",
                ..Fake::default()
            },
            "unreadable-accounts",
        );

        let report = household(&context, None).await.unwrap_or_default();

        assert!(!report.available, "{report:?}");
        assert!(report.members.is_empty(), "{report:?}");
        assert!(
            report
                .findings
                .iter()
                .any(|finding| finding.contains("would not say who holds an account")),
            "{report:?}"
        );
    }

    /// Libraries that will not read cost their names, not the access.
    ///
    /// The member still reports what they may watch — the server said which libraries,
    /// and only what they are *called* is missing, so the identifiers stand in and a
    /// finding says why.
    #[tokio::test]
    async fn libraries_that_will_not_read_cost_their_names_and_not_the_access() {
        let context = ctx_with(
            &Fake {
                folders: "not json",
                ..Fake::default()
            },
            "unnamed-libraries",
        );

        let report = household(&context, None).await.unwrap_or_default();

        assert!(report.available, "{report:?}");
        assert!(
            report
                .findings
                .iter()
                .any(|finding| finding.contains("libraries could not be read")),
            "{report:?}"
        );
    }

    #[tokio::test]
    async fn an_unreadable_library_costs_names_not_the_view() {
        let context = ctx_with(
            &Fake {
                sign_in: "",
                requests: r#"{"pageInfo":{"results":1},"results":[
                    {"status":2,"type":"tv","media":{"status":5,"externalServiceId":1},
                     "requestedBy":{"displayName":"Alex"}}
                ]}"#,
                library: "not json",
                refuse: false,
                ..Fake::default()
            },
            "unnamed",
        );
        let report = household(&context, None).await.unwrap_or_default();
        // The request still reports where it stands; only its name is missing, and the
        // gap is said rather than left to look like an item with no title.
        assert!(report.available);
        let first = report.members.first().and_then(|m| m.requests.first());
        assert_eq!(first.and_then(|request| request.title.clone()), None);
        assert_eq!(first.and_then(|request| request.state), Some(State::Here));
        assert!(report
            .findings
            .iter()
            .any(|finding| finding.contains("library could not be read")));
    }

    #[tokio::test]
    async fn a_stack_with_no_recorded_password_has_nothing_to_ask_with() {
        // No env file, so no recorded media-server password — there is no account to
        // sign in as, which is said rather than shown as a household that asked for
        // nothing.
        let context = a_context()
            .build()
            .with_filesystem(Arc::new(SeedFs::keyed(Some(KEYED), None)))
            .with_http(
                Fake {
                    sign_in: "",
                    requests: "",
                    library: "[]",
                    refuse: false,
                    ..Fake::default()
                }
                .transport(),
            );
        let report = household(&context, None).await.unwrap_or_default();
        assert!(!report.available);
        assert!(
            report
                .findings
                .iter()
                .any(|finding| finding.contains("no recorded")),
            "{report:?}"
        );
    }

    #[tokio::test]
    async fn a_household_view_over_an_unreadable_stack_is_an_error() {
        let mut context = ctx_with(
            &Fake {
                sign_in: "",
                requests: "",
                library: "[]",
                refuse: false,
                ..Fake::default()
            },
            "badstack",
        );
        context.stack = crate::stack::Source::External(std::path::Path::new("/nowhere/at/all"));
        assert!(household(&context, None).await.is_err());
    }
}
