//! Everything that leaves this machine, on a terminal.
//!
//! Ordered so the answer to "what does this thing send about me" is the first thing
//! on the screen and the answer to "what can I stop" is beside each entry rather
//! than in a paragraph at the end. What a request *sends* leads each entry, because
//! it is the sentence somebody came here to read; the purpose and the cost follow,
//! because they are what turning it off is weighed against.
//!
//! The stack's own requests are last and are headed as theirs. An operator reading
//! this is entitled to know what running the stack reaches, and equally entitled not
//! to have it counted against the product that started it.

use lemonfiber_core::config::OFFLINE_KEY;
use lemonfiber_core::outbound::{nothing_configured, Elsewhere, Leaving, Outbound};

use super::Lines;

/// Everything that leaves this machine, and what refusing each of it costs.
pub(crate) fn leaving(report: &Leaving) -> Lines {
    let mut lines = Lines::default();
    lines.put("What lemonfiber sends, on its own account:");
    for entry in &report.ours {
        lines.extend(ours(entry));
    }
    lines.spaced(format!(
        "Stop all of it at once with:  lemonfiber config set {OFFLINE_KEY} on"
    ));
    lines.extend(theirs(&report.theirs));
    lines
}

/// One of lemonfiber's own requests.
fn ours(entry: &Outbound) -> Lines {
    let mut lines = Lines::default();
    lines.spaced(format!(
        "  {} — {}",
        entry.reach.as_str(),
        if entry.allowed { "on" } else { "off" }
    ));
    lines.put(format!("    to      {}", where_to(entry)));
    lines.put(format!("    sends   {}", entry.sends));
    lines.put(format!("    for     {}", entry.purpose));
    lines.put(format!(
        "    off by  lemonfiber config set {} off",
        entry.switch
    ));
    lines.put(format!("    costs   {}", entry.cost));
    lines
}

/// Where a request goes, or that there is nowhere for it to go.
fn where_to(entry: &Outbound) -> String {
    if entry.destination.is_empty() {
        return nothing_configured().to_owned();
    }
    entry.destination.join(", ")
}

/// What the stack's own services reach, headed as theirs.
fn theirs(services: &[Elsewhere]) -> Lines {
    let mut lines = Lines::default();
    if services.is_empty() {
        return lines;
    }
    lines.spaced("What the services in this stack send, which is theirs and not lemonfiber's:");
    for service in services {
        lines.spaced(format!(
            "  {}{} — {}",
            service.service,
            brought_by(&service.origin),
            goes(service)
        ));
        lines.put(format!("    {}", service.purpose));
    }
    lines
}

/// Whose request it is, where that is not the stack's own, beside the name.
///
/// Nothing for the stack's own, which is every row this heading has always held; the
/// heading already says whose those are.
fn brought_by(origin: &lemonfiber_core::origin::Origin) -> String {
    match origin {
        lemonfiber_core::origin::Origin::Plugin { named } => format!(" (plugin {named})"),
        _ => String::new(),
    }
}

/// Where one service reaches, as the line above its explanation.
///
/// Three answers rather than two, and the third is the one worth having: a service
/// lemonfiber ships no record for is marked as such, because the sentence for a
/// service that reaches nothing is a promise and must never be given to a service
/// nobody has looked at.
fn goes(service: &Elsewhere) -> String {
    if !service.recorded {
        return format!("{} (not one lemonfiber knows)", service.destination);
    }
    if service.destination.is_empty() {
        return "nothing leaves this machine".to_owned();
    }
    service.destination.clone()
}

#[cfg(test)]
mod tests {
    use lemonfiber_core::config::{Reaching, Settings};
    use lemonfiber_core::origin::Origin;
    use lemonfiber_core::outbound::{leaving as gathered, Elsewhere, Leaving};

    use super::leaving;

    /// The report as the core builds it for a machine with no stack materialised —
    /// which is every entry lemonfiber makes on its own account, and nothing of the
    /// stack's, since what the services reach is read from the services there are.
    fn shown(settings: &Settings) -> String {
        leaving(&gathered(settings, &[], &[])).text()
    }

    #[test]
    fn every_request_is_named_with_where_it_goes_and_what_it_sends() {
        let said = shown(&Settings::default());
        for named in ["registry", "guides", "echo", "indexer", "usenet"] {
            assert!(said.contains(named), "{named} is not on the screen: {said}");
        }
        assert!(said.contains("sends"), "{said}");
        assert!(said.contains("costs"), "{said}");
    }

    #[test]
    fn a_request_that_is_on_says_so_and_one_that_is_off_says_so() {
        let on = shown(&Settings::default());
        assert!(on.contains("registry — on"), "{on}");
        let off = shown(&Settings {
            ip_echo: Vec::new(),
            reaching: Reaching::none(),
            ..Settings::default()
        });
        assert!(off.contains("registry — off"), "{off}");
        assert!(off.contains("echo — off"), "{off}");
    }

