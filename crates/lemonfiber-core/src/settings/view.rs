//! The settings and secret stores as a run reads them.

use std::path::Path;

use crate::app::Ctx;
use crate::config::paths::{SECRETS, SETTINGS};
use crate::config::store::Failure;

use super::{filing, Secrets, Settings};

/// The settings and secret stores as this run reads them: as they stand, with what filing
/// keeps in them taken in where the run is a rehearsal, which files nothing.
#[derive(Debug, Default)]
pub(crate) struct Stores {
    pub(crate) settings: Settings,
    pub(crate) secrets: Secrets,
}

impl Stores {
    /// The stores this run reads, each empty where it is not there or cannot be read.
    pub(crate) fn of(ctx: &Ctx) -> Self {
        ctx.settings
            .env_file
            .as_deref()
            .map_or_else(Self::default, |env| Self::beside(env).planned(ctx))
    }

    /// The stores beside the environment file at `env` as they stand, each empty where it
    /// is not there or cannot be read.
    pub(crate) fn beside(env: &Path) -> Self {
        Self {
            settings: Settings::at(&env.with_file_name(SETTINGS)).unwrap_or_default(),
            secrets: Secrets::at(&env.with_file_name(SECRETS)).unwrap_or_default(),
        }
    }

    /// The stores this run reads, empty where there are none.
    ///
    /// # Errors
    ///
    /// Where a store is there and cannot be read.
    pub(crate) fn read(ctx: &Ctx) -> Result<Self, Failure> {
        let Some(env) = ctx.settings.env_file.as_deref() else {
            return Ok(Self::default());
        };
        Ok(Self {
            settings: Settings::at(&env.with_file_name(SETTINGS))?,
            secrets: Secrets::at(&env.with_file_name(SECRETS))?,
        }
        .planned(ctx))
    }

    fn planned(mut self, ctx: &Ctx) -> Self {
        if !ctx.dry_run {
            return self;
        }
        let plan = filing::plan(ctx);
        for (name, value) in &plan.settings {
            self.settings.planned(name, value);
        }
        let at = ctx.seconds();
        for (name, value, from) in &plan.secrets {
            self.secrets.planned(name, value, *from, at);
        }
        self
    }
}
