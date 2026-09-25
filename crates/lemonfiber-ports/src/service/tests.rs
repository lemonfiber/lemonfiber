use super::{
    Application, ApplicationKind, Category, ClientKind, Credential, Diagnose, DownloadClient,
    Failure, Identity, RegisteredApplication, RootFolder,
};
use lemonfiber_error::{Severity, State};

#[test]
fn an_absent_service_is_skipped_rather_than_failed() {
    let problem = Failure::Unavailable {
        service: "sonarr".to_owned(),
    }
    .problem();
    assert_eq!(problem.severity, Severity::Warning);
    assert!(problem.summary.contains("skipped"));
}

#[test]
fn a_rejected_credential_is_something_lemonfiber_can_fix() {
    let problem = Failure::Unauthorised {
        service: "sonarr".to_owned(),
    }
    .problem();
    assert_eq!(problem.state, State::Remediable);
}

#[test]
fn an_unrecognised_answer_admits_ignorance_rather_than_guessing() {
    let problem = Failure::Refused {
        service: "sonarr".to_owned(),
        detail: "500 Internal Server Error".to_owned(),
    }
    .problem();
    assert_eq!(problem.state, State::Unknown);
    assert_eq!(problem.detail.as_deref(), Some("500 Internal Server Error"));
    assert!(!problem.remedies.is_empty(), "escalation is still offered");
}

#[test]
fn an_unsupported_api_version_is_reported_with_a_remedy() {
    let problem = Failure::Unsupported {
        service: "sonarr".to_owned(),
        detail: "there is no /api/v3".to_owned(),
    }
    .problem();
    assert_eq!(problem.severity, Severity::Error);
    assert_eq!(problem.detail.as_deref(), Some("there is no /api/v3"));
    assert!(
        !problem.remedies.is_empty(),
        "aligning the versions is offered as the way out"
    );
}

#[test]
fn every_failure_names_the_service_it_is_about() {
    let failures = [
        Failure::Unavailable {
            service: "sonarr".to_owned(),
        },
        Failure::Unauthorised {
            service: "sonarr".to_owned(),
        },
        Failure::Refused {
            service: "sonarr".to_owned(),
            detail: "boom".to_owned(),
        },
        Failure::Unsupported {
            service: "sonarr".to_owned(),
            detail: "boom".to_owned(),
        },
    ];
    for failure in &failures {
        assert!(failure.to_string().contains("sonarr"));
        assert!(!failure.problem().remedies.is_empty());
    }
}

#[test]
fn the_things_a_service_is_told_about_are_plain_data() {
    let identity = Identity {
        name: "Sonarr".to_owned(),
        version: "4.0.15".to_owned(),
    };
    assert_eq!(identity.clone(), identity);

    let client = DownloadClient {
        name: "SABnzbd".to_owned(),
        host: "sabnzbd".to_owned(),
        port: 8080,
        kind: ClientKind::Sabnzbd,
        credential: Credential::ApiKey("the-key".to_owned()),
        category: Category {
            field: "tvCategory".to_owned(),
            value: "tv".to_owned(),
        },
    };
    assert_eq!(client.clone().port, 8080);

    let folder = RootFolder {
        path: "/data/media/tv".to_owned(),
        media_type: "tv".to_owned(),
    };
    assert_eq!(folder.clone().media_type, "tv");

    let application = Application {
        name: "Sonarr".to_owned(),
        kind: ApplicationKind::Sonarr,
        prowlarr_url: "http://prowlarr:9696".to_owned(),
        base_url: "http://sonarr:8989".to_owned(),
        api_key: "the-key".to_owned(),
    };
    assert_eq!(application.clone().kind, ApplicationKind::Sonarr);

    let registered = RegisteredApplication {
        id: "3".to_owned(),
        base_url: "http://sonarr:8989".to_owned(),
    };
    assert_eq!(registered.clone(), registered);
}
