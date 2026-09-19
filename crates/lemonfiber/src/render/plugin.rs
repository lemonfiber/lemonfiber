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

use lemonfiber_core::filling::Filling;
use lemonfiber_core::plugin::{
    Capabilities, Claimed, Claiming, Credential, Evidence, Points, Probe, Provenance, Ran, Verdict,
    Vouched,
};

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
        lines.put(format!(
            "  declared by  {}",
            capability.declared_by.join(", ")
        ));
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

/// What one plugin's source claims, in whichever form was asked for.
///
/// The machine-readable form is the report as it stands rather than a second shape
/// written beside it: what an author's CI branches on and what a person reads are the
/// same answer, and two renderings of one answer is the disagreement this avoids.
pub(crate) fn claimed(read: &Claimed, json: bool) -> Option<Lines> {
    if json {
        return serde_json::to_string_pretty(read)
            .ok()
            .as_deref()
            .map(document);
    }
    Some(claims(read))
}

/// What anybody has said about the images a plugin pins, in whichever form was asked.
pub(crate) fn vouched(read: &Vouched, json: bool) -> Option<Lines> {
    if json {
        return serde_json::to_string_pretty(read)
            .ok()
            .as_deref()
            .map(document);
    }
    Some(provenance(read))
}

/// Three answers, each said as itself.
///
/// The words are the whole point. *Unproven* is not a softer *refused* and not a
/// quieter *signed*: it is the answer that says nobody has claimed this image, and an
/// operator deciding whether to go on needs it kept apart from a claim that did not
/// hold. So each line says which of the three it is before it says anything else.
fn provenance(read: &Vouched) -> Lines {
    let mut lines = Lines::default();
    lines.put(format!("{} — what its images are vouched for", read.id));
    lines.put(String::new());
    for one in &read.images {
        lines.put(format!("  {}  {}", one.held.as_str(), one.service));
        lines.put(format!("    {}@{}", one.image, one.digest));
        let why = match &one.held {
            Provenance::Signed { by } => format!("signed, and the key that made it is {by}"),
            Provenance::Unproven { why } | Provenance::Refused { why } => why.clone(),
        };
        lines.put(format!("    {why}"));
        lines.put(String::new());
    }
    if read.images.is_empty() {
        lines.put("  it pins no image, so nothing was asked about anything.".to_owned());
        lines.put(String::new());
    }
    lines.put(if read.installable {
        "nothing said about these images stops an install.".to_owned()
    } else {
        "this would not be installed as it stands.".to_owned()
    });
    lines
}

/// What one plugin's source claims, and what this build makes of it.
///
/// The refusals come first and are the whole answer where there are any: a manifest
/// that contradicts what this build publishes is refused outright rather than partly
/// applied, so reporting what its claims would have come to would be describing an
/// install that is not going to happen.
fn claims(read: &Claimed) -> Lines {
    let mut lines = Lines::default();
    lines.put(format!("{} {} — {}", read.id, read.version, read.name));
    lines.put(format!(
        "held to capability vocabulary generation {} and extension points generation {}.",
        read.vocabulary_version, read.extension_points_version
    ));

    if !read.refusals.is_empty() {
        lines.spaced(format!(
            "Refused, {}:",
            counted(read.refusals.len(), "violation")
        ));
        for refusal in &read.refusals {
            lines.put(format!("  {} — {}", refusal.location, refusal.message));
        }
    }

    if read.capabilities.is_empty() {
        lines.spaced("It claims no capabilities, so nothing can ask for what it installs.");
    } else {
        lines.spaced("What it can do");
        for claiming in &read.capabilities {
            capability(&mut lines, claiming);
        }
    }

    if !read.contributions.is_empty() {
        lines.spaced("What it adds to lemonfiber's own registers");
        for row in &read.contributions {
            let about = row
                .about
                .as_ref()
                .map_or_else(String::new, |check| format!("  (for {check})"));
            lines.put(format!("  {} {}{about}", row.at, row.id));
            if !row.says.is_empty() {
                lines.put(format!("    {}", row.says));
            }
        }
    }

    // A match rather than a sentence written here, so that the day a verdict can come
    // from a service this stops compiling instead of going on saying the wrong thing.
    // And said on every run rather than only on the one that passed: the refused
    // report is the one whose reader is about to go and change their service, and it
    // was the report that never told them nothing had been asked of it.
    lines.spaced(match read.against {
        Evidence::Recordings => {
            "No service was asked anything: every verdict above is against the \
             recordings this plugin ships."
        }
    });
    lines.put(if read.installable {
        "Nothing here stops it being installed."
    } else {
        "As it stands this would not be installed."
    });
    lines
}

