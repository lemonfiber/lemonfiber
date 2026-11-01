use std::collections::BTreeMap;

use crate::app::plugins::writing;
use crate::app::Ctx;
use crate::config::paths::SETTINGS;
use crate::device_keys::run::apps_changed;
use crate::error::codes::setting::{ALREADY_HOLDS, SETTINGS_MOVED};
use crate::error::{Diagnose, Problem, Remedy, State};
use crate::journal::{Change, Kind};
use crate::model::{SettingChange, SettingMove, SettingsReport, Waiting};

use super::readers::{self, Readers};
use super::recorded::Recorded;
use super::{fences, legacy, restarting, Resolved};

pub(super) const REFUSING: writing::Refusing = writing::Refusing {
    moved: SETTINGS_MOVED,
    unwritten: crate::error::codes::config::CONFIG_NOT_WRITTEN,
};

const OFFERED: [&str; 5] = [
    "the change itself",
    "what it holds now",
    "the entries it regenerates",
    "the services it restarts",
    "the services it starts",
];

pub(super) struct Asked<'a> {
    pub(super) changing: BTreeMap<&'a str, Option<&'a str>>,
    pub(super) agreement: Option<&'a str>,
}

pub(super) async fn changed(ctx: &Ctx, asked: &Asked<'_>) -> Result<SettingsReport, Box<Problem>> {
    let manifest = ctx
        .stack
        .checked_manifest(ctx.today())
        .map_err(|err| Box::new(err.problem()))?;
    let register = crate::app::plugins::read(ctx)?;
    let installed = register.installed();
    let recorded = Recorded::of(ctx);
    let held = |name: &str| recorded.value(name).map(str::to_owned);
    let readers = readers::readers(&manifest, installed, &held);
    for (setting, value) in &asked.changing {
        named(ctx, setting, &readers)?;
        if let Some(value) = value {
            fences::valued(setting, value)?;
        }
    }
    let changing: BTreeMap<&str, Option<&str>> = asked
        .changing
        .iter()
        .filter(|(setting, value)| recorded.value(setting) != **value)
        .map(|(setting, value)| (*setting, *value))
        .collect();
    if changing.is_empty() {
        return Err(Box::new(already(&asked.changing)));
    }
    let outside = super::choosing::outside(&manifest, installed, &changing, &held);
    if !outside.is_empty() {
        return Err(Box::new(super::choosing::refused(&outside)));
    }
    let settings: Vec<SettingMove> = changing
        .iter()
        .map(|(setting, value)| SettingMove {
            setting: (*setting).to_owned(),
            was: held(setting),
            value: value.map(str::to_owned),
            consequence: consequence(ctx, setting, *value),
        })
        .collect();
    let regenerates = readers::services(
        &changing
            .keys()
            .filter_map(|setting| readers.settings.get(*setting))
            .flatten()
            .cloned()
            .collect::<Vec<_>>(),
    );
    let after = Resolved::of(ctx, Some(&manifest), installed).changed(&changing);
    let overwrites = writing::regenerated(
        ctx,
        &manifest,
        &register,
        &crate::app::targets::chosen_fillers(ctx),
        &after,
    )?;
    let restarts = restarting::running(ctx, &regenerates).await;
    let waiting = super::run::waiting_after(ctx, &manifest, installed, &changing);
    let starts = released(&super::run::waiting(ctx, &manifest, installed), &waiting);
    let standing = offer(&settings, &regenerates, &restarts, &starts);
    let mut change = SettingChange {
        settings,
        regenerates,
        restarts,
        starts,
        applied: false,
        agreement: standing.clone(),
        unrestarted: None,
        waiting,
    };
    let Some(answered) = asked.agreement else {
        return Ok(super::listing::report(
            ctx,
            &manifest,
            installed,
            Some(change),
        ));
    };
    let moved = crate::agreement::differs(answered, &standing, &OFFERED);
    if !moved.is_empty() {
        return Err(Box::new(offer_moved(&changing, &moved, &standing)));
    }
    if ctx.dry_run {
        return Ok(super::listing::report(
            ctx,
            &manifest,
            installed,
            Some(change),
        ));
    }
    written(ctx, &mut change, &changing, &overwrites).await?;
    Ok(super::listing::report(
        ctx,
        &manifest,
        installed,
        Some(change),
    ))
}

