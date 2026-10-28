use lemonfiber_core::model::{
    Choosable, Owner, Reader, SecretFrom, SecretRead, SettingChange, SettingRead, SettingsReport,
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
    lines.extend(super::waiting::waiting(&report.waiting));
    lines.spaced("Change one with lemonfiber settings set <setting>=<value>; a secret is replaced with lemonfiber credentials --rotate.");
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
        lines.put(format!(
            "    reads     {}{}",
            read_as(reader),
            written_where(reader)
        ));
    }
    if let Some(choices) = &setting.choices {
        let listed: Vec<String> = choices.iter().map(choosable).collect();
        lines.put(format!("    choices   {}", listed.join(", ")));
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
                SecretFrom::Minted => "made by lemonfiber at install",
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

/// Where a reference is written only beside other settings' values: what it needs, and
/// whether that holds now.
fn written_where(reader: &Reader) -> String {
    if reader.needs.is_empty() {
        return String::new();
    }
    format!(
        ", written where {}{}",
        needing(&reader.needs),
        if reader.met {
            ""
        } else {
            ", which is not so now"
        }
    )
}

/// One value a setting accepts, with what it needs where it is a choice only beside other
/// settings' values.
fn choosable(choice: &Choosable) -> String {
    if choice.needs.is_empty() {
        return choice.value.clone();
    }
    format!(
        "{} (where {}{})",
        choice.value,
        needing(&choice.needs),
        if choice.open { "" } else { "; not now" }
    )
}

/// What `needs` asks of the other settings, as a clause.
fn needing(needs: &std::collections::BTreeMap<String, Vec<String>>) -> String {
    needs
        .iter()
        .map(|(setting, values)| match values.as_slice() {
            [one] => format!("{setting} is {one}"),
            _ => format!("{setting} is one of {}", values.join(", ")),
        })
        .collect::<Vec<_>>()
        .join(" and ")
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
    for (index, one) in change.settings.iter().enumerate() {
        let now = match (&one.value, change.applied) {
            (Some(value), true) => format!("{} now holds {value}.", one.setting),
            (Some(value), false) => format!("{} would hold {value}.", one.setting),
            (None, true) => format!(
                "{} was cleared, so its default is written again.",
                one.setting
            ),
            (None, false) => format!(
                "{} would be cleared, so its default is written again.",
                one.setting
            ),
        };
        if index == 0 {
            lines.put(now);
        } else {
            lines.spaced(now);
        }
        lines.put(format!(
            "  was          {}",
            one.was.as_deref().unwrap_or("at its default")
        ));
    }
    let regenerates = format!(
        "  regenerates  {}",
        listed_or(&change.regenerates, "no entry")
    );
    let unread = if change.settings.len() > 1 {
        "nothing, as nothing running reads them"
    } else {
        "nothing, as nothing running reads it"
    };
    if change.settings.len() > 1 {
        lines.spaced(regenerates);
    } else {
        lines.put(regenerates);
    }
    lines.put(format!(
        "  restarts     {}",
        listed_or(&change.restarts, unread)
    ));
    if !change.starts.is_empty() {
        lines.put(format!("  starts       {}", change.starts.join(", ")));
    }
    for consequence in change
        .settings
        .iter()
        .filter_map(|one| one.consequence.as_ref())
    {
        lines.spaced(consequence.clone());
    }
    lines.extend(super::waiting::waiting(&change.waiting));
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
    let named: Vec<String> = change
        .settings
        .iter()
        .map(|one| match &one.value {
            Some(value) => super::wiring::quoted(&format!("{}={value}", one.setting)),
            None => one.setting.clone(),
        })
        .collect();
    let cleared = change.settings.iter().all(|one| one.value.is_none());
    let verb = format!(
        "{} {}",
        if cleared { "clear" } else { "set" },
        named.join(" ")
    );
    lines.put(format!(
        "  lemonfiber settings {verb} --offer {}",
        change.agreement
    ));
    lines
}

#[cfg(test)]
mod tests;
