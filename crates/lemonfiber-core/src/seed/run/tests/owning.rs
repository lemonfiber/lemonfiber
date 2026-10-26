//! Claiming every part that belongs to whoever claims it first, over `owner.claim`.

use super::*;
use crate::baseline::Baseline;
use crate::test_support::{contracted, first_party};

/// The plugin whose adapter speaks `owner.claim`.
const PLUGIN: &str = "claimer";

/// The service it claims through.
const SERVICE: &str = "claimed-part";

/// The secret its adapter's `upstream.json` holds the password in.
const PASSWORD: &str = "password";

/// Where the adapter answers `claim`.
const CLAIM: &str = "/lemonfiber/owner.claim/v1/claim";

/// Where the adapter answers `owner`.
const OWNER: &str = "/lemonfiber/owner.claim/v1/owner";

/// The adapter answering who holds its part as `owner`.
fn holding(owner: &str) -> Answer {
    Answer::reply(200, format!("\"{owner}\""))
}

/// The plugin, whose adapter writes `password` from its secret of that name.
fn claimer() -> crate::plugin::Installed {
    let mut installed = contracted(PLUGIN, SERVICE, "owner.claim");
    for placed in &mut installed.services {
        placed.provides.clear();
        placed
            .upstream
            .insert(PASSWORD.to_owned(), PASSWORD.to_owned());
    }
    installed
}

/// The stack beside [`claimer`], trusting `trusted` as first-party.
fn beside(
    project: &std::path::Path,
    trusted: &[crate::plugin::first_party::FirstParty],
) -> crate::wiring::Fillers {
    fillers_trusting(Vec::new(), &[claimer()], project, trusted)
}

/// Every request the adapter was asked at `path`.
fn asked_at(http: &Fake, path: &str) -> usize {
    http.requests()
        .iter()
        .filter(|one| one.url.ends_with(path))
        .count()
}

/// A baseline recording the part as claimed.
fn recorded() -> Baseline {
    let mut baseline = Baseline::new();
    baseline.record(
        SERVICE,
        super::super::owning::FIELD,
        "ours",
        "2026-10-25T00:00:00Z",
    );
    baseline
}

#[tokio::test]
async fn a_part_nobody_holds_is_claimed_and_recorded() {
    let project = lemonfiber_fixtures::scratch::Scratch::new("owning-claimed");
    let http = Fake::by_path(vec![(CLAIM, holding("ours"))]);
    let ctx = contracted_ctx(&project, SERVICE, true, http.clone());
    let mut baseline = Baseline::new();

    let wirings =
        super::super::owning::owned(&ctx, &beside(&project, &first_party(PLUGIN)), &mut baseline)
            .await;

    assert_eq!(wirings.len(), 1, "{wirings:?}");
    assert_eq!(
        wirings.first().map(|one| &one.state),
        Some(&crate::seed::State::Wired)
    );
    assert_eq!(asked_at(&http, CLAIM), 1);
    assert_eq!(
        baseline.expected(SERVICE, super::super::owning::FIELD),
        Some("ours")
    );
}

#[tokio::test(start_paused = true)]
async fn a_part_taking_up_the_claim_is_waited_out() {
    let project = lemonfiber_fixtures::scratch::Scratch::new("owning-settling");
    let http = Fake::by_path_in_turn(vec![
        (CLAIM, vec![holding("settling")]),
        (OWNER, vec![holding("settling"), holding("ours")]),
    ]);
    let ctx = contracted_ctx(&project, SERVICE, true, http.clone());

    let wirings = super::super::owning::owned(
        &ctx,
        &beside(&project, &first_party(PLUGIN)),
        &mut Baseline::new(),
    )
    .await;

    assert_eq!(
        wirings.first().map(|one| &one.state),
        Some(&crate::seed::State::Wired),
        "{wirings:?}"
    );
    assert_eq!(asked_at(&http, OWNER), 2);
}

