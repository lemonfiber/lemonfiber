use super::{capabilities, claimed, claims, document, points};

use lemonfiber_core::filling::{Filling, Shown};
use lemonfiber_core::plugin::{
    Asserted, Assertion, Claimed, Claiming, Contributed, Evidence, Ran, Verdict, Violation, Vouched,
};

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
        proofs: Vec::new(),
        checks: Vec::new(),
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

/// The sentence this page cannot reach is written rather than wildcarded, and is
/// put to the renderer directly. A wildcard is what would let a verdict reached
/// against a recording be shown under the sentence saying the service answered,
/// and a case that never runs is a sentence nobody has read.
#[test]
fn each_kind_of_evidence_has_a_sentence_of_its_own_here_too() {
    let said = |against: Evidence| {
        claims(&Claimed {
            against,
            ..read(Vec::new(), Vec::new())
        })
        .text()
    };
    assert!(said(Evidence::Recordings).contains("No service was asked anything"));
    assert!(said(Evidence::Service).contains("against the service itself"));
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

/// One assertion that is not a probe, as the reader hands it over.
fn asserting(kind: Assertion, id: &str, says: &str, verdict: Verdict) -> Asserted {
    Asserted {
        kind,
        id: id.to_owned(),
        says: says.to_owned(),
        service: "kavita".to_owned(),
        verdict,
    }
}

/// The two assertions that are not about a capability each get a heading saying what
/// a verdict under it costs, because the two cost opposite things.
///
/// A refused proof is the plugin failing its own condition for being installed and a
/// refused check is a check finding the thing it exists to find, on a machine in the
/// state it was recorded in. They are one shape on the page, so the page has to say
/// which of the two a reader is looking at rather than leave it to be inferred from
/// a verdict that reads identically either way.
#[test]
fn a_proof_and_a_contributed_check_are_shown_under_headings_that_say_what_each_costs() {
    let mut read = read(Vec::new(), Vec::new());
    read.proofs = vec![asserting(
        Assertion::Proof,
        "guarded",
        "It refuses a read nobody signed in for",
        Verdict::Passed,
    )];
    read.checks = vec![asserting(
        Assertion::Check,
        "kavita:claimed",
        "Kavita has an administrator",
        Verdict::Failed {
            faults: vec!["answered 200 where it declares 401".to_owned()],
        },
    )];
    let text = claims(&read).text();
    assert!(
        text.contains("What must hold before it is installed"),
        "{text}"
    );
    assert!(
        text.contains("  guarded — It refuses a read nobody signed in for"),
        "{text}"
    );
    assert!(
        text.contains("    kavita — the recording answers it"),
        "{text}"
    );
    assert!(
        text.contains("What it would check, every day after"),
        "{text}"
    );
    assert!(
        text.contains("    kavita — refuted: answered 200 where it declares 401"),
        "{text}"
    );
    assert!(
        text.contains("none of these decides whether this is installed"),
        "a refuted check is the opposite news from a refuted claim: {text}"
    );
}

/// A plugin asserting nothing of its own gets neither heading, and never the
/// sentence about what a check does not decide.
///
/// The pair above is only worth a heading where there is something under it: a
/// report headed *what must hold before it is installed* with nothing beneath it
/// reads as a condition that was not reached rather than as one nobody wrote.
#[test]
fn a_plugin_that_asserts_nothing_of_its_own_gets_no_heading_for_either() {
    let text = claims(&read(
        vec![claiming("media.serve", Shown::Demonstrated, None)],
        Vec::new(),
    ))
    .text();
    assert!(
        !text.contains("What must hold before it is installed"),
        "{text}"
    );
    assert!(
        !text.contains("What it would check, every day after"),
        "{text}"
    );
    assert!(
        !text.contains("none of these decides whether this is installed"),
        "{text}"
    );
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
            message: "media.stream names no capability the published vocabulary carries".to_owned(),
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
        text.contains("refuted: answered 200 where it declares 401; the body carries no content"),
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
