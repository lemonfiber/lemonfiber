//! Where each value of a run may be sent.

use std::collections::BTreeMap;

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
    let bounds = Bounds::of(&recipe, &approved, &[]);
    assert_eq!(bounds.withheld("key", "v", "sonarr", false), None);
    for (to, outside) in [("komga", false), ("metadata.example.org", true)] {
        assert!(bounds
            .withheld("key", "v", to, outside)
            .is_some_and(|why| why.contains("key is the credential lemonfiber holds for sonarr")));
    }
}

/// A step whose call carried a credential holds what it captured as the credential is;
/// one to a host outside carrying nothing held leaves its captures to their pairs.
#[test]
fn a_capture_is_traded_only_where_its_call_carried_a_credential() {
    let recipe = recipe();
    let approved = ["other@metadata.example.org".to_owned()];
    let mut bounds = Bounds::of(&recipe, &approved, &[]);
    let other = "other".to_owned();
    bounds.captured(
        &BTreeMap::from([("nothing".to_owned(), "v".to_owned())]),
        None,
        [&other].into_iter(),
    );
    assert_eq!(bounds.withheld("other", "v", "komga", false), None);
    assert_eq!(
        bounds.withheld("other", "v", "metadata.example.org", true),
        None
    );

    let token = "token".to_owned();
    bounds.captured(
        &BTreeMap::from([
            ("other".to_owned(), "v".to_owned()),
            ("key".to_owned(), "v".to_owned()),
        ]),
        Some("sonarr"),
        [&token].into_iter(),
    );
    assert_eq!(bounds.withheld("token", "v", "sonarr", false), None);
    assert!(bounds.withheld("token", "v", "komga", false).is_some_and(
        |why| why.contains("token was traded for the credential lemonfiber holds for sonarr")
    ));
}

/// A recipe whose `lib` goes to sonarr by a released pair, and to radarr by a plain one.
fn releasing() -> Recipe {
    toml::from_str(
        "id = \"r\"\ntitle = \"R\"\nwhy = \"Held\"\n[[input]]\nname = \"key\"\n\
         origin = \"credential-store\"\nof = \"sonarr\"\n\
         [[pair]]\nvalue = \"lib\"\nto = \"sonarr\"\nrelease = \"Sonarr files into it.\"\n\
         [[pair]]\nvalue = \"lib\"\nto = \"radarr\"\n\
         [[pair]]\nvalue = \"bought\"\nto = \"komga\"\nrelease = \"Why.\"\n",
    )
    .unwrap_or_else(|_| recipe())
}

/// What a service answered by a call carrying no credential goes back to it, and
/// elsewhere only by a released pair this act approved.
#[test]
fn an_answer_goes_elsewhere_only_by_an_approved_release() {
    let recipe = releasing();
    let lib = "lib".to_owned();
    let unapproved: [String; 0] = [];
    let mut bounds = Bounds::of(&recipe, &unapproved, &[]);
    bounds.captured(&BTreeMap::new(), Some("komga"), [&lib].into_iter());
    assert!(bounds
        .withheld("lib", "v", "radarr", false)
        .is_some_and(|why| why
            .contains("captured from the answer of komga, and no pair releases it to radarr")));
    assert!(bounds
        .withheld("lib", "v", "sonarr", false)
        .is_some_and(|why| why.contains("releasing it as lib@sonarr was not approved")));
    assert!(bounds
        .decided("lib", "v", "sonarr")
        .is_some_and(|why| why.starts_with("decides on lib for a call to sonarr")));

    let approved = ["lib@sonarr".to_owned()];
    let mut bounds = Bounds::of(&recipe, &approved, &[]);
    bounds.captured(&BTreeMap::new(), Some("komga"), [&lib].into_iter());
    assert_eq!(bounds.withheld("lib", "v", "sonarr", false), None);
    assert_eq!(bounds.decided("lib", "v", "sonarr"), None);
}

/// No release and no approval frees what a credential bought.
#[test]
fn no_approved_release_frees_what_a_credential_bought() {
    let recipe = releasing();
    let approved = ["bought@komga".to_owned()];
    let mut bounds = Bounds::of(&recipe, &approved, &[]);
    let bought = "bought".to_owned();
    bounds.captured(
        &BTreeMap::from([("key".to_owned(), "v".to_owned())]),
        Some("sonarr"),
        [&bought].into_iter(),
    );
    assert!(bounds.withheld("bought", "v", "komga", false).is_some_and(
        |why| why.contains("bought was traded for the credential lemonfiber holds for sonarr")
    ));
}

