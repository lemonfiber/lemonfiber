//! A diagnosis: which checks run, and what their findings carry.

use super::*;

fn diagnosis(
    outcome: Result<Outcome, Box<super::super::Problem>>,
) -> Option<crate::model::DoctorReport> {
    match outcome {
        Ok(Outcome::Doctor(report)) => Some(report),
        Ok(
            Outcome::Version(_)
            | Outcome::Alerts(_)
            | Outcome::Migration(_)
            | Outcome::History(_)
            | Outcome::Adoption(_)
            | Outcome::Beside(_)
            | Outcome::Replacement(_)
            | Outcome::Import(_)
            | Outcome::Forms(_)
            | Outcome::Preview(_)
            | Outcome::Lifecycle(_)
            | Outcome::Config(_)
            | Outcome::Quality(_)
            | Outcome::Upgrade(_)
            | Outcome::Music(_)
            | Outcome::Trace(_)
            | Outcome::Hosting(_)
            | Outcome::Household(_)
            | Outcome::Held(_)
            | Outcome::FrontDoor(_)
            | Outcome::Stuck(_)
            | Outcome::Word(_)
            | Outcome::Glossary(_)
            | Outcome::Clients(_)
            | Outcome::Invited(_)
            | Outcome::Removed(_)
            | Outcome::Catalogue(_)
            | Outcome::Wiring(_)
            | Outcome::Substituted(_)
            | Outcome::Outbound(_)
            | Outcome::Plugins(_)
            | Outcome::Provenance(_)
            | Outcome::Credentials(_)
            | Outcome::Stored(_)
            | Outcome::SelfUpdate(_)
            | Outcome::Space(_)
            | Outcome::Letting(_)
            | Outcome::Bandwidth(_)
            | Outcome::Status(_)
            | Outcome::Repair(_)
            | Outcome::Undo(_)
            | Outcome::Seed(_)
            | Outcome::Reset(_)
            | Outcome::Uninstall(_)
            | Outcome::Wizard(_)
            | Outcome::Update(_)
            | Outcome::Backup(_)
            | Outcome::Support(_)
            | Outcome::Archives(_)
            | Outcome::Restore(_)
            | Outcome::Watch(_)
            | Outcome::Walkthrough(_),
        )
        | Err(_) => None,
    }
}

/// Asked for twice, an operation does the same thing twice rather than refusing the
/// second time. The stack is the state, not a record of what has been asked for, so
/// an operator who is unsure whether a command landed can simply run it again.
#[tokio::test]
async fn asking_twice_is_not_an_error_the_second_time() {
    let settings = Settings {
        protocols: crate::config::Protocols::both(),
        ..Settings::default()
    };
    let ctx = a_context()
        .runner(Arc::new(Scripted(Ok(spoke("")))))
        .engine(Arc::new(Reporting::holding(
            &LIBRARY,
            Lifecycle::Running,
            Health::Healthy,
        )))
        .settings(settings)
        .build()
        .with_http(Fake::scripted(Vec::new()));

    for command in [
        Command::Up {
            forms: vec!["library".to_owned()],
        },
        Command::Down {
            forms: vec!["library".to_owned()],
            wait: Waiting::Never,
        },
        Command::Switch {
            forms: vec!["library".to_owned()],
        },
    ] {
        let once = report(dispatch(command.clone(), &ctx).await)
            .map(|report| (report.action, report.status));
        let again = report(dispatch(command.clone(), &ctx).await)
            .map(|report| (report.action, report.status));

        assert_eq!(
            once, again,
            "the second {command:?} answers as the first did"
        );
        assert_eq!(
            once.as_ref().map(|(_, status)| *status),
            Some(Some(0)),
            "and neither is a refusal: {once:?}"
        );
    }
}

