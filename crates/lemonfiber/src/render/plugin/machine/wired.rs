//! What an install or an update does to the wiring.

use super::super::super::Lines;

/// Every ask of the stack's the install leaves contested, and what to do about it.
///
/// Nothing at all where it contests nothing, which is the common case and needs no
/// line: this is a warning, and a warning printed on every install stops being read.
pub(super) fn contesting(contests: &[lemonfiber_core::wiring::Contest], recorded: bool) -> Lines {
    let mut lines = Lines::default();
    if contests.is_empty() {
        return lines;
    }
    lines.spaced(format!(
        "    What {} contested, and reaches nothing until you choose:",
        if recorded { "is now" } else { "it would leave" }
    ));
    for one in contests {
        lines.put(format!(
            "      {} asks for {} — claimed by {}",
            one.by,
            one.capability,
            one.claimants.join(", ")
        ));
    }
    lines.put("      Choose which fills it with `lemonfiber wiring fill`.");
    lines
}

/// Every ask the plugin's services make, with what each reaches and how that is settled.
pub(super) fn asking(asks: &[lemonfiber_core::wiring::Wired], recorded: bool) -> Lines {
    let mut lines = Lines::default();
    if asks.is_empty() {
        return lines;
    }
    lines.spaced(format!(
        "    What it {} for:",
        if recorded { "asks" } else { "would ask" }
    ));
    for link in asks {
        match &link.reaches {
            lemonfiber_core::wiring::Reaches::Asked {
                capability,
                services,
                settled,
                origins,
            } => {
                lines.put(format!(
                    "      {} asks for {capability}, reaching {}",
                    link.by,
                    super::super::super::wiring::reached(services, origins)
                ));
                for said in super::super::super::wiring::settling(settled) {
                    lines.put(format!("        {said}"));
                }
            }
            lemonfiber_core::wiring::Reaches::ByName { service, .. } => {
                lines.put(format!("      {} is wired to {service} by name", link.by));
            }
        }
    }
    lines
}