/// One capability, its probes, and what asking for it would come to.
fn capability(lines: &mut Lines, claiming: &Claiming) {
    lines.put(format!(
        "  {}  [{}]",
        claiming.name,
        claiming.shown.as_str()
    ));
    for ran in &claiming.probes {
        lines.put(format!("    probe {} — {}", ran.probe, verdict(ran)));
    }
    if !claiming.shown.can_fill() && claiming.filling.is_some() {
        // What follows is what asking for the capability comes to, and this claim is not
        // part of it. Said before the line rather than after, because a list of
        // claimants under a refuted claim otherwise reads as the claim standing.
        lines.put(format!(
            "    this claim is {} and does not fill it",
            claiming.shown.as_str()
        ));
    }
    // Which of the two kinds this is decides the line, rather than whether there is an
    // answer about what fills it. There is no answer for a core name the vocabulary
    // does not carry either, and calling that one *this plugin's own* would describe a
    // capability the refusal above has just said does not exist.
    if claiming.own {
        lines.put(
            "    this plugin's own, and inert: nothing asks for it, and no published \
             contract defines it"
                .to_owned(),
        );
        return;
    }
    match &claiming.filling {
        None => lines
            .put("    nothing published carries this name, so nothing can ask for it".to_owned()),
        Some(Filling::By { service }) => {
            lines.put(format!("    asking for it reaches {service}"));
        }
        Some(Filling::Contested { claimants }) => {
            lines.put(format!(
                "    contested between {} — nothing wires to it until the operator chooses, and \
                 lemonfiber does not choose by install order",
                claimants.join(", ")
            ));
        }
        Some(Filling::Unfilled) => lines.put(
            "    nothing fills it: this claim is not one that can, and nothing else claims it"
                .to_owned(),
        ),
    }
}

/// What one probe came to, in one line.
fn verdict(ran: &Ran) -> String {
    match &ran.verdict {
        Verdict::Passed => "the recording answers it".to_owned(),
        Verdict::Failed { faults } => format!("refuted: {}", faults.join("; ")),
        Verdict::Unproven { why } => format!("unproven: {why}"),
    }
}

/// A count and the thing counted, pluralised where it is not one.
fn counted(number: usize, thing: &str) -> String {
    if number == 1 {
        format!("{number} {thing}")
    } else {
        format!("{number} {thing}s")
    }
}

#[cfg(test)]
mod tests {
    use lemonfiber_core::filling::{Filling, Shown};
    use lemonfiber_core::plugin::{
        Claimed, Claiming, Contributed, Evidence, Ran, Verdict, Violation, Vouched,
    };

    use super::{capabilities, claimed, claims, document, points};

    /// One capability as the reader hands it over, with one probe that passed.
    fn claiming(name: &str, shown: Shown, filling: Option<Filling>) -> Claiming {
        with(name, shown, filling, Verdict::Passed)
    }

    /// The same, with whatever the probe came to.
    fn with(name: &str, shown: Shown, filling: Option<Filling>, verdict: Verdict) -> Claiming {
        Claiming {
            name: name.to_owned(),
            service: "kavita".to_owned(),
            own: !name.contains('.'),
            shown,
            probes: vec![Ran {
                probe: "guarded".to_owned(),
                verdict,
            }],
            filling,
        }
    }

    /// One plugin as the reader hands it over, carrying the capabilities given.
    fn read(capabilities: Vec<Claiming>, refusals: Vec<Violation>) -> Claimed {
        Claimed {
            id: "kavita".to_owned(),
            name: "Kavita".to_owned(),
            version: "1.0.0".to_owned(),
            vocabulary_version: 1,
            extension_points_version: 1,
            against: Evidence::Recordings,
            installable: refusals.is_empty(),
            refusals,
            capabilities,
            contributions: vec![Contributed {
                at: "doctor.check".to_owned(),
                id: "kavita:settings-guarded".to_owned(),
                says: "The settings are not readable by the household".to_owned(),
                about: None,
            }],
        }
    }

