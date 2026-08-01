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
    pub const fn is_adopted(self) -> bool {
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
mod tests {
    use super::{Baseline, Origin, Record};

    #[test]
    fn a_recorded_value_is_written_and_an_adopted_one_is_adopted() {
        // The origin is the whole of the difference a later comparison reads: what
        // lemonfiber wrote is written, what it took from the operator is adopted.
        let mut baseline = Baseline::new();
        baseline.record("sonarr", "downloadclient:sabnzbd:8080", "tv", "1");
        baseline.adopt("radarr", "downloadclient:qbittorrent:8081", "mine", "1");
        assert_eq!(
            baseline
                .entry("sonarr", "downloadclient:sabnzbd:8080")
                .map(|record| record.origin),
            Some(Origin::Written),
        );
        let adopted = baseline.entry("radarr", "downloadclient:qbittorrent:8081");
        assert_eq!(adopted.map(|record| record.origin), Some(Origin::Adopted));
        assert!(adopted.is_some_and(|record| record.origin.is_adopted()));
    }

    #[test]
    fn a_record_without_an_origin_reads_as_written() {
        // A baseline written before adoption existed has no origin field; it must
        // deserialize as written, the only kind those runs recorded.
        let record: Record = serde_json::from_str(r#"{"value":"tv","at":"1"}"#).unwrap_or(Record {
            value: String::new(),
            at: String::new(),
            origin: Origin::Adopted,
        });
        assert_eq!(record.origin, Origin::Written);
        assert!(!record.origin.is_adopted());
    }

    #[test]
    fn merging_keeps_each_record_s_origin() {
        // A record folded in from another baseline keeps whether it was written or
        // adopted, so an adopted value does not become written on the way back into
        // the one baseline the run persists.
        let mut main = Baseline::new();
        let mut other = Baseline::new();
        other.adopt("sonarr", "downloadclient:sabnzbd:8080", "mine", "1");
        main.merge(&other);
        assert_eq!(
            main.entry("sonarr", "downloadclient:sabnzbd:8080")
                .map(|record| record.origin),
            Some(Origin::Adopted),
        );
    }

    #[test]
    fn a_recorded_value_is_read_back_as_the_expected_state() {
        let mut baseline = Baseline::new();
        baseline.record("sonarr", "downloadclient:sabnzbd:8080", "tv", "100");
        assert_eq!(
            baseline.expected("sonarr", "downloadclient:sabnzbd:8080"),
            Some("tv"),
        );
    }

    #[test]
    fn a_field_never_written_has_no_expected_value() {
        let mut baseline = Baseline::new();
        baseline.record("sonarr", "downloadclient:sabnzbd:8080", "tv", "100");
        // A different field on the same service, and a service never touched, both
        // read as nothing recorded — not as an empty value.
        assert_eq!(
            baseline.expected("sonarr", "rootfolder:/data/media/tv"),
            None
        );
        assert_eq!(
            baseline.expected("radarr", "downloadclient:sabnzbd:8080"),
            None
        );
    }

    #[test]
    fn the_newest_write_to_a_field_is_what_is_expected() {
        // lemonfiber's intent for a field can change between runs; the baseline
        // keeps the latest, since that is what a next run compares against.
        let mut baseline = Baseline::new();
        baseline.record("sonarr", "downloadclient:sabnzbd:8080", "tv", "100");
        baseline.record("sonarr", "downloadclient:sabnzbd:8080", "tv-hd", "200");
        assert_eq!(
            baseline.expected("sonarr", "downloadclient:sabnzbd:8080"),
            Some("tv-hd"),
        );
    }

    #[test]
    fn re_recording_the_same_value_keeps_its_original_timestamp() {
        // `at` is when the value was written, not when it was last confirmed, so an
        // idempotent re-record leaves it — and a no-op re-seed does not restamp the
        // file. A genuine value change does take the new stamp.
        let mut baseline = Baseline::new();
        baseline.record("sonarr", "downloadclient:sabnzbd:8080", "tv", "100");
        baseline.record("sonarr", "downloadclient:sabnzbd:8080", "tv", "200");
        let unchanged = serde_json::to_string(&baseline).unwrap_or_default();
        assert!(
            unchanged.contains(r#""at":"100""#) && !unchanged.contains(r#""at":"200""#),
            "a re-record of the same value keeps the first timestamp: {unchanged}"
        );
        baseline.record("sonarr", "downloadclient:sabnzbd:8080", "tv-hd", "300");
        let changed = serde_json::to_string(&baseline).unwrap_or_default();
        assert!(
            changed.contains(r#""at":"300""#),
            "a changed value takes the new timestamp: {changed}"
        );
    }

    #[test]
    fn an_empty_baseline_is_empty_and_a_written_one_is_not() {
        let mut baseline = Baseline::new();
        assert!(baseline.is_empty(), "nothing has been written yet");
        baseline.record("sonarr", "downloadclient:sabnzbd:8080", "tv", "100");
        assert!(!baseline.is_empty(), "a written baseline is not empty");
    }

    #[test]
    fn merging_gathers_each_baselines_records_and_keeps_timestamps() {
        let mut main = Baseline::new();
        main.record("sonarr", "downloadclient:sabnzbd:8080", "tv", "1");
        let mut other = Baseline::new();
        // Another service's record, and the same service's unchanged value.
        other.record("radarr", "downloadclient:sabnzbd:8080", "movies", "2");
        other.record("sonarr", "downloadclient:sabnzbd:8080", "tv", "9");
        main.merge(&other);
        assert_eq!(
            main.expected("radarr", "downloadclient:sabnzbd:8080"),
            Some("movies"),
            "the other baseline's records are gathered in"
        );
        // The unchanged sonarr value keeps its original timestamp, not the merged one.
        let json = serde_json::to_string(&main).unwrap_or_default();
        assert!(
            json.contains(r#""at":"1""#) && !json.contains(r#""at":"9""#),
            "an unchanged value keeps its timestamp through a merge: {json}"
        );
    }

    #[test]
    fn a_baseline_round_trips_through_its_serialised_form() {
        // The baseline is stored as one file between runs, so it must survive a
        // round trip through its serialised form unchanged.
        let mut baseline = Baseline::new();
        baseline.record("sonarr", "downloadclient:sabnzbd:8080", "tv", "100");
        baseline.record("radarr", "downloadclient:qbittorrent:8081", "movies", "100");
        let json = serde_json::to_string(&baseline).unwrap_or_default();
        let restored: Baseline = serde_json::from_str(&json).unwrap_or_default();
        assert_eq!(restored, baseline);
    }
}
