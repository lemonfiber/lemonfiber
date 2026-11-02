use crate::app::Ctx;
use crate::config::env::EnvFile;

use super::{legacy, Stores};

/// What this run reads as recorded: the stores, and lemonfiber's own lines of the
/// environment file, which hold no setting and no secret.
#[derive(Debug, Default)]
pub(crate) struct Recorded {
    env: EnvFile,
    stores: Stores,
}

impl Recorded {
    pub(crate) fn of(ctx: &Ctx) -> Self {
        Self {
            env: ctx
                .settings
                .env_file
                .as_deref()
                .and_then(|file| crate::config::store::read(file).ok())
                .map(legacy::unstored)
                .unwrap_or_default(),
            stores: Stores::of(ctx),
        }
    }

    pub(crate) fn value(&self, name: &str) -> Option<&str> {
        self.stores.settings.value(name)
    }

    pub(crate) fn holds_secret(&self, name: &str) -> bool {
        self.stores.secrets.value(name).is_some()
    }

    pub(crate) const fn env(&self) -> &EnvFile {
        &self.env
    }

    pub(crate) fn line(&self, key: &str) -> Option<&str> {
        self.env.get(key)
    }
}
