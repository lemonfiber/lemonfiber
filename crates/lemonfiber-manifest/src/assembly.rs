//! A stack's manifest files, read as the one manifest they describe.
//!
//! A stack is described by a root, `stack.toml`, and a file per service in
//! `services/`, named by the root's `include` list. Every reader reads the one text
//! this joins them into, so no reader has to know the stack is written in pieces. The
//! rules about the pieces are checked here, once, before any reader sees the result.
//!
//! Each service file is read on its own before it is joined: its syntax, the one
//! service it holds and every name it declares. A fault is then reported against the
//! file it is in, at the line it is on there, rather than at a line of a text nobody
//! wrote. The root goes first in the joined text, so its own lines keep their numbers.

use std::collections::{BTreeMap, BTreeSet};
use std::ffi::OsStr;
use std::io::ErrorKind;
use std::path::Path;

use serde::Deserialize;
use toml::{Table, Value};

use crate::recognising::unrecognised;
use crate::{Failure, Manifest, Service, Violation};

/// The root manifest's file name, at the top of a stack directory.
pub const ROOT: &str = "stack.toml";

/// The directory each service's file is in, beside the root.
pub const SERVICES: &str = "services";

/// What a service file's name ends with.
const EXTENSION: &str = ".toml";

/// The root's list of service files.
const INCLUDE: &str = "include";

/// The one key a service file holds.
const SERVICE: &str = "service";

/// One service's file, `services/<id>.toml`: exactly one `[[service]]`, whose `id` is
/// the name the file is filed under, and nothing else.
#[derive(Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
#[schemars(rename = "StackServiceFile")]
struct ServiceFile {
    #[serde(rename = "service")]
    #[schemars(length(min = 1, max = 1))]
    _services: Vec<Service>,
}

/// The schema of a stack's root manifest, `stack.toml`: the manifest's own fields,
/// with its services in files of their own rather than in it.
#[must_use]
pub fn root_schema() -> schemars::Schema {
    let mut schema = schemars::schema_for!(Manifest);
    if let Some(properties) = schema
        .get_mut("properties")
        .and_then(serde_json::Value::as_object_mut)
    {
        properties.remove(SERVICE);
    }
    schema
}

/// The schema of one service's file, `services/<id>.toml`.
#[must_use]
pub fn service_schema() -> schemars::Schema {
    schemars::schema_for!(ServiceFile)
}

/// The manifest a root and its service files describe, as one text.
///
/// `services` is every file in `services/`, by file name, with its text. One whose
/// name does not end in `.toml` is not a service file and is left alone.
///
/// # Errors
///
/// [`Failure::Syntax`] where the root is not TOML. [`Failure::Assembly`] with every
/// rule the files break, each naming the entry or file. [`Failure::Unrecognised`]
/// where they keep the rules and a service file declares a name this build does not
/// know.
pub fn assemble(root: &str, services: &BTreeMap<String, String>) -> Result<String, Failure> {
    let table: Table = toml::from_str(root)?;
    let mut faults = Vec::new();
    declared_in_the_root(&table, &mut faults);
    let named = included(&table, &mut faults);

    let mut unknown = Vec::new();
    let mut joined = root.to_owned();
    for (entry, file) in &named {
        let Some(text) = services.get(file) else {
            faults.push(fault(&entry_at(entry), "names no file"));
            continue;
        };
        unknown.extend(read_alone(file, text, &mut faults));
        for part in ["\n# ", SERVICES, "/", file, "\n", text] {
            joined.push_str(part);
        }
    }

    let listed: BTreeSet<&String> = named.iter().map(|(_, file)| file).collect();
    let strays = services
        .keys()
        .filter(|file| file.ends_with(EXTENSION) && !listed.contains(file));
    for file in strays {
        faults.push(fault(
            &file_at(file),
            "is in services/ and no include entry names it",
        ));
    }

    if !faults.is_empty() {
        return Err(Failure::Assembly(faults));
    }
    if !unknown.is_empty() {
        return Err(Failure::Unrecognised(unknown));
    }
    Ok(joined)
}

/// The manifest of the stack directory at `stack`, read from disk and assembled.
///
/// A stack with no `services/` directory is a stack with no service files, which the
/// root's `include` then decides is right or wrong.
///
/// # Errors
///
/// [`Failure::Unreadable`] where the root or the directory cannot be read, and
/// whatever [`assemble`] refuses.
pub fn read(stack: &Path) -> Result<String, Failure> {
    let at = stack.join(ROOT);
    let root = std::fs::read_to_string(&at).map_err(|why| Failure::unreadable(&at, &why))?;
    assemble(&root, &service_files(&stack.join(SERVICES))?)
}

/// Every `.toml` file in `directory`, by name, with its text. Nothing else there is
/// read: a stray `.DS_Store` is not a service file, and need not be text.
fn service_files(directory: &Path) -> Result<BTreeMap<String, String>, Failure> {
    let entries = match std::fs::read_dir(directory) {
        Ok(entries) => entries,
        Err(why) if why.kind() == ErrorKind::NotFound => return Ok(BTreeMap::new()),
        Err(why) => return Err(Failure::unreadable(directory, &why)),
    };
    let mut files = BTreeMap::new();
    // An entry the directory cannot list is left out, and the `include` entry that
    // names it is then refused as naming no file, which says where to look.
    for path in entries.flatten().map(|entry| entry.path()) {
        let name = path.file_name().and_then(OsStr::to_str).unwrap_or_default();
        if !name.ends_with(EXTENSION) || !path.is_file() {
            continue;
        }
        let text =
            std::fs::read_to_string(&path).map_err(|why| Failure::unreadable(&path, &why))?;
        files.insert(name.to_owned(), text);
    }
    Ok(files)
}

