//! The parts the bundle installs, kept as the stack they replace declares them until
//! each moves.

use toml::{Table, Value};

use crate::assembly::same_ask;
use crate::{Held, Stage};

/// The stack `assembled` with each part `held` installs kept as `frozen`, the stack the
/// bundle replaces, declares it: its one service and every wiring row it asks by.
///
/// # Errors
///
/// Either text where it does not parse, and each installed part `frozen` declares no
/// service for, named.
pub fn kept(assembled: &str, frozen: &str, held: &[Held]) -> Result<String, Vec<String>> {
    let mut stack: Table =
        toml::from_str(assembled).map_err(|refused| vec![refused.to_string()])?;
    let frozen: Table = toml::from_str(frozen).map_err(|refused| vec![refused.to_string()])?;
    let rows = |table: &Table, name: &str| {
        table
            .get(name)
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default()
    };
    let mut services = rows(&stack, "service");
    let mut wiring = rows(&stack, "wiring");
    let mut problems = Vec::new();
    for id in held
        .iter()
        .filter(|one| one.pin.stage == Some(Stage::Installed))
        .map(|one| one.pin.id.as_str())
    {
        let Some(service) = rows(&frozen, "service")
            .into_iter()
            .find(|service| service.get("id").and_then(Value::as_str) == Some(id))
        else {
            problems.push(format!(
                "{id}: the stack the bundle replaces declares no service by that id"
            ));
            continue;
        };
        services.push(service);
        for row in rows(&frozen, "wiring")
            .into_iter()
            .filter(|row| row.get("by").and_then(Value::as_str) == Some(id))
        {
            if !wiring.iter().any(|ours| same_ask(ours, &row)) {
                wiring.push(row);
            }
        }
    }
    if !problems.is_empty() {
        return Err(problems);
    }
    for (name, rows) in [("wiring", wiring), ("service", services)] {
        if !rows.is_empty() {
            stack.insert(name.into(), Value::Array(rows));
        }
    }
    toml::to_string(&stack).map_err(|unwritten| vec![unwritten.to_string()])
}

#[cfg(test)]
mod tests;
