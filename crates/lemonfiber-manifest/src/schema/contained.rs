//! A stack service's container fields, as the one model every service's are.

use crate::container::Container;
use crate::Service;

impl Service {
    /// Its container fields, its settings in its own id's namespace.
    #[must_use]
    pub fn container(&self) -> Container<'_> {
        Container {
            id: &self.id,
            namespace: &self.id,
            port: self.port,
            listens: self.listens,
            publish: &self.publish,
            takes_data: self.takes_data,
            config_path: self.config_path.as_deref(),
            config_ro: self.config_ro,
            mounts: &self.mount,
            tmpfs: &self.tmpfs,
            env: &self.env,
            secrets: &self.secret,
            user_mode: self.user_mode,
            posture: self.posture,
            mem_limit_mib: self.mem_limit_mib,
            network: self.network.as_ref(),
            shape: self.shape,
            provides: &self.provides,
            speaks: false,
        }
    }
}
