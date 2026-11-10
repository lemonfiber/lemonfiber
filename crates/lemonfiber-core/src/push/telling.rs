//! Each phone as the account of where alerts go reads it, and what a member's own phone
//! is owed.

use crate::app::Ctx;

use super::{Kept, Record};

/// Whether `member` has registered a phone of their own.
pub(crate) fn owns_one(ctx: &Ctx, member: &str) -> bool {
    super::run::kept_at(ctx)
        .ok()
        .and_then(|path| super::run::read_kept(&path).ok())
        .is_some_and(|kept| {
            kept.endpoints
                .iter()
                .any(|one| one.owner.as_deref() == Some(member))
        })
}

/// Owe `member`'s own phones one wake, that something they asked for is ready.
pub(crate) fn ready(ctx: &Ctx, member: &str) {
    if let Some(path) = super::run::waiting_at(ctx) {
        let _ = crate::settings::kept::changed(&path, &mut |kept: &mut Kept| {
            for one in kept
                .endpoints
                .iter_mut()
                .filter(|one| one.owner.as_deref() == Some(member))
            {
                one.owed = one.owed.saturating_add(1).min(super::run::MOST_OWED);
            }
            Ok(())
        });
    }
}

/// Every endpoint as a back-end the stack delivers to, by the host its distributor is on.
pub(crate) fn as_backends(ctx: &Ctx) -> Vec<crate::model::BackendReport> {
    super::run::kept_at(ctx)
        .ok()
        .and_then(|path| super::run::read_kept(&path).ok())
        .map(|kept| kept.endpoints.iter().map(as_backend).collect())
        .unwrap_or_default()
}

/// One endpoint as a back-end.
fn as_backend(record: &Record) -> crate::model::BackendReport {
    crate::model::BackendReport {
        id: record.name.clone(),
        kind: crate::model::BackendKind::Unifiedpush,
        reaches: url::Url::parse(&record.endpoint)
            .ok()
            .and_then(|url| url.host_str().map(str::to_owned))
            .unwrap_or_default(),
        chosen_by: record.owner.clone().map_or(
            crate::model::ChosenBy::Operator,
            crate::model::ChosenBy::Member,
        ),
        hosted: None,
        proof: record.proven.map_or(
            crate::model::BackendProof::Unproven,
            crate::model::BackendProof::Proven,
        ),
        last: record.last.clone(),
        failures: record.failures.clone(),
        retry: None,
    }
}