    /// Each of the three answers an ask can come to reaches the page in its own words.
    #[test]
    fn what_asking_for_a_capability_comes_to_is_said_three_ways() {
        let text = claims(&read(
            vec![
                claiming(
                    "media.serve",
                    Shown::Demonstrated,
                    Some(Filling::Contested {
                        claimants: vec!["jellyfin".to_owned(), "kavita (plugin kavita)".to_owned()],
                    }),
                ),
                claiming(
                    "identity.source",
                    Shown::Demonstrated,
                    Some(Filling::By {
                        service: "jellyfin".to_owned(),
                    }),
                ),
                claiming("request.intake", Shown::Refuted, Some(Filling::Unfilled)),
            ],
            Vec::new(),
        ))
        .text();
        assert!(
            text.contains("contested between jellyfin, kavita (plugin kavita)"),
            "{text}"
        );
        assert!(text.contains("does not choose by install order"), "{text}");
        assert!(text.contains("asking for it reaches jellyfin"), "{text}");
        assert!(text.contains("nothing fills it"), "{text}");
        assert!(
            text.contains("this claim is refuted and does not fill it"),
            "{text}"
        );
    }

    /// A plugin adding nothing to lemonfiber's own registers gets no heading for one.
    ///
    /// Read here rather than left to the test that drives the binary, because this file
    /// is compiled twice under coverage — once for these cases and once for the binary
    /// those drive — and a branch taken in only one of the two is a line the summary
    /// counts as missed and the line list cannot name.
    #[test]
    fn a_plugin_that_contributes_nothing_gets_no_heading_for_it() {
        let mut read = read(
            vec![claiming("media.serve", Shown::Demonstrated, None)],
            Vec::new(),
        );
        read.contributions = Vec::new();
        let text = claims(&read).text();
        assert!(
            !text.contains("What it adds to lemonfiber's own registers"),
            "{text}"
        );
        assert!(text.contains("media.serve"), "{text}");
    }

    /// What the verdicts were reached against is on the page either way.
    ///
    /// Read here as well as through the binary, because this file is compiled twice
    /// under coverage and a branch taken in only one of the two reads as missed. It
    /// used to be half of the sentence that said the plugin could be installed, which
    /// left the report most likely to send somebody off to look at their own service
    /// as the one that never told them nothing had been asked of it.
    #[test]
    fn the_page_says_what_it_was_against_whichever_answer_it_reaches() {
        let installable = claims(&read(
            vec![claiming("media.serve", Shown::Demonstrated, None)],
            Vec::new(),
        ))
        .text();
        let refused = claims(&read(
            vec![claiming("media.serve", Shown::Demonstrated, None)],
            vec![Violation {
                location: "service kavita.digest".to_owned(),
                message: "a digest is a sha256 content address of sixty-four characters".to_owned(),
            }],
        ))
        .text();
        for text in [&installable, &refused] {
            assert!(text.contains("No service was asked anything"), "{text}");
            assert!(text.contains("recordings this plugin ships"), "{text}");
        }
        assert!(
            installable.contains("Nothing here stops it being installed"),
            "{installable}"
        );
        assert!(refused.contains("would not be installed"), "{refused}");
    }

    /// A remedy names the check it is for, and a row saying nothing takes no line for it.
    ///
    /// Both halves of a contributed row's line. A row carrying neither a title nor an
    /// action is one the rules refuse — and the listing is rendered anyway, because an
    /// author needs to see the row that was refused rather than a plugin with nothing in
    /// it. So the empty line is reachable from a manifest rather than defensive.
    #[test]
    fn a_contributed_row_names_what_it_is_for_and_says_nothing_where_it_holds_nothing() {
        let mut read = read(Vec::new(), Vec::new());
        read.contributions = vec![
            Contributed {
                at: "doctor.remedy".to_owned(),
                id: "kavita:close-the-settings".to_owned(),
                says: "Stop Kavita and check what is in front of it".to_owned(),
                about: Some("kavita:settings-guarded".to_owned()),
            },
            Contributed {
                at: "doctor.check".to_owned(),
                id: "kavita:holds-nothing".to_owned(),
                says: String::new(),
                about: None,
            },
        ];
        let text = claims(&read).text();
        assert!(
            text.contains("doctor.remedy kavita:close-the-settings  (for kavita:settings-guarded)"),
            "{text}"
        );
        assert!(
            text.contains("doctor.check kavita:holds-nothing\n"),
            "{text}"
        );
        assert!(!text.contains("kavita:holds-nothing\n    \n"), "{text}");
    }