    #[test]
    fn a_request_with_nowhere_to_go_says_that_rather_than_leaving_the_line_blank() {
        let said = shown(&Settings::default());
        assert!(said.contains("nothing configured"), "{said}");
    }

    #[test]
    fn the_setting_that_stops_everything_is_offered_once_under_the_list() {
        let said = shown(&Settings::default());
        assert_eq!(said.matches("LEMONFIBER_OFFLINE").count(), 1, "{said}");
    }

    #[test]
    fn each_request_is_offered_the_command_that_switches_it_off() {
        let said = shown(&Settings::default());
        for key in [
            "LEMONFIBER_REACH_REGISTRY",
            "LEMONFIBER_REACH_GUIDES",
            "LEMONFIBER_IP_ECHO",
            "LEMONFIBER_REACH_INDEXER",
            "LEMONFIBER_REACH_USENET",
        ] {
            assert!(
                said.contains(&format!("lemonfiber config set {key} off")),
                "{key} is listed with no way to switch it off: {said}"
            );
        }
    }

    #[test]
    fn the_stacks_own_requests_are_headed_as_the_stacks() {
        let said = leaving(&Leaving {
            ours: Vec::new(),
            theirs: vec![Elsewhere {
                service: "prowlarr".to_owned(),
                destination: "the indexers you configured".to_owned(),
                purpose: "Runs the searches everything else asks for.".to_owned(),
                recorded: true,
                origin: Origin::Bundled,
            }],
        })
        .text();
        assert!(said.contains("theirs and not lemonfiber's"), "{said}");
        assert!(
            said.contains("prowlarr — the indexers you configured"),
            "{said}"
        );
    }

    #[test]
    fn a_plugins_request_is_marked_with_the_plugin_that_brought_it() {
        let said = leaving(&Leaving {
            ours: Vec::new(),
            theirs: vec![Elsewhere {
                service: "komga".to_owned(),
                destination: "metadata.example".to_owned(),
                purpose: "A destination this plugin's recipes declare.".to_owned(),
                recorded: true,
                origin: Origin::Plugin {
                    named: "comics".to_owned(),
                },
            }],
        })
        .text();
        assert!(said.contains("komga (plugin comics) — "), "{said}");
    }

    #[test]
    fn a_service_that_reaches_nothing_says_nothing_leaves_rather_than_nowhere() {
        let said = leaving(&Leaving {
            ours: Vec::new(),
            theirs: vec![Elsewhere {
                service: "unpackerr".to_owned(),
                destination: String::new(),
                purpose: "Nothing. It extracts what it finds on this machine.".to_owned(),
                recorded: true,
                origin: Origin::Bundled,
            }],
        })
        .text();
        assert!(
            said.contains("unpackerr — nothing leaves this machine"),
            "{said}"
        );
    }

    /// Through the dispatcher rather than by calling this module, because what the
    /// terminal draws is what the printer chose for the outcome — and an arm nothing
    /// reaches renders nowhere however good the renderer under it is.
    #[test]
    fn the_printer_reaches_this_renderer_for_this_outcome() {
        let report = gathered(&Settings::default(), &[], &[]);
        let drawn = crate::render::shaped(&lemonfiber_core::app::Outcome::Outbound(report)).text();
        assert!(drawn.contains("What lemonfiber sends"), "{drawn}");
        assert!(drawn.contains("registry"), "{drawn}");
    }

    /// The line an unknown service gets, which must not be the one that promises
    /// nothing leaves the machine.
    #[test]
    fn a_service_lemonfiber_has_no_record_for_says_so_rather_than_promising_anything() {
        let said = leaving(&Leaving {
            ours: Vec::new(),
            theirs: vec![Elsewhere {
                service: "somebodys-own-service".to_owned(),
                destination: "not known to lemonfiber".to_owned(),
                purpose: "This service is not one lemonfiber knows.".to_owned(),
                recorded: false,
                origin: Origin::Bundled,
            }],
        })
        .text();
        assert!(said.contains("somebodys-own-service"), "{said}");
        assert!(said.contains("not one lemonfiber knows"), "{said}");
        assert!(
            !said.contains("nothing leaves this machine"),
            "an unknown service was given the promise that belongs to a known one: {said}"
        );
    }

    #[test]
    fn a_stack_with_no_services_says_nothing_about_them_at_all() {
        let said = leaving(&Leaving {
            ours: Vec::new(),
            theirs: Vec::new(),
        })
        .text();
        assert!(!said.contains("theirs"), "{said}");
    }
}
