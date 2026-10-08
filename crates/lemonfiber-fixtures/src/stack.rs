//! The stack this repository carries, written where a test can change it.
//!
//! Its manifest is a root and a file per service. A test that wants
//! its own copy copies the files, and one that wants a service gone removes that
//! service's file, its `include` entry and every link that named it — a stack that
//! drops a service drops what reached it, and one that kept those links would be
//! refused for naming a service it no longer declares.

use std::path::Path;

use lemonfiber_manifest::assembly::{ROOT, SERVICES};

/// The stack this repository carries.
pub const CARRIED: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../assets/media-stack");

/// The carried stack's manifest files written into `into`, which is made if it is not
/// there.
pub fn manifest_into(into: &Path) {
    let from = Path::new(CARRIED);
    let _ = std::fs::create_dir_all(into.join(SERVICES));
    let _ = std::fs::copy(from.join(ROOT), into.join(ROOT));
    for entry in std::fs::read_dir(from.join(SERVICES))
        .into_iter()
        .flatten()
        .flatten()
    {
        let _ = std::fs::copy(entry.path(), into.join(SERVICES).join(entry.file_name()));
    }
}

/// The carried stack's manifest written into `into` without `service`.
pub fn manifest_without(into: &Path, service: &str) {
    manifest_into(into);
    let _ = std::fs::remove_file(into.join(SERVICES).join(format!("{service}.toml")));
    let root = std::fs::read_to_string(into.join(ROOT)).unwrap_or_default();
    let entry = format!("\"{SERVICES}/{service}.toml\"");
    let named = format!("\"{service}\"");
    let listed: String = root
        .split_inclusive('\n')
        .filter(|line| !line.contains(&entry))
        .collect();
    let kept = listed
        .split("[[wiring]]")
        .filter(|block| !block.contains(&named))
        .collect::<Vec<_>>()
        .join("[[wiring]]");
    let _ = std::fs::write(into.join(ROOT), kept);
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
