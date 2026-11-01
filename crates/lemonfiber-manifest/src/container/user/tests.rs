use std::collections::BTreeMap;

use super::super::tests::{plain, wrong, NONE};
use super::super::{SecretOf, Value};
use super::{IdVariables, UserMode};

#[test]
fn puid_env_is_the_pair_of_named_variables_and_is_written_back_by_its_name() {
    let named: Result<BTreeMap<String, UserMode>, _> = toml::from_str(
        "a = { uid_env = \"PUID\", gid_env = \"PGID\" }\nb = { uid_env = \"PLEX_UID\", gid_env = \"PLEX_GID\" }\nc = \"image\"\nd = \"uid\"",
    );
    let written = named.ok().and_then(|read| toml::to_string(&read).ok());
    assert_eq!(
        written.as_deref(),
        Some("a = \"puid-env\"\nc = \"image\"\nd = \"uid\"\n\n[b]\nuid_env = \"PLEX_UID\"\ngid_env = \"PLEX_GID\"\n")
    );
}

#[test]
fn named_id_variables_are_two_different_variables_lemonfiber_alone_writes() {
    let none = NONE;
    let declared = BTreeMap::from([(
        "PLEX_UID".to_owned(),
        Value::Secret(SecretOf {
            secret: "admin-key".to_owned(),
        }),
    )]);
    let cases = [
        (("PLEX_UID", "PLEX_GID"), &none, 0),
        (("plex_uid", "PLEX_GID"), &none, 1),
        (("TZ", "PLEX_GID"), &none, 1),
        (("PLEX_ID", "PLEX_ID"), &none, 1),
        (("PLEX_UID", "PLEX_GID"), &declared, 1),
    ];
    for (case, ((uid_env, gid_env), declared, refused)) in cases.into_iter().enumerate() {
        let mode = UserMode::IdsIn(IdVariables {
            uid_env: uid_env.to_owned(),
            gid_env: gid_env.to_owned(),
        });
        let mut service = plain(declared);
        service.user_mode = Some(&mode);
        assert_eq!(wrong(&service).len(), refused, "case {case}");
    }
}

#[test]
fn a_mode_that_is_neither_a_name_nor_a_table_is_refused_saying_what_it_may_be() {
    let said = toml::from_str::<BTreeMap<String, UserMode>>("a = 3")
        .err()
        .map(|refused| refused.to_string())
        .unwrap_or_default();
    assert!(
        said.contains("image, puid-env, uid, or a table of uid_env and gid_env"),
        "{said}"
    );
    let unnamed = toml::from_str::<BTreeMap<String, UserMode>>("a = \"root\"")
        .err()
        .map(|refused| refused.to_string())
        .unwrap_or_default();
    assert!(unnamed.contains("unknown variant"), "{unnamed}");
}