/// The environment checks are about the machine rather than about anything
/// running on it, so none of their findings names a service — and a run with
/// nothing to quote asks the engine for nothing at all.
#[tokio::test]
async fn a_run_with_no_service_in_trouble_quotes_nothing() {
    let report = diagnosis(
        dispatch(
            Command::Doctor {
                narrowing: Narrowing::Category(Category::Environment),
                disruptive: false,
                accept: None,
            },
            &watching(Reporting::holding(
                &LIBRARY,
                Lifecycle::Running,
                Health::Healthy,
            )),
        )
        .await,
    );

    assert!(
        report
            .as_ref()
            .is_some_and(|report| !report.findings.is_empty()),
        "the environment checks did run: {report:?}"
    );
    assert!(
        report
            .iter()
            .flat_map(|report| report.findings.iter())
            .all(|finding| finding.said.is_none()),
        "nothing here is about a service, so nothing has a service to quote"
    );
}

/// A check can say a service is not answering; only the service can say why. So
/// what it said lately travels with the finding rather than waiting for the
/// operator to go and fetch it.
///
/// Driven against the decision itself rather than through a whole run, because
/// which checks name a service is a separate question from what happens to a
/// finding that does — today only the credential check names one, and this has to
/// keep working as more of them do.
#[tokio::test]
async fn a_failing_finding_carries_what_its_service_said() {
    let ctx = watching(
        Reporting::holding(&LIBRARY, Lifecycle::Running, Health::Healthy)
            .saying("jellyfin", "auth failed: bad credentials"),
    );

    let quoted = super::super::engine::quoted(
        &ctx,
        vec![crate::doctor::Finding::in_category(
            Category::Services,
            "services.jellyfin",
            "Jellyfin answers",
            crate::doctor::Verdict::Fail(crate::error::Problem::new(
                crate::error::Code::new("TEST-1"),
                crate::error::Severity::Error,
                "it is not answering",
                "it means what it says",
                crate::error::Remedy::new("put it right"),
            )),
        )
        .about("jellyfin")],
    )
    .await;

    assert_eq!(
        quoted.first().and_then(|finding| finding.said.clone()),
        Some("auth failed: bad credentials\n".to_owned()),
        "the service's own words, unprefixed — the finding already names it"
    );
}

/// A stand-in for a credential a service quotes back at itself, assembled rather
/// than written out so no value that reads as one sits in this source.
fn a_credential() -> String {
    ["abcdef", "1234", "567890"].concat()
}

/// What a service said is somebody else's text, and a service that fails while
/// authenticating says so with the credential in hand.
///
/// Withheld where the field is built rather than where it is drawn, which is what
/// this asserts: `said` is served on `/api/checks` as well as printed, so a report
/// that looked clean would still have been publishing the key.
#[tokio::test]
async fn what_a_service_said_reaches_a_finding_with_no_credential_in_it() {
    let secret = a_credential();
    let ctx = watching(
        Reporting::holding(&LIBRARY, Lifecycle::Running, Health::Healthy)
            .saying(
                "jellyfin",
                &format!("startup: api_key={secret} was rejected"),
            )
            .saying("jellyfin", &format!("INDEXER_APIKEY={secret}")),
    );

    let said = what_it_said(&ctx).await;
    assert!(
        !said.contains(&secret),
        "the credential survived into the finding"
    );
    // The sentence around it, still whole. A redactor that took the credential by
    // taking the line with it would leave a finding with no evidence under it, which
    // is the failure this path is here to prevent.
    assert!(
        said.contains("startup:"),
        "what the service was doing went with the credential"
    );
    assert!(
        said.contains("was rejected"),
        "what the service reported went with the credential"
    );
    assert!(
        said.contains("INDEXER_APIKEY"),
        "which setting the service named went with its value"
    );
}

/// A container quoting an outbound URL back at itself, with the key not the first
/// parameter in it.
///
/// The shape this product builds its own indexer request in — `t=search` first and
/// the key last — and the \*arrs log an outbound URL whenever one is refused. What a
/// container writes is the input this field is made of, so a rule that reads a URL
/// only as far as its first `=` leaves the key in the evidence.
#[tokio::test]
async fn a_url_a_container_quotes_back_arrives_with_no_key_in_it() {
    let secret = a_credential();
    let ctx = watching(
        Reporting::holding(&LIBRARY, Lifecycle::Running, Health::Healthy).saying(
            "jellyfin",
            &format!("GET https://indexer.example/api?t=search&apikey={secret} returned 401"),
        ),
    );

    let said = what_it_said(&ctx).await;
    assert!(
        !said.contains(&secret),
        "the key in the quoted address survived into the finding"
    );
    assert!(
        said.contains("https://indexer.example/api"),
        "the address went with the key riding in its query"
    );
    assert!(
        said.contains("returned 401"),
        "what the indexer answered went with the key"
    );
}

