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

use super::targets::{open_servarrs, seerr_reader};
use super::Ctx;
use crate::error::{Diagnose, Problem};
use crate::household::State;
use crate::model::{HouseholdMember, HouseholdReport, MemberRequest};
use crate::ports::service::{HouseholdRequest, Pipeline, Requests};
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

    let Some(access) = seerr_reader(ctx, &manifest.services) else {
        // No request service, or no credential to ask it with. Neither is a fault: there
        // is simply no household request to report, which is said rather than shown as an
        // empty list that would read as "nobody has asked for anything".
        return Ok(unavailable(
            "there is no request service to ask, or no recorded media-server password to \
             sign in with — so what the household has asked for cannot be read",
        ));
    };

    if access
        .seerr
        .sign_in(
            crate::config::JELLYFIN_ADMIN_USER,
            &access.password,
            &access.server_url,
        )
        .await
        .is_err()
    {
        return Ok(unavailable(
            "the request service would not accept the household's sign-in, so what it has \
             been asked for could not be read — reported as unavailable, not as nothing",
        ));
    }

    let Ok(requests) = access.seerr.requests().await else {
        return Ok(unavailable(
            "the request service's own record could not be read — reported as unavailable, \
             not read as nobody having asked for anything",
        ));
    };

    // One library read per service names every request that has been handed over, rather
    // than a lookup per request: the same read either way, made once.
    let (titles, named) = library_titles(ctx, &manifest.services).await;
    let mut report = assemble(requests, &titles, member);
    if !named {
        report.findings.push(
            "a library could not be read, so some requests are named by what they are \
             rather than by their title"
                .to_owned(),
        );
    }
    Ok(report)
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

