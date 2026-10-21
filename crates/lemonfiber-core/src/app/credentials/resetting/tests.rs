use super::{copy, UNTAKEN};
use crate::credential::Reach;
use crate::seed::State;

#[test]
fn a_copy_wired_or_already_holding_it_has_the_new_key() {
    for state in [State::Wired, State::AlreadyWired] {
        assert_eq!(copy("Prowlarr".to_owned(), state).reach, Reach::Updated);
    }
}

#[test]
fn a_copy_that_failed_says_why_in_its_own_words() {
    let copied = copy(
        "Bazarr".to_owned(),
        State::Failed {
            detail: "HTTP 500".to_owned(),
        },
    );

    assert_eq!(
        copied.reach,
        Reach::Failed {
            detail: "HTTP 500".to_owned()
        }
    );
}

#[test]
fn a_copy_left_unsettled_without_words_points_at_the_seeding() {
    let copied = copy(
        "the request gate".to_owned(),
        State::Skipped {
            reason: "no stack directory".to_owned(),
        },
    );

    assert_eq!(
        copied.reach,
        Reach::Failed {
            detail: UNTAKEN.to_owned()
        }
    );
}

#[tokio::test]
async fn a_reset_key_is_handed_to_a_quality_sync_holding_the_curator() {
    let project = lemonfiber_fixtures::scratch::Scratch::new("resetting-quality");
    let http = lemonfiber_fixtures::http::Fake::by_path(vec![(
        "/lemonfiber/quality.sync/v1/apply",
        lemonfiber_fixtures::http::Answer::reply(204, ""),
    )]);
    let ctx = crate::test_support::contracted_context(&project, "sync", true)
        .build()
        .with_http(http.clone())
        .with_filesystem(std::sync::Arc::new(crate::test_support::SeedFs::keyed(
            Some("<Config><ApiKey>the-new-key</ApiKey></Config>"),
            None,
        )));
    let services: Vec<lemonfiber_manifest::Service> = crate::test_support::stack()
        .manifest()
        .map(|manifest| manifest.services)
        .unwrap_or_default()
        .into_iter()
        .filter(|service| service.id == "sonarr")
        .collect();
    let mut sync = crate::test_support::contracted("syncer", "sync", "quality.sync");
    for placed in &mut sync.services {
        placed.asks = vec![crate::plugin::Asking {
            capability: "library.curate".to_owned(),
            each: true,
        }];
    }
    let fillers = crate::test_support::stack_fillers(
        services.clone(),
        &[sync],
        Some(&project),
        &crate::test_support::first_party("syncer"),
    );
    let mut copies = Vec::new();
    for target in services
        .iter()
        .filter_map(|service| crate::app::targets::target_for(service, &project))
    {
        copies.extend(super::copies(&ctx, &services, &fillers, Some(&project), &target).await);
    }

    let consumers: Vec<(&str, &Reach)> = copies
        .iter()
        .map(|copy| (copy.consumer.as_str(), &copy.reach))
        .collect();
    assert_eq!(
        consumers,
        vec![(
            "sync the stand-in, which holds Sonarr's quality profiles",
            &Reach::Updated
        )]
    );
    assert!(http.requests().iter().any(|one| one
        .body
        .as_deref()
        .is_some_and(|body| body.contains("the-new-key"))));
}
