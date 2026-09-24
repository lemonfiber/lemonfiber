//! How the line is shared, on a terminal.
//!
//! Ordered by what somebody came here to find out. Where the line stands is first,
//! because that is the question. What is in force is second, with the measured
//! figure beside every share, since a share without it is a setting nobody can
//! check. What the clients are actually doing is third — a limit nobody can see
//! the effect of is a limit that gets turned off.
//!
//! What is *not* limited is last and is always said. The two things an operator
//! fears from a feature like this are that it will throttle the household's own
//! viewing and that it will meddle with the machine, and a report that leaves both
//! to be inferred is a report that gets read as doing them.

use lemonfiber_core::bandwidth::{Answer, Cap, Capacity, Held, Holding, Metered, Reached, Sharing};
use lemonfiber_core::bytes::{a_second, humanize};

use super::Lines;

/// Where the line stands, and what became of anything asked of it.
pub(crate) fn sharing(report: &Sharing) -> Lines {
    let mut lines = Lines::default();
    lines.put(format!("The line — {}", report.means));
    lines.extend(carrying(report.capacity.as_ref(), &report.cautions));
    lines.extend(in_force(report));
    lines.extend(against_the_cap(report));
    lines.extend(keeping(&report.clients));
    lines.extend(untouched(report));
    lines
}

/// What the line was measured to carry, and what that reading is worth.
fn carrying(capacity: Option<&Capacity>, cautions: &[String]) -> Lines {
    let mut lines = Lines::default();
    let Some(line) = capacity else {
        lines.spaced("Nothing has measured this line yet, so a share of it cannot be");
        lines.put("worked out. Give a rate instead, or say what the line carries.");
        return lines;
    };
    lines.spaced(format!(
        "Measured at {} down and {} up — {}",
        a_second(line.down),
        a_second(line.up),
        line.source.means()
    ));
    for caution in cautions {
        lines.put(format!("  {caution}"));
    }
    lines
}

/// What the stack is being held to, and when.
fn in_force(report: &Sharing) -> Lines {
    let mut lines = Lines::default();
    lines.spaced("What the stack may take:");
    // The limit's own sentence rather than one assembled here, because the rule
    // that a share is never shown without the figure it is a share of belongs in
    // one place rather than in each of three surfaces.
    lines.put(format!("  down  {}", report.down.says));
    lines.put(format!("  up    {}", report.up.says));
    if let Some(rhythm) = report.rhythm.as_ref() {
        lines.put(format!(
            "  when  {} — outside those hours the stack has the line",
            rhythm.says()
        ));
        lines.put(match report.zone.as_deref() {
            Some(zone) => format!("        kept by the clients themselves, on {zone} time"),
            None => "        no zone is set, so the clients keep these hours in UTC".to_owned(),
        });
    }
    if let Some(said) = report.respite_says.as_deref() {
        lines.put(format!("  now   {said}"));
    }
    if let Some(ratio) = report.ratio {
        lines.spaced(ratio.to_owned());
    }
    lines
}

/// Where the month stands against a declared cap.
fn against_the_cap(report: &Sharing) -> Lines {
    let mut lines = Lines::default();
    let Some(cap) = report.cap.as_ref() else {
        return lines;
    };
    lines.spaced(format!(
        "A cap of {} a month, and at it: {}",
        humanize(cap.monthly),
        cap.exceeded.means()
    ));
    let Some(month) = report.metered.as_ref() else {
        lines.put("  Nothing could say what has been moved this month, so nothing is");
        lines.put("  counted against it. A cap nothing counts against is not a cap.");
        return lines;
    };
    lines.extend(spent(cap, month, report.reached));
    if let Some(acting) = report.acting {
        lines.put(format!("  {acting}"));
    }
    lines
}

/// What the month has spent, and what that count leaves out.
fn spent(cap: &Cap, month: &Metered, reached: Option<Reached>) -> Lines {
    let mut lines = Lines::default();
    lines.put(format!(
        "  {} moved in {} — {} down, {} up",
        humanize(month.moved()),
        month.month,
        humanize(month.down),
        humanize(month.up)
    ));
    lines.put(match reached {
        Some(Reached::Exceeded) => "  The cap is spent.".to_owned(),
        Some(Reached::Within | Reached::Warning) | None => {
            format!("  {} of it left.", humanize(cap.left(month.moved())))
        }
    });
    for missing in &month.incomplete {
        lines.put(format!("  {missing}"));
    }
    lines.put(format!("  {}", month.excludes));
    lines
}

/// What each client was asked and what it is doing about it.
fn keeping(clients: &[Holding]) -> Lines {
    let mut lines = Lines::default();
    if clients.is_empty() {
        return lines;
    }
    lines.spaced("What the clients say:");
    for client in clients {
        match &client.answer {
            Answer::Silent { said } => {
                lines.put(format!("  {} — would not answer: {said}", client.client));
                lines.put("    So what it is limited to is unknown, not unlimited.");
            }
            Answer::Held { down, up, period } => {
                lines.put(format!(
                    "  {}{}",
                    client.client,
                    period.map_or_else(String::new, |side| format!(" — {}", side.means()))
                ));
                lines.extend(direction("down", down));
                lines.extend(direction("up", up));
            }
        }
        // Beneath the limits rather than beside the name, because it is the
        // louder fact: a client that is not fetching at all is one whose limits
        // are being kept by having nothing to keep them on.
        if let Some(pulling) = client.pulling {
            lines.put(format!("    fetching  {}", pulling.means()));
        }
    }
    lines
}

/// One direction on one client: what it took, what it is moving, and the verdict
/// where there is one worth reading.
fn direction(way: &str, held: &Held) -> Lines {
    let mut lines = Lines::default();
    lines.put(format!(
        "    {way:5} held to {}, moving {}",
        held.accepted.map_or_else(|| "nothing".to_owned(), a_second),
        held.moving.map_or_else(|| "unknown".to_owned(), a_second)
    ));
    if held.verdict.worth_saying() {
        lines.put(format!("      {}", held.verdict.means()));
    }
    lines
}

/// What is outside every limit here, always said.
fn untouched(report: &Sharing) -> Lines {
    let mut lines = Lines::default();
    lines.spaced("Not limited, and never will be:");
    for line in &report.untouched {
        lines.put(format!("  {line}"));
    }
    if report.applied {
        lines.spaced("These limits are now in the clients themselves.");
    }
    lines
}

#[cfg(test)]
mod tests;
