//! Recording a proof that runs the image's own program, by running it.

use std::path::Path;

use lemonfiber_plugin::{Manifest, Proof, Run};

use super::written::{before, written};
use super::{placed, Taken, Took};
use crate::plugin::program::{performed, recorded, Came, Operator, Starting};
use crate::plugin::recorded::{Of, Recording, RunAsked, RunResult};
use crate::ports::program::Program;

/// One run proof, run and recorded.
pub(super) async fn taken(
    manifest: &Manifest,
    proof: &Proof,
    run: &Run,
    root: &Path,
    program: &dyn Program,
) -> Taken {
    let took = match (
        proof.fixture.as_deref(),
        placed(manifest, proof.service.as_deref()),
    ) {
        (Some(fixture), Some(placed)) => {
            let starting = Starting {
                root,
                service: &placed.service,
            };
            let came = performed(
                program,
                &placed,
                run,
                &Operator::recording(),
                Some(starting),
            )
            .await;
            let pinned = format!("{}@{}", placed.image, placed.digest);
            kept(root, fixture, &pinned, run, &came)
        }
        _ => Took::Unrecorded {
            why: "names no fixture, or no one service of this plugin, so there is nowhere to \
                  write what it ran"
                .to_owned(),
        },
    };
    Taken {
        named: format!("proof {}", proof.id),
        fixture: proof.fixture.clone(),
        took,
    }
}

/// What a run came to, written to `fixture` beneath `root` where it ended.
fn kept(root: &Path, fixture: &str, pinned: &str, run: &Run, came: &Came) -> Took {
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
