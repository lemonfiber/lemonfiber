use crate::refusing::installing::tests::uninstallable;
use crate::refusing::tests::{names, said, with, INSTALLABLE};

const ADAPTER: &str = "komga";

const UPSTREAM: &str = "komga-server";

const UPSTREAM_SERVICE: &str = r#"[[service]]
id          = "komga-server"
name        = "Komga server"
image       = "docker.io/gotson/komga"
digest      = "sha256:4f53cda18c2baa0c0354bb5f9a3ecbe5ed12ab4d8e11ba873c2f11161202b945"
tag         = "1.11.0"
criticality = "important"
"#;

const ASKED: &str = "shape = \"egress-guard\"\n";

/// The fixture's service as an adapter speaking `speaks` in front of an upstream of its
/// own, with `upstream` added to the upstream's table and `adapter` to the adapter's.
fn fronting(speaks: &str, upstream: &str, adapter: &str) -> Vec<String> {
    said(
        &with(&format!(
            "speaks      = {speaks}\nlistens     = 8080\nfronts      = \"{UPSTREAM}\"\n{adapter}"
        ))
        .replace(
            "[[claim]]\n",
            &format!("{UPSTREAM_SERVICE}{upstream}\n[[claim]]\n"),
        )
        .replace(
            "[[wiring]]\n",
            &format!("[[wiring]]\nservice         = \"{ADAPTER}\"\n"),
        )
        .replace(
            "id      = \"komga.serves\"\n",
            &format!("id      = \"komga.serves\"\nservice = \"{ADAPTER}\"\n"),
        ),
    )
}

fn refused_at(said: &[String], service: &str) -> bool {
    names(
        said,
        &[
            &format!("service {service}.shape"),
            "egress-guard",
            "network.egress-guard",
        ],
    )
}

#[test]
fn the_upstream_an_egress_guard_adapter_fronts_may_take_the_shape() {
    let said = fronting(r#"["network.egress-guard@1"]"#, ASKED, "");
    assert!(
        names(&said, &["service komga.speaks", "network.egress-guard@1"]),
        "the manifest was read and its values judged: {said:?}"
    );
    assert!(!refused_at(&said, UPSTREAM), "{said:?}");
    assert!(!names(&said, &[".shape"]), "{said:?}");
}

#[test]
fn the_adapter_itself_may_not_take_the_shape() {
    let said = fronting(r#"["network.egress-guard@1"]"#, "", ASKED);
    assert!(refused_at(&said, ADAPTER), "{said:?}");
}

#[test]
fn the_upstream_of_an_adapter_for_another_capability_may_not_take_the_shape() {
    let said = fronting(r#"["media.serve@1"]"#, ASKED, "");
    assert!(refused_at(&said, UPSTREAM), "{said:?}");
}

#[test]
fn a_service_nothing_fronts_may_not_take_the_shape() {
    let said = said(&with(ASKED));
    assert!(refused_at(&said, ADAPTER), "{said:?}");
}

#[test]
fn an_adapter_fronting_itself_may_not_take_the_shape() {
    let said = said(&with(&format!(
        "speaks      = [\"network.egress-guard@1\"]\nlistens     = 8080\nfronts      = \"{ADAPTER}\"\n{ASKED}"
    )));
    assert!(refused_at(&said, ADAPTER), "{said:?}");
}

#[test]
fn a_shape_outside_the_closed_set_is_refused_by_name() {
    let said = said(&with("shape = \"privileged\"\n"));
    assert!(names(&said, &["shape", "privileged"]), "{said:?}");
}

#[test]
fn a_service_serving_media_may_take_the_render_shape() {
    assert!(said(&with("shape = \"render\"\n")).is_empty());
}

#[test]
fn a_service_serving_no_media_may_not_take_the_render_shape() {
    let said = said(&with("shape = \"render\"\n").replace(
        "provides    = [\"media.serve\", \"komga:kobo-sync\"]",
        "provides = [\"komga:kobo-sync\"]",
    ));
    assert!(
        names(&said, &["service komga.shape", "render", "media.serve"]),
        "{said:?}"
    );
}

/// The fixture's service providing the egress guard itself, with `added` after its
/// configuration directory.
fn guarding(added: &str) -> String {
    with(&format!("{ASKED}{added}")).replace(
        "provides    = [\"media.serve\", \"komga:kobo-sync\"]",
        "provides    = [\"network.egress-guard\"]",
    )
}

#[test]
fn a_plugin_s_one_service_providing_the_egress_guard_may_take_the_shape() {
    let said = said(&guarding(""));
    assert!(!refused_at(&said, ADAPTER), "{said:?}");
}

#[test]
fn a_service_providing_the_egress_guard_beside_another_may_not_take_the_shape() {
    let said =
        said(&guarding("").replace("[[claim]]\n", &format!("{UPSTREAM_SERVICE}\n[[claim]]\n")));
    assert!(refused_at(&said, ADAPTER), "{said:?}");
}

#[test]
fn critical_without_the_egress_guard_s_shape_is_refused_naming_the_four_without_it() {
    let said = said(&INSTALLABLE.replacen(
        "criticality = \"important\"",
        "criticality = \"critical\"",
        1,
    ));
    assert!(
        names(
            &said,
            &[
                "service komga.criticality",
                "is critical,",
                "shape = \"egress-guard\"",
                "core, important, enhancing, optional"
            ]
        ),
        "{said:?}"
    );
}

#[test]
fn critical_with_the_egress_guard_s_shape_is_read() {
    let said = said(&guarding("").replacen(
        "criticality = \"important\"",
        "criticality = \"critical\"",
        1,
    ));
    assert!(!names(&said, &[".criticality"]), "{said:?}");
}

#[test]
fn a_plugin_s_one_egress_guard_is_refused_at_install() {
    let refused = uninstallable(&guarding(""));
    assert!(
        names(
            &refused,
            &[
                "service komga.shape",
                "egress-guard",
                "the bundle assembles"
            ]
        ),
        "{refused:?}"
    );
}

#[test]
fn an_egress_guard_an_adapter_fronts_is_installable() {
    let text = with(&format!(
        "speaks      = [\"network.egress-guard@1\"]\nlistens     = 8080\nfronts      = \"{UPSTREAM}\"\n"
    ))
    .replace(
        "[[claim]]\n",
        &format!("{UPSTREAM_SERVICE}{ASKED}\n[[claim]]\n"),
    );
    let refused = uninstallable(&text);
    assert!(!names(&refused, &[".shape"]), "{refused:?}");
}
