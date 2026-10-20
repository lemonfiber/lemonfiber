//! The parts the bundle installs, kept as the stack they replace declares them until
//! each moves.

use toml::{Table, Value};

use crate::assembly::same_ask;
use crate::{Held, Stage};

/// The stack `assembled` with each part `held` installs kept as `frozen`, the stack the
/// bundle replaces, declares it: its one service and every wiring row it asks by. A choice
/// of filler naming any service of an installed part's copy names that one service, which
/// stands in for every service of the part until it moves.
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
    for one in held
        .iter()
        .filter(|one| one.pin.stage == Some(Stage::Installed))
    {
        let id = one.pin.id.as_str();
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
        for named in serviced(&one.copy) {
            for row in &mut wiring {
                if let Some(Value::String(filler)) =
                    row.as_table_mut().and_then(|row| row.get_mut("filled_by"))
                {
                    if *filler == named {
                        id.clone_into(filler);
                    }
                }
            }
        }
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
    Ok(toml::to_string(&stack).unwrap_or_default())
}

/// The id of every service the manifest `copy` declares.
fn serviced(copy: &str) -> Vec<String> {
    toml::from_str::<Table>(copy)
        .ok()
        .and_then(|manifest| manifest.get("service").and_then(Value::as_array).cloned())
        .unwrap_or_default()
        .iter()
        .filter_map(|service| service.get("id").and_then(Value::as_str))
        .map(str::to_owned)
        .collect()
}

#[cfg(test)]
mod tests;
