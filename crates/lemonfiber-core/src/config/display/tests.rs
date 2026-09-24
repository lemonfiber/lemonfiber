use super::{in_full, shown_in_a_file, without_credentials, SHOWN};

#[test]
fn a_setting_on_the_list_is_displayed_and_one_beside_it_is_not() {
    assert!(in_full("DATA_ROOT"));
    assert!(in_full("INDEXER_URL"));
    assert!(in_full("USENET_VALIDATED"));
    assert!(!in_full("INDEXER_APIKEY"));
    assert!(!in_full("USENET_PASS"));
}

#[test]
fn a_setting_nobody_has_decided_about_is_not_displayed() {
    // The property the list is bought for, and the one a marker list cannot have:
    // none of these carries a word any keyword rule recognises, and every one of
    // them is a credential somewhere.
    for name in [
        "OPENVPN_USER",
        "PLEX_CLAIM",
        "DISCORD_WEBHOOK",
        "DB_PWD",
        "SESSION_SALT",
        "DATABASE_URL",
        "SOME_SERVICE_ADDED_NEXT_YEAR",
    ] {
        assert!(!in_full(name), "{name} is displayed and nobody said why");
    }
}

#[test]
fn a_files_line_is_read_against_the_list_only_where_its_name_is_a_setting() {
    // An environment setting shouts, and the list answers for it — including for
    // the account number no marker word names, which is the case the list exists
    // for and the case a diff of the same file used to print.
    assert!(!shown_in_a_file("OPENVPN_USER"));
    assert!(!shown_in_a_file("SONARR_API_KEY"));
    assert!(shown_in_a_file("DATA_ROOT"));
    assert!(shown_in_a_file("  TZ  "));
    // Compose's own schema is lower case throughout, and no list vouches for it.
    // The marker rule answers there, as it did before, so a diff of the lines an
    // operator actually edits still reads as a diff.
    assert!(shown_in_a_file("image"));
    assert!(shown_in_a_file("network_mode"));
    assert!(!shown_in_a_file("api_key"));
}

#[test]
fn a_name_written_with_spaces_around_it_is_still_the_same_name() {
    assert!(in_full("  DATA_ROOT  "));
}

#[test]
fn a_displayed_address_keeps_its_address_and_loses_what_it_carries() {
    assert_eq!(
        without_credentials("https://indexer.example/api"),
        "https://indexer.example/api"
    );
    // Assembled rather than written out, and a placeholder rather than a plausible
    // key: what this fixture has to be is withheld, and a run of hex in source is a
    // secret scanner's finding for as long as the commit exists.
    let key = ["the", "indexer", "key"].join("-");
    let shown = without_credentials(&format!("https://indexer.example/api?apikey={key}"));
    assert!(
        shown.starts_with("https://indexer.example/api?"),
        "the address went with the key riding in its query"
    );
    assert!(
        !shown.contains(&key),
        "the key in the query survived into the displayed address"
    );
    // The other half a URL can carry a credential in, on the surface an operator
    // reads their own settings back from.
    let behind = without_credentials(&format!("https://operator:{key}@indexer.example/api"));
    assert!(
        !behind.contains(&key),
        "the password in front of the host survived into the displayed address"
    );
    assert!(
        behind.starts_with("https://operator:"),
        "the account went with the password beside it"
    );
    assert!(
        behind.ends_with("@indexer.example/api"),
        "the host and the path went with the password in front of them"
    );
}

#[test]
fn the_named_front_door_is_displayed_with_what_naming_one_costs() {
    // The one setting whose reason has a second job. Naming a door is safe to
    // display and that is not the interesting half: what an operator has to be
    // told where they set it is what it costs them, which is that lemonfiber
    // stops keeping the answer right. A reason saying only what the setting is
    // would leave them to find that out by growing a request surface later and
    // wondering why nobody is being sent to it.
    let reason = SHOWN
        .iter()
        .find(|(name, _)| *name == super::super::FRONT_DOOR_KEY)
        .map(|(_, reason)| *reason);
    assert!(
        reason.is_some_and(|reason| reason.contains("stops keeping right")),
        "{reason:?}"
    );
}

#[test]
fn no_setting_is_written_down_twice() {
    // Two entries for one name are two reasons, and the second is the one nobody
    // reads when they change the first.
    let mut names: Vec<&str> = SHOWN.iter().map(|(name, _)| *name).collect();
    names.sort_unstable();
    let mut unique = names.clone();
    unique.dedup();
    assert_eq!(names, unique, "a setting is on the list twice");
}
