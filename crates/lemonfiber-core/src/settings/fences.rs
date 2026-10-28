use crate::error::codes::setting::{A_SECRET, NO_SUCH_SETTING, TAKEN_ELSEWHERE, UNWRITABLE_VALUE};
use crate::error::{Problem, Remedy};

use super::{legacy, own};

pub(crate) fn secret(name: &str) -> Problem {
    Problem::new(
        A_SECRET,
        format!("{name} is a secret, which is not written as a setting"),
        "Nothing was changed. A secret is entered where it is first needed and replaced at \
         the terminal, never repeated back or written here.",
        Remedy::new("Replace it at the terminal")
            .with_detail(format!("lemonfiber credentials --rotate {name}")),
    )
}

pub(crate) fn settings_takes(name: &str, read: bool) -> Result<(), Box<Problem>> {
    if let Some(row) = legacy::keyed(name) {
        if row.secret {
            return Err(Box::new(secret(row.name)));
        }
        return Err(Box::new(
            unknown(name).or_try(
                Remedy::new(format!("{name} is held for the setting {}", row.name))
                    .with_detail(format!("lemonfiber settings set {}=<value>", row.name)),
            ),
        ));
    }
    if legacy::named(name).is_some_and(|row| row.secret) {
        return Err(Box::new(secret(name)));
    }
    if own::is_configs(name) {
        return Err(Box::new(taken(name, "lemonfiber config set")));
    }
    if read || own::is_core(name) {
        return Ok(());
    }
    Err(Box::new(unknown(name)))
}

pub(crate) fn config_takes(key: &str, writing: bool) -> Result<(), Box<Problem>> {
    if own::is_core(key) {
        return Err(Box::new(taken(key, "lemonfiber settings set")));
    }
    match legacy::keyed(key) {
        Some(row) if writing && !row.secret => {
            Err(Box::new(taken(row.name, "lemonfiber settings set")))
        }
        _ => Ok(()),
    }
}

pub(crate) fn valued(name: &str, value: &str) -> Result<(), Box<Problem>> {
    let why = if value.chars().any(char::is_control) {
        Some(format!(
            "{name} is written as one line, so its value may hold no line break or other \
             control character"
        ))
    } else if value.is_empty() && (legacy::setting_key(name).is_some() || own::is_core(name)) {
        Some(format!(
            "{name} cannot hold an empty value; lemonfiber settings clear {name} takes the \
             value away"
        ))
    } else {
        own::refusal(name, value)
    };
    why.map_or(Ok(()), |why| {
        Err(Box::new(Problem::new(
            UNWRITABLE_VALUE,
            format!("That value cannot be written as {name}"),
            format!("{why}. Nothing was changed."),
            Remedy::new("Give the value in that form"),
        )))
    })
}

fn taken(name: &str, command: &str) -> Problem {
    Problem::new(
        TAKEN_ELSEWHERE,
        format!("{name} is written with {command}"),
        "Nothing was changed. One name has one writer, and this one is the other command's.",
        Remedy::new(format!("Run {command}")).with_detail(format!("{command} {name} <value>")),
    )
}

fn unknown(name: &str) -> Problem {
    Problem::new(
        NO_SUCH_SETTING,
        format!("nothing in this stack reads a setting called {name}"),
        "Nothing was changed. A setting is written only where a service reads it, or where \
         it is one of the core's own.",
        Remedy::new("List the settings").with_detail("lemonfiber settings"),
    )
}
