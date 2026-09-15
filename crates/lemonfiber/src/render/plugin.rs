//! The published artefacts, on a terminal.
//!
//! The reader here is a plugin author rather than an operator, and what they are after
//! is different: not *is my stack well* but *what may I write down*. So a capability
//! leads with the prose a claimant is held to and the probes a claim has to bind, and a
//! point leads with what a row carries — the parts somebody is about to copy into a
//! manifest.
//!
//! Every one of them opens by saying which generation it is reporting. An author
//! comparing what they were told with what their manifest was refused for needs to know
//! whether the difference is their build or their file, and the generation is the only
//! thing that answers that.

use lemonfiber_core::plugin::{Capabilities, Credential, Declared, Points, Probe};

use super::Lines;

/// A document going out to whatever asked for it, exactly as it is committed.
pub(crate) fn document(text: &str) -> Lines {
    let mut lines = Lines::for_a_parser();
    lines.block(text);
    lines
}

/// What a service can be asked for, and what claiming one undertakes.
pub(crate) fn capabilities(published: &Capabilities) -> Lines {
    let mut lines = Lines::default();
    lines.put(format!(
        "The capabilities a service can claim — generation {}, {} of them.",
        published.vocabulary_version,
        published.capabilities.len()
    ));
    for capability in &published.capabilities {
        lines.spaced(capability.name.to_owned());
        lines.put(format!("  {}", capability.summary));
        lines.put(format!("  {}", capability.contract));
        lines.put(format!("  declared by  {}", published_by(capability)));
        for probe in capability.probes {
            lines.put(format!("  probe {} — {}", probe.id, probe.title));
            lines.put(format!("    asks     {}", probe.asks));
            lines.put(format!("    asked as {}", asked_as(probe.credential)));
            lines.put(format!("    answers  {}", answers(probe)));
        }
    }
    lines.spaced(
        "A name here is the only kind you may claim unnamespaced. Your own capability is \
         written <plugin-id>:<name>, and nothing asks for one yet.",
    );
    lines
}

/// Which bundled services declare a capability, or that none does.
///
/// Never an empty list. Generation refuses a capability nothing declares, so an empty
/// one here would be a state that cannot arise — but saying so costs a word and is the
/// difference between a reader believing the line and wondering about it.
fn published_by(capability: &Declared) -> String {
    if capability.declared_by.is_empty() {
        return "nothing bundled".to_owned();
    }
    capability.declared_by.join(", ")
}

/// Who a probe is asked as, in the words an author would use.
fn asked_as(credential: Credential) -> &'static str {
    match credential {
        Credential::None => "anybody, presenting nothing",
        Credential::Operator => "the operator, with the credential they hold",
    }
}

/// What a probe accepts as an answer.
fn answers(probe: &Probe) -> String {
    let statuses: Vec<String> = probe
        .requires
        .status
        .iter()
        .map(ToString::to_string)
        .collect();
    if probe.requires.body.is_empty() {
        // A refusal is the one answer no port proxy can produce, which is why this
        // probe is allowed to constrain nothing but the status. Said rather than left
        // blank, so it does not read as a gap in the document.
        return format!("{} — the status is the whole of it", statuses.join(" or "));
    }
    let kinds: Vec<String> = probe
        .requires
        .body
        .iter()
        .map(|constraint| constraint.as_str().to_owned())
        .collect();
    format!("{}, and one of {}", statuses.join(" or "), kinds.join(", "))
}

