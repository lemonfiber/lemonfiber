//! What an author asks of a plugin's own source, as each answer is written.

use lemonfiber_core::app::Outcome;
use lemonfiber_core::filling::{Filling, Shown};
use lemonfiber_core::plugin::{
    Asserted, Assertion, CaseVerdict, Claimed, Claiming, Conformed, Contributed, Evidence, Failed,
    Judged, Provenance, Ran, Recorded, Taken, Took, Verdict, Violation, Vouch, Vouched,
};

/// What the four reads of a plugin's source answer with.
pub(crate) fn samples() -> Vec<Outcome> {
    vec![
        Outcome::PluginClaims(claimed()),
        Outcome::PluginConformance(conformed()),
        Outcome::PluginProvenance(vouched()),
        Outcome::PluginRecording(recorded()),
    ]
}

fn refusal() -> Violation {
    Violation {
        location: "service subber.image".to_owned(),
        message: "is not pinned to a digest".to_owned(),
    }
}

fn claimed() -> Claimed {
    Claimed {
        id: "subber".to_owned(),
        name: "Subber".to_owned(),
        version: "1.2.0".to_owned(),
        vocabulary_version: 1,
        extension_points_version: 1,
        against: Evidence::Recordings,
        refusals: vec![refusal()],
        capabilities: vec![Claiming {
            name: "subtitles.fetch".to_owned(),
            service: "subber".to_owned(),
            own: false,
            shown: Shown::Demonstrated,
            probes: vec![Ran {
                probe: "watch-answers".to_owned(),
                verdict: Verdict::Passed,
            }],
            filling: Some(Filling::By {
                service: "subber".to_owned(),
            }),
        }],
        proofs: vec![Asserted {
            kind: Assertion::Proof,
            id: "subber.runs".to_owned(),
            says: "it answers its version".to_owned(),
            service: "subber".to_owned(),
            verdict: Verdict::Passed,
        }],
        checks: vec![Asserted {
            kind: Assertion::Check,
            id: "subber:reachable".to_owned(),
            says: "it answers on its port".to_owned(),
            service: "subber".to_owned(),
            verdict: Verdict::Unproven {
                why: "no recording of it is there".to_owned(),
            },
        }],
        contributions: vec![Contributed {
            at: "doctor.remedies".to_owned(),
            id: "subber:restart".to_owned(),
            says: "Restart subber".to_owned(),
            about: Some("subber:reachable".to_owned()),
        }],
        installable: false,
    }
}

fn conformed() -> Conformed {
    Conformed {
        id: "subber".to_owned(),
        refusals: vec![refusal()],
        cases: vec![Judged {
            contract: "subtitles.fetch@1".to_owned(),
            case: "watch-answers".to_owned(),
            recording: "conformance/subtitles.fetch@1/watch-answers.json".to_owned(),
            verdict: CaseVerdict::Failed {
                faults: vec!["answered 200; unkeyed".to_owned()],
            },
        }],
        conforms: false,
    }
}

fn vouched() -> Vouched {
    Vouched {
        id: "subber".to_owned(),
        images: vec![Vouch {
            service: "subber".to_owned(),
            image: "ghcr.io/example/subber".to_owned(),
            digest: "sha256:0000000000000000000000000000000000000000000000000000000000000000"
                .to_owned(),
            held: Provenance::Signed {
                by: "keys/subber.pem".to_owned(),
            },
        }],
        installable: true,
    }
}

fn recorded() -> Recorded {
    Recorded {
        id: "subber".to_owned(),
        refusals: Vec::new(),
        recordings: vec![Taken {
            named: "proof subber.runs".to_owned(),
            fixture: Some("recordings/subber/runs-version.json".to_owned()),
            took: Took::Overran { seconds: 5 },
        }],
        failed: vec![Failed {
            service: "subber".to_owned(),
            step: "health".to_owned(),
            why: "it exited 3 before its health passed".to_owned(),
            log: vec!["starting".to_owned()],
        }],
        holds: false,
    }
}