    /// A capability of the plugin's own is inert; a core name nothing publishes is not
    /// the same thing, and calling it one would describe a capability that does not
    /// exist.
    #[test]
    fn a_name_nothing_publishes_is_not_reported_as_the_plugins_own() {
        let text = claims(&read(
            vec![claiming("media.stream", Shown::Unproven, None)],
            vec![Violation {
                location: "service kavita.provides".to_owned(),
                message: "media.stream names no capability the published vocabulary carries"
                    .to_owned(),
            }],
        ))
        .text();
        assert!(
            text.contains("nothing published carries this name"),
            "{text}"
        );
        assert!(!text.contains("this plugin's own"), "{text}");
        assert!(text.contains("Refused, 1 violation"), "{text}");
        assert!(text.contains("would not be installed"), "{text}");
    }

    /// The two forms are two renderings of one answer, and the machine-readable one is
    /// the report itself rather than a shape written beside it.
    #[test]
    fn the_machine_readable_form_is_the_same_answer_as_the_page() {
        let read = read(
            vec![claiming("kavita:opds", Shown::Claimed, None)],
            Vec::new(),
        );
        let document = claimed(&read, true)
            .map(|lines| lines.text())
            .unwrap_or_default();
        assert!(document.contains(r#""installable": true"#), "{document}");
        assert!(document.contains(r#""name": "kavita:opds""#), "{document}");
        let page = claimed(&read, false)
            .map(|lines| lines.text())
            .unwrap_or_default();
        assert_eq!(page, claims(&read).text());
    }

    /// What a probe came to is said in its own words, whichever of the three it was —
    /// and a refutation carries what was wrong with the answer rather than the fact
    /// that something was.
    #[test]
    fn each_of_the_three_verdicts_says_itself() {
        let text = claims(&read(
            vec![
                with(
                    "media.serve",
                    Shown::Refuted,
                    Some(Filling::Unfilled),
                    Verdict::Failed {
                        faults: vec![
                            "answered 200 where it declares 401".to_owned(),
                            "the body carries no content".to_owned(),
                        ],
                    },
                ),
                with(
                    "identity.source",
                    Shown::Unproven,
                    Some(Filling::Unfilled),
                    Verdict::Unproven {
                        why: "fixtures/identity.json: could not be read".to_owned(),
                    },
                ),
            ],
            Vec::new(),
        ))
        .text();
        assert!(
            text.contains(
                "refuted: answered 200 where it declares 401; the body carries no content"
            ),
            "{text}"
        );
        assert!(
            text.contains("unproven: fixtures/identity.json: could not be read"),
            "{text}"
        );
    }

    /// One violation and several are counted as they are read.
    #[test]
    fn a_page_counts_what_it_refused() {
        let refusal = |what: &str| Violation {
            location: "service kavita.provides".to_owned(),
            message: what.to_owned(),
        };
        let one = claims(&read(Vec::new(), vec![refusal("the first")])).text();
        assert!(one.contains("Refused, 1 violation:"), "{one}");
        let several = claims(&read(
            Vec::new(),
            vec![refusal("the first"), refusal("the second")],
        ))
        .text();
        assert!(several.contains("Refused, 2 violations:"), "{several}");
        assert!(
            several.contains("It claims no capabilities"),
            "and a plugin that claims nothing says so: {several}"
        );
    }

    /// The plugin's own capability says what inert means rather than leaving a blank.
    #[test]
    fn a_capability_of_the_plugins_own_says_what_inert_means() {
        let text = claims(&read(
            vec![claiming("kavita:opds", Shown::Claimed, None)],
            Vec::new(),
        ))
        .text();
        assert!(text.contains("this plugin's own, and inert"), "{text}");
        assert!(
            text.contains("held to capability vocabulary generation 1"),
            "an author is told which generation refused them: {text}"
        );
        assert!(
            text.contains("doctor.check kavita:settings-guarded"),
            "{text}"
        );
        assert!(
            text.contains("Nothing here stops it being installed"),
            "{text}"
        );
    }

    /// The capability listing this build produces.
    ///
    /// Taken through the result rather than out of it, because an arm for a build whose
    /// own vocabulary does not publish is a line no run can enter. Such a build renders
    /// nothing at all here, which every assertion below notices rather than passes over.
    fn listed() -> String {
        lemonfiber_core::plugin::capabilities()
            .iter()
            .map(|published| capabilities(published).text())
            .collect()
    }

    #[test]
    fn a_capability_carries_its_contract_its_claimants_and_its_probes() {
        let text = listed();
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
        let text = listed();
        assert!(
            text.contains("401 or 403 — the status is the whole of it"),
            "{text}"
        );
        assert!(text.contains("and one of json, json_has_keys"), "{text}");
    }

    /// Every capability the vocabulary carries reaches the listing.
    #[test]
    fn nothing_the_vocabulary_carries_is_left_out() {
        let text = listed();
        let missing: Vec<&str> = lemonfiber_core::plugin::capabilities()
            .iter()
            .flat_map(|published| published.capabilities.iter())
            .map(|capability| capability.name)
            .filter(|name| !text.contains(name))
            .collect();
        assert!(
            !text.is_empty(),
            "this build published no vocabulary at all"
        );
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

    /// One image, in each of the three answers, rendered as itself.
    ///
    /// The words are what this is for. An operator reading the report has to be able
    /// to tell a publisher who signed nothing from a claim that did not hold, and the
    /// two would look alike the moment either stopped naming itself.
    #[test]
    fn each_of_the_three_answers_says_which_one_it_is() {
        use lemonfiber_core::plugin::{Provenance, Vouch};

        let one = |held: Provenance| Vouched {
            id: "komga".to_owned(),
            installable: !held.refuses(),
            images: vec![Vouch {
                service: "komga".to_owned(),
                image: "docker.io/gotson/komga".to_owned(),
                digest: "sha256:abc".to_owned(),
                held,
            }],
        };

        let signed = super::provenance(&one(Provenance::Signed {
            by: "the operator's own".to_owned(),
        }))
        .text();
        assert!(signed.contains("signed"), "{signed}");
        assert!(signed.contains("the operator's own"), "{signed}");
        assert!(
            signed.contains("docker.io/gotson/komga@sha256:abc"),
            "{signed}"
        );
        assert!(signed.contains("stops an install"), "{signed}");

        let unproven = super::provenance(&one(Provenance::Unproven {
            why: "its publisher has made no claim".to_owned(),
        }))
        .text();
        assert!(unproven.contains("unproven"), "{unproven}");
        assert!(unproven.contains("made no claim"), "{unproven}");

        let refused = super::provenance(&one(Provenance::Refused {
            why: "no key this build holds made it".to_owned(),
        }))
        .text();
        assert!(refused.contains("refused"), "{refused}");
        assert!(refused.contains("would not be installed"), "{refused}");
    }

    /// A plugin pinning nothing says so rather than printing an empty list.
    #[test]
    fn a_read_over_no_image_says_it_asked_about_nothing() {
        let said = super::provenance(&Vouched {
            id: "komga".to_owned(),
            images: Vec::new(),
            installable: false,
        })
        .text();
        assert!(said.contains("pins no image"), "{said}");
        assert!(said.contains("would not be installed"), "{said}");
    }

    #[test]
    fn what_was_vouched_for_is_carried_as_json_where_a_script_asked() {
        let read = Vouched {
            id: "komga".to_owned(),
            images: Vec::new(),
            installable: false,
        };
        let json = super::vouched(&read, true)
            .map(|lines| lines.text())
            .unwrap_or_default();
        assert!(json.contains(r#""id": "komga""#), "{json}");
        assert!(json.contains(r#""installable": false"#), "{json}");
    }
}
