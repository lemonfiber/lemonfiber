use super::{fills, Claimant, Filling, Shown};

/// The answer where one service fills it.
fn filled(service: &str) -> Filling {
    Filling::By {
        service: service.to_owned(),
    }
}

/// One claimant, in the state named.
fn claiming(service: &str, shown: Shown) -> Claimant {
    Claimant {
        service: service.to_owned(),
        plugin: None,
        shown,
    }
}

#[test]
fn one_candidate_fills_it() {
    let held = fills(&[claiming("jellyfin", Shown::Demonstrated)]);
    assert_eq!(held, filled("jellyfin"));
}

#[test]
fn nothing_claiming_it_is_unfilled() {
    assert_eq!(fills(&[]), Filling::Unfilled);
}

/// Every claimant is named, so an operator resolving it is choosing from a list
/// rather than being told there is a conflict.
#[test]
fn more_than_one_candidate_is_contested_and_names_each() {
    let held = fills(&[
        claiming("jellyfin", Shown::Demonstrated),
        claiming("navidrome", Shown::Claimed),
        Claimant {
            service: "komga".to_owned(),
            plugin: Some("komga".to_owned()),
            shown: Shown::Demonstrated,
        },
    ]);
    assert_eq!(
        held,
        Filling::Contested {
            claimants: vec![
                "jellyfin".to_owned(),
                "navidrome".to_owned(),
                "komga (plugin komga)".to_owned(),
            ]
        }
    );
}

/// A claim whose probes refused it is false, so it is not one of the claimants a
/// contest is between — and where it was the only one, nothing fills the capability.
#[test]
fn a_refuted_claim_is_not_a_candidate() {
    assert_eq!(
        fills(&[claiming("plex", Shown::Refuted)]),
        Filling::Unfilled
    );
    let held = fills(&[
        claiming("plex", Shown::Refuted),
        claiming("jellyfin", Shown::Demonstrated),
    ]);
    assert_eq!(held, filled("jellyfin"));
}

/// Unproven is not satisfied. A runner that could not ask has established nothing,
/// and wiring to it would be treating an unanswered question as a yes.
#[test]
fn an_unproven_claim_is_not_a_candidate_either() {
    assert_eq!(
        fills(&[claiming("plex", Shown::Unproven)]),
        Filling::Unfilled
    );
}

/// A state a claim is in before anything has asked is still a candidate, or a stack
/// whose probes have not been run would wire nothing at all.
#[test]
fn a_claim_nothing_has_asked_about_yet_can_still_fill() {
    assert_eq!(
        fills(&[claiming("sonarr", Shown::Claimed)]),
        filled("sonarr")
    );
}

#[test]
fn each_state_says_itself_in_the_word_the_vocabulary_uses() {
    let said: Vec<&str> = [
        Shown::Claimed,
        Shown::Demonstrated,
        Shown::Unproven,
        Shown::Refuted,
    ]
    .iter()
    .map(Shown::as_str)
    .collect();
    assert_eq!(said, vec!["claimed", "demonstrated", "unproven", "refuted"]);
}

/// The word a report prints and the word a machine-readable run carries are the
/// same word. Two spellings of one state is a consumer and a reader disagreeing
/// about what happened.
#[test]
fn the_word_it_is_printed_as_is_the_word_it_is_carried_as() {
    for state in [
        Shown::Claimed,
        Shown::Demonstrated,
        Shown::Unproven,
        Shown::Refuted,
    ] {
        let carried = serde_json::to_string(&state).unwrap_or_default();
        assert_eq!(carried, format!("\"{}\"", state.as_str()));
    }
}

/// Each answer carries the word it is, and what that word is about.
#[test]
fn what_an_ask_came_to_is_one_word_and_its_subject() {
    let answers: Vec<String> = [
        filled("jellyfin"),
        Filling::Contested {
            claimants: vec!["a".to_owned(), "b".to_owned()],
        },
        Filling::Unfilled,
    ]
    .iter()
    .filter_map(|one| serde_json::to_string(one).ok())
    .collect();
    assert_eq!(
        answers,
        vec![
            r#"{"answer":"by","service":"jellyfin"}"#,
            r#"{"answer":"contested","claimants":["a","b"]}"#,
            r#"{"answer":"unfilled"}"#,
        ]
    );
}
