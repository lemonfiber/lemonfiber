use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use lemonfiber_manifest::Manifest;

use super::recorded::Recorded;
use super::Settings;
use crate::app::Ctx;
use crate::config::paths::{SECRETS_FILES, SETTINGS};
use crate::plugin::{Holding, Installed, Reading};
use crate::wiring::Fillers;

pub(crate) struct Resolved {
    recorded: Recorded,
    changing: BTreeMap<String, Option<String>>,
    files: Option<PathBuf>,
    fillers: Option<Fillers>,
    nfs: Option<crate::config::storage::Export>,
    render_group: Option<u32>,
    moved: Vec<crate::model::MovedReport>,
    name: String,
    pass: Option<String>,
    secrets: Option<super::Secrets>,
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
        let recorded = Recorded::of(ctx);
        Self {
            render_group: recorded
                .line(crate::config::RENDER_GROUP_KEY)
                .and_then(|group| group.trim().parse().ok()),
            moved: crate::app::beside::moved(recorded.env()),
            recorded,
            changing: BTreeMap::new(),
            name: ctx.settings.project.clone(),
            pass: crate::app::project::pinned(ctx),
            files: files_of(ctx),
            secrets: super::secrets_at(ctx).and_then(|at| super::Secrets::at(&at).ok()),
            nfs: ctx.settings.nfs.clone(),
            fillers: manifest.map(|manifest| {
                Fillers::trusting(
                    manifest,
                    installed,
                    &crate::app::targets::chosen_fillers(ctx),
                    None,
                    trusted,
                )
                .numbered(crate::app::targets::ordinals(ctx))
            }),
        }
    }

    pub(crate) fn changed(self, changing: &BTreeMap<&str, Option<&str>>) -> Self {
        Self {
            changing: changing
                .iter()
                .map(|(name, value)| ((*name).to_owned(), value.map(str::to_owned)))
                .collect(),
            ..self
        }
    }

    pub(crate) fn setting(&self, name: &str) -> Option<String> {
        match self.changing.get(name) {
            Some(value) => value.clone(),
            None => self.recorded.value(name).map(str::to_owned),
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
        let recipient = fillers
            .services()
            .find(|one| one.brought_by() == Some(plugin));
        Some(crossed(fillers, owner, recipient))
    }

    /// Where the stack's service `recipient` reads the credential of the service `of`:
    /// where that is held, under its holder's name for it, or withheld where nothing may
    /// hand it over or nothing here holds it.
    pub(crate) fn crossing_into(&self, recipient: &str, of: &str) -> Holding {
        self.fillers
            .as_ref()
            .and_then(|fillers| {
                let owner = fillers.service(of)?;
                Some(crossed(fillers, owner, fillers.service(recipient)))
            })
            .unwrap_or(Holding::Withheld)
    }

    /// Where the stack's service `service` keeps its own key: under the name the stack
    /// gives it, and nowhere where it has none.
    pub(crate) fn own_key(&self, service: &str) -> Holding {
        self.fillers
            .as_ref()
            .and_then(|fillers| {
                fillers.setting(fillers.service(service)?, crate::config::API_KEY_SUFFIX)
            })
            .map_or(Holding::Withheld, Holding::Crossing)
    }

    pub(crate) fn reading<T>(&self, write: impl FnOnce(&Reading<'_>) -> T) -> T {
        let setting = |_: &str, name: &str| self.setting(name);
        let secrets_file = |service: &str| self.secrets_file(service);
        let crossing = |plugin: &str, of: &str| self.crossing(plugin, of);
        write(&Reading {
            setting: &setting,
            secrets_file: &secrets_file,
            crossing: &crossing,
        })
    }

    pub(crate) fn project<T>(&self, write: impl FnOnce(&crate::project::Reading<'_>) -> T) -> T {
        let setting = |_: &str, name: &str| self.setting(name);
        let secrets_file = |service: &str| self.secrets_file(service);
        let operator = |key: &str| {
            OPERATOR_KEYS
                .contains(&key)
                .then(|| self.recorded.line(key).map(str::to_owned))
                .flatten()
        };
        write(&crate::project::Reading {
            setting: &setting,
            secrets_file: &secrets_file,
            render_group: self.render_group,
            nfs: self.nfs.as_ref(),
            moved: &self.moved,
            name: &self.name,
            operator: &operator,
            pass: self.pass.as_deref(),
            held: &|name| {
                self.secrets
                    .as_ref()
                    .is_some_and(|kept| kept.value(name).is_some())
            },
        })
    }
}

/// Where `recipient` reads `owner`'s credential: under its holder's name for it where the
/// credential may cross to `recipient`, and withheld otherwise.
fn crossed(
    fillers: &Fillers,
    owner: &crate::wiring::Filler,
    recipient: Option<&crate::wiring::Filler>,
) -> Holding {
    let held = match &owner.minted {
        Some(minted) => Some(Holding::Store(minted.clone())),
        None => fillers
            .setting(owner, crate::config::API_KEY_SUFFIX)
            .map(Holding::Crossing),
    };
    held.filter(|_| recipient.is_some_and(|recipient| fillers.hands(owner, recipient)))
        .unwrap_or(Holding::Withheld)
}

/// The variables of the operator's environment file the project is written with: none of
/// them a secret.
pub(crate) const OPERATOR_KEYS: [&str; 5] = [
    crate::config::PUID_KEY,
    crate::config::PGID_KEY,
    crate::config::ZONE_KEY,
    crate::config::DATA_ROOT_KEY,
    crate::config::LAN_BIND_KEY,
];

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