#[tokio::test]
async fn a_part_claimed_before_and_open_again_is_drift_and_never_claimed_on_a_seed() {
    let project = lemonfiber_fixtures::scratch::Scratch::new("owning-drifted");
    let http = Fake::by_path(vec![(OWNER, holding("open")), (CLAIM, holding("ours"))]);
    let ctx = contracted_ctx(&project, SERVICE, true, http.clone());
    let mut baseline = recorded();

    let wirings =
        super::super::owning::owned(&ctx, &beside(&project, &first_party(PLUGIN)), &mut baseline)
            .await;

    assert_eq!(
        wirings.first().map(|one| &one.state),
        Some(&crate::seed::State::Drifted),
        "{wirings:?}"
    );
    assert!(
        wirings.first().is_some_and(|one| matches!(&one.severity, crate::seed::Severity::Warning { remediation, .. } if remediation.contains("lemonfiber reset"))),
        "{wirings:?}"
    );
    assert_eq!(asked_at(&http, CLAIM), 0);
}

#[tokio::test]
async fn a_part_somebody_else_holds_is_refused_naming_the_secret_lemonfiber_holds() {
    let project = lemonfiber_fixtures::scratch::Scratch::new("owning-theirs");
    let http = Fake::by_path(vec![(CLAIM, holding("theirs"))]);
    let ctx = contracted_ctx(&project, SERVICE, true, http.clone());
    let mut baseline = Baseline::new();

    let wirings =
        super::super::owning::owned(&ctx, &beside(&project, &first_party(PLUGIN)), &mut baseline)
            .await;

    let secret = crate::settings::named(PLUGIN, PASSWORD);
    assert!(
        wirings.first().is_some_and(|one| matches!(&one.state, crate::seed::State::Refused { reason } if reason.contains(&secret))),
        "{wirings:?}"
    );
    assert_eq!(
        baseline.expected(SERVICE, super::super::owning::FIELD),
        None
    );
}

#[tokio::test]
async fn a_rehearsal_says_it_would_claim_and_claims_nothing() {
    let project = lemonfiber_fixtures::scratch::Scratch::new("owning-rehearsed");
    let http = Fake::by_path(vec![(OWNER, holding("open")), (CLAIM, holding("ours"))]);
    let mut ctx = contracted_ctx(&project, SERVICE, true, http.clone());
    ctx.dry_run = true;

    let wirings = super::super::owning::owned(
        &ctx,
        &beside(&project, &first_party(PLUGIN)),
        &mut Baseline::new(),
    )
    .await;

    assert!(
        wirings
            .first()
            .is_some_and(|one| matches!(one.state, crate::seed::State::WouldWire { .. })),
        "{wirings:?}"
    );
    assert_eq!(asked_at(&http, CLAIM), 0);
}

#[tokio::test]
async fn a_part_that_cannot_be_asked_over_its_contract_is_said_as_unknown() {
    let project = lemonfiber_fixtures::scratch::Scratch::new("owning-unkeyed");
    let http = Fake::always(holding("ours"));
    let ctx = contracted_ctx(&project, SERVICE, false, http.clone());

    let wirings = super::super::owning::owned(
        &ctx,
        &beside(&project, &first_party(PLUGIN)),
        &mut Baseline::new(),
    )
    .await;

    assert!(
        wirings.first().is_some_and(|one| matches!(&one.state, crate::seed::State::Failed { detail } if detail.contains("owner.claim@1"))),
        "{wirings:?}"
    );
    assert!(http.requests().is_empty(), "{:?}", http.requests());
}

#[tokio::test]
async fn a_reset_claims_back_a_part_lemonfiber_claimed_once_confirmed() {
    for confirm in [false, true] {
        let project =
            lemonfiber_fixtures::scratch::Scratch::new(&format!("owning-reset-{confirm}"));
        let http = Fake::by_path(vec![(OWNER, holding("open")), (CLAIM, holding("ours"))]);
        let ctx = contracted_ctx(&project, SERVICE, true, http.clone());

        let wirings = super::super::owning::put_back(
            &ctx,
            &beside(&project, &first_party(PLUGIN)),
            &recorded(),
            confirm,
        )
        .await;

        let expected = if confirm {
            wirings
                .first()
                .is_some_and(|one| matches!(one.state, crate::seed::State::Wired))
        } else {
            wirings
                .first()
                .is_some_and(|one| matches!(one.state, crate::seed::State::WouldWire { .. }))
        };
        assert!(expected, "{confirm}: {wirings:?}");
        assert_eq!(asked_at(&http, CLAIM), usize::from(confirm));
    }
}

