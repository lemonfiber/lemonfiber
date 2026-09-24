//! What is running, as a status reports it.

use super::*;

#[tokio::test]
async fn a_non_status_outcome_has_no_services_to_report() {
    let ctx = ctx(Ok(spoke("v2.32.1")));
    assert_eq!(stated(dispatch(Command::Version, &ctx).await), None);
}

#[tokio::test]
async fn asking_what_is_running_names_every_service_a_form_declares() {
    let engine = Reporting::holding(&["jellyfin"], Lifecycle::Running, Health::Healthy);
    let ctx = watching(engine);
    let command = Command::Ps {
        forms: vec!["library".to_owned()],
    };

    let seen = stated(dispatch(command, &ctx).await).unwrap_or_default();
    assert_eq!(seen.len(), LIBRARY.len());
    assert!(
        seen.iter()
            .any(|(id, state)| id == "jellyfin" && *state == ServiceState::Healthy),
        "{seen:?}"
    );
    assert!(
        seen.iter()
            .any(|(id, state)| id == "seerr" && *state == ServiceState::Absent),
        "a service that was never started is absent, not missing: {seen:?}"
    );
}

#[tokio::test]
async fn asking_what_is_running_without_naming_a_form_covers_the_whole_stack() {
    let ctx = watching(Reporting::holding(&[], Lifecycle::Running, Health::None));
    let seen = stated(dispatch(Command::Ps { forms: Vec::new() }, &ctx).await);

    assert_eq!(
        seen.map(|services| services.len() > LIBRARY.len()),
        Some(true),
        "what is running is a question about the machine, not about a form"
    );
}

#[tokio::test]
async fn asking_what_is_running_reports_an_engine_it_cannot_see() {
    let ctx = watching(Reporting::absent());
    let refusal = dispatch(Command::Ps { forms: Vec::new() }, &ctx)
        .await
        .err()
        .map(|problem| problem.code);
    assert_eq!(
        refusal,
        Some(crate::ports::docker::ENGINE_UNREACHABLE),
        "an unreachable engine is not a stack with nothing in it"
    );
}

#[tokio::test]
async fn asking_about_a_form_this_stack_does_not_have_is_refused() {
    let ctx = watching(Reporting::default());
    let command = Command::Ps {
        forms: vec!["telly".to_owned()],
    };
    assert_eq!(
        dispatch(command, &ctx).await.err().map(|p| p.code),
        Some(crate::stack::closure::NO_SUCH_FORM)
    );
}

#[tokio::test]
async fn asking_what_is_running_from_a_stack_that_cannot_be_read_is_refused() {
    let nowhere = Source::External(std::path::Path::new("/lemonfiber/no/such/stack"));
    let ctx = a_context()
        .engine(Arc::new(Reporting::default()))
        .over(nowhere)
        .build();
    assert_eq!(
        dispatch(Command::Ps { forms: Vec::new() }, &ctx)
            .await
            .err()
            .map(|problem| problem.code),
        Some(crate::stack::STACK_UNREADABLE),
        "an operator's own --stack-dir mistake reaches them here too"
    );
}

#[tokio::test]
async fn a_status_serialises_under_its_own_kind() {
    let engine = Reporting::holding(&["jellyfin"], Lifecycle::Running, Health::Healthy);
    let ctx = watching(engine);
    let command = Command::Ps {
        forms: vec!["library".to_owned()],
    };

    let rendered = dispatch(command, &ctx)
        .await
        .ok()
        .map(Outcome::envelope)
        .and_then(|envelope| envelope.to_json().map(|json| (envelope.kind, json)));

    assert_eq!(
        rendered.map(|(kind, json)| (
            kind,
            json.starts_with(r#"{"api_version":1,"kind":"status","data":{"forms":["library"]"#),
            json.contains(r#""state":"healthy""#)
        )),
        Some((crate::model::kind::STATUS, true, true))
    );
}
