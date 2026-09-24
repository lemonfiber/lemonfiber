//! Walking the steps and recording each answer.

use super::*;

/// What a service that would not take the credential answered.
fn turned_away() -> Arc<dyn Validator> {
    Arc::new(Saying(Validation::Rejected {
        detail: "the indexer answered 401".to_owned(),
    }))
}

/// An indexer credential, as a caller submits one.
fn an_indexer(validated: bool) -> Answer {
    Answer::Credentials(Some(Indexer {
        url: "http://indexer.invalid/api".to_owned(),
        key: withheld_value("indexer"),
        validated,
    }))
}

/// A Usenet provider, as a caller submits one.
fn a_provider(validated: bool) -> Answer {
    Answer::Provider(Some(Provider {
        host: "news.invalid".to_owned(),
        port: 563,
        user: "someone".to_owned(),
        pass: withheld_value("provider"),
        tls: true,
        validated,
    }))
}

#[tokio::test]
async fn a_fresh_machine_is_offered_setup_and_stands_at_its_first_step() {
    let paths = scratch("fresh");
    let report = walked(&ctx(&paths), SetupAction::Where).await;

    assert_eq!(report.as_ref().map(|report| report.at), Some(Step::Welcome));
    assert_eq!(
        report.as_ref().map(|report| report.asks),
        Some(false),
        "the welcome only informs"
    );
    assert_eq!(
        report.as_ref().map(|report| report.offered),
        Some(true),
        "there is no configuration to protect"
    );
    assert_eq!(
        report.as_ref().map(|report| report.ready_for_review),
        Some(false),
        "nothing is answered yet"
    );
    assert_eq!(
        report.map(|report| report.written.len()),
        Some(0),
        "and no apply stopped part-way"
    );
    assert!(
        !paths.setup_progress().exists(),
        "asking where setup stands writes nothing"
    );
}

#[tokio::test]
async fn a_step_that_only_informs_is_passed_without_an_answer() {
    let paths = scratch("informing");
    let report = walked(&ctx(&paths), SetupAction::Next).await;

    assert_eq!(report.map(|report| report.at), Some(Step::Preflight));
    assert!(
        paths.setup_progress().exists(),
        "where the walk reached survives quitting"
    );
}

#[tokio::test]
async fn an_answer_is_recorded_and_the_walk_moves_on() {
    let paths = scratch("recorded");
    let context = ctx(&paths);
    let report = walked(
        &context,
        SetupAction::Answer(Answer::Protocols(Protocols::both())),
    )
    .await;

    assert_eq!(
        report
            .as_ref()
            .map(|report| report.unanswered.contains(&Step::Protocols)),
        Some(false),
        "the question it answered is no longer outstanding"
    );
    assert_eq!(
        report.as_ref().map(|report| report.at),
        Some(Step::Preflight),
        "and the walk has moved on"
    );
    assert_eq!(
        report.map(|report| report.proof),
        Some(None),
        "an answer about this machine has no service to prove it against"
    );
    // Read back through a second request, which is the point of the file: this
    // surface holds nothing between one call and the next.
    assert_eq!(
        walked(&context, SetupAction::Where)
            .await
            .map(|report| report.unanswered.contains(&Step::Protocols)),
        Some(false)
    );
}

#[tokio::test]
async fn going_back_returns_to_the_previous_step_that_applies() {
    let paths = scratch("back");
    let context = ctx(&paths);

    assert_eq!(
        walked(&context, SetupAction::Next).await.map(|r| r.at),
        Some(Step::Preflight)
    );
    assert_eq!(
        walked(&context, SetupAction::Back).await.map(|r| r.at),
        Some(Step::Welcome)
    );
}

#[tokio::test]
async fn an_answer_this_platform_does_not_offer_is_refused_and_nothing_is_kept() {
    let paths = scratch("rejected");
    // Ownership is mapped away on this platform, so a container user would have
    // no observable effect and the wizard refuses to record one.
    let met = refused(
        &ctx(&paths),
        SetupAction::Answer(Answer::ServiceUser(Some((1000, 1000)))),
    )
    .await;

    assert_eq!(met, Some(DOES_NOT_APPLY));
    assert!(
        !paths.setup_progress().exists(),
        "an answer that was refused is not one that was saved"
    );
}

#[tokio::test]
async fn a_credential_records_what_the_service_said_and_says_what_that_was() {
    let paths = scratch("proven");
    let context = ctx(&paths);
    assert!(setting_up(
        &context,
        SetupAction::Answer(Answer::Protocols(Protocols::both()))
    )
    .await
    .is_ok());

    // Submitted claiming nothing, and proven anyway, because what is recorded is
    // what the live test established rather than what arrived.
    let report = walked(&context, SetupAction::Answer(an_indexer(false))).await;

    assert_eq!(
        planned(report.as_ref(), "INDEXER_VALIDATED").as_deref(),
        Some("on"),
        "the indexer answered"
    );
    assert_eq!(
        report.and_then(|report| report.proof),
        Some(Validation::Valid {
            observed: "answered a search — 40 results".to_owned()
        }),
        "and what it did is said, not merely that it answered"
    );
}

