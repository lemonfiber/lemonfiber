//! Where alerts go, as the operator and a member read it.

use lemonfiber_core::alert::Notification;
use lemonfiber_core::app::Outcome;
use lemonfiber_core::model::{
    BackendKind, BackendProof, BackendReport, ChosenBy, Delivery, DeliveryOutcome, DeliveryRetry,
    HostedBackend, NotifyMineReport, NotifyReport, NotifyState, OfferedBackend, UnreadBackend,
};

/// What the back-ends and a member's choice answer with.
pub(crate) fn samples() -> Vec<Outcome> {
    vec![
        Outcome::Notify(NotifyReport {
            state: NotifyState::Degraded,
            backends: vec![BackendReport {
                id: "away".to_owned(),
                kind: BackendKind::Ntfy,
                reaches: "https://ntfy.sh/stack".to_owned(),
                chosen_by: ChosenBy::Member("a7f3".to_owned()),
                hosted: Some(HostedBackend {
                    host: "ntfy.sh".to_owned(),
                    leaves: vec!["the kind of each alert and its summary".to_owned()],
                }),
                proof: BackendProof::Held("it refused its token".to_owned()),
                last: Some(Delivery {
                    at: 1_793_000_000,
                    kind: Notification::Problem,
                    outcome: DeliveryOutcome::Failed("403".to_owned()),
                }),
                failures: vec![Delivery {
                    at: 1_793_000_000,
                    kind: Notification::Ready,
                    outcome: DeliveryOutcome::Confirmed,
                }],
                retry: Some(DeliveryRetry {
                    next: 1_793_000_060,
                    attempt: 2,
                    of: 5,
                }),
            }],
            unread: vec![UnreadBackend {
                backend: "broken".to_owned(),
                because: "its record is not JSON".to_owned(),
            }],
            rehearsed: false,
        }),
        Outcome::NotifyMine(NotifyMineReport {
            offered: vec![OfferedBackend {
                id: "phones".to_owned(),
                name: "Phones".to_owned(),
                receiving: true,
            }],
            phone: true,
            rehearsed: false,
        }),
    ]
}
