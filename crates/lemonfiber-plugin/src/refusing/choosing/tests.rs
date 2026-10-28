use super::super::tests::{names, said, with};

#[test]
fn a_plugins_choices_and_needs_are_held_to_the_rules_every_manifest_is() {
    let text = with(
        "[service.env]\nA = { setting = \"komga:mode\", default = \"fast\", one_of = [\"fast\", \
         \"slow\"] }\nB = { setting = \"komga:mode\", default = \"fast\", one_of = [\"fast\"] }\nC \
         = { setting = \"komga:speed\", default = \"1\", needs = { \"komga:mode\" = [\"warp\"] } \
         }\n",
    );
    let found = said(&text);
    assert!(
        names(
            &found,
            &[
                "service komga.env.B.one_of",
                "declares the choices of setting komga:mode, which service komga.env.A declares"
            ]
        ),
        "{found:?}"
    );
    assert!(
        names(
            &found,
            &[
                "service komga.env.C.needs",
                "lists warp for setting komga:mode, which is not one of its values: fast, slow"
            ]
        ),
        "{found:?}"
    );

    let fine = with(
        "[service.env]\nA = { setting = \"komga:mode\", default = \"fast\", one_of = [\"fast\", \
         \"slow\"] }\n",
    );
    let fine = said(&fine);
    assert!(!fine.iter().any(|line| line.contains("one_of")), "{fine:?}");
}

#[test]
fn a_secrets_needs_are_judged_in_the_service_that_reads_it() {
    let text = format!(
        "{}\n[[secret]]\nid = \"openvpn-user\"\nof = \"komga\"\nwhy = \"Signs in over \
         OpenVPN.\"\nneeds = {{ \"komga:tunnel\" = [\"openvpn\"] }}\n",
        with("[service.env]\nUSER = { secret = \"openvpn-user\" }\n")
    );
    let found = said(&text);
    assert!(
        names(
            &found,
            &[
                "secret komga:openvpn-user.needs",
                "names setting komga:tunnel, which service komga does not read"
            ]
        ),
        "{found:?}"
    );
}
