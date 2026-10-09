use super::{
    Application, ApplicationKind, Category, Credential, Diagnose, DownloadClient, Failure,
    Identity, Image, Protocol, RegisteredApplication, RootFolder, Signed,
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
        protocol: Protocol("sabnzbd".to_owned()),
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
        kind: ApplicationKind::Tv,
        indexer_url: "http://prowlarr:9696".to_owned(),
        base_url: "http://sonarr:8989".to_owned(),
        api_key: "the-key".to_owned(),
    };
    assert_eq!(application.clone().kind, ApplicationKind::Tv);

    let registered = RegisteredApplication {
        id: "3".to_owned(),
        base_url: "http://sonarr:8989".to_owned(),
    };
    assert_eq!(registered.clone(), registered);
}

#[test]
fn a_secret_never_reaches_a_debug_rendering() {
    let rendered = [
        format!("{:?}", Credential::ApiKey("key-secret".to_owned())),
        format!(
            "{:?}",
            Credential::UserPass {
                username: "admin".to_owned(),
                password: "password-secret".to_owned(),
            }
        ),
        format!(
            "{:?}",
            Signed {
                id: "id-alex".to_owned(),
                token: "token-secret".to_owned(),
            }
        ),
    ];
    assert!(
        rendered.iter().all(|one| !one.contains("secret")),
        "{rendered:?}"
    );
    assert!(rendered[1].contains("admin") && rendered[2].contains("id-alex"));
}

#[test]
fn a_picture_crosses_as_base64_and_is_debugged_by_its_size() {
    let image = Image {
        media_type: "image/png".to_owned(),
        bytes: vec![0, 159, 255],
    };
    let written = serde_json::to_value(&image).ok();
    assert_eq!(
        written,
        Some(serde_json::json!({ "media_type": "image/png", "bytes": "AJ//" }))
    );
    let read: Option<Image> = written.and_then(|value| serde_json::from_value(value).ok());
    assert_eq!(read.as_ref(), Some(&image));
    assert_eq!(
        format!("{image:?}"),
        r#"Image { media_type: "image/png", bytes: 3 }"#
    );
    let unreadable = serde_json::json!({ "media_type": "image/png", "bytes": "not base64!" });
    assert!(serde_json::from_value::<Image>(unreadable).is_err());
}
