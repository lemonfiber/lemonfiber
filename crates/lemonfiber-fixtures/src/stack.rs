//! The stack this repository carries, written where a test can change it.
//!
//! Its manifest is the stack the bundle declares, written apart as a root and a file
//! per service as a stack directory holds it. A test that wants its own copy writes
//! the files, and one that wants a service gone removes that service's file, its
//! `include` entry and every link that named it — a stack that drops a service drops
//! what reached it, and one that kept those links would be refused for naming a
//! service it no longer declares.

use std::path::Path;

use lemonfiber_manifest::assembly::{split, ROOT, SERVICES};
use toml::{Table, Value};

/// The carried stack's service `id` as its manifest pins it, or nothing where the
/// stack does not declare it.
#[must_use]
pub fn pinned(id: &str) -> Option<lemonfiber_manifest::Service> {
    lemonfiber_manifest::Manifest::from_toml(lemonfiber_bundled::STACK)
        .ok()?
        .services
        .into_iter()
        .find(|service| service.id == id)
}

/// The carried stack's manifest files written into `into`, which is made if it is not
/// there.
pub fn manifest_into(into: &Path) {
    let Ok((root, services)) = split(lemonfiber_bundled::STACK) else {
        return;
    };
    let _ = std::fs::create_dir_all(into.join(SERVICES));
    let _ = std::fs::write(into.join(ROOT), root);
    for (file, text) in services {
        let _ = std::fs::write(into.join(SERVICES).join(file), text);
    }
}

/// The carried stack's manifest written into `into` without `service`.
pub fn manifest_without(into: &Path, service: &str) {
    manifest_into(into);
    let _ = std::fs::remove_file(into.join(SERVICES).join(format!("{service}.toml")));
    let mut root: Table = std::fs::read_to_string(into.join(ROOT))
        .ok()
        .and_then(|text| toml::from_str(&text).ok())
        .unwrap_or_default();
    let entry = Value::String(format!("{SERVICES}/{service}.toml"));
    if let Some(Value::Array(include)) = root.get_mut("include") {
        include.retain(|listed| *listed != entry);
    }
    let named = Value::String(service.to_owned());
    if let Some(Value::Array(wiring)) = root.get_mut("wiring") {
        wiring.retain(|row| {
            row.as_table()
                .is_none_or(|row| !row.values().any(|value| *value == named))
        });
    }
    let _ = std::fs::write(into.join(ROOT), toml::to_string(&root).unwrap_or_default());
}

/// A manifest written as one text, laid out the way a stack is written: each
/// `[[service]]` moved into a file of its own and named by `include`, in order.
///
/// For tests that state a whole manifest in one string. A service with no `id` line
/// is filed by its position, and the refusal it earns is then about that file.
pub fn manifest_written(into: &Path, text: &str) {
    let _ = std::fs::create_dir_all(into.join(SERVICES));
    let mut root = String::new();
    let mut services: Vec<String> = Vec::new();
    let mut inside = false;
    for line in text.split_inclusive('\n') {
        let header = line.trim_start();
        if header.starts_with("[[service]]") {
            services.push(String::new());
            inside = true;
        } else if header.starts_with('[')
            && !header.starts_with("[service.")
            && !header.starts_with("[[service.")
        {
            inside = false;
        }
        match services.last_mut() {
            Some(service) if inside => service.push_str(line),
            _ => root.push_str(line),
        }
    }
    let mut include = Vec::new();
    for (at, service) in services.iter().enumerate() {
        let id = service
            .lines()
            .find_map(|line| {
                line.trim()
                    .strip_prefix("id")?
                    .trim()
                    .strip_prefix('=')?
                    .trim()
                    .strip_prefix('"')?
                    .split('"')
                    .next()
            })
            .map_or_else(|| format!("service-{at}"), str::to_owned);
        let _ = std::fs::write(into.join(SERVICES).join(format!("{id}.toml")), service);
        include.push(format!("\"{SERVICES}/{id}.toml\""));
    }
    let listed = if include.is_empty() {
        String::new()
    } else {
        format!("include = [{}]\n", include.join(", "))
    };
    let _ = std::fs::write(into.join(ROOT), format!("{listed}{root}"));
}

#[cfg(test)]
mod tests;