/// A log line opening on one word, which is how most of them open.
///
/// `ERROR:`, `WARN:`, `Unauthorized:` — a single word and a colon, which is the exact
/// shape of a setting whose value follows it. Every word this reads as a marker is
/// ordinary English, so a line opening on one loses the sentence it introduced, and
/// the sentence is the whole of what the finding was gathering.
#[tokio::test]
async fn a_line_opening_on_one_word_keeps_the_sentence_after_it() {
    let refused = "the request was refused by the indexer";
    let ctx = watching(
        Reporting::holding(&LIBRARY, Lifecycle::Running, Health::Healthy)
            .saying("jellyfin", &format!("Unauthorized: {refused}")),
    );

    let said = what_it_said(&ctx).await;
    assert!(said.contains(refused), "{said}");
}

/// What one troubled service's finding carries, for the tests that are about the
/// text rather than about which findings get one.
async fn what_it_said(ctx: &Ctx) -> String {
    super::super::engine::quoted(
        ctx,
        vec![crate::doctor::Finding::in_category(
            Category::Services,
            "services.jellyfin",
            "Jellyfin answers",
            crate::doctor::Verdict::Fail(crate::error::Problem::new(
                crate::error::Code::new("TEST-1"),
                crate::error::Severity::Error,
                "it is not answering",
                "it means what it says",
                crate::error::Remedy::new("put it right"),
            )),
        )
        .about("jellyfin")],
    )
    .await
    .first()
    .and_then(|finding| finding.said.clone())
    .unwrap_or_default()
}

/// Evidence for something that works is noise, and on a healthy run it would be
/// the bulk of the output.
#[tokio::test]
async fn a_finding_that_passed_carries_nothing() {
    let ctx = watching(
        Reporting::holding(&LIBRARY, Lifecycle::Running, Health::Healthy)
            .saying("jellyfin", "started"),
    );

    let quoted = super::super::engine::quoted(
        &ctx,
        vec![crate::doctor::Finding::in_category(
            Category::Services,
            "services.jellyfin",
            "Jellyfin answers",
            crate::doctor::Verdict::Pass { note: None },
        )
        .about("jellyfin")],
    )
    .await;

    assert!(quoted.first().is_some_and(|finding| finding.said.is_none()));
}

