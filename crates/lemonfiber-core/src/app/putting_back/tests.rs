use super::inverted;
use crate::journal::Kind;

#[test]
fn putting_a_setting_or_a_secret_back_is_recorded_the_other_way_round() {
    assert_eq!(
        inverted(&Kind::Setting {
            name: "komga:mode".to_owned(),
            previous: Some("slow".to_owned()),
            current: Some("fast".to_owned()),
        }),
        Some(Kind::Setting {
            name: "komga:mode".to_owned(),
            previous: Some("fast".to_owned()),
            current: Some("slow".to_owned()),
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