/// Group the requests by the member who made each one, naming what can be named.
///
/// Members come out in name order, and each member's requests in the order the service
/// gave them — newest first, so the ones still worth asking about lead.
fn assemble(
    requests: Vec<HouseholdRequest>,
    titles: &BTreeMap<(&'static str, i64), String>,
    member: Option<&str>,
) -> HouseholdReport {
    let wanted = member.map(str::to_lowercase);
    let mut by_member: BTreeMap<String, Vec<MemberRequest>> = BTreeMap::new();
    for request in requests {
        if wanted
            .as_ref()
            .is_some_and(|name| !request.member.to_lowercase().contains(name))
        {
            continue;
        }
        by_member
            .entry(request.member.clone())
            .or_default()
            .push(MemberRequest {
                title: title_of(&request, titles),
                media: request.kind.map(Kind::noun).map(str::to_owned),
                state: State::of(request.request_status, request.media_status),
            });
    }

    HouseholdReport {
        members: by_member
            .into_iter()
            .map(|(name, requests)| HouseholdMember { name, requests })
            .collect(),
        findings: Vec::new(),
        available: true,
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

    use super::{assemble, household, title_of, Ctx};
    use crate::config::Settings;
    use crate::household::State;
    use crate::platform::Environment;
    use crate::ports::service::HouseholdRequest;
    use crate::recyclarr::Kind;
    use crate::test_support::{a_password, spoke, stack, Reporting, Scripted, SeedFs};
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

    /// A transport answering the request service's sign-in and read, and the \*arr
    /// libraries, by the shape of the URL.
    struct Fake {
        sign_in: &'static str,
        requests: &'static str,
        library: &'static str,
        refuse: bool,
    }

    impl Fake {
        /// The scripted answers as a transport, routed by what each call asks for.
        fn transport(&self) -> Arc<Transport> {
            Transport::by_path(vec![
                (
                    "/auth/jellyfin",
                    Answer::reply(if self.refuse { 500 } else { 200 }, self.sign_in),
                ),
                ("/api/v1/request", Answer::reply(200, self.requests)),
                ("", Answer::reply(200, self.library)),
            ])
        }
    }

    /// A context whose request service can be reached: the media-server password is
    /// recorded, so `seerr_reader` resolves a client. Tagged so each test keeps its own
    /// env file rather than racing on a shared one.
    fn ctx_with(fake: Fake, tag: &str) -> Ctx {
        let dir =
            std::env::temp_dir().join(format!("lemonfiber-household-{tag}-{}", std::process::id()));
        let _ = std::fs::create_dir_all(&dir);
        let mut context = Ctx::new(
            Arc::new(Scripted(Ok(spoke("")))),
            Arc::new(Reporting::absent()),
            Arc::new(crate::adapters::System),
            Arc::new(crate::adapters::Disk),
            stack(),
            Settings::default(),
            Environment::MacOs,
        )
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
            vec![
                request("Sam", Some(Kind::Radarr), Some(7), (2, 5)),
                request("Alex", Some(Kind::Sonarr), Some(11), (2, 4)),
                request("Alex", Some(Kind::Radarr), None, (1, 1)),
            ],
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

    #[test]
    fn a_request_is_named_by_the_library_the_service_handed_it_to() {
        let report = assemble(
            vec![request("Alex", Some(Kind::Sonarr), Some(11), (2, 4))],
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
            vec![request("Sam", Some(Kind::Radarr), None, (1, 1))],
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
        let report = assemble(requests, &titles(), Some("alex"));
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
            Fake {
                sign_in: "",
                requests: r#"{"pageInfo":{"results":1},"results":[
                    {"status":2,"type":"tv","media":{"status":5,"externalServiceId":1},
                     "requestedBy":{"displayName":"Alex"}}
                ]}"#,
                library: r#"[{"id":1,"title":"The Expanse","monitored":true}]"#,
                refuse: false,
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
            Fake {
                sign_in: "",
                requests: r#"{"pageInfo":{"results":1},"results":[
                    {"status":2,"type":"tv","media":{"status":5,"externalServiceId":1},
                     "requestedBy":{"displayName":"Alex"}}
                ]}"#,
                library: r#"[{"id":1,"title":"The Expanse","monitored":true}]"#,
                refuse: false,
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
    async fn a_refused_sign_in_is_reported_rather_than_read_as_an_empty_household() {
        let context = ctx_with(
            Fake {
                sign_in: "no",
                requests: "",
                library: "[]",
                refuse: true,
            },
            "refused",
        );
        let report = household(&context, None).await.unwrap_or_default();
        assert!(!report.available);
        assert!(report.members.is_empty());
        assert!(report
            .findings
            .iter()
            .any(|finding| finding.contains("would not accept")));
    }

    #[tokio::test]
    async fn an_unreadable_request_record_is_reported_as_unavailable() {
        let context = ctx_with(
            Fake {
                sign_in: "",
                requests: "not json",
                library: "[]",
                refuse: false,
            },
            "unreadable",
        );
        let report = household(&context, None).await.unwrap_or_default();
        assert!(!report.available);
        assert!(report
            .findings
            .iter()
            .any(|finding| finding.contains("could not be read")));
    }

    #[tokio::test]
    async fn an_unreadable_library_costs_names_not_the_view() {
        let context = ctx_with(
            Fake {
                sign_in: "",
                requests: r#"{"pageInfo":{"results":1},"results":[
                    {"status":2,"type":"tv","media":{"status":5,"externalServiceId":1},
                     "requestedBy":{"displayName":"Alex"}}
                ]}"#,
                library: "not json",
                refuse: false,
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
        let context = Ctx::new(
            Arc::new(Scripted(Ok(spoke("")))),
            Arc::new(Reporting::absent()),
            Arc::new(crate::adapters::System),
            Arc::new(crate::adapters::Disk),
            stack(),
            Settings::default(),
            Environment::MacOs,
        )
        .with_filesystem(Arc::new(SeedFs::keyed(Some(KEYED), None)))
        .with_http(
            Fake {
                sign_in: "",
                requests: "",
                library: "[]",
                refuse: false,
            }
            .transport(),
        );
        let report = household(&context, None).await.unwrap_or_default();
        assert!(!report.available);
        assert!(report
            .findings
            .iter()
            .any(|finding| finding.contains("no request service")));
    }

    #[tokio::test]
    async fn a_household_view_over_an_unreadable_stack_is_an_error() {
        let mut context = ctx_with(
            Fake {
                sign_in: "",
                requests: "",
                library: "[]",
                refuse: false,
            },
            "badstack",
        );
        context.stack = crate::stack::Source::External(std::path::Path::new("/nowhere/at/all"));
        assert!(household(&context, None).await.is_err());
    }
}
