//! Recording a proof or a claim's probe that runs the image's own program, by running it.

use std::path::Path;

use lemonfiber_plugin::{Manifest, Run};

use super::written::{before, written};
use super::{placed, Recorder, Retaking, Taken, Took};
use crate::plugin::program::{performed, recorded, Came, Operator, Starting};
use crate::plugin::recorded::{Of, Recording, RunAsked, RunResult};

/// One run a manifest names a recording for.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct Running<'a> {
    /// What names it.
    pub(super) named: String,
    /// The service whose image it runs, where the manifest names one.
    pub(super) service: Option<&'a str>,
    /// The run.
    pub(super) run: &'a Run,
    /// The recording it names.
    pub(super) fixture: Option<&'a str>,
}

/// Every run recording `manifest` names: each run proof's, then each claim probe's that
/// is a run, in the order the manifest declares them.
pub(super) fn runs(manifest: &Manifest) -> Vec<Running<'_>> {
    let mut runs: Vec<Running<'_>> = manifest
        .proofs
        .iter()
        .filter_map(|proof| {
            proof.run.as_ref().map(|run| Running {
                named: format!("proof {}", proof.id),
                service: proof.service.as_deref(),
                run,
                fixture: proof.fixture.as_deref(),
            })
        })
        .collect();
    for claim in &manifest.claims {
        let service = manifest
            .services
            .iter()
            .find(|service| service.provides.contains(&claim.capability))
            .map(|service| service.id.as_str());
        runs.extend(claim.probes.iter().filter_map(|probe| {
            probe.run.as_ref().map(|run| Running {
                named: format!("claim {} probe {}", claim.capability, probe.id),
                service,
                run,
                fixture: Some(probe.fixture.as_str()),
            })
        }));
    }
    runs
}

/// One run, run and recorded.
pub(super) async fn taken(
    manifest: &Manifest,
    running: Running<'_>,
    root: &Path,
    recorder: Recorder<'_>,
) -> Taken {
    let took = match (running.fixture, placed(manifest, running.service)) {
        (Some(fixture), Some(placed)) => {
            let starting = Starting {
                root,
                service: &placed.service,
            };
            let came = performed(
                recorder.program,
                &placed,
                running.run,
                &Operator::recording(),
                Some(starting),
                recorder.random,
            )
            .await;
            let pinned = format!("{}@{}", placed.image, placed.digest);
            kept(root, fixture, &pinned, running.run, &came, recorder.taking)
        }
        _ => Took::Unrecorded {
            why: "names no fixture, or no one service of this plugin, so there is nowhere to \
                  write what it ran"
                .to_owned(),
        },
    };
    Taken {
        named: running.named,
        fixture: running.fixture.map(str::to_owned),
        took,
    }
}

/// What a run came to, written to `fixture` beneath `root` where it ended.
fn kept(
    root: &Path,
    fixture: &str,
    pinned: &str,
    run: &Run,
    came: &Came,
    taking: &Retaking,
) -> Took {
    let result = match came {
        Came::Ended(result) => recorded(result),
        Came::Overran(seconds) => return Took::Overran { seconds: *seconds },
        Came::Unproven(why) => return Took::Unrecorded { why: why.clone() },
    };
    let before = before(root, fixture);
    if let Some(differs) = before
        .as_ref()
        .and_then(|before| unreproduced(before, pinned, run, &result))
    {
        return Took::Unreproduced { why: differs };
    }
    if let Some(before) = before.filter(|before| {
        before.recorded_from == pinned
            && matches!(&before.of, Of::Run { run: was, result: had } if was.args == run.args && *had == result)
    }) {
        return if before.note.is_empty() {
            Took::Unnoted
        } else {
            Took::Reproduced
        };
    }
    written(
        root,
        fixture,
        pinned,
        Of::Run {
            run: RunAsked {
                args: run.args.clone(),
            },
            result,
        },
        taking,
        &[],
    )
}

/// How a recording of this same run of this same image differs from what the run wrote,
/// where it does.
fn unreproduced(before: &Recording, pinned: &str, run: &Run, result: &RunResult) -> Option<String> {
    let Of::Run {
        run: was,
        result: had,
    } = &before.of
    else {
        return None;
    };
    if before.recorded_from != pinned || was.args != run.args || had == result {
        return None;
    }
    Some(format!(
        "it recorded exit {} and output {:?}, and running it again came to exit {} and output \
         {:?}; a run that differs from one run to the next is evidence of nothing",
        had.exit,
        crate::text::plain(&had.stdout_starts_with),
        result.exit,
        crate::text::plain(&result.stdout_starts_with)
    ))
}
