use lemonfiber_manifest::container::earlier::Earlier;

use super::super::tests::{names, said, with, INSTALLABLE};
use crate::schema::Manifest;

#[test]
fn a_variable_a_plugin_names_twice_or_badly_is_refused_naming_both() {
    let text = with(
        "[service.env]\nA = { setting = \"komga:one\", default = \"\", was = \"KOMGA_ONE\" }\nB = { \
         setting = \"komga:two\", default = \"\", was = \"KOMGA_ONE\" }\nC = { setting = \
         \"komga:three\", default = \"\", was = \"komga-three\" }\n",
    );
    let found = said(&text);
    assert!(
        names(
            &found,
            &[
                "setting komga:two.was",
                "KOMGA_ONE is named by setting komga:one and setting komga:two"
            ]
        ),
        "{found:?}"
    );
    assert!(
        names(
            &found,
            &["setting komga:three.was", "komga-three is not upper-case"]
        ),
        "{found:?}"
    );
}

#[test]
fn a_variable_the_stack_names_for_another_setting_is_refused_and_for_the_same_is_not() {
    let stack = |name: &str| Earlier {
        key: "LOG_LEVEL".to_owned(),
        name: name.to_owned(),
        secret: false,
    };
    let manifest = Manifest::from_toml(&with(
        "[service.env]\nLEVEL = { setting = \"komga:log-level\", default = \"\", was = \
         \"LOG_LEVEL\" }\n",
    ));
    let found = |held: &str| {
        let mut found = Vec::new();
        if let Ok(manifest) = &manifest {
            super::against(manifest, &[stack(held)], &mut found);
        }
        found.iter().map(ToString::to_string).collect::<Vec<_>>()
    };
    assert!(
        names(
            &found("flaresolverr:log-level"),
            &[
                "setting komga:log-level.was",
                "LOG_LEVEL is named by the stack's setting flaresolverr:log-level"
            ]
        ),
        "{:?}",
        found("flaresolverr:log-level")
    );
    assert_eq!(found("komga:log-level"), Vec::<String>::new());
    assert_eq!(said(INSTALLABLE), Vec::<String>::new());
}
