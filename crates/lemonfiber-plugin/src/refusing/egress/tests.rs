use crate::refusing::tests::{names, said, INSTALLABLE};

/// The fixture with a second service carrying `carried` out of the machine, its table ending
/// in `added`, and `after` appended to the manifest.
fn carrying(carried: &str, added: &str, after: &str) -> Vec<String> {
    let posture = if added.contains("posture") {
        ""
    } else {
        "posture     = \"confined\"\n"
    };
    let egress = format!(
        r#"[[service]]
id          = "komga-egress"
name        = "Komga egress"
image       = "docker.io/gotson/komga"
digest      = "sha256:4f53cda18c2baa0c0354bb5f9a3ecbe5ed12ab4d8e11ba873c2f11161202b945"
tag         = "1.11.0"
criticality = "optional"
{posture}listens     = 3128
network     = {{ egress_for = "{carried}" }}
{added}
"#
    );
    let text = INSTALLABLE
        .replace("[[claim]]\n", &format!("{egress}\n[[claim]]\n"))
        .replace("[[wiring]]\n", "[[wiring]]\nservice         = \"komga\"\n")
        .replace(
            "id      = \"komga.serves\"\n",
            "id      = \"komga.serves\"\nservice = \"komga\"\n",
        );
    said(&format!("{text}{after}"))
}

#[test]
fn a_service_may_carry_another_of_its_plugin_out_of_the_machine_holding_nothing() {
    let said = carrying("komga", "", "");
    assert!(said.is_empty(), "{said:?}");
}

#[test]
fn an_egress_naming_no_other_service_of_its_plugin_is_refused() {
    for carried in ["seerr", "komga-egress"] {
        let said = carrying(carried, "", "");
        assert!(
            names(&said, &["service komga-egress.network.egress_for"]),
            "{carried}: {said:?}"
        );
    }
}

#[test]
fn an_egress_that_asks_for_anything_is_refused() {
    let said = carrying(
        "komga",
        "",
        "\n[[ask]]\nservice    = \"komga-egress\"\ncapability = \"media.serve\"\n",
    );
    assert!(
        names(
            &said,
            &["service komga-egress.network.egress_for", "makes an ask"]
        ),
        "{said:?}"
    );
}

#[test]
fn an_egress_holding_anything_or_reachable_from_outside_is_refused() {
    for (added, why) in [
        ("config_path = \"/config\"\n", "a configuration directory"),
        ("takes_data  = true\n", "the data mount"),
        ("port        = 3128\n", "a port"),
        ("posture     = \"standard\"\n", "is not confined"),
        ("provides    = [\"media.serve\"]\n", "provides a capability"),
    ] {
        let said = carrying("komga", added, "");
        assert!(
            names(&said, &["service komga-egress.network.egress_for", why]),
            "{why}: {said:?}"
        );
    }
}
