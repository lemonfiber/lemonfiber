//! The parts the bundle installs, kept in the stack as their own copies declare the
//! service each takes the place of, until each moves.

use toml::{Table, Value};

use crate::assembly::{services, stacked};
use crate::{Held, Stage};

/// The fields of an installed part's service its stack service does not carry: the API its
/// adapter speaks to it in, which only the plugin's adapter reads, and the contracts it
/// speaks itself and the port it speaks them on, which only the plugin's install reaches.
const PLUGIN_ONLY: [&str; 3] = ["native", "speaks", "contract"];

/// The stack `assembled` with each part `held` installs kept as its copy declares the
/// service of its id: that one service, providing what any service of the part provides,
/// with each secret of its environment that only the operator holds declared as the copy
/// declares it, so it reads the value the plugin reads once it moves, and without the
/// environment values a secret lemonfiber mints at install fills, since nothing mints
/// one for it before it moves. A choice of filler
/// naming any service of an installed part's copy names that one service, which stands
/// in for every service of the part until it moves.
///
/// No ask of the copy is carried: a kept service only keeps its container running until
/// the part moves, nothing answers its asks before then, and after it the plugin's own
/// services make them.
///
/// # Errors
///
/// The text where it does not parse, and each installed part whose copy does not parse,
/// declares no service of its id, or whose pin names no profile or last release, named.
pub fn kept(assembled: &str, held: &[Held]) -> Result<String, Vec<String>> {
    let mut stack: Table =
        toml::from_str(assembled).map_err(|refused| vec![refused.to_string()])?;
    let rows = |table: &Table, name: &str| {
        table
            .get(name)
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default()
    };
    let mut services_kept = rows(&stack, "service");
    let mut wiring = rows(&stack, "wiring");
    let mut problems = Vec::new();
    for one in held
        .iter()
        .filter(|one| one.pin.stage == Some(Stage::Installed))
    {
        let id = one.pin.id.as_str();
        match standing_in(one) {
            Ok(service) => services_kept.push(Value::Table(service)),
            Err(said) => {
                problems.push(format!("{id}: {said}"));
                continue;
            }
        }
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
    }
    if !problems.is_empty() {
        return Err(problems);
    }
    for (name, rows) in [("wiring", wiring), ("service", services_kept)] {
        if !rows.is_empty() {
            stack.insert(name.into(), Value::Array(rows));
        }
    }
    Ok(toml::to_string(&stack).unwrap_or_default())
}

/// The stack service the installed part `one` stands in for until it moves.
fn standing_in(one: &Held) -> Result<Table, String> {
    let manifest: Table = toml::from_str(&one.copy).map_err(|refused| refused.to_string())?;
    let mut service = services(&manifest)
        .into_iter()
        .filter_map(|service| service.as_table().cloned())
        .find(|service| service.get("id").and_then(Value::as_str) == Some(one.pin.id.as_str()))
        .ok_or("its copy declares no service by that id")?;
    for field in PLUGIN_ONLY {
        service.remove(field);
    }
    let provided: Vec<Value> = services(&manifest)
        .iter()
        .filter_map(|one| one.get("provides").and_then(Value::as_array))
        .flatten()
        .fold(Vec::new(), |mut all, capability| {
            if !all.contains(capability) {
                all.push(capability.clone());
            }
            all
        });
    if !provided.is_empty() {
        service.insert("provides".into(), Value::Array(provided));
    }
    let declared: Vec<&Value> = manifest
        .get("secret")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter(|secret| !minted(secret))
        .collect();
    let held = |id: &str| {
        declared
            .iter()
            .any(|secret| secret.get("id").and_then(Value::as_str) == Some(id))
    };
    if let Some(Value::Table(env)) = service.get_mut("env") {
        env.retain(|_, value| value.get("secret").and_then(Value::as_str).is_none_or(held));
        if env.is_empty() {
            service.remove("env");
        }
    }
    let named: Vec<&str> = service
        .get("env")
        .and_then(Value::as_table)
        .into_iter()
        .flat_map(Table::values)
        .filter_map(|value| value.get("secret").and_then(Value::as_str))
        .collect();
    let secrets: Vec<Value> = declared
        .into_iter()
        .filter(|secret| {
            secret
                .get("id")
                .and_then(Value::as_str)
                .is_some_and(|id| named.contains(&id))
        })
        .cloned()
        .collect();
    if !secrets.is_empty() {
        service.insert("secret".into(), Value::Array(secrets));
    }
    stacked(one, &manifest, service).map(|(service, _)| service)
}

/// Whether the declared `secret` is one lemonfiber mints at install.
fn minted(secret: &Value) -> bool {
    secret
        .get("minted")
        .is_some_and(|minted| minted.as_bool() != Some(false))
}

/// The id of every service the manifest `copy` declares.
fn serviced(copy: &str) -> Vec<String> {
    toml::from_str::<Table>(copy)
        .ok()
        .map(|manifest| services(&manifest))
        .unwrap_or_default()
        .iter()
        .filter_map(|service| service.get("id").and_then(Value::as_str))
        .map(str::to_owned)
        .collect()
}

#[cfg(test)]
mod tests;
