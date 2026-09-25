// The fixtures the whole of `refusing` is driven against: a manifest this build
// would act on, the one edit each case makes to it, and the two ways of reading
// what came back. Shared rather than copied, so a rule proved against a fixture
// nobody else uses is a rule proved against nothing in particular.
use crate::refusing::tests::{names, paired, said, INSTALLABLE};

/// A wiring that does not say which service it is about, where there are two.
#[test]
fn a_wiring_naming_no_service_is_refused_where_a_plugin_declares_two() {
    let said = said(&paired().replace("service         = \"komga\"\n", ""));
    assert!(
        names(&said, &["wiring", "names no service", "komga-stats"]),
        "got: {said:?}"
    );
}

/// And is not refused where there is one, because there is nothing to choose.
#[test]
fn a_wiring_naming_no_service_is_accepted_where_a_plugin_declares_one() {
    assert!(said(INSTALLABLE).is_empty(), "the fixture holds");
    let named =
        said(&INSTALLABLE.replace("[[wiring]]\n", "[[wiring]]\nservice         = \"komga\"\n"));
    assert!(named.is_empty(), "naming it is also fine: {named:?}");
}

#[test]
fn a_wiring_naming_a_service_this_plugin_does_not_declare_is_refused() {
    let said = said(&paired().replace(
        "service         = \"komga\"",
        "service         = \"elsewhere\"",
    ));
    assert!(
        names(
            &said,
            &["wiring elsewhere.service", "elsewhere", "komga-stats"]
        ),
        "got: {said:?}"
    );
}

/// One address and one panel, so a second wiring for one service is refused.
#[test]
fn a_service_wired_twice_is_refused_by_name() {
    let said = said(&paired().replace(
        "[[wiring]]\nservice         = \"komga\"\n",
        "[[wiring]]\nservice         = \"komga\"\n\n[[wiring]]\nservice = \"komga\"\n",
    ));
    assert!(names(&said, &["komga is wired twice"]), "got: {said:?}");
}

/// The tier decides whether a service is reachable by name, and a plugin may not.
#[test]
fn a_loopback_service_given_a_hostname_is_refused_naming_the_tier() {
    let said = said(&paired().replace(
        "[[wiring]]\nservice         = \"komga\"\n",
        "[[wiring]]\nservice         = \"komga-stats\"\nhostname = \"stats\"\n\n\
         [[wiring]]\nservice         = \"komga\"\n",
    ));
    assert!(
        names(&said, &["komga-stats", "loopback", "not proxied"]),
        "got: {said:?}"
    );
}

/// A loopback service may still say which dashboard group it belongs in.
///
/// The tier decides the route and nothing else, so refusing the whole wiring would
/// be refusing something lemonfiber accepts — which is the shape of mistake a rule
/// like this makes when it is written one word too wide.
#[test]
fn a_loopback_service_may_be_given_a_dashboard_group() {
    let said = said(&paired().replace(
        "[[wiring]]\nservice         = \"komga\"\n",
        "[[wiring]]\nservice         = \"komga-stats\"\ndashboard_group = \"Library\"\n\n\
         [[wiring]]\nservice         = \"komga\"\n",
    ));
    assert!(said.is_empty(), "got: {said:?}");
}

/// A proof that does not say which service it asks, where there are two.
#[test]
fn a_proof_naming_no_service_is_refused_where_a_plugin_declares_two() {
    let said = said(&paired().replace("service = \"komga\"\n", ""));
    assert!(
        names(&said, &["proof komga.serves.service", "names no service"]),
        "got: {said:?}"
    );
}

#[test]
fn a_contributed_check_naming_a_service_this_plugin_does_not_declare_is_refused() {
    let said = said(&paired().replace("service   = \"komga\"", "service   = \"elsewhere\""));
    assert!(
        names(
            &said,
            &[
                "contribution komga:claimed.service",
                "elsewhere",
                "komga-stats"
            ]
        ),
        "got: {said:?}"
    );
}

/// A remedy asks nothing, so it is not asked which service it asks.
#[test]
fn a_remedy_is_not_asked_which_service_it_is_about() {
    let said = said(&paired());
    assert!(
        !names(&said, &["komga:claim-it"]),
        "a remedy carries no request: {said:?}"
    );
}
