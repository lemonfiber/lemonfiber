use crate::app::plugins::writing;
use crate::app::Ctx;
use crate::config::paths::SETTINGS;
use crate::error::codes::setting::{ALREADY_HOLDS, SETTINGS_MOVED};
use crate::error::{Diagnose, Problem, Remedy, State};
use crate::journal::{Change, Kind};
use crate::model::{SettingChange, SettingsReport};

use super::readers::{self, Readers};
use super::recorded::Recorded;
use super::{fences, legacy, restarting, Resolved};

pub(super) const REFUSING: writing::Refusing = writing::Refusing {
    moved: SETTINGS_MOVED,
    unwritten: crate::error::codes::config::CONFIG_NOT_WRITTEN,
};

const OFFERED: [&str; 4] = [
    "the change itself",
    "what it holds now",
    "the entries it regenerates",
    "the services it restarts",
];

pub(super) struct Asked<'a> {
    pub(super) setting: &'a str,
    pub(super) value: Option<&'a str>,
    pub(super) agreement: Option<&'a str>,
}

pub(super) async fn changed(ctx: &Ctx, asked: &Asked<'_>) -> Result<SettingsReport, Box<Problem>> {
    let manifest = ctx
        .stack
        .checked_manifest(ctx.today())
        .map_err(|err| Box::new(err.problem()))?;
    let register = crate::app::plugins::read(ctx)?;
    let installed = register.installed();
    let readers = readers::readers(&manifest, installed);
    named(ctx, asked.setting, &readers)?;
    if let Some(value) = asked.value {
        fences::valued(asked.setting, value)?;
    }
    let was = Recorded::of(ctx).value(asked.setting).map(str::to_owned);
    if was.as_deref() == asked.value {
        return Err(Box::new(already(asked.setting, asked.value)));
    }
    let regenerates = readers::services(
        readers
            .settings
            .get(asked.setting)
            .map(Vec::as_slice)
            .unwrap_or_default(),
    );
    let after = Resolved::of(ctx, Some(&manifest), installed).changed(asked.setting, asked.value);
    let overwrites = writing::regenerated(
        ctx,
        &manifest,
        &register,
        &crate::app::targets::chosen_fillers(ctx),
        &after,
    )?;
    let restarts = restarting::running(ctx, &regenerates).await;
    let standing = offer(asked, was.as_deref(), &regenerates, &restarts);
    let mut change = SettingChange {
        setting: asked.setting.to_owned(),
        was: was.clone(),
        value: asked.value.map(str::to_owned),
        regenerates,
        restarts,
        consequence: consequence(ctx, asked),
        applied: false,
        agreement: standing.clone(),
        unrestarted: None,
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
        return Err(Box::new(offer_moved(asked.setting, &moved, &standing)));
    }
    if ctx.dry_run {
        return Ok(super::listing::report(
            ctx,
            &manifest,
            installed,
            Some(change),
        ));
    }
    let (Some(env), Some(paths)) = (
        ctx.settings.env_file.as_deref(),
        crate::app::targets::layout(ctx),
    ) else {
        return Err(Box::new(crate::config::store::Failure::Nowhere.problem()));
    };
    writing::unmoved(&overwrites, REFUSING)?;
    let stamp = ctx.stamp();
    let mut changes = vec![journalled(asked, was.as_deref(), &stamp)];
    changes.extend(
        overwrites
            .iter()
            .map(|overwrite| overwrite.journalled(super::OPERATION, &stamp)),
    );
    crate::app::recover::journalled(&paths.journal(), &changes, ctx.seams.random.as_ref())
        .map_err(|failure| Box::new(failure.problem()))?;
    super::store::recorded(&env.with_file_name(SETTINGS), asked.setting, asked.value)
        .map_err(|failure| Box::new(failure.problem()))?;
    writing::overwritten(ctx, &overwrites, REFUSING)?;
    if asked.setting == super::own::DEVICE_KEY_APPS {
        crate::device_keys::run::apps_changed(ctx, was.as_deref(), asked.value).await?;
    }
    if asked.setting == super::own::DASHBOARD_SEARCH {
        let _ = crate::seed::run::reshow(ctx).await;
    }
    change.applied = true;
    change.unrestarted = restarting::restarted(ctx, &change.restarts).await;
    Ok(super::listing::report(
        ctx,
        &manifest,
        installed,
        Some(change),
    ))
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

fn journalled(asked: &Asked<'_>, was: Option<&str>, stamp: &str) -> Change {
    Change {
        at: stamp.to_owned(),
        operation: super::OPERATION.to_owned(),
        target: asked.setting.to_owned(),
        kind: Kind::Setting {
            name: asked.setting.to_owned(),
            previous: was.map(str::to_owned),
            current: asked.value.map(str::to_owned),
            filed: None,
        },
    }
}

fn consequence(ctx: &Ctx, asked: &Asked<'_>) -> Option<String> {
    let key = legacy::setting_key(asked.setting)?;
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
    crate::app::configuring::consequence(ctx, &held, key, asked.value.unwrap_or_default())
}

fn offer(
    asked: &Asked<'_>,
    was: Option<&str>,
    regenerates: &[String],
    restarts: &[String],
) -> String {
    let regenerates: Vec<&str> = regenerates.iter().map(String::as_str).collect();
    let restarts: Vec<&str> = restarts.iter().map(String::as_str).collect();
    crate::agreement::parted(&[
        &[asked.setting, asked.value.unwrap_or_default()],
        &[was.unwrap_or_default()],
        &regenerates,
        &restarts,
    ])
}

fn already(setting: &str, value: Option<&str>) -> Problem {
    let summary = match value {
        Some(_) => format!("{setting} already holds that"),
        None => format!("{setting} holds no value of yours to take away"),
    };
    Problem::new(
        ALREADY_HOLDS,
        summary,
        "Nothing was changed, and nothing needed to be.",
        Remedy::new("See what each setting holds").with_detail("lemonfiber settings"),
    )
    .in_state(State::Guided)
}

fn offer_moved(setting: &str, moved: &[&str], standing: &str) -> Problem {
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
