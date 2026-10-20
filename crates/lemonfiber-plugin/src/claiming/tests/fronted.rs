//! A claim that binds a probe asked as the operator, on a service an adapter of the same
//! plugin fronts.

use super::{against, says, CLAIMANT};

const ADAPTER: &str = r#"
[[service]]
id          = "komga-adapter"
name        = "Komga adapter"
image       = "example.invalid/komga-adapter"
digest      = "sha256:4f53cda18c2baa0c0354bb5f9a3ecbe5ed12ab4d8e11ba873c2f11161202b945"
tag         = "1.0.0"
criticality = "enhancing"
fronts      = "komga"
speaks      = ["subtitles.fetch@1"]
"#;

#[test]
fn a_probe_asked_as_the_operator_of_a_service_an_adapter_fronts_is_refused_naming_both() {
    let fronted = CLAIMANT.replacen("\n[[claim]]", &format!("{ADAPTER}\n[[claim]]"), 1);
    let said = against(&fronted);
    assert_eq!(
        said,
        vec![
            "claim media.serve.probe catalogue: catalogue is asked with the operator's \
             credential, and komga is fronted by the adapter komga-adapter, so lemonfiber holds \
             no credential for komga but the key it mints for komga-adapter; what this probe \
             would show is shown through komga-adapter's conformance cases instead"
                .to_owned()
        ]
    );
    let elsewhere = fronted.replace("fronts      = \"komga\"", "fronts      = \"kavita\"");
    assert_eq!(against(&elsewhere), Vec::<String>::new());
    let unprovided = fronted.replace(
        "provides    = [\"media.serve\", \"komga:opds\"]",
        "provides    = [\"komga:opds\"]",
    );
    let said = against(&unprovided);
    assert!(
        says(&said, &["media.serve is in no service's provides"]),
        "{said:?}"
    );
    assert!(!says(&said, &["operator"]), "{said:?}");
}

#[test]
fn a_claim_on_a_capability_the_vocabulary_does_not_carry_asks_nothing_of_its_probes() {
    let said = against(
        &CLAIMANT
            .replacen("\n[[claim]]", &format!("{ADAPTER}\n[[claim]]"), 1)
            .replace(
                "capability = \"media.serve\"",
                "capability = \"media.stream\"",
            ),
    );
    assert!(
        says(&said, &["media.stream is in no service's provides"]),
        "{said:?}"
    );
    assert!(!says(&said, &["probe"]), "{said:?}");
}
