//! A plugin's forms held to the ones the stack declares, before anything is written.

use crate::error::codes::plugin::UNDECLARED_FORM;
use crate::error::{Problem, Remedy, State};
use crate::plugin::Installed;

/// Refuse a plugin naming a form `manifest` does not declare, naming each.
///
/// # Errors
///
/// Where it names one, naming each and the forms the stack declares.
pub(crate) fn declared(
    would: &Installed,
    manifest: &lemonfiber_manifest::Manifest,
) -> Result<(), Box<Problem>> {
    let known: Vec<&str> = manifest.forms.iter().map(|form| form.id.as_str()).collect();
    let undeclared: Vec<&str> = would
        .forms
        .iter()
        .map(String::as_str)
        .filter(|form| !known.contains(form))
        .collect();
    if undeclared.is_empty() {
        return Ok(());
    }
    Err(Box::new(
        Problem::new(
            UNDECLARED_FORM,
            format!(
                "{} names forms this stack does not declare: {}",
                would.plugin,
                undeclared.join(", ")
            ),
            format!(
                "Nothing was written. A plugin's services join the forms it names, and this \
                 stack declares {}, so they would join nothing for those.",
                known.join(", ")
            ),
            Remedy::new(format!(
                "Install a version of {} whose forms are among the ones this stack declares",
                would.plugin
            ))
            .with_detail("lemonfiber forms"),
        )
        .in_state(State::Guided),
    ))
}

#[cfg(test)]
mod tests;