#[tokio::test]
async fn doctor_runs_the_checks_and_reports_them_in_the_envelope() {
    // The engine here does not host the torrent pair, so the findings are
    // not green — but dispatch's job is only to run the checks and hand back
    // what they found, named in the machine-readable envelope.
    let ctx = watching(Reporting::holding(
        &LIBRARY,
        Lifecycle::Running,
        Health::Healthy,
    ));
    let command = Command::Doctor {
        narrowing: Narrowing::Category(Category::Vpn),
        disruptive: false,
        accept: None,
    };
    let outcome = dispatch(command, &ctx).await;

    let json = outcome
        .as_ref()
        .ok()
        .and_then(|outcome| outcome.clone().envelope().to_json());
    assert!(
        json.as_deref().is_some_and(
            |json| json.contains(r#""kind":"doctor""#) && json.contains(r#""category":"vpn""#)
        ),
        "the doctor envelope should name itself and carry vpn findings: {json:?}"
    );

    let report = diagnosis(outcome);
    assert!(report.is_some_and(|report| !report.findings.is_empty()
        && report
            .findings
            .iter()
            .all(|finding| finding.category == Category::Vpn)));
}

#[tokio::test]
async fn a_full_doctor_run_includes_the_quality_guide_check() {
    // The guide-source check is wired into the suite: an unfiltered run carries
    // its finding. The ctx's offline http makes it unverified rather than
    // reaching the real upstream.
    let ctx = watching(Reporting::holding(
        &LIBRARY,
        Lifecycle::Running,
        Health::Healthy,
    ));
    let outcome = dispatch(
        Command::Doctor {
            narrowing: Narrowing::Suite,
            disruptive: false,
            accept: None,
        },
        &ctx,
    )
    .await;

    let names = diagnosis(outcome)
        .map(|report| {
            report
                .findings
                .into_iter()
                .map(|finding| finding.check)
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    assert!(
        names.iter().any(|check| check == "services.quality-guides"),
        "the guide-source check should appear in a full run: {names:?}"
    );
}

/// One check, named the way the report names it.
///
/// The whole point of the identifier being the same on both sides: the guide check
/// shares its family with the release search, so a family cannot single it out.
#[tokio::test]
async fn naming_one_check_runs_that_check_alone() {
    let ctx = watching(Reporting::holding(
        &LIBRARY,
        Lifecycle::Running,
        Health::Healthy,
    ));
    let outcome = dispatch(
        Command::Doctor {
            narrowing: Narrowing::Check("services.quality-guides".to_owned()),
            disruptive: false,
            accept: None,
        },
        &ctx,
    )
    .await;

    let names = diagnosis(outcome)
        .map(|report| {
            report
                .findings
                .into_iter()
                .map(|finding| finding.check)
                .collect::<Vec<String>>()
        })
        .unwrap_or_default();
    assert_eq!(
        names,
        vec!["services.quality-guides".to_owned()],
        "the run should hold the named check and nothing beside it"
    );
}

/// A name nothing reports is refused rather than answered with an empty report,
/// which reads as a stack with nothing wrong with it.
#[tokio::test]
async fn a_check_this_stack_does_not_report_is_refused() {
    let ctx = watching(Reporting::holding(
        &LIBRARY,
        Lifecycle::Running,
        Health::Healthy,
    ));
    let outcome = dispatch(
        Command::Doctor {
            narrowing: Narrowing::Check("services.nothing-of-the-kind".to_owned()),
            disruptive: false,
            accept: None,
        },
        &ctx,
    )
    .await;

    assert_eq!(
        outcome.as_ref().err().map(|problem| problem.code),
        Some(crate::error::codes::diag::NO_SUCH_CHECK),
        "a name this stack does not report should be refused: {outcome:?}"
    );
}

/// The two budget constants are bounded beside their own definition. This is about
/// the suite that actually gets built: a check overriding `budget()` with a minute
/// of its own would break the promise without touching either constant.
///
/// The slowest check is the whole of what has to fit, because the checks run
/// concurrently — a run costs its slowest rather than their sum, which is the only
/// reason a filesystem may ask for twice what a container command gets.
#[tokio::test]
async fn no_check_in_a_non_disruptive_run_may_outlast_the_run_itself() {
    let ctx = a_context()
        .engine(Arc::new(Reporting::holding(
            &[],
            Lifecycle::Exited,
            Health::None,
        )))
        .build();

    let slowest = super::super::engine::assembled(&ctx, false)
        .await
        .ok()
        .and_then(|(_, checks)| checks.iter().map(|check| check.budget()).max());

    assert!(
        slowest.is_some_and(|budget| budget <= Duration::from_secs(30)),
        "a full non-disruptive run is meant to finish inside thirty seconds: {slowest:?}"
    );
}

#[tokio::test]
async fn doctor_reports_an_unreadable_stack_rather_than_guessing() {
    let nowhere = Source::External(std::path::Path::new("/lemonfiber/no/such/stack"));
    let ctx = a_context()
        .runner(Arc::new(Scripted(Ok(spoke("v2.32.1")))))
        .engine(Arc::new(Reporting::default()))
        .over(nowhere)
        .build();
    let outcome = dispatch(
        Command::Doctor {
            narrowing: Narrowing::Suite,
            disruptive: false,
            accept: None,
        },
        &ctx,
    )
    .await;
    assert_eq!(
        outcome.as_ref().err().map(|problem| problem.code),
        Some(crate::error::codes::stack::STACK_UNREADABLE)
    );
    assert!(diagnosis(outcome).is_none());
}
