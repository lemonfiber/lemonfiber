use lemonfiber_core::model::{
    Choosable, Owner, Reader, ReaderDefault, SecretFrom, SecretRead, SettingChange, SettingDefault,
    SettingMove, SettingRead, SettingsReport,
};

use super::Lines;

pub(crate) fn settings(report: &SettingsReport) -> Lines {
    match &report.change {
        Some(change) => {
            let mut lines = changed(change, report.rehearsed);
            lines.extend(super::overlay::unvetted(report.unvetted.as_ref()));
            lines.extend(super::moving::guards(&report.guards));
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
    let holds = match (&setting.value, &setting.default) {
        (Some(value), _) => format!("set to {value}"),
        (None, Some(default)) => format!("at its default, {default}"),
        (None, None) => "at its default".to_owned(),
    };
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
    defaulted(&reader.service, &reader.variable, reader.default.as_deref())
}

/// A service reading a setting into a variable, with its default where it declares one.
fn defaulted(service: &str, variable: &str, default: Option<&str>) -> String {
    match default {
        Some("") => format!("{service} as {variable}, empty by default"),
        Some(default) => format!("{service} as {variable}, {default} by default"),
        None => format!("{service} as {variable}"),
    }
}

fn changed(change: &SettingChange, rehearsed: bool) -> Lines {
    let mut lines = Lines::default();
    for (index, one) in change.settings.iter().enumerate() {
        let now = match &one.value {
            Some(value) => format!(
                "{} {} {value}{}.",
                one.setting,
                if change.applied {
                    "now holds"
                } else {
                    "would hold"
                },
                if one.default.value() == Some(value.as_str()) {
                    ", the same as its default"
                } else {
                    ""
                }
            ),
            None => cleared(one, change.applied),
        };
        if index == 0 {
            lines.put(now);
        } else {
            lines.spaced(now);
        }
        lines.put(format!(
            "  was          {}",
            one.was
                .clone()
                .unwrap_or_else(|| at_its_default(&one.default))
        ));
        if one.value.is_none() {
            if let SettingDefault::PerReader { readers } = &one.default {
                for reader in readers.iter().filter(|reader| reader.met) {
                    lines.put(format!("  {}", reader_default(reader)));
                }
            }
        }
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

/// The sentence a cleared setting is said in, naming what is written in its place.
fn cleared(one: &SettingMove, applied: bool) -> String {
    let is = if applied {
        "was cleared"
    } else {
        "would be cleared"
    };
    match (&one.default, one.default.said()) {
        (_, Some(said)) => format!(
            "{} {is}, so its default, {said}, is written again.",
            one.setting
        ),
        (SettingDefault::Nothing, None) => format!(
            "{} {is}, and nothing declares a default, so nothing is written for it.",
            one.setting
        ),
        (_, None) => format!(
            "{} {is}, so each reader's own default is written again:",
            one.setting
        ),
    }
}

/// What a setting held before the change where it held no operator value.
fn at_its_default(default: &SettingDefault) -> String {
    match (default, default.said()) {
        (_, Some(said)) => format!("at its default, {said}"),
        (SettingDefault::Nothing, None) => "at no value, as nothing declares a default".to_owned(),
        (_, None) => "at each reader's own default".to_owned(),
    }
}

/// One reader's own default, as a cleared setting lists it.
fn reader_default(reader: &ReaderDefault) -> String {
    defaulted(&reader.service, &reader.variable, reader.default.as_deref())
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