/// Where a plugin may put a row, and what a row there carries.
pub(crate) fn points(published: &Points) -> Lines {
    let mut lines = Lines::default();
    lines.put(format!(
        "The places a plugin may extend lemonfiber — generation {}, {} of them.",
        published.extension_points_version,
        published.points.len()
    ));
    for point in &published.points {
        lines.spaced(point.name.to_owned());
        lines.put(format!("  {}", point.summary));
        lines.put(format!("  joins     {}", point.register));
        lines.put(format!("  read by   {}", point.engine));
        lines.put(format!("  asks for  {}", point.requires));
        lines.put(format!("  required  {}", point.row.required.join(", ")));
        lines.put(format!("  optional  {}", point.row.optional.join(", ")));
        for bound in point.row.bounds {
            lines.put(format!(
                "  {} is {} to {}, and {} where a row does not say",
                bound.field, bound.limits.min, bound.limits.max, bound.limits.default
            ));
        }
        for closed in point.row.enums {
            lines.put(format!(
                "  {} is one of  {}",
                closed.field,
                closed.values.join(", ")
            ));
        }
        lines.put(format!("  taken     {}", taken(&point.occupied)));
    }
    lines.spaced(
        "Every identity you contribute is namespaced with your plugin's id, so it cannot \
         collide with one of those.",
    );
    lines
}

/// What is already standing in a register, or that nothing is.
fn taken(occupied: &[String]) -> String {
    if occupied.is_empty() {
        return "nothing yet".to_owned();
    }
    format!("{} — {}", occupied.len(), occupied.join(", "))
}

#[cfg(test)]
mod tests {
    use super::{capabilities, document, points};

    /// The vocabulary this build publishes, as the binary would read it.
    fn vocabulary() -> lemonfiber_core::plugin::Capabilities {
        match lemonfiber_core::plugin::capabilities() {
            Ok(published) => published,
            Err(problem) => unreachable!("this build's own vocabulary must publish: {problem}"),
        }
    }

    #[test]
    fn a_capability_carries_its_contract_its_claimants_and_its_probes() {
        let text = capabilities(&vocabulary()).text();
        assert!(text.contains("generation 1"), "{text}");
        assert!(text.contains("media.serve"), "{text}");
        assert!(text.contains("declared by  jellyfin"), "{text}");
        assert!(text.contains("probe guarded"), "{text}");
        assert!(
            text.contains("the operator, with the credential they hold"),
            "{text}"
        );
    }

    /// A refusal constrains nothing but the status, and the listing says why rather
    /// than leaving the line blank.
    #[test]
    fn a_probe_that_constrains_only_a_status_says_that_is_the_whole_of_it() {
        let text = capabilities(&vocabulary()).text();
        assert!(
            text.contains("401 or 403 — the status is the whole of it"),
            "{text}"
        );
        assert!(text.contains("and one of json, json_has_keys"), "{text}");
    }

    /// Every capability the vocabulary carries reaches the listing.
    #[test]
    fn nothing_the_vocabulary_carries_is_left_out() {
        let published = vocabulary();
        let text = capabilities(&published).text();
        let missing: Vec<&str> = published
            .capabilities
            .iter()
            .map(|capability| capability.name)
            .filter(|name| !text.contains(name))
            .collect();
        assert!(missing.is_empty(), "{missing:?}");
    }

    #[test]
    fn a_point_carries_its_row_its_bounds_and_what_is_already_in_it() {
        let text = points(&lemonfiber_core::plugin::extension_points()).text();
        assert!(text.contains("generation 1"), "{text}");
        assert!(text.contains("doctor.check"), "{text}");
        assert!(text.contains("asks for  doctor.contribute"), "{text}");
        assert!(
            text.contains("timeout_s is 1 to 30, and 10 where a row does not say"),
            "{text}"
        );
        assert!(text.contains("category is one of  environment"), "{text}");
        assert!(text.contains("storage.space"), "{text}");
    }

    /// A register nothing is standing in says so rather than showing an empty list.
    #[test]
    fn a_register_with_nothing_in_it_says_nothing_yet() {
        let text = points(&lemonfiber_core::plugin::extension_points()).text();
        assert!(text.contains("taken     nothing yet"), "{text}");
    }

    /// A document goes out exactly as it is committed, unfolded and unplained.
    #[test]
    fn a_document_is_carried_through_as_it_was_written() {
        let lines = document("{\n  \"a\": 1\n}\n");
        assert_eq!(lines.text(), "{\n  \"a\": 1\n}");
    }
}