#[tokio::test]
async fn a_credential_the_service_will_not_take_is_kept_unproven_and_said_so() {
    let paths = scratch("unproven");
    let context = proving(&paths, turned_away());
    assert!(setting_up(
        &context,
        SetupAction::Answer(Answer::Protocols(Protocols::both()))
    )
    .await
    .is_ok());

    // Both arrive asserting they were proven, and the service refused both.
    assert!(setting_up(&context, SetupAction::Answer(an_indexer(true)))
        .await
        .is_ok());
    let report = walked(&context, SetupAction::Answer(a_provider(true))).await;

    assert_eq!(
        planned(report.as_ref(), "INDEXER_VALIDATED").as_deref(),
        Some("off"),
        "saying a key works is not proving it"
    );
    assert_eq!(
        planned(report.as_ref(), "USENET_VALIDATED").as_deref(),
        Some("off"),
        "nor the login"
    );
    assert_eq!(
        report.and_then(|report| report.proof),
        Some(Validation::Rejected {
            detail: "the indexer answered 401".to_owned()
        }),
        "and the refusal is carried back rather than swallowed"
    );
}

#[tokio::test]
async fn entering_no_credential_at_all_asks_nothing_of_any_service() {
    let paths = scratch("none-entered");
    let context = ctx(&paths);
    assert!(setting_up(
        &context,
        SetupAction::Answer(Answer::Protocols(Protocols::both()))
    )
    .await
    .is_ok());

    let report = walked(&context, SetupAction::Answer(Answer::Credentials(None))).await;

    assert_eq!(
        report.map(|report| report.proof),
        Some(None),
        "there was nothing to prove"
    );
}

#[tokio::test]
async fn what_was_entered_is_never_repeated_back() {
    let paths = scratch("withholding");
    let context = ctx(&paths);
    assert!(setting_up(
        &context,
        SetupAction::Answer(Answer::Protocols(Protocols::both()))
    )
    .await
    .is_ok());
    assert!(setting_up(&context, SetupAction::Answer(an_indexer(false)))
        .await
        .is_ok());
    let report = walked(&context, SetupAction::Answer(a_provider(false))).await;

    assert_eq!(
        planned(report.as_ref(), INDEXER_APIKEY_KEY).as_deref(),
        Some(store::REDACTED)
    );
    assert_eq!(
        planned(report.as_ref(), PROVIDER_PASS_KEY).as_deref(),
        Some(store::REDACTED)
    );
    // The address beside the key is not itself a secret, and a review that hid
    // it would be hiding what the operator is there to check.
    assert_eq!(
        planned(report.as_ref(), INDEXER_URL_KEY).as_deref(),
        Some("http://indexer.invalid/api")
    );
    let rendered = report
        .as_ref()
        .and_then(|report| serde_json::to_string(report).ok())
        .unwrap_or_default();
    assert!(
        !rendered.contains(&withheld_value("indexer"))
            && !rendered.contains(&withheld_value("provider")),
        "a credential reaches anything a caller could log"
    );
}

#[tokio::test]
async fn what_a_service_said_about_a_credential_is_reported_without_the_credential() {
    // An indexer refuses with the key it was given in hand, and setup is where that
    // key is being entered. What the service said is reported back, and the report
    // is serialised to a caller that may log it.
    let paths = scratch("proof");
    let said = format!(
        "the indexer refused the key: apikey={} has expired",
        withheld_value("indexer")
    );
    let context = proving(
        &paths,
        Arc::new(Saying(Validation::Rejected { detail: said })),
    );
    assert!(setting_up(
        &context,
        SetupAction::Answer(Answer::Protocols(Protocols::both()))
    )
    .await
    .is_ok());

    let report = walked(&context, SetupAction::Answer(an_indexer(true))).await;
    let rendered = report
        .as_ref()
        .and_then(|report| serde_json::to_string(report).ok())
        .unwrap_or_default();

    assert!(
        !rendered.contains(&withheld_value("indexer")),
        "the key reaches a caller that may log it"
    );
    // And what the operator is deciding on survives. A refusal whose reason has
    // been withheld says only that something went wrong.
    assert!(
        rendered.contains("the indexer refused the key"),
        "the reason the key was refused went with the key"
    );
    assert!(
        rendered.contains("has expired"),
        "what the indexer said about the key went with the key"
    );
}

#[tokio::test]
async fn the_walk_settles_on_what_the_wizard_itself_would() {
    // The whole claim of this module: it drives the wizard rather than deciding
    // anything of its own, so what it comes to is what the wizard comes to.
    let paths = scratch("agrees");
    let context = ctx(&paths);
    let root = paths.data_dir().join("media");
    answer_everything(&context, &root).await;

    let mut wizard = Wizard::new(context.environment);
    for answer in all_of_them(&root) {
        assert!(wizard.answer(answer).is_ok());
    }

    let over_requests: Vec<(String, String)> = walked(&context, SetupAction::Where)
        .await
        .map(|report| {
            report
                .plan
                .into_iter()
                .map(|setting| (setting.key, setting.value))
                .collect()
        })
        .unwrap_or_default();
    assert_eq!(over_requests, wizard.plan().settings().to_vec());
    assert!(
        over_requests.contains(&("DATA_ROOT".to_owned(), root.display().to_string())),
        "and it is not an empty agreement: {over_requests:?}"
    );
}

#[tokio::test]
async fn a_rehearsed_answer_moves_the_walk_on_and_records_nothing() {
    // The progress file is the state and nothing else is, so a rehearsal that
    // wrote it would have answered the question on the operator's behalf.
    let paths = scratch("rehearsed-answer");
    let rehearsing = ctx(&paths).rehearsing();

    let before = walked(&rehearsing, SetupAction::Where)
        .await
        .map(|report| report.at);
    let after = walked(
        &rehearsing,
        SetupAction::Answer(Answer::Protocols(Protocols::both())),
    )
    .await
    .map(|report| report.at);

    assert!(after.is_some(), "the answer was refused rather than read");
    assert_ne!(before, after, "a rehearsal left the walk where it was");
    assert!(
        !paths.setup_progress().exists(),
        "the answer was written down anyway"
    );
}
