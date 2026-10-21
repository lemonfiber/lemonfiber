use crate::app::Ctx;
use crate::config::env::EnvFile;
use crate::config::paths::SETTINGS;

use super::{legacy, Settings};

#[derive(Debug, Default)]
pub(crate) struct Recorded {
    env: EnvFile,
    settings: Settings,
}

impl Recorded {
    pub(crate) fn of(ctx: &Ctx) -> Self {
        let file = ctx.settings.env_file.as_deref();
        Self {
            env: file
                .and_then(|file| crate::config::store::read(file).ok())
                .unwrap_or_default(),
            settings: file
                .and_then(|file| Settings::at(&file.with_file_name(SETTINGS)).ok())
                .unwrap_or_default(),
        }
    }

    pub(crate) fn value(&self, name: &str) -> Option<&str> {
        self.settings
            .value(name)
            .or_else(|| legacy::value(&self.env, name))
    }

    pub(crate) const fn env(&self) -> &EnvFile {
        &self.env
    }

    pub(crate) fn line(&self, key: &str) -> Option<&str> {
        self.env.get(key)
    }
}
