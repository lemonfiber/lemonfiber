//! Where each value of a run may be sent.

use std::collections::BTreeSet;

use lemonfiber_plugin::Recipe;

use super::Bounds;

/// A recipe holding `key` to sonarr, with pairs carrying `key` to sonarr and `token`
/// and `other` to sonarr, komga and a host outside.
fn recipe() -> Recipe {
    let pairs: String = ["sonarr", "komga", "metadata.example.org"]
        .iter()
        .flat_map(|to| {
            ["key", "token", "other"]
                .map(|value| format!("[[pair]]\nvalue = \"{value}\"\nto = \"{to}\"\n"))
        })
        .collect();
    toml::from_str(&format!(
        "id = \"r\"\ntitle = \"R\"\nwhy = \"Held\"\n[[input]]\nname = \"key\"\n\
         origin = \"credential-store\"\nof = \"sonarr\"\n{pairs}"
    ))
    .unwrap_or_else(|_| Recipe {
        id: "unread".to_owned(),
        title: String::new(),
        why: String::new(),
        on: lemonfiber_plugin::On::Install,
        inputs: Vec::new(),
        steps: Vec::new(),
        pairs: Vec::new(),
    })
}

/// A credential goes back to its service and nowhere else, whatever pairs say.
#[test]
fn a_credential_is_held_to_its_service_over_its_pairs() {
    let recipe = recipe();
    let approved = ["key@metadata.example.org".to_owned()];
    let bounds = Bounds::of(&recipe, &approved);
    assert_eq!(bounds.withheld("key", "sonarr", false), None);
    for (to, outside) in [("komga", false), ("metadata.example.org", true)] {
        assert!(bounds
            .withheld("key", to, outside)
            .is_some_and(|why| why.contains("key is held to sonarr")));
    }
}

/// Only a step whose call carried a value held to a service holds what it captured to
/// that service; one carrying nothing held leaves its captures to their pairs.
#[test]
fn a_capture_is_held_only_where_its_call_carried_something_held() {
    let recipe = recipe();
    let approved = ["other@metadata.example.org".to_owned()];
    let mut bounds = Bounds::of(&recipe, &approved);
    let other = "other".to_owned();
    bounds.traded(
        &BTreeSet::from(["nothing".to_owned()]),
        [&other].into_iter(),
    );
    assert_eq!(bounds.withheld("other", "komga", false), None);
    assert_eq!(bounds.withheld("other", "metadata.example.org", true), None);

    let token = "token".to_owned();
    bounds.traded(
        &BTreeSet::from(["other".to_owned(), "key".to_owned()]),
        [&token].into_iter(),
    );
    assert_eq!(bounds.withheld("token", "sonarr", false), None);
    assert!(bounds
        .withheld("token", "komga", false)
        .is_some_and(|why| why.contains("token is held to sonarr")));
}

/// A value outside the stack needs its pair and its approval both.
#[test]
fn outside_a_value_needs_its_pair_and_its_approval() {
    let recipe = recipe();
    let bounds = Bounds::of(&recipe, &[]);
    assert!(bounds
        .withheld("token", "metadata.example.org", true)
        .is_some_and(|why| why.contains("token@metadata.example.org was not approved")));
    assert!(bounds
        .withheld("token", "elsewhere.example.org", true)
        .is_some_and(|why| why.contains("no pair of the recipe declares")));
    assert_eq!(bounds.withheld("token", "komga", false), None);
    let approved = ["other@metadata.example.org".to_owned()];
    let another = Bounds::of(&recipe, &approved);
    assert!(another
        .withheld("token", "metadata.example.org", true)
        .is_some_and(|why| why.contains("token@metadata.example.org was not approved")));
    assert_eq!(
        another.withheld("other", "metadata.example.org", true),
        None
    );
}

/// Only a value the credential store brings in is held to a service; an input the
/// operator gives is not, whatever service it names.
#[test]
fn only_a_credential_store_value_is_held_to_its_service() {
    let recipe: Option<Recipe> = toml::from_str(
        "id = \"r\"\ntitle = \"R\"\nwhy = \"Held\"\n[[input]]\nname = \"typed\"\n\
         origin = \"operator\"\nof = \"sonarr\"\n[[pair]]\nvalue = \"typed\"\nto = \"komga\"\n",
    )
    .ok();
    let held = recipe
        .as_ref()
        .map(|recipe| Bounds::of(recipe, &[]).withheld("typed", "komga", false));
    assert_eq!(held, Some(None));
}
