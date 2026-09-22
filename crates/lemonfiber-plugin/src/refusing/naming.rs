//! Which of a plugin's services a declaration is about.
//!
//! A question with no content while a plugin declared one service, and three
//! questions the moment it could declare two: which service a hostname is for, which
//! one a proof asks, and which one a contributed check reports on. Each was answered
//! by *the plugin's own*, and that phrase stopped having a referent.
//!
//! One module rather than three rules scattered among the others, because the mistake
//! is the same mistake each time — a name left out, or a name nothing answers to — and
//! an author meeting it in three wordings would be reading three rules where there is
//! one.

use std::collections::BTreeSet;

use crate::claiming;
use crate::schema::{Bind, Manifest, Service};
use crate::Violation;

/// How the stack's own proxy and dashboard reach each service.
///
/// A hostname is a fact about one service rather than about the plugin, which only
/// became visible when a plugin could declare two: the label in front of the operator's
/// domain has to name one of them, and a default taken from the plugin's own id would
/// have given two containers one address. So a wiring names its service, and may leave
/// the name out only where there is nothing to choose between.
///
/// **The tier still governs.** A `loopback` service is not proxied, there is no field by
/// which it could ask to be, and a wiring that gave one a hostname would be a plugin
/// putting an admin surface on the household network, which is the reason the refusal
/// names the tier rather than the field.
pub(super) fn wired(manifest: &Manifest, found: &mut Vec<Violation>) {
    let one = manifest.services.len() == 1;
    let mut seen: BTreeSet<&str> = BTreeSet::new();
    for (at, wiring) in manifest.wirings.iter().enumerate() {
        let location = wiring.service.as_deref().map_or_else(
            || format!("wiring #{}", at + 1),
            |id| format!("wiring {id}"),
        );
        let Some(service) = names(manifest, wiring.service.as_deref(), &location, one, found)
        else {
            continue;
        };
        if !seen.insert(service.id.as_str()) {
            found.push(Violation {
                location: location.clone(),
                message: format!(
                    "{} is wired twice, and a service has one address and one panel",
                    service.id
                ),
            });
        }
        if wiring.hostname.is_some() && service.bind == Some(Bind::Loopback) {
            found.push(Violation {
                location: format!("{location}.hostname"),
                message: format!(
                    "{} is bound to loopback, and the tier decides whether a service is reachable \
                     by name; a loopback service is not proxied and cannot ask to be",
                    service.id
                ),
            });
        }
    }
}

/// Which service a proof or a contribution is about.
///
/// Both ask one service a question, and with one service there was nothing to say. With
/// two, a proof that did not name one would be checked against whichever the reader
/// reached first — including the digest its recording is held to, which is the whole of
/// what ties a recording to the image somebody will actually run.
pub(super) fn about(manifest: &Manifest, found: &mut Vec<Violation>) {
    let one = manifest.services.len() == 1;
    for proof in &manifest.proofs {
        let at = format!("proof {}", proof.id);
        names(manifest, proof.service.as_deref(), &at, one, found);
    }
    for entry in &manifest.contributions {
        let at = format!("contribution {}", entry.id);
        // A remedy asks nothing, so it names nothing. Requiring a service of a row that
        // carries no request would be asking an author to answer a question about a call
        // their row does not make.
        if entry.request.is_none() && entry.service.is_none() {
            continue;
        }
        names(manifest, entry.service.as_deref(), &at, one, found);
    }
}

/// The service a declaration names, or the refusal for naming none or naming wrong.
///
/// One function for the three places that name a service, because the two mistakes are
/// the same two each time and an author meeting them in three wordings would be reading
/// three rules where there is one.
fn names<'a>(
    manifest: &'a Manifest,
    named: Option<&str>,
    at: &str,
    one: bool,
    found: &mut Vec<Violation>,
) -> Option<&'a Service> {
    let Some(named) = named else {
        if one {
            return manifest.services.first();
        }
        found.push(Violation {
            location: format!("{at}.service"),
            message: format!(
                "names no service, and this plugin declares more than one; it declares: {}",
                claiming::listed(manifest.services.iter().map(|service| &service.id))
            ),
        });
        return None;
    };
    let found_service = manifest.services.iter().find(|service| service.id == named);
    if found_service.is_none() {
        found.push(Violation {
            location: format!("{at}.service"),
            message: format!(
                "{named} is no service this plugin declares; it declares: {}",
                claiming::listed(manifest.services.iter().map(|service| &service.id))
            ),
        });
    }
    found_service
}

#[cfg(test)]
mod tests {
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
}
