//! The reports the renderer tests are written against.
//!
//! Shared because several renderers are proven against the same shapes, and a
//! fixture copied per module is a fixture that drifts per module.

use lemonfiber_core::changelog::{told, Notes, Record};
use lemonfiber_core::docker::{Criticality, Service, State};
use lemonfiber_core::error::{Code, Problem, Remedy, Severity};
use lemonfiber_core::glossary::{explain, Term};
use lemonfiber_core::model::{
    FormReport, FormsReport, LifecycleReport, MusicChoice, PresetChoice, SupervisionReport,
    TraceReport, VersionReport,
};
use lemonfiber_core::seed::{
    Assessment as SeedAssessment, Report as SeedReport, Severity as SeedSeverity,
    State as SeedState, Wiring,
};
use lemonfiber_core::stack::closure::{Dropped, Plan};

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
        changelog: notes("0.4.0"),
    }
}

/// A record of three releases, which is enough for every shape one can take.
///
/// `0.4.0` changed something somebody asked for; `0.3.1` is a patch, withdrawn,
/// with the reason; `0.3.0` changed nothing an operator would notice. Written here
/// rather than taken from the record this build carries, because a renderer test
/// asserting against the real history would be a test that has to be rewritten
/// every time a release is cut.
const THREE: &str = r##"{
  "releases": [
    {
      "version": "0.4.0", "tag": "v0.4.0", "released_on": "2026-04-01",
      "delivers": "Seeing what is happening", "patches": null, "carried": null,
      "withdrawn": null, "user_facing": true,
      "groups": [
        {"title": "New", "entries": [
          {"summary": "The panel shows the forwarded port",
           "requirements": ["C2-R4", "C2-R9"], "reference": "#42"}
        ]},
        {"title": "Maintenance", "entries": [
          {"summary": "Bump a dependency", "requirements": []}
        ]}
      ]
    },
    {
      "version": "0.3.1", "tag": "v0.3.1", "released_on": "2026-03-14",
      "delivers": null, "patches": "0.3.0", "carried": null,
      "withdrawn": "the installer shipped a broken pin", "user_facing": true,
      "groups": [{"title": "Fixed", "entries": [
        {"summary": "Put the broken pin back", "requirements": ["A1-R2"]}
      ]}]
    },
    {
      "version": "0.3.0", "tag": "v0.3.0", "released_on": "2026-03-01",
      "delivers": null, "patches": null, "carried": null, "withdrawn": null,
      "user_facing": false,
      "groups": [{"title": "Maintenance", "entries": [
        {"summary": "Move the modules about", "requirements": []}
      ]}]
    }
  ],
  "requirements": {
    "A1-R2": {"feature": "Prerequisites", "url": "https://example.test/a1",
              "shipped_in": ["0.3.1"]},
    "C2-R4": {"feature": "VPN verification", "url": "https://example.test/c2",
              "shipped_in": ["0.4.0"]},
    "C2-R9": {"feature": "VPN verification", "withdrawn": true,
              "shipped_in": ["0.4.0"]}
  }
}"##;

/// What that record tells a build of the named version.
pub(super) fn notes(running: &str) -> Notes {
    told(Record::read(THREE).as_ref(), running)
}

/// One glossary entry, with every part of one filled.
///
/// Taken from the table rather than written out. The entry that has every part is
/// the one that records what another service calls the same thing, and this product
/// writes those words nowhere but the table.
pub(super) fn a_term() -> Term {
    explain("indexer").copied().unwrap_or(Term {
        word: "word",
        short: "what it is for.",
        deep: None,
        also_called: &[],
    })
}

/// A watch that ended having stopped its forms.
pub(super) fn a_watch() -> SupervisionReport {
    SupervisionReport {
        forms: vec!["media".to_owned()],
        reason: "the data location went away".to_owned(),
        stopped: true,
    }
}

/// Two forms: one that combines and one that does not, which is the whole of what a
/// listing has to tell apart.
pub(super) fn some_forms() -> FormsReport {
    FormsReport {
        forms: vec![
            FormReport {
                id: "search".to_owned(),
                name: "Search".to_owned(),
                description: "Find things. Nothing else runs.".to_owned(),
                composable: true,
            },
            FormReport {
                id: "everything".to_owned(),
                name: "Everything".to_owned(),
                description: "The whole stack.".to_owned(),
                composable: false,
            },
        ],
    }
}

/// A resolved plan naming one form, which holds one profile of the same name.
///
/// A form and a profile sharing a name is not how a real stack reads. A fixture
/// that made them differ would put two arbitrary names into every assertion
/// written against it, and the assertions are about neither.
pub(crate) fn a_plan(name: &str, dropped: Vec<Dropped>) -> Plan {
    Plan {
        forms: vec![name.to_owned()],
        profiles: [name.to_owned()].into_iter().collect(),
        services: vec!["sonarr".to_owned()],
        dropped,
    }
}

/// A lifecycle report over that plan, with everything else left quiet.
///
/// Shared across the renderer, the exit codes and the plan itself, because all
/// three are written against the same shape and a fixture per module is a
/// fixture that drifts per module.
pub(crate) fn a_lifecycle(action: &str, plan: Plan) -> LifecycleReport {
    LifecycleReport {
        action: action.to_owned(),
        plan,
        command: Vec::new(),
        rehearsed: false,
        status: None,
        services: Vec::new(),
        condition: None,
        stack_edits: Vec::new(),
        forwarding: None,
        switched: None,
    }
}
