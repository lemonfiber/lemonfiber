//! The settings a run starts from, read from the env file and from what only the
//! surface starting it knows.

use std::path::{Path, PathBuf};

use lemonfiber_words::setting::{reads_as_off, reads_as_on};

use super::paths::Paths;
use super::{
    data_root_from_env, events_address_from_env, exposed_from_env, front_door_from_env,
    household_domain_from_env, household_host_from_env, indexer_from_env, ip_echo_from_env,
    nfs_from_env, port_forward_from_env, project_from_env, provider_host_from_env, quiet_from_env,
    service_user_from_env, store, unmanaged_from_env, vpn_declined_from_env, Protocols, Reaching,
    Resending, Settings, AUTOSTART_ON_BATTERY_KEY, EXPLANATIONS_KEY,
};
use crate::app::Ctx;
use crate::ports::docker::Target;
use crate::settings::legacy::{overlaid, unstored};

/// What only the surface starting a run knows about where it runs.
#[derive(Debug, Clone, Default)]
pub struct Surroundings {
    /// Where lemonfiber keeps its own files, where this machine says.
    pub paths: Option<Paths>,
    /// The program running.
    pub program: Option<PathBuf>,
    /// The home directory of the user running it.
    pub home: Option<PathBuf>,
    /// The engine it reaches.
    pub docker: Target,
    /// The locale the host names.
    pub host_locale: Option<String>,
    /// The mount table of the process, where it can be read.
    pub mounts: Option<String>,
}

impl Settings {
    /// The settings a run starts from: the env file's, with what the run's own
    /// records hold over it where there is a run to read them through.
    #[must_use]
    pub fn read(run: Option<&Ctx>, around: &Surroundings) -> Self {
        let paths = around.paths.as_ref();
        let env_file = paths.map(Paths::env_file);
        let recorded = recorded(run, env_file.as_deref());
        Self {
            project: project_from_env(&recorded),
            quiet: quiet_from_env(&recorded),
            overlays: crate::overlay::layered(env_file.as_deref()),
            protocols: Protocols::from_env(&recorded),
            vpn_declined: vpn_declined_from_env(&recorded),
            ip_echo: ip_echo_from_env(&recorded),
            data_root: data_root_from_env(&recorded),
            nfs: nfs_from_env(&recorded),
            storage_state: paths.map(Paths::storage_state),
            service_user: service_user_from_env(&recorded),
            port_forward: port_forward_from_env(&recorded),
            indexer: indexer_from_env(&recorded),
            admission: paths.map(Paths::admission),
            companion: paths.map(Paths::companion),
            host_locale: around.host_locale.clone(),
            host_zone: crate::zone::Zone::host(),
            household_host: household_host_from_env(&recorded),
            events_address: events_address_from_env(&recorded),
            household_domain: household_domain_from_env(&recorded),
            exposed: exposed_from_env(&recorded),
            unmanaged: unmanaged_from_env(&recorded),
            front_door: front_door_from_env(&recorded),
            reaching: Reaching::from_env(&recorded),
            provider_host: provider_host_from_env(&recorded),
            program: around.program.clone(),
            hosted: paths.map(Paths::hosted),
            home: around.home.clone(),
            container: around
                .mounts
                .as_deref()
                .and_then(crate::contained::container),
            docker: around.docker.clone(),
            explanations: !recorded.get(EXPLANATIONS_KEY).is_some_and(reads_as_off),
            autostart_on_battery: recorded
                .get(AUTOSTART_ON_BATTERY_KEY)
                .is_some_and(reads_as_on),
            env_file,
            stack_dir: paths.map(Paths::stack),
        }
    }

    /// `ctx`'s settings with what setup decides read back from the env file at
    /// `env_file`.
    #[must_use]
    pub fn as_written(ctx: &Ctx, env_file: &Path) -> Self {
        let recorded = recorded(Some(ctx), Some(env_file));
        Self {
            protocols: Protocols::from_env(&recorded),
            vpn_declined: vpn_declined_from_env(&recorded),
            data_root: data_root_from_env(&recorded),
            service_user: service_user_from_env(&recorded),
            ..ctx.settings.clone()
        }
    }

    /// What this machine is, from the host it runs on and the engine it reaches.
    #[must_use]
    pub fn environment(&self) -> crate::platform::Environment {
        crate::platform::Environment::resolve(
            crate::platform::HOST_OS,
            crate::platform::engine_is_desktop(&self.docker),
        )
    }

    /// The engine host every envelope of this run names, settled on its first asking.
    #[must_use]
    pub fn settle_host(&self) -> Option<String> {
        crate::model::settle_host(self.docker.host())
    }

    /// How often and for how long a request sent again is answered from what it was
    /// answered before, as the env file says.
    #[must_use]
    pub fn resending(&self) -> Resending {
        self.env_file
            .as_deref()
            .and_then(|path| store::read(path).ok())
            .map_or_else(Resending::default, |file| Resending::from_env(&file))
    }
}

impl Ctx {
    /// File what the operator's environment file and the stack's hold into the stores,
    /// as a run starting does before it reads its settings again.
    pub async fn filed(&self) {
        crate::settings::filing::everything(self).await;
    }
}

fn recorded(run: Option<&Ctx>, env_file: Option<&Path>) -> super::env::EnvFile {
    env_file
        .and_then(|path| store::read(path).ok())
        .map(|file| match run {
            Some(ctx) => overlaid(ctx, file),
            None => unstored(file),
        })
        .unwrap_or_default()
}

#[cfg(test)]
mod tests;
