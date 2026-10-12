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
        if legacy::setting_key(name).is_some() {
            legacy::value(&self.env, name)
        } else {
            self.settings.value(name)
        }
    }
}
