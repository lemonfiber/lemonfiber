//! Whether everything a manifest declares can be read by whoever reviews its diff.

use crate::schema::{Contribution, Manifest, Proof};
use crate::Violation;

/// Whether everything declared can be read by the person reviewing the diff.
///
/// A manifest is judged before it is trusted, and judging it means reading it. A value
/// carrying something a diff cannot show is content that has to be executed or decoded
/// to be understood, which is the opposite of what makes a stranger's contribution
/// reviewable at all.
pub(super) fn readable(manifest: &Manifest, found: &mut Vec<Violation>) {
    for (at, text) in declared(manifest) {
        if let Some(hidden) = text.chars().find(|letter| unshown(*letter)) {
            found.push(Violation {
                location: at,
                message: format!(
                    "carries U+{:04X}, which a diff cannot show; everything a manifest declares \
                     has to be readable by whoever is deciding whether to trust it",
                    u32::from(hidden)
                ),
            });
        }
    }
}

/// Whether a diff shows nothing where this character is.
///
/// The control characters other than a line break and a tab, and then everything that
/// draws nothing: Unicode's format characters (`Cf`) and its default-ignorable code
/// points. Those are the bidirectional overrides that draw a word backwards, the
/// zero-widths and joiners, the soft hyphen, the invisible operators, the fillers and
/// the tag characters — each of them lets one name read as another, or carries text
/// nobody reviewing the diff can see. A manifest is read rather than typeset, so none
/// of them is needed to write one.
fn unshown(letter: char) -> bool {
    (letter.is_control() && letter != '\n' && letter != '\t') || draws_nothing(letter)
}

/// Whether a character is a format character or a default-ignorable one.
///
/// The union of `Cf` and `Default_Ignorable_Code_Point` as Unicode 16 assigns them,
/// written out because the standard library answers neither.
const fn draws_nothing(letter: char) -> bool {
    matches!(
        letter,
        '\u{ad}'
            | '\u{34f}'
            | '\u{600}'..='\u{605}'
            | '\u{61c}'
            | '\u{6dd}'
            | '\u{70f}'
            | '\u{890}'..='\u{891}'
            | '\u{8e2}'
            | '\u{115f}'..='\u{1160}'
            | '\u{17b4}'..='\u{17b5}'
            | '\u{180b}'..='\u{180f}'
            | '\u{200b}'..='\u{200f}'
            | '\u{202a}'..='\u{202e}'
            | '\u{2060}'..='\u{206f}'
            | '\u{3164}'
            | '\u{fe00}'..='\u{fe0f}'
            | '\u{feff}'
            | '\u{ffa0}'
            | '\u{fff0}'..='\u{fffb}'
            | '\u{110bd}'
            | '\u{110cd}'
            | '\u{13430}'..='\u{1343f}'
            | '\u{1bca0}'..='\u{1bca3}'
            | '\u{1d173}'..='\u{1d17a}'
            | '\u{e0000}'..='\u{e0fff}'
    )
}

/// Every string a manifest declares, with where it was declared.
///
/// Gathered rather than each rule reaching for the ones it cares about, because the
/// question here is about all of them: one field left out is one place something
/// unreadable can be written.
pub(super) fn declared(manifest: &Manifest) -> Vec<(String, &str)> {
    let plugin = &manifest.plugin;
    let mut every: Vec<(String, &str)> = vec![
        ("plugin.name".to_owned(), plugin.name.as_str()),
        ("plugin.version".to_owned(), plugin.version.as_str()),
        ("plugin.description".to_owned(), plugin.description.as_str()),
        ("plugin.without_it".to_owned(), plugin.without_it.as_str()),
        ("plugin.license".to_owned(), plugin.license.as_str()),
        ("plugin.upstream".to_owned(), plugin.upstream.as_str()),
    ];
    for service in &manifest.services {
        let at = format!("service {}", service.id);
        every.push((format!("{at}.id"), service.id.as_str()));
        every.push((format!("{at}.name"), service.name.as_str()));
        every.push((format!("{at}.image"), service.image.as_str()));
        every.push((format!("{at}.tag"), service.tag.as_str()));
        if let Some(path) = &service.config_path {
            every.push((format!("{at}.config_path"), path.as_str()));
        }
        for (field, text) in [
            ("reaches", service.reaches.as_deref()),
            ("asks_for", service.asks_for.as_deref()),
        ] {
            if let Some(text) = text {
                every.push((format!("{at}.{field}"), text));
            }
        }
    }
    for (at, wiring) in manifest.wirings.iter().enumerate() {
        let at = wiring.service.as_deref().map_or_else(
            || format!("wiring #{}", at + 1),
            |id| format!("wiring {id}"),
        );
        for (field, text) in [
            ("hostname", wiring.hostname.as_deref()),
            ("dashboard_group", wiring.dashboard_group.as_deref()),
            (
                "dashboard_description",
                wiring.dashboard_description.as_deref(),
            ),
            ("dashboard_icon", wiring.dashboard_icon.as_deref()),
        ] {
            if let Some(text) = text {
                every.push((format!("{at}.{field}"), text));
            }
        }
    }
    for claim in &manifest.claims {
        for probe in &claim.probes {
            let at = format!("claim {} probe {}", claim.capability, probe.id);
            if let Some(request) = &probe.request {
                every.push((format!("{at}.request.path"), request.path.as_str()));
            }
            for arg in probe.run.iter().flat_map(|run| &run.args) {
                every.push((format!("{at}.run.args"), arg.as_str()));
            }
        }
    }
    proofs(&manifest.proofs, &mut every);
    contributions(&manifest.contributions, &mut every);
    for recipe in &manifest.recipes {
        let at = format!("recipe {}", recipe.id);
        every.push((format!("{at}.title"), recipe.title.as_str()));
        every.push((format!("{at}.why"), recipe.why.as_str()));
        inputs(&at, &recipe.inputs, &mut every);
        steps(&at, &recipe.steps, &mut every);
    }
    for secret in &manifest.secrets {
        every.push((format!("secret {}.why", secret.id), secret.why.as_str()));
    }
    for over in &manifest.overrides {
        every.push((format!("override {}.why", over.id), over.why.as_str()));
    }
    every
}

