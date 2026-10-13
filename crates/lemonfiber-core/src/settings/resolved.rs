use std::path::{Path, PathBuf};

use lemonfiber_manifest::container::UrlOf;
use lemonfiber_manifest::Manifest;

use super::recorded::Recorded;
use super::Settings;
use crate::app::Ctx;
use crate::config::paths::{SECRETS_FILES, SETTINGS};
use crate::plugin::{Holding, Installed, Reading};
use crate::wiring::{crosses, Fillers};

pub(crate) struct Resolved {
    recorded: Recorded,
    changing: Option<(String, Option<String>)>,
    files: Option<PathBuf>,
    fillers: Option<Fillers>,
    nfs: Option<crate::config::storage::Export>,
}

impl Resolved {
    pub(crate) fn of(ctx: &Ctx, manifest: Option<&Manifest>, installed: &[Installed]) -> Self {
        Self::trusting(
            ctx,
            manifest,
            installed,
            crate::plugin::first_party::EMBEDDED,
        )
    }

    pub(crate) fn trusting(
        ctx: &Ctx,
        manifest: Option<&Manifest>,
        installed: &[Installed],
        trusted: &[crate::plugin::first_party::FirstParty],
    ) -> Self {
        Self {
            recorded: Recorded::of(ctx),
            changing: None,
            files: files_of(ctx),
            nfs: ctx.settings.nfs.clone(),
            fillers: manifest.map(|manifest| {
                Fillers::trusting(
                    manifest,
                    installed,
                    &crate::app::targets::chosen_fillers(ctx),
                    None,
                    trusted,
                )
            }),
        }
    }

    pub(crate) fn changed(self, name: &str, value: Option<&str>) -> Self {
        Self {
            changing: Some((name.to_owned(), value.map(str::to_owned))),
            ..self
        }
    }

    pub(crate) fn setting(&self, name: &str) -> Option<String> {
        match &self.changing {
            Some((changing, value)) if changing == name => value.clone(),
            _ => self.recorded.value(name).map(str::to_owned),
        }
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
            nfs: self.nfs.as_ref(),
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
        if super::is_stored(name) {
            settings.value(name).map(str::to_owned)
        } else {
            file.get(name).map(str::to_owned)
        }
    }
}