#[tokio::test]
async fn a_reset_owes_a_part_lemonfiber_never_claimed_nothing() {
    let project = lemonfiber_fixtures::scratch::Scratch::new("owning-reset-unrecorded");
    let http = Fake::by_path(vec![(OWNER, holding("open")), (CLAIM, holding("ours"))]);
    let ctx = contracted_ctx(&project, SERVICE, true, http.clone());

    let wirings = super::super::owning::put_back(
        &ctx,
        &beside(&project, &first_party(PLUGIN)),
        &Baseline::new(),
        true,
    )
    .await;

    assert!(wirings.is_empty(), "{wirings:?}");
    assert!(http.requests().is_empty(), "{:?}", http.requests());
}

#[tokio::test]
async fn a_part_claimed_before_is_asked_who_holds_it_and_never_claimed_again() {
    for (owner, held) in [("ours", true), ("theirs", false)] {
        let project = lemonfiber_fixtures::scratch::Scratch::new(&format!("owning-asked-{owner}"));
        let http = Fake::by_path(vec![(OWNER, holding(owner)), (CLAIM, holding("ours"))]);
        let ctx = contracted_ctx(&project, SERVICE, true, http.clone());

        let wirings = super::super::owning::owned(
            &ctx,
            &beside(&project, &first_party(PLUGIN)),
            &mut recorded(),
        )
        .await;

        let state = wirings.first().map(|one| &one.state);
        if held {
            assert_eq!(
                state,
                Some(&crate::seed::State::AlreadyWired),
                "{wirings:?}"
            );
        } else {
            assert!(
                matches!(state, Some(crate::seed::State::Refused { .. })),
                "{owner}: {wirings:?}"
            );
        }
        assert_eq!(asked_at(&http, CLAIM), 0, "{owner}");
    }
}

#[tokio::test]
async fn a_claim_that_leaves_the_part_open_or_goes_unanswered_records_nothing() {
    for (name, answer) in [
        ("open", holding("open")),
        ("unanswered", Answer::reply(500, "")),
    ] {
        let project = lemonfiber_fixtures::scratch::Scratch::new(&format!("owning-claim-{name}"));
        let http = Fake::by_path(vec![(CLAIM, answer)]);
        let ctx = contracted_ctx(&project, SERVICE, true, http.clone());
        let mut baseline = Baseline::new();

        let wirings = super::super::owning::owned(
            &ctx,
            &beside(&project, &first_party(PLUGIN)),
            &mut baseline,
        )
        .await;

        let state = wirings.first().map(|one| &one.state);
        assert!(
            matches!(
                state,
                Some(crate::seed::State::Failed { .. } | crate::seed::State::Skipped { .. })
            ),
            "{name}: {wirings:?}"
        );
        if name == "open" {
            assert!(
                matches!(state, Some(crate::seed::State::Failed { detail }) if detail.contains("anybody may still take it")),
                "{wirings:?}"
            );
        }
        assert_eq!(
            baseline.expected(SERVICE, super::super::owning::FIELD),
            None,
            "{name}"
        );
    }
}

#[tokio::test(start_paused = true)]
async fn a_part_still_taking_up_the_claim_once_patience_runs_out_is_a_failure() {
    let project = lemonfiber_fixtures::scratch::Scratch::new("owning-settling-forever");
    let http = Fake::by_path(vec![
        (CLAIM, holding("settling")),
        (OWNER, holding("settling")),
    ]);
    let ctx = contracted_ctx(&project, SERVICE, true, http.clone());

    let wirings = super::super::owning::owned(
        &ctx,
        &beside(&project, &first_party(PLUGIN)),
        &mut Baseline::new(),
    )
    .await;

    assert!(
        wirings.first().is_some_and(|one| matches!(&one.state, crate::seed::State::Failed { detail } if detail.contains("still taking up its claim"))),
        "{wirings:?}"
    );
}

#[tokio::test]
async fn a_reset_leaves_a_part_lemonfiber_holds_and_refuses_one_somebody_else_holds() {
    for (owner, owed) in [("ours", false), ("theirs", true)] {
        let project =
            lemonfiber_fixtures::scratch::Scratch::new(&format!("owning-reset-held-{owner}"));
        let http = Fake::by_path(vec![(OWNER, holding(owner)), (CLAIM, holding("ours"))]);
        let ctx = contracted_ctx(&project, SERVICE, true, http.clone());

        let wirings = super::super::owning::put_back(
            &ctx,
            &beside(&project, &first_party(PLUGIN)),
            &recorded(),
            true,
        )
        .await;

        if owed {
            assert!(
                wirings
                    .first()
                    .is_some_and(|one| matches!(one.state, crate::seed::State::Refused { .. })),
                "{owner}: {wirings:?}"
            );
        } else {
            assert!(wirings.is_empty(), "{owner}: {wirings:?}");
        }
        assert_eq!(asked_at(&http, CLAIM), 0, "{owner}");
    }
}