/// Write `change`: journalled, recorded, the entries it regenerates overwritten, and what
/// reads it restarted.
async fn written(
    ctx: &Ctx,
    change: &mut SettingChange,
    changing: &BTreeMap<&str, Option<&str>>,
    overwrites: &[writing::Overwrite],
) -> Result<(), Box<Problem>> {
    let (Some(env), Some(paths)) = (
        ctx.settings.env_file.as_deref(),
        crate::app::targets::layout(ctx),
    ) else {
        return Err(Box::new(crate::config::store::Failure::Nowhere.problem()));
    };
    writing::unmoved(overwrites, REFUSING)?;
    let stamp = ctx.stamp();
    let mut journal: Vec<Change> = change
        .settings
        .iter()
        .map(|one| journalled(one, &stamp))
        .collect();
    journal.extend(
        overwrites
            .iter()
            .map(|overwrite| overwrite.journalled(super::OPERATION, &stamp)),
    );
    crate::app::recover::journalled(&paths.journal(), &journal)
        .map_err(|failure| Box::new(failure.problem()))?;
    super::store::recorded(&env.with_file_name(SETTINGS), changing)
        .map_err(|failure| Box::new(failure.problem()))?;
    writing::overwritten(ctx, overwrites, REFUSING)?;
    let apps = change
        .settings
        .iter()
        .find(|one| one.setting == super::own::DEVICE_KEY_APPS);
    if let Some(apps) = apps {
        apps_changed(ctx, apps.was.as_deref(), apps.value.as_deref()).await?;
    }
    if changing.contains_key(super::own::DASHBOARD_SEARCH) {
        let _ = crate::seed::run::reshow(ctx).await;
    }
    if changing.contains_key(super::own::DOMAIN) {
        let _ = crate::seed::run::refront(ctx).await;
    }
    change.applied = true;
    change.unrestarted = restarting::restarted(
        ctx,
        &[change.restarts.clone(), change.starts.clone()].concat(),
    )
    .await;
    Ok(())
}

fn named(ctx: &Ctx, setting: &str, readers: &Readers) -> Result<(), Box<Problem>> {
    let held = super::secrets_at(ctx)
        .and_then(|at| super::Secrets::at(&at).ok())
        .is_some_and(|kept| kept.value(setting).is_some());
    if held || readers.secrets.contains_key(setting) {
        return Err(Box::new(fences::secret(setting)));
    }
    fences::settings_takes(setting, readers.settings.contains_key(setting))
}

fn journalled(one: &SettingMove, stamp: &str) -> Change {
    Change {
        at: stamp.to_owned(),
        operation: super::OPERATION.to_owned(),
        target: one.setting.clone(),
        kind: Kind::Setting {
            name: one.setting.clone(),
            previous: one.was.clone(),
            current: one.value.clone(),
            filed: None,
        },
    }
}

fn consequence(ctx: &Ctx, setting: &str, value: Option<&str>) -> Option<String> {
    let key = legacy::setting_key(setting)?;
    let held = ctx
        .settings
        .env_file
        .as_deref()
        .and_then(|env| {
            crate::config::store::read(env)
                .ok()
                .map(|file| legacy::overlaid(env, file))
        })
        .unwrap_or_default();
    crate::app::configuring::consequence(ctx, &held, key, value.unwrap_or_default())
}

/// Every service `before` holds back that `after` no longer does.
fn released(before: &[Waiting], after: &[Waiting]) -> Vec<String> {
    before
        .iter()
        .filter(|held| !after.iter().any(|still| still.service == held.service))
        .map(|held| held.service.clone())
        .collect()
}

fn offer(
    moves: &[SettingMove],
    regenerates: &[String],
    restarts: &[String],
    starts: &[String],
) -> String {
    let asked: Vec<&str> = moves
        .iter()
        .flat_map(|one| {
            [
                one.setting.as_str(),
                one.value.as_deref().unwrap_or_default(),
            ]
        })
        .collect();
    let was: Vec<&str> = moves
        .iter()
        .map(|one| one.was.as_deref().unwrap_or_default())
        .collect();
    let regenerates: Vec<&str> = regenerates.iter().map(String::as_str).collect();
    let restarts: Vec<&str> = restarts.iter().map(String::as_str).collect();
    let starts: Vec<&str> = starts.iter().map(String::as_str).collect();
    crate::agreement::parted(&[&asked, &was, &regenerates, &restarts, &starts])
}

/// The names of `changing`'s settings, as one phrase.
fn listed(changing: &BTreeMap<&str, Option<&str>>) -> String {
    changing.keys().copied().collect::<Vec<_>>().join(" and ")
}

fn already(asked: &BTreeMap<&str, Option<&str>>) -> Problem {
    let settings = listed(asked);
    let holds = if asked.len() > 1 { "hold" } else { "holds" };
    let summary = if asked.values().any(Option::is_some) {
        format!("{settings} already {holds} that")
    } else {
        format!("{settings} {holds} no value of yours to take away")
    };
    Problem::new(
        ALREADY_HOLDS,
        summary,
        "Nothing was changed, and nothing needed to be.",
        Remedy::new("See what each setting holds").with_detail("lemonfiber settings"),
    )
    .in_state(State::Guided)
}

fn offer_moved(changing: &BTreeMap<&str, Option<&str>>, moved: &[&str], standing: &str) -> Problem {
    let setting = listed(changing);
    Problem::new(
        SETTINGS_MOVED,
        format!("That change was agreed to against a different reading of {setting}"),
        format!(
            "Since it was read, {} changed, so nothing was changed. Agreeing to it now would \
             be agreeing to something nobody saw.",
            moved.join(" and ")
        ),
        Remedy::new("Read the change again, and answer the name it prints")
            .with_detail(format!("the offer standing now is {standing}")),
    )
    .in_state(State::Guided)
}