/// Each service the root declares itself, which belongs in a file of its own.
fn declared_in_the_root(root: &Table, faults: &mut Vec<Violation>) {
    let Some(declared) = root.get(SERVICE) else {
        return;
    };
    let each = declared
        .as_array()
        .map_or_else(|| vec![declared], |all| all.iter().collect());
    for service in each {
        let message = service.get("id").and_then(Value::as_str).map_or_else(
            || "declares a [[service]], and each service is in a file of its own".to_owned(),
            |id| format!("declares service {id}, which belongs in {SERVICES}/{id}{EXTENSION}"),
        );
        faults.push(fault(ROOT, &message));
    }
}

/// Each `include` entry of the right form, the first time it appears, with the name
/// of the file it names inside `services/`.
fn included(root: &Table, faults: &mut Vec<Violation>) -> Vec<(String, String)> {
    let Some(include) = root.get(INCLUDE) else {
        return Vec::new();
    };
    let Some(entries) = include.as_array() else {
        faults.push(fault(ROOT, "include is a list of the service files"));
        return Vec::new();
    };
    let mut seen = BTreeSet::new();
    let mut named = Vec::new();
    for entry in entries {
        let Some(entry) = entry.as_str() else {
            faults.push(fault(
                ROOT,
                &format!("include holds {entry}, which is not a path"),
            ));
            continue;
        };
        let Some(file) = file_named(entry) else {
            faults.push(fault(
                &entry_at(entry),
                &format!("is not of the form {SERVICES}/<id>{EXTENSION}"),
            ));
            continue;
        };
        if !seen.insert(entry) {
            faults.push(fault(&entry_at(entry), "appears more than once"));
            continue;
        }
        named.push((entry.to_owned(), file.to_owned()));
    }
    named
}

/// The file inside `services/` an entry names, where it is of the form
/// `services/<id>.toml`.
fn file_named(entry: &str) -> Option<&str> {
    let file = entry.strip_prefix(SERVICES)?.strip_prefix('/')?;
    let id = file.strip_suffix(EXTENSION)?;
    let plain = !id.is_empty() && !id.starts_with('.') && !id.contains(['/', '\\']) && id != "..";
    plain.then_some(file)
}

/// One service file read on its own: every rule it breaks into `faults`, and the
/// names it declares that this build does not know.
fn read_alone(file: &str, text: &str, faults: &mut Vec<Violation>) -> Vec<Violation> {
    let at = file_at(file);
    let table: Table = match toml::from_str(text) {
        Ok(table) => table,
        Err(why) => {
            faults.push(fault(&at, &said(text, &why)));
            return Vec::new();
        }
    };
    let before = faults.len();
    for key in table.keys().filter(|key| *key != SERVICE) {
        faults.push(fault(
            &at,
            &format!("holds {key}, and a service file holds only its [[service]]"),
        ));
    }
    let services = table
        .get(SERVICE)
        .and_then(Value::as_array)
        .map_or(&[][..], Vec::as_slice);
    let id = file.strip_suffix(EXTENSION).unwrap_or(file);
    match services {
        [] => faults.push(fault(&at, "holds no [[service]]")),
        [one] => {
            let declared = one.get("id").and_then(Value::as_str);
            if declared != Some(id) {
                faults.push(fault(
                    &at,
                    &format!(
                        "holds service {}, and its name says {id}",
                        declared.unwrap_or("with no id")
                    ),
                ));
            }
        }
        many => faults.push(fault(
            &at,
            &format!(
                "holds {} [[service]], and a service file holds one",
                many.len()
            ),
        )),
    }
    if faults.len() > before {
        return Vec::new();
    }
    let unknown = unrecognised(text);
    if unknown.is_empty() {
        if let Err(why) = toml::from_str::<ServiceFile>(text) {
            faults.push(fault(&at, &said(text, &why)));
        }
    }
    unknown
}

/// What the parser said about a file, at the line of that file it is on.
fn said(text: &str, why: &toml::de::Error) -> String {
    let message = why.message().trim_end();
    why.span().map_or_else(
        || message.to_owned(),
        |span| {
            let line = text
                .get(..span.start)
                .map_or(0, |before| before.matches('\n').count())
                + 1;
            format!("line {line}: {message}")
        },
    )
}

/// An `include` entry, as a fault names it.
fn entry_at(entry: &str) -> String {
    format!("include entry {entry}")
}

/// A service file, as a fault names it.
fn file_at(file: &str) -> String {
    format!("{SERVICES}/{file}")
}

/// One fault, where it is and what it is.
fn fault(location: &str, message: &str) -> Violation {
    Violation {
        location: location.to_owned(),
        message: message.to_owned(),
    }
}

#[cfg(test)]
mod tests;
