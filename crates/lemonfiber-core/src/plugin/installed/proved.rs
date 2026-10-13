//! A proof that runs the image's own program, as an install keeps it.

use lemonfiber_plugin::{Expect, Manifest, Run};
use serde::{Deserialize, Serialize};

use super::{Installed, Placed};

/// One proof that runs the image's own program, kept so the doctor and a later proof can
/// run it again without the plugin's source.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
#[schemars(rename = "PluginRunProof")]
pub struct RunProof {
    /// The proof's id.
    pub proof: String,
    /// What it establishes, in one line.
    pub title: String,
    /// Why it is worth asserting.
    pub why: String,
    /// The service whose image it runs.
    pub service: String,
    /// What the image's own program is handed.
    pub run: Run,
    /// What the run must come to.
    pub expect: Expect,
}

/// Every proof in `manifest` that runs the image's own program and settles its service.
pub(super) fn run_proofs(manifest: &Manifest) -> Vec<RunProof> {
    manifest
        .proofs
        .iter()
        .filter(|proof| proof.request.is_none())
        .filter_map(|proof| {
            Some(RunProof {
                proof: proof.id.clone(),
                title: proof.title.clone(),
                why: proof.why.clone(),
                service: manifest.asks(proof.service.as_deref())?.id.clone(),
                run: proof.run.clone()?,
                expect: proof.expect.clone(),
            })
        })
        .collect()
}

impl Installed {
    /// The same record, with the digest of each run-proved service's starting files.
    #[must_use]
    pub fn starting(self, digests: &dyn Fn(&str) -> Option<String>) -> Self {
        let proved: Vec<String> = self
            .run_proofs
            .iter()
            .map(|one| one.service.clone())
            .collect();
        Self {
            services: self
                .services
                .into_iter()
                .map(|placed| Placed {
                    starting: proved
                        .contains(&placed.service)
                        .then(|| digests(&placed.service))
                        .flatten(),
                    ..placed
                })
                .collect(),
            ..self
        }
    }
}
