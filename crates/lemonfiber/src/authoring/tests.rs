use lemonfiber_core::plugin::Ungenerated;

use super::{document_of, published, vocabulary, Answered, Document};
use crate::exit::FAILURE;

/// What was shown, or nothing where the read had no lines to show.
fn said(answered: &Answered) -> String {
    answered
        .lines
        .as_ref()
        .map(super::render::Lines::text)
        .unwrap_or_default()
}

/// Each published document arrives in both forms, and neither is a fault.
#[test]
fn every_document_this_build_publishes_has_a_form_for_each_reader() {
    let asked = [
        ("schema", Document::Schema),
        ("extension-points", Document::ExtensionPoints),
        ("capabilities", Document::Capabilities),
    ];
    let mut seen = 0_usize;
    for (named, one) in asked {
        for json in [true, false] {
            let answered = published(one, json);
            assert_eq!(answered.code, 0, "{named} at json={json}");
            assert!(answered.fault.is_none(), "{named} at json={json}");
            assert!(!said(&answered).is_empty(), "{named} at json={json}");
            seen += 1;
        }
    }
    assert_eq!(seen, 6, "the documents were not put in front of this");
}

/// The schema is the document whichever form was asked for.
///
/// It is a thing an editor reads rather than a listing a person does, so unlike
/// the other two it has no second form to fall back to.
#[test]
fn the_schema_is_the_same_document_either_way() {
    let plain = said(&published(Document::Schema, false));
    let machine = said(&published(Document::Schema, true));
    assert_eq!(plain, machine);
    assert!(plain.contains("schema_version"), "{plain}");
}

/// The two listings are listings for a person and documents for a parser.
#[test]
fn a_listing_reads_as_prose_and_a_document_reads_as_json() {
    let listed = said(&published(Document::ExtensionPoints, false));
    let document = said(&published(Document::ExtensionPoints, true));
    assert_ne!(listed, document);
    assert!(document.starts_with('{'), "{document}");
}

/// A vocabulary this build could not generate is reported as this build's fault.
///
/// Put in front of the read rather than provoked, because a stack that disagreed
/// with the vocabulary fails generation long before an operator asks — and the
/// sentence they get if it ever does is the whole of what this is for.
#[test]
fn a_vocabulary_that_could_not_be_generated_is_this_builds_fault() {
    let answered = vocabulary(&Err(Ungenerated::Unrenderable), false);
    assert_eq!(answered.code, FAILURE);
    assert!(answered.lines.is_none());
    assert!(answered
        .fault
        .is_some_and(|why| why.contains("could not be written as JSON")));
}

/// A document this build could not write, said as this build's own fault.
///
/// Reached by calling them rather than by driving a document that fails to
/// render, because neither can be made to happen from outside: a generated
/// document that would not serialise fails long before an operator asks for it.
/// What is worth holding is the sentence each gives, since that sentence is the
/// whole of what reaches somebody when it does.
#[test]
fn a_document_this_build_could_not_write_is_reported_as_this_builds_fault() {
    let unwritten = document_of(None);
    assert_eq!(unwritten.code, FAILURE);
    assert!(unwritten.lines.is_none());
    assert!(unwritten
        .fault
        .is_some_and(|why| why.contains("could not be written")));
}

/// A document that did render is shown and the run ends well.
#[test]
fn a_document_that_rendered_is_shown_without_a_fault() {
    let written = document_of(Some("{}"));
    assert_eq!(written.code, 0);
    assert!(written.fault.is_none());
    assert_eq!(
        written.lines.as_ref().map(super::render::Lines::text),
        Some("{}".to_owned())
    );
}
