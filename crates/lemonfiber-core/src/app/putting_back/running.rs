//! The parts a reversal starts or stops again.

use crate::journal::{Action, Undo};

use super::{Ctx, Left};

/// One part to start or stop again: the reversal, its service, and whether it is left
/// running.
pub(super) type Running = (Undo, String, bool);

/// `left` split into the parts to start or stop again and everything else.
pub(super) fn parted(left: &[Undo]) -> (Vec<Running>, Vec<Undo>) {
    let mut running = Vec::new();
    let mut rest = Vec::new();
    for undo in left {
        match &undo.action {
            Action::Run {
                service,
                running: up,
            } => {
                running.push((undo.clone(), service.clone(), *up));
            }
            _ => rest.push(undo.clone()),
        }
    }
    (running, rest)
}

/// Each of `running` carried out in the project in `stack`: the ones started or stopped, and each one that could
/// not be, with why.
pub(super) async fn ran(
    ctx: &Ctx,
    stack: &std::path::Path,
    running: &[Running],
) -> (Vec<Undo>, Vec<Left>) {
    let mut ran = Vec::new();
    let mut unran = Vec::new();
    for (undo, service, up) in running {
        match crate::app::engine::ran(ctx, stack, service, *up).await {
            Ok(()) => ran.push(undo.clone()),
            Err(because) => unran.push(Left {
                target: undo.target.clone(),
                because,
            }),
        }
    }
    (ran, unran)
}