#[test]
fn a_refusal_names_every_setting_lemonfiber_holds_for_the_part() {
    for (settings, named) in [
        (Vec::new(), "the secret its plugin minted"),
        (vec!["a".to_owned()], "the secret a,"),
        (vec!["a".to_owned(), "b".to_owned()], "the secrets a, b,"),
    ] {
        assert!(
            super::super::owning::refusal("Part", &settings).contains(named),
            "the refusal does not name {named}"
        );
    }
}

/// The shipped stack with its Usenet indexer aggregator, book curator and listening server
/// moved into their plugins, and a context asking each adapter in front of them over
/// `http`.
fn three_moved(http: Arc<Fake>) -> (Ctx, crate::wiring::Fillers) {
    use crate::test_support::{aggregator, books, listening, media};
    let installed: Vec<crate::plugin::Installed> = [
        aggregator::MOVED.installed(),
        books::moved(),
        listening::MOVED.installed(),
    ]
    .into_iter()
    .flatten()
    .collect();
    let fillers = crate::test_support::stack()
        .manifest()
        .map(|manifest| {
            crate::wiring::Fillers::of(
                &manifest,
                &installed,
                &crate::wiring::Chosen::default(),
                Some(stack_root()),
            )
        })
        .unwrap_or_default();
    let mut ctx = seed_ctx(true, None, None).with_http(http);
    ctx.seams.engine = Arc::new(
        aggregator::MOVED
            .running()
            .alongside(books::running())
            .alongside(listening::MOVED.running()),
    );
    ctx.seams.confined = media::Keyed::holding(
        Arc::clone(&ctx.seams.confined),
        &[aggregator::ADAPTER, books::ADAPTER, listening::ADAPTER],
    );
    (ctx, fillers)
}

/// Each part moved into its plugin whose adapter speaks `owner.claim` is claimed through
/// that adapter, and named and recorded as the part itself rather than as its adapter.
#[tokio::test]
async fn every_moved_part_is_claimed_through_its_adapter_and_recorded_as_itself() {
    let http = Fake::by_path(vec![crate::test_support::aggregator::held_by(
        "claim",
        crate::ports::service::Owner::Ours,
    )]);
    let (ctx, fillers) = three_moved(http.clone());
    let mut baseline = Baseline::new();

    let wirings = super::super::owning::owned(&ctx, &fillers, &mut baseline).await;

    assert_eq!(
        wirings
            .iter()
            .map(|one| (one.connection.as_str(), &one.state))
            .collect::<Vec<_>>(),
        vec![
            ("NZBHydra2 claimed", &crate::seed::State::Wired),
            ("Bindery claimed", &crate::seed::State::Wired),
            ("Audiobookshelf claimed", &crate::seed::State::Wired),
        ]
    );
    assert_eq!(asked_at(&http, CLAIM), 3);
    for part in ["nzbhydra2", "bindery", "audiobookshelf"] {
        assert_eq!(
            baseline.expected(part, super::super::owning::FIELD),
            Some("ours"),
            "{part}"
        );
    }
}

/// A moved part somebody else holds is refused naming every secret its plugin minted for
/// the adapter to claim it with.
#[tokio::test]
async fn a_moved_part_somebody_else_holds_is_refused_naming_its_minted_account() {
    let http = Fake::by_path(vec![crate::test_support::aggregator::held_by(
        "claim",
        crate::ports::service::Owner::Theirs,
    )]);
    let (ctx, fillers) = three_moved(http);

    let wirings = super::super::owning::owned(&ctx, &fillers, &mut Baseline::new()).await;

    let aggregator = wirings
        .iter()
        .find(|one| one.connection == "NZBHydra2 claimed");
    assert!(
        aggregator.is_some_and(
            |one| matches!(&one.state, crate::seed::State::Refused { reason }
            if reason.contains("nzbhydra2:admin-username")
                && reason.contains("nzbhydra2:admin-password"))
        ),
        "{wirings:?}"
    );
}
