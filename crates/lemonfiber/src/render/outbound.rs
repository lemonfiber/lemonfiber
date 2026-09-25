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
mod tests;
