//! The stack a schema 2 bundle assembles: its own tables, and each assembled part's one
//! service written as a stack service.

use toml::{Table, Value};

use crate::{Held, Stage};

/// The schema version of the stack manifest the assembly writes.
const STACK_SCHEMA: i64 = 1;

/// The fields of a part's `[plugin]` table its stack service carries, by the name the
/// stack gives each.
const FROM_PLUGIN: [(&str, &str); 4] = [
    ("description", "describes"),
    ("without_it", "without_it"),
    ("upstream", "upstream"),
    ("license", "license"),
];

/// The fields of a part's `[[wiring]]` about its service that its stack service carries
/// in its `dashboard` table, by the name the table gives each.
const DASHBOARD: [(&str, &str); 4] = [
    ("dashboard_group", "group"),
    ("dashboard_description", "description"),
    ("dashboard_icon", "icon"),
    ("dashboard_hidden", "hidden"),
];

/// The tables of a part's manifest its stack service carries as written: its claims, and
/// the secrets it holds in its own id's namespace.
const FROM_MANIFEST: [&str; 2] = ["claim", "secret"];

/// The field of a part's service that names the stack id it takes the place of, which a
/// stack service does not carry.
const REPLACES: &str = "replaces";

/// The stack tables a bundle declares, carried as written.
pub(crate) const STACK_TABLES: [&str; 3] = ["profile", "form", "removed"];

/// The stack manifest the bundle file `text` assembles from the copies in `held`, at
/// `version`, or every reason it cannot.
///
/// # Errors
///
/// The bundle file where it does not parse, and each assembled part whose copy does not
/// parse, declares other than one service, or whose pin names no profile or last
/// release, named with what is wrong.
pub fn assembled(text: &str, held: &[Held], version: &str) -> Result<String, Vec<String>> {
    let bundle: Table = toml::from_str(text).map_err(|refused| vec![refused.to_string()])?;
    let rows = |name: &str| {
        bundle
            .get(name)
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default()
    };
    let mut wiring = rows("wiring");
    let mut services = Vec::new();
    let mut problems = Vec::new();
    for one in held
        .iter()
        .filter(|one| one.pin.stage == Some(Stage::Assembled))
    {
        match service(one) {
            Ok((written, asks)) => {
                for ask in asks {
                    if !wiring.iter().any(|row| same_ask(row, &ask)) {
                        wiring.push(ask);
                    }
                }
                services.push(Value::Table(written));
            }
            Err(said) => problems.push(format!("{}: {said}", one.pin.id)),
        }
    }
    if !problems.is_empty() {
        return Err(problems);
    }
    let mut stack = Table::new();
    stack.insert("schema_version".into(), Value::Integer(STACK_SCHEMA));
    stack.insert("stack_version".into(), Value::String(version.to_owned()));
    stack.insert("min_cli_version".into(), Value::String(version.to_owned()));
    let tables = STACK_TABLES
        .iter()
        .map(|name| (*name, rows(name)))
        .chain([("wiring", wiring), ("service", services)]);
    for (name, rows) in tables {
        if !rows.is_empty() {
            stack.insert(name.into(), Value::Array(rows));
        }
    }
    Ok(toml::to_string(&stack).unwrap_or_default())
}

/// One part's service as a stack service, with the asks it makes as wiring rows.
fn service(one: &Held) -> Result<(Table, Vec<Value>), String> {
    let manifest: Table = toml::from_str(&one.copy).map_err(|refused| refused.to_string())?;
    let mut declared = services(&manifest);
    if declared.len() != 1 {
        return Err(format!("declares {} services, not one", declared.len()));
    }
    let Some(Value::Table(written)) = declared.pop() else {
        return Err("its service is not a table".into());
    };
    let (mut written, asks) = stacked(one, &manifest, written)?;
    for table in FROM_MANIFEST {
        if let Some(rows) = manifest.get(table) {
            written.insert(table.into(), rows.clone());
        }
    }
    Ok((written, asks))
}

/// Every service the part's manifest declares.
pub(crate) fn services(manifest: &Table) -> Vec<Value> {
    manifest
        .get("service")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default()
}

/// The service `written` of the part `one`, whose manifest is `manifest`, as a stack
/// service: without the id it takes the place of, with its profile and last release from
/// its pin, what the manifest's `[plugin]` table says of it and its dashboard; with each
/// ask the manifest makes for it as a wiring row.
pub(crate) fn stacked(
    one: &Held,
    manifest: &Table,
    mut written: Table,
) -> Result<(Table, Vec<Value>), String> {
    written.remove(REPLACES);
    let id = written
        .get("id")
        .and_then(Value::as_str)
        .ok_or("its service names no id")?
        .to_owned();
    for (field, value) in [
        ("profile", one.pin.profile.as_deref()),
        ("last_release", one.pin.last_release.as_deref()),
    ] {
        let value = value.ok_or_else(|| format!("its pin names no {field}"))?;
        written.insert(field.into(), Value::String(value.to_owned()));
    }
    let plugin = manifest.get("plugin").and_then(Value::as_table);
    for (from, to) in FROM_PLUGIN {
        if let Some(value) = plugin.and_then(|plugin| plugin.get(from)) {
            written.insert(to.into(), value.clone());
        }
    }
    if let Some(dashboard) = dashboard(manifest, &id) {
        written.insert("dashboard".into(), Value::Table(dashboard));
    }
    let asks = manifest
        .get("ask")
        .and_then(Value::as_array)
        .map(|asks| {
            asks.iter()
                .filter(|ask| {
                    ask.get("service")
                        .and_then(Value::as_str)
                        .is_none_or(|named| named == id)
                })
                .filter_map(|ask| asked(&id, ask))
                .collect()
        })
        .unwrap_or_default();
    Ok((written, asks))
}

/// The `dashboard` table of the service `id`, from the part's `[[wiring]]` about it, where
/// that names any of it.
fn dashboard(manifest: &Table, id: &str) -> Option<Table> {
    let wiring = manifest
        .get("wiring")
        .and_then(Value::as_array)?
        .iter()
        .find(|row| {
            row.get("service")
                .and_then(Value::as_str)
                .is_none_or(|named| named == id)
        })?;
    let table: Table = DASHBOARD
        .iter()
        .filter_map(|(from, to)| Some(((*to).to_owned(), wiring.get(*from)?.clone())))
        .collect();
    (!table.is_empty()).then_some(table)
}

/// One `[[ask]]` of the service `by`, as the stack's wiring row.
fn asked(by: &str, ask: &Value) -> Option<Value> {
    let capability = ask.get("capability")?.as_str()?;
    let mut row = Table::new();
    row.insert("by".into(), Value::String(by.to_owned()));
    row.insert("asks".into(), Value::String(capability.to_owned()));
    if ask.get("each").and_then(Value::as_bool) == Some(true) {
        row.insert("each".into(), Value::Boolean(true));
    }
    Some(Value::Table(row))
}

/// Whether `row` is the bundle's own row for the ask `ask` makes: the same asker and
/// capability, which the bundle settles with a choice of filler.
pub(crate) fn same_ask(row: &Value, ask: &Value) -> bool {
    ["by", "asks"]
        .iter()
        .all(|field| row.get(field).is_some() && row.get(field) == ask.get(field))
}

#[cfg(test)]
mod tests;
