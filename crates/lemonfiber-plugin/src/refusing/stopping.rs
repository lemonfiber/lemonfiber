//! Each `[[reach]]` held to a host outside the stack that a recipe reaches, once, with what
//! refusing it stops.

use std::collections::BTreeSet;

use crate::schema::Manifest;
use crate::Violation;

/// Every `[[reach]]` naming a host no recipe reaches outside the stack, one naming a host
/// another already names, and one whose `stops` says nothing.
pub(super) fn said(manifest: &Manifest, found: &mut Vec<Violation>) {
    let reached = |host: &str| {
        manifest.recipes.iter().any(|recipe| {
            recipe.steps.iter().any(|step| step.call.to == host)
                || recipe.pairs.iter().any(|pair| pair.to == host)
        }) && super::recipes::outside(manifest, host)
    };
    let mut named = BTreeSet::new();
    for (index, reach) in manifest.reaches.iter().enumerate() {
        let at = format!("reach[{index}]");
        if !reached(&reach.host) {
            found.push(Violation {
                location: format!("{at}.host"),
                message: format!(
                    "{} is no host outside the stack that a recipe calls or carries a value to, \
                     so nothing refusing it would stop is this plugin's to say",
                    reach.host
                ),
            });
        }
        if !named.insert(reach.host.as_str()) {
            found.push(Violation {
                location: format!("{at}.host"),
                message: format!(
                    "{} is named by an earlier [[reach]] too; what refusing a host stops is said \
                     once",
                    reach.host
                ),
            });
        }
        if reach.stops.trim().is_empty() {
            found.push(Violation {
                location: format!("{at}.stops"),
                message: "is blank; it is the line an operator reads beside the setting that \
                          refuses the host, saying what stops working"
                    .to_owned(),
            });
        }
    }
}

#[cfg(test)]
mod tests;
