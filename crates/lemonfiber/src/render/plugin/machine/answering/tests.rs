use lemonfiber_core::plugin::{Adapter, Owner, Pair, Recipe, Step};
use lemonfiber_manifest::ApiKind;

use super::{recipes, unanswered};

/// A recipe with one call through lemonfiber's adapter, one to a host outside, and a
/// value carried there.
fn recipe() -> Recipe {
    Recipe {
        id: "adopt".to_owned(),
        title: "Hand the library over".to_owned(),
        why: "So the comics are read where the household looks".to_owned(),
        steps: vec![
            Step {
                id: "ask".to_owned(),
                method: "GET".to_owned(),
                to: "sonarr".to_owned(),
                path: "/api/v3/series".to_owned(),
                adapter: Some(Adapter {
                    kind: ApiKind::Servarr,
                    owner: Owner::Lemonfiber,
                }),
            },
            Step {
                id: "tell".to_owned(),
                method: "POST".to_owned(),
                to: "metadata.example.org".to_owned(),
                path: "/v1/series".to_owned(),
                adapter: None,
            },
        ],
        pairs: vec![Pair {
            value: "token".to_owned(),
            origin: "komga".to_owned(),
            to: "metadata.example.org".to_owned(),
            approval: Some("token@metadata.example.org".to_owned()),
        }],
    }
}

#[test]
fn every_call_is_said_in_order_with_the_adapter_it_reaches_through() {
    let said = recipes(&[recipe()]).text();
    assert!(
        said.contains("Recipe adopt: Hand the library over"),
        "{said}"
    );
    assert!(
        said.contains("1. GET sonarr /api/v3/series, through lemonfiber's servarr adapter"),
        "{said}"
    );
    assert!(
        said.contains("2. POST metadata.example.org /v1/series\n"),
        "{said}"
    );
    assert!(
        said.contains(
            "sends token (komga) to metadata.example.org — approve with --approve \
             token@metadata.example.org"
        ),
        "{said}"
    );
}

#[test]
fn a_value_no_step_captures_is_said_with_no_origin() {
    let mut bare = recipe();
    if let Some(pair) = bare.pairs.first_mut() {
        pair.origin.clear();
    }
    let said = recipes(&[bare]).text();
    assert!(
        said.contains("sends token to metadata.example.org"),
        "{said}"
    );
}

#[test]
fn a_pair_inside_the_stack_says_there_is_nothing_to_approve() {
    let mut inside = recipe();
    if let Some(pair) = inside.pairs.first_mut() {
        pair.to = "sonarr".to_owned();
        pair.approval = None;
    }
    let said = recipes(&[inside]).text();
    assert!(
        said.contains("sends token (komga) to sonarr — inside the stack, nothing to approve"),
        "{said}"
    );
    assert!(!said.contains("--approve"), "{said}");
}

#[test]
fn a_plugin_that_declares_no_recipe_draws_nothing() {
    assert!(recipes(&[]).text().is_empty());
}

/// Whatever reaches the renderer, nothing a terminal would obey is drawn: an escape, a
/// carriage return and a bidirectional override are each taken out.
#[test]
fn nothing_a_plugin_wrote_can_redraw_the_line_an_operator_approves_on() {
    let mut hostile = recipe();
    hostile.title = "Hand \u{1b}[2Kover".to_owned();
    hostile.why = "So it\rApproved".to_owned();
    if let Some(pair) = hostile.pairs.first_mut() {
        pair.value = "tok\u{202e}en".to_owned();
        pair.approval = Some("tok\u{202e}en@metadata.example.org".to_owned());
    }
    let said = recipes(&[hostile]).text();
    for obeyed in ['\u{1b}', '\r', '\u{202e}'] {
        assert!(
            !said.contains(obeyed),
            "{obeyed:?} reached the terminal: {said:?}"
        );
    }
    let answered = unanswered("install", Some("1a2b3c4d"), &["tok\u{202e}en@x"], false).text();
    assert!(!answered.contains('\u{202e}'), "{answered:?}");
}

#[test]
fn a_reading_is_answered_with_its_offer_and_every_approval() {
    let said = unanswered(
        "install",
        Some("1a2b3c4d-5e6f7a8b"),
        &["token@metadata.example.org"],
        false,
    )
    .text();
    assert!(
        said.contains("Nothing has been changed. To install it"),
        "{said}"
    );
    assert!(
        said.contains("--offer 1a2b3c4d-5e6f7a8b --approve token@metadata.example.org"),
        "{said}"
    );
    let rehearsed = unanswered("remove", Some("1a2b3c4d"), &[], true).text();
    assert!(
        rehearsed.contains("because this was a rehearsal. To remove it"),
        "{rehearsed}"
    );
    assert!(rehearsed.ends_with("  --offer 1a2b3c4d"), "{rehearsed:?}");
}

/// A report with no offer has nothing to answer, so it says only that nothing changed.
#[test]
fn a_report_with_no_offer_says_only_that_nothing_changed() {
    let said = unanswered("update", None, &[], false).text();
    assert_eq!(said.trim(), "Nothing has been changed.");
}
