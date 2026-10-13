use lemonfiber_core::model::{
    Owner, Reader, SecretFrom, SecretRead, SettingChange, SettingRead, SettingsReport,
};

use super::Lines;

pub(crate) fn settings(report: &SettingsReport) -> Lines {
    match &report.change {
        Some(change) => {
            let mut lines = changed(change, report.rehearsed);
            lines.extend(super::overlay::unvetted(report.unvetted.as_ref()));
            lines
        }
        None => listed(report),
    }
}

fn listed(report: &SettingsReport) -> Lines {
    let mut lines = Lines::default();
    lines.put("Settings the services read:");
    for setting in &report.settings {
        lines.extend(setting_lines(setting));
    }
    lines.spaced("Secrets the services read, never shown:");
    if report.secrets.is_empty() {
        lines.put("  No service reads a secret.");
    }
    for secret in &report.secrets {
        lines.extend(secret_lines(secret));
    }
    lines.spaced("Change one with lemonfiber settings set <setting> <value>; a secret is replaced with lemonfiber credentials --rotate.");
    lines
}

fn setting_lines(setting: &SettingRead) -> Lines {
    let mut lines = Lines::default();
    let holds = setting.value.as_deref().map_or_else(
        || "at its default".to_owned(),
        |value| format!("set to {value}"),
    );
    lines.spaced(format!(
        "  {}  {holds}  ({})",
        setting.name,
        whose(&setting.whose)
    ));
    if setting.readers.is_empty() {
        lines.put("    reads     nothing in the stack; the core reads it");
    }
    for reader in &setting.readers {
        lines.put(format!("    reads     {}", read_as(reader)));
    }
    if let Some(key) = &setting.environment {
        lines.put(format!("    held      in the environment file as {key}"));
    }
    lines
}

fn secret_lines(secret: &SecretRead) -> Lines {
    let mut lines = Lines::default();
    let held = if secret.held { "held" } else { "not held" };
    let from = secret.from.map_or(String::new(), |from| {
        format!(
            ", {}",
            match from {
                SecretFrom::Operator => "entered by you",
                SecretFrom::Recipe => "captured by a recipe",
                SecretFrom::Granted => "another service's credential, granted",
            }
        )
    });
    lines.spaced(format!(
        "  {}  {held}{from}  ({})",
        secret.name,
        whose(&secret.whose)
    ));
    for reader in &secret.readers {
        lines.put(format!("    reads     {}", read_as(reader)));
    }
    lines
}

fn whose(whose: &Owner) -> String {
    match whose {
        Owner::Core => "the core's".to_owned(),
        Owner::Namespace { namespace } => format!("{namespace}'s"),
    }
}

fn read_as(reader: &Reader) -> String {
    match &reader.default {
        Some(default) if default.is_empty() => format!(
            "{} as {}, empty by default",
            reader.service, reader.variable
        ),
        Some(default) => format!(
            "{} as {}, {default} by default",
            reader.service, reader.variable
        ),
        None => format!("{} as {}", reader.service, reader.variable),
    }
}

fn changed(change: &SettingChange, rehearsed: bool) -> Lines {
    let mut lines = Lines::default();
    let now = match (&change.value, change.applied) {
        (Some(value), true) => format!("{} now holds {value}.", change.setting),
        (Some(value), false) => format!("{} would hold {value}.", change.setting),
        (None, true) => format!(
            "{} was cleared, so its default is written again.",
            change.setting
        ),
        (None, false) => format!(
            "{} would be cleared, so its default is written again.",
            change.setting
        ),
    };
    lines.put(now);
    lines.put(format!(
        "  was          {}",
        change.was.as_deref().unwrap_or("at its default")
    ));
    lines.put(format!(
        "  regenerates  {}",
        listed_or(&change.regenerates, "no entry")
    ));
    lines.put(format!(
        "  restarts     {}",
        listed_or(&change.restarts, "nothing, as nothing running reads it")
    ));
    if let Some(consequence) = &change.consequence {
        lines.spaced(consequence.clone());
    }
    if let Some(why) = &change.unrestarted {
        lines.spaced(format!(
            "The change is made, and what reads it was not restarted: {why}"
        ));
    }
    if !change.applied {
        lines.extend(answering(change, rehearsed));
    }
    lines
}

fn listed_or(names: &[String], none: &str) -> String {
    if names.is_empty() {
        none.to_owned()
    } else {
        names.join(", ")
    }
}

fn answering(change: &SettingChange, rehearsed: bool) -> Lines {
    let mut lines = Lines::default();
    lines.spaced(if rehearsed {
        "Nothing was written, because this was a rehearsal. To make the change, answer \
         this offer by name:"
    } else {
        "Nothing has been changed. To make the change, answer this offer by name:"
    });
    let verb = match &change.value {
        Some(value) => format!("set {} {}", change.setting, super::wiring::quoted(value)),
        None => format!("clear {}", change.setting),
    };
    lines.put(format!(
        "  lemonfiber settings {verb} --offer {}",
        change.agreement
    ));
    lines
}

#[cfg(test)]
mod tests;
