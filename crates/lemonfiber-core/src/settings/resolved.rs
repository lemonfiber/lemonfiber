use std::path::{Path, PathBuf};

use lemonfiber_manifest::container::UrlOf;
use lemonfiber_manifest::Manifest;

use super::Settings;
use crate::app::Ctx;
use crate::config::paths::{SECRETS_FILES, SETTINGS};
use crate::plugin::{Holding, Installed, Reading};
use crate::wiring::{crosses, Fillers};

pub(crate) struct Resolved {
    settings: Settings,
    files: Option<PathBuf>,
    fillers: Option<Fillers>,
}

impl Resolved {
    pub(crate) fn of(ctx: &Ctx, manifest: Option<&Manifest>, installed: &[Installed]) -> Self {
        let env = ctx.settings.env_file.as_deref();
        Self {
            settings: env
                .and_then(|file| Settings::at(&file.with_file_name(SETTINGS)).ok())
                .unwrap_or_default(),
            files: files_of(ctx),
            fillers: manifest.map(|manifest| {
                Fillers::of(
                    manifest,
                    installed,
                    &crate::app::targets::chosen_fillers(ctx),
                    None,
                )
            }),
        }
    }

    pub(crate) fn setting(&self, name: &str) -> Option<String> {
        self.settings.value(name).map(str::to_owned)
    }

    pub(crate) fn secrets_file(&self, service: &str) -> Option<PathBuf> {
        self.files.as_deref().map(|files| file_in(files, service))
    }

    pub(crate) fn crossing(&self, plugin: &str, of: &str) -> Option<Holding> {
        let fillers = self.fillers.as_ref()?;
        let owner = fillers
            .service(of)
            .filter(|owner| owner.brought_by() != Some(plugin))?;
        let crossed = fillers
            .services()
            .find(|one| one.brought_by() == Some(plugin))
            .is_some_and(|recipient| crosses(owner.holder(), recipient.holder()));
        Some(
            fillers
                .setting(owner, crate::config::API_KEY_SUFFIX)
                .filter(|_| crossed)
                .map_or(Holding::Withheld, Holding::Crossing),
        )
    }

    pub(crate) fn reading<T>(
        &self,
        address: &dyn Fn(&str, &UrlOf) -> Option<String>,
        write: impl FnOnce(&Reading<'_>) -> T,
    ) -> T {
        let setting = |name: &str| self.setting(name);
        let secrets_file = |service: &str| self.secrets_file(service);
        let crossing = |plugin: &str, of: &str| self.crossing(plugin, of);
        write(&Reading {
            setting: &setting,
            secrets_file: &secrets_file,
            crossing: &crossing,
            address,
            ..Reading::bare()
        })
    }
}

pub(super) fn files_of(ctx: &Ctx) -> Option<PathBuf> {
    ctx.settings
        .env_file
        .as_deref()
        .map(|file| file.with_file_name(SECRETS_FILES))
}

pub(super) fn file_in(files: &Path, service: &str) -> PathBuf {
    files.join(format!("{service}.env"))
}

pub(crate) fn holding(ctx: &Ctx) -> impl Fn(&str) -> Option<String> {
    let env = ctx.settings.env_file.as_deref();
    let file = env
        .and_then(|file| crate::config::store::read(file).ok())
        .unwrap_or_default();
    let settings = env
        .and_then(|file| Settings::at(&file.with_file_name(SETTINGS)).ok())
        .unwrap_or_default();
    move |name: &str| {
        if super::is_named(name) {
            settings.value(name).map(str::to_owned)
        } else {
            file.get(name).map(str::to_owned)
        }
    }
}
