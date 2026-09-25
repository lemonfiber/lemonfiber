//! What reaches the terminal is made plain, and what a parser reads is left as built.

use super::*;

/// The case this exists for. The container writes the line, the terminal reads
/// the escape, and the screen stops saying what this product said.
/// An event set apart from the preset reads as the answer it was given, and a
/// rehearsal says it would save rather than that it did.
///
/// Both halves of both branches: a renderer that says "always told" for an event
/// switched off, or "saved" for a run that wrote nothing, is wrong in the direction
/// the operator has no way to check.
#[test]
fn an_event_set_apart_reads_as_the_answer_it_was_given() {
    let told = super::super::alerts(&AlertReport {
        preset: "problems-only".to_owned(),
        means: "Told when something is wrong.".to_owned(),
        exceptions: vec![
            ExceptionReport {
                kind: "storage.space".to_owned(),
                wanted: true,
            },
            ExceptionReport {
                kind: "queue.stalled".to_owned(),
                wanted: false,
            },
        ],
        changed: true,
        rehearsed: true,
    })
    .text();
    assert!(told.contains("storage.space — always told"), "{told}");
    assert!(told.contains("queue.stalled — never told"), "{told}");
    assert!(told.contains("would save"), "{told}");

    // A reading changes nothing, so it says nothing about saving — the branch the
    // three assertions above never enter.
    let read = super::super::alerts(&AlertReport {
        preset: "everything".to_owned(),
        means: "Told about everything.".to_owned(),
        exceptions: Vec::new(),
        changed: false,
        rehearsed: false,
    })
    .text();
    assert!(read.contains("telling you about: everything"), "{read}");
    assert!(!read.contains("save"), "{read}");
}

#[test]
fn a_container_cannot_clear_the_screen_through_its_own_log_line() {
    let said = logged("sonarr", "starting\u{1b}[2Jup");
    assert!(!said.contains('\u{1b}'), "{said:?}");
    assert!(
        said.ends_with("starting[2Jup"),
        "what a terminal would have obeyed is gone; the rest is left alone: {said:?}"
    );
}

/// A log line is one line. A container that puts a newline in the middle of one
/// is forging a second, and the column that names the service is what it would
/// forge its way out of.
#[test]
fn a_container_cannot_forge_a_second_log_line() {
    let said = logged("sonarr", "innocent\nqbittorrent   leaked the password");
    assert_eq!(said.lines().count(), 1, "{said:?}");
}

/// Padded on what will be drawn rather than on what arrived, or a name carrying
/// control characters pushes every line after it out of true.
#[test]
fn the_service_column_is_measured_after_the_name_is_made_plain() {
    let clean = logged("sonarr", "up");
    let sneaky = logged("son\u{7f}arr", "up");
    assert_eq!(clean, sneaky, "the delete never counted toward the width");
}

#[test]
fn lines_join_in_order_and_a_spaced_one_is_preceded_by_a_blank() {
    let mut lines = Lines::default();
    lines.put("first");
    lines.spaced("second");
    assert_eq!(lines.text(), "first\n\nsecond");
}

#[test]
fn a_block_is_split_into_the_lines_it_is_made_of() {
    // A diff arrives as one string carrying its own breaks; it has to become lines
    // like everything else, or the printer would put it out as a single blob.
    let mut lines = Lines::default();
    lines.block("-old\n+new\n");
    assert_eq!(lines.text(), "-old\n+new");
}

#[test]
fn what_a_parser_reads_goes_out_exactly_as_it_was_built() {
    // A curly quote is the one that matters: folded to `"` inside a JSON string
    // it is not a character but the end of the string.
    let document = "{\"name\":\"The “Burbs” 1989 — 1080p\"}";
    let mut lines = Lines::for_a_parser();
    lines.put(document);
    lines.print();

    assert_eq!(lines.text(), document, "unfolded and unaltered");
}

/// A refusal a script asked for leaves by the same door on the other stream.
#[test]
fn what_a_parser_reads_goes_out_unfolded_on_the_error_stream_too() {
    let document = "{\"kind\":\"error\",\"data\":{\"summary\":\"the “Burbs” — gone\"}}";
    let mut lines = Lines::for_a_parser();
    lines.put(document);
    lines.eprint();

    assert_eq!(lines.text(), document, "unfolded and unaltered");
}

/// The other half of the same rule: what a person reads is made safe, because a
/// terminal reads an escape in the middle of a name as an instruction.
#[test]
fn what_a_person_reads_is_still_made_plain() {
    let mut lines = Lines::default();
    lines.put(format!("a name{}with an instruction in it", char::from(27)));

    let said = lines.text();
    assert!(!said.contains(char::from(27)), "{said}");
}

/// Both halves of the wiring request reach a renderer of their own: the listing
/// is a report and the substitution is what one change came to, and a dispatcher
/// that sent either to the other would answer the wrong question in full.
#[test]
fn what_this_stack_wires_to_what_and_one_change_to_it_render_apart() {
    let listing = answer(
        &Outcome::Wiring(lemonfiber_core::model::WiringReport {
            wired: vec![lemonfiber_core::wiring::Wired {
                by: "seerr".to_owned(),
                reaches: lemonfiber_core::wiring::Reaches::Asked {
                    capability: "identity.source".to_owned(),
                    services: vec!["jellyfin".to_owned()],
                    settled: lemonfiber_core::wiring::Settled::Outright,
                    origins: std::collections::BTreeMap::new(),
                },
            }],
            unfilled: Vec::new(),
        }),
        false,
    )
    .text();
    assert!(
        listing.contains("seerr asks for identity.source"),
        "{listing}"
    );

    let changed = answer(
        &Outcome::Substitution(lemonfiber_core::model::SubstitutionReport {
            substitution: lemonfiber_core::wiring::Substitution {
                capability: "indexer.search".to_owned(),
                was: Some("prowlarr".to_owned()),
                now: "nzbhydra2".to_owned(),
                asked_by: vec!["bindery".to_owned()],
                leaves_unfilled: Vec::new(),
                setting: "indexer.search=nzbhydra2".to_owned(),
            },
            applied: true,
        }),
        false,
    )
    .text();
    assert!(
        changed.contains("nzbhydra2 now fills indexer.search."),
        "{changed}"
    );
}

#[test]
fn every_shape_this_module_prints_is_reachable() {
    // The one place this module reaches the terminal, exercised so it cannot rot.
    let mut lines = Lines::default();
    lines.put("printed");
    lines.print();
    render(&Outcome::Version(a_version()), false);
    render(&Outcome::Watch(a_watch()), false);
    render(
        &Outcome::Clients(lemonfiber_core::clients::guidance(None)),
        false,
    );
}
