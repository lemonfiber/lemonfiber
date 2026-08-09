//! The reports the renderer tests are written against.
//!
//! Shared because several renderers are proven against the same shapes, and a
//! fixture copied per module is a fixture that drifts per module.

use lemonfiber_core::docker::{Criticality, Service, State};
use lemonfiber_core::error::{Code, Problem, Remedy, Severity};
use lemonfiber_core::model::{
    MusicChoice, PresetChoice, SupervisionReport, TraceReport, VersionReport,
};
use lemonfiber_core::seed::{
    Assessment as SeedAssessment, Report as SeedReport, Severity as SeedSeverity,
    State as SeedState, Wiring,
};

/// One wiring in the given state, with no severity raised.
pub(super) fn wiring(connection: &str, state: SeedState) -> Wiring {
    Wiring {
        connection: connection.to_owned(),
        state,
        severity: SeedSeverity::Informational,
    }
}

/// A seed report over the given wirings, with drift assessable.
pub(super) fn seed_report(wirings: Vec<Wiring>) -> SeedReport {
    SeedReport {
        wirings,
        assessment: SeedAssessment::Assessed,
    }
}

/// A problem carrying one remedy, for the diagnosis renderers.
pub(super) fn a_problem() -> Problem {
    Problem::new(
        Code::new("TEST"),
        Severity::Error,
        "it broke",
        "nothing will import",
        Remedy::new("restart it").with_detail("docker compose restart"),
    )
}

/// One service in the given state.
pub(super) fn service(id: &str, state: State, exit: Option<i32>) -> Service {
    Service {
        id: id.to_owned(),
        name: format!("{id} service"),
        profile: "media".to_owned(),
        state,
        criticality: Criticality::Core,
        exit,
        depends_on: Vec::new(),
    }
}

/// A preset choice, transcoding warning off unless asked for.
pub(super) fn preset(needs_transcoding_here: bool) -> PresetChoice {
    PresetChoice {
        scope: "everything".to_owned(),
        preset: "Balanced".to_owned(),
        means: "1080p".to_owned(),
        resolution: "1080p".to_owned(),
        size_per_hour: "3 GB".to_owned(),
        transcoding: "direct play".to_owned(),
        needs_transcoding_here,
    }
}

/// An audio-format choice.
pub(super) fn music_pick() -> MusicChoice {
    MusicChoice {
        scope: "music".to_owned(),
        format: "FLAC".to_owned(),
        means: "lossless".to_owned(),
        targets: "albums".to_owned(),
        size_per_hour: "400 MB".to_owned(),
        note: "large".to_owned(),
    }
}

/// A trace of a matched item, with nothing else claimed.
pub(super) fn a_trace() -> TraceReport {
    TraceReport {
        item: "The Expanse".to_owned(),
        matched: true,
        ..TraceReport::default()
    }
}

/// A version report naming a reachable compose.
pub(super) fn a_version() -> VersionReport {
    VersionReport {
        binary: "0.4.0".to_owned(),
        supported_schema: vec![1],
        stack: "1.2.3".to_owned(),
        compose: Some("2.29".to_owned()),
    }
}

/// A watch that ended having stopped its forms.
pub(super) fn a_watch() -> SupervisionReport {
    SupervisionReport {
        forms: vec!["media".to_owned()],
        reason: "the data location went away".to_owned(),
        stopped: true,
    }
}
