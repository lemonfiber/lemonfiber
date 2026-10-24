use super::inverted;
use crate::journal::Kind;

#[test]
fn putting_a_setting_or_a_secret_back_is_recorded_the_other_way_round() {
    assert_eq!(
        inverted(&Kind::Setting {
            name: "komga:mode".to_owned(),
            previous: Some("slow".to_owned()),
            current: Some("fast".to_owned()),
            filed: None,
        }),
        Some(Kind::Setting {
            name: "komga:mode".to_owned(),
            previous: Some("fast".to_owned()),
            current: Some("slow".to_owned()),
            filed: None,
        })
    );
    assert_eq!(
        inverted(&Kind::Secret {
            name: "komga:token".to_owned(),
            from: "recipe".to_owned(),
            previous: None,
            current: Some("~1".to_owned()),
            filed: Some("PLUGIN_KOMGA_TOKEN_SECRET".to_owned()),
        }),
        Some(Kind::Secret {
            name: "komga:token".to_owned(),
            from: "recipe".to_owned(),
            previous: Some("~1".to_owned()),
            current: None,
            filed: None,
        })
    );
}

#[test]
fn every_kind_with_an_inverse_is_recorded_the_other_way_round_and_the_rest_are_not() {
    assert_eq!(
        inverted(&Kind::Set {
            key: "PORT".to_owned(),
            previous: None,
            current: "1".to_owned(),
        }),
        Some(Kind::Set {
            key: "PORT".to_owned(),
            previous: Some("1".to_owned()),
            current: String::new(),
        })
    );
    assert_eq!(
        inverted(&Kind::Configured {
            resource: "downloadclient".to_owned(),
            id: "1".to_owned(),
            field: "category".to_owned(),
            previous: Some("tv".to_owned()),
            current: "series".to_owned(),
        }),
        Some(Kind::Configured {
            resource: "downloadclient".to_owned(),
            id: "1".to_owned(),
            field: "category".to_owned(),
            previous: Some("series".to_owned()),
            current: "tv".to_owned(),
        })
    );
    assert_eq!(
        inverted(&Kind::KeyMinted {
            name: "ha".to_owned(),
            scope: "read".to_owned(),
        }),
        Some(Kind::KeyRevoked {
            name: "ha".to_owned(),
            scope: "read".to_owned(),
        })
    );
    assert_eq!(
        inverted(&Kind::DeviceKeyRegistered {
            name: "pixel".to_owned(),
            backing: "tee".to_owned(),
        }),
        Some(Kind::DeviceKeyRevoked {
            name: "pixel".to_owned(),
        })
    );
    assert_eq!(
        inverted(&Kind::Made {
            path: "/srv".to_owned()
        }),
        None
    );
}

#[test]
fn moving_state_back_is_recorded_as_the_move_the_other_way() {
    assert_eq!(
        inverted(&Kind::Moved {
            from: "/stack/config/jellyfin/cache".to_owned(),
            to: "/stack/mounts/jellyfin/cache".to_owned(),
        }),
        Some(Kind::Moved {
            from: "/stack/mounts/jellyfin/cache".to_owned(),
            to: "/stack/config/jellyfin/cache".to_owned(),
        })
    );
}

#[test]
fn putting_a_grant_back_is_recorded_as_the_grant_the_other_way() {
    let granted = |given: bool| Kind::Granted {
        plugin: "reader".to_owned(),
        grant: crate::test_support::a_grant(),
        given,
    };
    assert_eq!(inverted(&granted(true)), Some(granted(false)));
    assert_eq!(inverted(&granted(false)), Some(granted(true)));
}