/// A value outside the stack needs its pair and its approval both.
#[test]
fn outside_a_value_needs_its_pair_and_its_approval() {
    let recipe = recipe();
    let bounds = Bounds::of(&recipe, &[], &[]);
    assert!(bounds
        .withheld("token", "v", "metadata.example.org", true)
        .is_some_and(|why| why.contains("token@metadata.example.org was not approved")));
    assert!(bounds
        .withheld("token", "v", "elsewhere.example.org", true)
        .is_some_and(|why| why.contains("no pair of the recipe declares")));
    assert_eq!(bounds.withheld("token", "v", "komga", false), None);
    let approved = ["other@metadata.example.org".to_owned()];
    let another = Bounds::of(&recipe, &approved, &[]);
    assert!(another
        .withheld("token", "v", "metadata.example.org", true)
        .is_some_and(|why| why.contains("token@metadata.example.org was not approved")));
    assert_eq!(
        another.withheld("other", "v", "metadata.example.org", true),
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
        .map(|recipe| Bounds::of(recipe, &[], &[]).withheld("typed", "v", "komga", false));
    assert_eq!(held, Some(None));
}

/// A value holding a credential lemonfiber holds is held to that credential's service
/// whatever it is called, whole or inside a longer value, and a guard reading one is
/// held the same way; what a call carrying one captures is held to it too.
#[test]
fn a_value_holding_a_credential_is_held_to_its_service_whatever_its_name() {
    let recipe = recipe();
    let credentials = [
        ("sonarr".to_owned(), "k3y-of-sonarr".to_owned()),
        ("radarr".to_owned(), String::new()),
    ];
    let approved = ["other@metadata.example.org".to_owned()];
    let mut bounds = Bounds::of(&recipe, &approved, &credentials);
    for (value, to) in [
        ("k3y-of-sonarr", "komga"),
        ("prefix k3y-of-sonarr suffix", "komga"),
        ("k3y-of-sonarr", "metadata.example.org"),
    ] {
        let why = bounds.withheld("other", value, to, to.contains('.'));
        assert!(
            why.as_deref().is_some_and(|why| why
                .contains("what other holds is the credential lemonfiber holds for sonarr")
                && !why.contains("k3y")),
            "{value} to {to}: {why:?}"
        );
    }
    assert_eq!(
        bounds.withheld("other", "k3y-of-sonarr", "sonarr", false),
        None
    );
    assert_eq!(bounds.withheld("other", "anything", "komga", false), None);
    assert!(bounds
        .decided("other", "x k3y-of-sonarr", "komga")
        .is_some_and(|why| why.starts_with("decides on other for a call to komga")));
    assert_eq!(bounds.decided("other", "k3y-of-sonarr", "sonarr"), None);
    assert_eq!(bounds.decided("other", "free", "komga"), None);

    let token = "token".to_owned();
    bounds.captured(
        &BTreeMap::from([("other".to_owned(), "k3y-of-sonarr".to_owned())]),
        None,
        [&token].into_iter(),
    );
    assert!(bounds.withheld("token", "v", "komga", false).is_some_and(
        |why| why.contains("token was traded for the credential lemonfiber holds for sonarr")
    ));
}

/// A value captured again is held at least as tightly as before: what a credential bought
/// stays bought when a plain answer later lands under the same name, and a plain answer
/// is tightened when a credential buys it.
#[test]
fn a_hold_only_grows_stronger_when_a_value_is_captured_again() {
    let recipe = releasing();
    let approved = ["bought@komga".to_owned()];
    let mut bounds = Bounds::of(&recipe, &approved, &[]);
    let bought = "bought".to_owned();
    bounds.captured(&BTreeMap::new(), Some("sonarr"), [&bought].into_iter());
    assert_eq!(bounds.withheld("bought", "v", "komga", false), None);
    bounds.captured(
        &BTreeMap::from([("key".to_owned(), "v".to_owned())]),
        Some("sonarr"),
        [&bought].into_iter(),
    );
    bounds.captured(&BTreeMap::new(), Some("sonarr"), [&bought].into_iter());
    assert!(bounds
        .withheld("bought", "v", "komga", false)
        .is_some_and(|why| why.contains("traded for the credential lemonfiber holds for sonarr")));
}
