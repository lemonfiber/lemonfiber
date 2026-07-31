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

/// One value lemonfiber wrote into a service, and when it wrote it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Record {
    /// The value lemonfiber last wrote.
    pub value: String,
    /// When it wrote it — the seed's own stamp, seconds since the epoch as text.
    pub at: String,
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
            },
        );
    }

    /// The value lemonfiber last wrote for a field, or `None` where it wrote none.
    /// That distinction is the point: a field with no record was never written by
    /// lemonfiber, so a value the service holds there is the operator's alone, not
    /// a difference from anything lemonfiber intended.
    #[must_use]
    pub fn expected(&self, service: &str, field: &str) -> Option<&str> {
        self.services
            .get(service)?
            .get(field)
            .map(|record| record.value.as_str())
    }

    /// Whether nothing has been recorded yet — the state before a first seed, and
    /// the one a later run reads as "the baseline was never formed" rather than "the
    /// service holds nothing lemonfiber set".
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.services.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::Baseline;

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