/// What each proof says: its title, what it asks or runs, why, and what its failing
/// declarations say.
fn proofs<'a>(proofs: &'a [Proof], every: &mut Vec<(String, &'a str)>) {
    for proof in proofs {
        let at = format!("proof {}", proof.id);
        every.push((format!("{at}.title"), proof.title.as_str()));
        if let Some(request) = &proof.request {
            every.push((format!("{at}.request.path"), request.path.as_str()));
        }
        for arg in proof.run.iter().flat_map(|run| &run.args) {
            every.push((format!("{at}.run.args"), arg.as_str()));
        }
        every.push((format!("{at}.why"), proof.why.as_str()));
        reasons(&at, &proof.expected, every);
    }
}

/// What each contributed check says: what its failing declarations say, its title,
/// why, the action and detail it offers, and the path it asks.
fn contributions<'a>(contributions: &'a [Contribution], every: &mut Vec<(String, &'a str)>) {
    for entry in contributions {
        let at = format!("contribution {}", entry.id);
        reasons(&at, &entry.expected, every);
        for (field, text) in [
            ("title", entry.title.as_deref()),
            ("why", entry.why.as_deref()),
            ("action", entry.action.as_deref()),
            ("detail", entry.detail.as_deref()),
            (
                "request.path",
                entry.request.as_ref().map(|request| request.path.as_str()),
            ),
        ] {
            if let Some(text) = text {
                every.push((format!("{at}.{field}"), text));
            }
        }
    }
}

/// What each step of a recipe sends and captures: its path, its body, each header's
/// name and value, and where each capture is read from and said to come from.
fn steps<'a>(at: &str, steps: &'a [crate::schema::Step], every: &mut Vec<(String, &'a str)>) {
    for step in steps {
        let here = format!("{at}.step {}", step.id);
        every.push((format!("{here}.call.path"), step.call.path.as_str()));
        if let Some(body) = &step.call.body {
            every.push((format!("{here}.call.body"), body.as_str()));
        }
        for (header, value) in step.call.headers.iter().flatten() {
            every.push((format!("{here}.call.headers"), header.as_str()));
            every.push((format!("{here}.call.headers.{header}"), value.as_str()));
        }
        for capture in &step.capture {
            every.push((format!("{here}.capture.from"), capture.from.as_str()));
        }
        let conditions = [("when", step.when.as_ref())]
            .into_iter()
            .chain(std::iter::once((
                "retry.until",
                step.retry.as_ref().map(|retry| &retry.until),
            )));
        for (field, condition) in conditions {
            for text in condition
                .into_iter()
                .flat_map(|one| [&one.value, &one.equals])
                .flatten()
            {
                every.push((format!("{here}.{field}"), text.as_str()));
            }
        }
        if let Some(retry) = &step.retry {
            every.push((format!("{here}.retry.every"), retry.every.as_str()));
        }
    }
}

/// What each input of a recipe is called, whose it is, and what the operator is asked.
fn inputs<'a>(at: &str, inputs: &'a [crate::schema::Input], every: &mut Vec<(String, &'a str)>) {
    for input in inputs {
        let here = format!("{at}.input {}", input.name);
        every.push((format!("{here}.name"), input.name.as_str()));
        for (field, text) in [("of", &input.of), ("ask", &input.ask)] {
            if let Some(text) = text {
                every.push((format!("{here}.{field}"), text.as_str()));
            }
        }
    }
}

/// What each declaration that an assertion fails says: the recording and the reason.
fn reasons<'a>(
    at: &str,
    expected: &'a [crate::schema::Declaration],
    every: &mut Vec<(String, &'a str)>,
) {
    for declaration in expected {
        let at = format!("{at}.expected {}", declaration.fixture);
        every.push((format!("{at}.fixture"), declaration.fixture.as_str()));
        every.push((format!("{at}.reason"), declaration.reason.as_str()));
    }
}
