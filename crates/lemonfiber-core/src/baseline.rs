//! The expected-state baseline: what lemonfiber last wrote into each service.
//!
//! Seeding re-asserts lemonfiber's view of a service's configuration, so to tell a
//! value the operator changed from one lemonfiber set itself, it has to remember
//! what it set. This is that memory — per service, per field, the value and when.
//!
//! It is read on a later run to decide whether a difference between what the
//! service now holds and what lemonfiber would write is an operator's edit to
//! preserve or lemonfiber's own intent to re-apply. Recording it is the whole of
//! this module; the comparison that reads it is the drift policy built on top.
//!
//! Unlike the seed's change journal — which is not persisted, because a re-run
//! recovers a partial seed — the baseline must survive across runs: "what
//! lemonfiber last wrote" is knowable only by having kept it. It is pure data with
//! serde, so it round-trips through the one file it is stored in and is decided
//! without a service.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

/// Where a recorded value came from: one lemonfiber wrote itself, or one it
/// adopted from the operator as the accepted state.
///
/// The distinction is what keeps an adopted edit from reading as lemonfiber's own
/// value fallen behind its intent: a value lemonfiber wrote and later finds its
/// intent has moved past is stale and to be brought up to date, but a value the
/// operator set and lemonfiber adopted is theirs to keep — lemonfiber will not push
/// its default over it.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Origin {
    /// lemonfiber wrote this value into the service.
    #[default]
    Written,
    /// The operator set this value and lemonfiber adopted it as the accepted state.
    Adopted,
}

impl Origin {
    /// Whether this value was adopted from the operator rather than written by
    /// lemonfiber.
    #[must_use]
    pub(crate) const fn is_adopted(self) -> bool {
        matches!(self, Self::Adopted)
    }
}

/// One value lemonfiber recorded for a service — one it wrote or one it adopted —
/// and when it recorded it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Record {
    /// The value lemonfiber last recorded.
    pub value: String,
    /// When it recorded it — the seed's own stamp, seconds since the epoch as text.
    pub at: String,
    /// Whether lemonfiber wrote this value or adopted it from the operator. Absent
    /// in a baseline written before adoption existed, where it reads as `Written` —
    /// the only kind those runs recorded.
    #[serde(default)]
    pub origin: Origin,
}

/// What lemonfiber last wrote into every service: per service, per field, the
/// value and when. Ordered maps, so the file it serialises to is stable from one
/// run to the next rather than reshuffled on every write.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Baseline {
    /// service → field → the value lemonfiber last wrote there.
    services: BTreeMap<String, BTreeMap<String, Record>>,
}

impl Baseline {
    /// An empty baseline — nothing written yet.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Record a value lemonfiber wrote, as the expected state a later run compares
    /// the service's actual value against. A write of a *different* value replaces
    /// the record, since the newest is what lemonfiber now expects — the field
    /// carries one value, not a history. A re-record of the *same* value keeps the
    /// original `at`: it is when the value was written, not when it was last
    /// confirmed, so an idempotent re-seed that changes nothing leaves the baseline
    /// — and the file it is stored in — untouched rather than restamped every run.
    pub fn record(&mut self, service: &str, field: &str, value: &str, at: &str) {
        self.write(service, field, value, at, Origin::Written);
    }

    /// Adopt a value the operator set as the expected state, so a later run reads it
    /// as theirs to keep rather than as drift or as lemonfiber's own value to bring
    /// up to date. It is recorded exactly as [`Self::record`] does, but marked as
    /// adopted, which is the whole of the difference a comparison later reads.
    pub fn adopt(&mut self, service: &str, field: &str, value: &str, at: &str) {
        self.write(service, field, value, at, Origin::Adopted);
    }

    /// Record a value with the given origin, keeping the original `at` where the
    /// value is unchanged — the one write path [`Self::record`], [`Self::adopt`] and
    /// [`Self::merge`] all funnel through, so the keep-the-timestamp rule holds
    /// however a value is set.
    fn write(&mut self, service: &str, field: &str, value: &str, at: &str, origin: Origin) {
        let fields = self.services.entry(service.to_owned()).or_default();
        let at = match fields.get(field) {
            Some(existing) if existing.value == value => existing.at.clone(),
            _ => at.to_owned(),
        };
        fields.insert(
            field.to_owned(),
            Record {
                value: value.to_owned(),
                at,
                origin,
            },
        );
    }

    /// The value lemonfiber last recorded for a field, or `None` where it recorded
    /// none. That distinction is the point: a field with no record was never written
    /// by lemonfiber, so a value the service holds there is the operator's alone, not
    /// a difference from anything lemonfiber intended.
    #[must_use]
    pub fn expected(&self, service: &str, field: &str) -> Option<&str> {
        self.entry(service, field)
            .map(|record| record.value.as_str())
    }

    /// The whole record lemonfiber last kept for a field — value, timestamp and
    /// whether it was written or adopted — or `None` where it kept none. The
    /// comparison reads the origin alongside the value, so it needs the record, not
    /// only the value [`Self::expected`] returns.
    #[must_use]
    pub fn entry(&self, service: &str, field: &str) -> Option<&Record> {
        self.services.get(service)?.get(field)
    }

    /// Whether nothing has been recorded yet — the state before a first seed, and
    /// the one a later run reads as "the baseline was never formed" rather than "the
    /// service holds nothing lemonfiber set".
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.services.is_empty()
    }

    /// Fold another baseline's records into this one — how a seed that recorded each
    /// service's writes in its own baseline, so the services could be wired at once,
    /// gathers them back into one. Each record is applied by [`Self::record`], so an
    /// unchanged value keeps its timestamp exactly as recording it directly would.
    pub fn merge(&mut self, other: &Baseline) {
        for (service, fields) in &other.services {
            for (field, record) in fields {
                self.write(service, field, &record.value, &record.at, record.origin);
            }
        }
    }
}

#[cfg(test)]
mod tests;
