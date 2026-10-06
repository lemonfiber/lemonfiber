use super::Origin;
use crate::schema::tests::WHOLE;
use crate::schema::Manifest;

/// Each origin is said as the word the manifest writes it as, so a refusal quoting one
/// quotes what the author can find in their file.
#[test]
fn every_origin_is_said_as_the_manifest_writes_it() {
    for (origin, written) in [
        (Origin::StackService, "stack-service"),
        (Origin::ExternalResponse, "external-response"),
        (Origin::CredentialStore, "credential-store"),
        (Origin::Operator, "operator"),
    ] {
        assert_eq!(origin.written(), written);
        let read = Manifest::from_toml(&WHOLE.replace(
            r#"origin = "stack-service" }]"#,
            &format!(r#"origin = "{written}" }}]"#),
        ));
        assert!(read.is_ok_and(|manifest| manifest
            .recipes
            .iter()
            .flat_map(|recipe| &recipe.steps)
            .flat_map(|step| &step.capture)
            .all(|capture| capture.origin == origin)));
    }
}
