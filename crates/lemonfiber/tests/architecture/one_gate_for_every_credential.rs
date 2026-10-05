//! Whom a credential may reach is decided in one place.
//!
//! A plugin's service is a stranger's code, and a credential handed to it is kept. So a
//! credential crosses from one service to another only where `wiring::crosses` lets it,
//! and seeding hands one over only through the connecting table, which asks it. The two
//! rules here hold that shape: nothing in seeding or in the readers it hands credentials
//! out of judges an origin for itself, and every seed pass that reads another service's
//! credential to hand on takes its pairs from that table.
//!
//! Each was found missing site by site before the gate existed — once for the indexer
//! aggregator's password, once for the media server's, once for the indexer's key — and
//! a rule checked at every site is a rule the next site forgets.

use std::path::Path;

use crate::source_tree::{production, sources, test_only};

/// Where seeding hands credentials over, and where it reads them from.
const GOVERNED: [&str; 2] = [
    "crates/lemonfiber-core/src/seed/",
    "crates/lemonfiber-core/src/app/targets/",
];

/// The readers of a service's credential that seeding hands on to another service, each
/// called as a function of the service it reads.
///
/// The media server's administrator's password is read as a method of the server the
/// lookup resolved, which names the request service it is handed to only where the gate
/// lets it cross; [`the_media_server_names_its_asker_through_the_gate`] holds that.
const READERS: [&str; 3] = ["servarr_key(", "usenet_key(", "recorded_password("];

/// Whether `line` calls one of [`READERS`] as a function rather than as a method.
fn reads(line: &str) -> bool {
    READERS.iter().any(|reader| {
        line.match_indices(reader)
            .any(|(at, _)| !line.get(..at).is_some_and(|before| before.ends_with('.')))
    })
}

/// Where seeding takes its pairs from the connecting table, which asks the gate.
const FROM_THE_GATE: [&str; 2] = ["pairings(", "fulfilling("];

/// The seed passes that read another service's credential without taking pairs from the
/// table themselves, each with why that is still the gate's answer.
const BESIDE_THE_GATE: [(&str, &str); 2] = [
    (
        "crates/lemonfiber-core/src/seed/run/clients.rs",
        "reads every download client's credential into the map the stack's own published \
         keys and the refused report are made from; it is handed to an asker only by \
         arrs.rs, through the table's pairings",
    ),
    (
        "crates/lemonfiber-core/src/seed/run/published.rs",
        "publishes the stack's own services' keys into the stack's own settings file, \
         which only the stack's own services read; no plugin's service is among them",
    ),
];

/// Whether `path` is a shipped file under one of `under`.
fn governed(path: &Path, under: &[&str]) -> bool {
    let path = path.to_string_lossy().replace('\\', "/");
    under.iter().any(|root| path.starts_with(root)) && !test_only(Path::new(&path))
}

/// The file's shipped text with every run of whitespace taken out, so a judgement
/// broken across lines reads as one.
fn joined(text: &str) -> String {
    production(text)
        .chars()
        .filter(|one| !one.is_whitespace())
        .collect()
}

/// Every place in `text` an origin is compared or matched as a judgement.
fn judgements(text: &str) -> Vec<String> {
    let joined = joined(text);
    let mut found = Vec::new();
    for compared in [".origin==", ".origin!=", "==Origin::", "!=Origin::"] {
        if joined.contains(compared) {
            found.push(compared.to_owned());
        }
    }
    for (at, _) in joined.match_indices("matches!(") {
        let argument = joined
            .get(at..)
            .and_then(|rest| rest.split(',').next())
            .unwrap_or_default();
        if argument.ends_with(".origin") {
            found.push(argument.to_owned());
        }
    }
    found
}

/// **Nothing in seeding judges whom a credential may reach.** An origin compared or
/// matched in a seed pass or a target reader is a decision the gate should have made:
/// naming whose plugin a service is reads its origin, and is not refused here.
#[test]
fn nothing_in_seeding_judges_an_origin_for_itself() {
    let mut judged = Vec::new();
    let mut seen = 0_usize;
    for (path, text) in sources() {
        if !governed(&path, &GOVERNED) {
            continue;
        }
        seen += 1;
        for judgement in judgements(&text) {
            judged.push(format!("{}: {judgement}", path.display()));
        }
    }

    assert!(
        seen > 20,
        "the scan read {seen} files, which means it is looking in the wrong place"
    );
    assert!(
        judged.is_empty(),
        "these judge an origin for themselves, where whom a credential may reach is \
         wiring::crosses's to decide: {judged:?}"
    );
}

/// **Every seed pass that hands on a credential takes its pairs from the gate.** A pass
/// reading another service's credential takes the services it hands it between from the
/// connecting table, which made each pair only where the gate lets the credential cross.
#[test]
fn every_seed_pass_handing_a_credential_takes_its_pairs_from_the_gate() {
    let mut ungated = Vec::new();
    let mut reading = 0_usize;
    for (path, text) in sources() {
        if !governed(&path, &["crates/lemonfiber-core/src/seed/run/"]) {
            continue;
        }
        let shipped = production(&text);
        let reads = shipped.lines().any(|line| {
            let line = line.trim_start();
            !line.starts_with("pub")
                && !line.starts_with("fn ")
                && !line.starts_with("async fn ")
                && !line.starts_with("//")
                && reads(line)
        });
        if !reads {
            continue;
        }
        reading += 1;
        let at = path.to_string_lossy().replace('\\', "/");
        let beside = BESIDE_THE_GATE.iter().any(|(file, _)| *file == at);
        if !beside && !FROM_THE_GATE.iter().any(|gate| shipped.contains(gate)) {
            ungated.push(at);
        }
    }

    assert!(
        reading > 3,
        "the scan found {reading} passes reading a credential, which means it is looking \
         in the wrong place"
    );
    assert!(
        ungated.is_empty(),
        "these read another service's credential without taking their pairs from the \
         connecting table, so nothing asked the gate whether it may cross: {ungated:?}"
    );
}

/// Every pass let read beside the gate still exists and still reads a credential, so
/// the list is never a hole left open by a pass that moved.
#[test]
fn every_pass_beside_the_gate_still_reads_a_credential() {
    let files = sources();
    for (file, why) in BESIDE_THE_GATE {
        let text = files
            .iter()
            .find(|(path, _)| path.to_string_lossy().replace('\\', "/") == file)
            .map(|(_, text)| production(text).to_owned())
            .unwrap_or_default();
        assert!(
            READERS.iter().any(|reader| text.contains(reader)),
            "{file} is let read beside the gate because it {why}, and reads no credential"
        );
    }
}

/// **The media server's administrator's password crosses the same gate.** It is handed
/// once to the request service that asks for the server, and the lookup names that
/// service only where `wiring::crosses` lets the password reach it.
#[test]
fn the_media_server_names_its_asker_through_the_gate() {
    let media = sources()
        .into_iter()
        .find(|(path, _)| {
            path.to_string_lossy().replace('\\', "/")
                == "crates/lemonfiber-core/src/app/targets/media.rs"
        })
        .map(|(_, text)| production(&text).to_owned())
        .unwrap_or_default();

    assert!(
        media.contains("asked_by") && media.contains("crate::wiring::crosses("),
        "the media server names the service it hands its administrator's password to \
         without asking wiring::crosses"
    );
}
