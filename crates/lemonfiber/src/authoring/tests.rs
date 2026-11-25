use lemonfiber_core::app::authoring::Document;
use lemonfiber_core::app::Ctx;

use super::{published, shown};

fn ctx() -> Ctx {
    lemonfiber_testing::a_context().build()
}

async fn said(read: Document, json: bool) -> String {
    published(&ctx(), read, json)
        .await
        .map(|lines| lines.text())
        .unwrap_or_default()
}

/// Each published document arrives in both forms, and neither is a fault.
#[tokio::test]
async fn every_document_this_build_publishes_has_a_form_for_each_reader() {
    let mut seen = 0_usize;
    for read in [Document::Schema, Document::Points, Document::Vocabulary] {
        for json in [true, false] {
            let answered = published(&ctx(), read, json).await;
            assert!(answered.is_ok(), "{read:?} at json={json}");
            assert!(
                answered.is_ok_and(|lines| !lines.text().is_empty()),
                "{read:?} at json={json}"
            );
            seen += 1;
        }
    }
    assert_eq!(seen, 6, "the documents were not put in front of this");
}

/// A document is the file this build publishes, byte for byte.
#[tokio::test]
async fn a_document_is_the_published_file_byte_for_byte() {
    for (read, published) in [
        (Document::Schema, lemonfiber_core::plugin::schema()),
        (Document::Points, lemonfiber_core::plugin::points()),
        (
            Document::Vocabulary,
            lemonfiber_core::plugin::vocabulary().ok(),
        ),
    ] {
        let printed = format!("{}\n", said(read, true).await);
        assert_eq!(Some(printed), published, "{read:?}");
    }
}

/// The schema is the document whichever form was asked for.
///
/// It is a thing an editor reads rather than a listing a person does, so unlike
/// the other two it has no second form to fall back to.
#[tokio::test]
async fn the_schema_is_the_same_document_either_way() {
    let plain = said(Document::Schema, false).await;
    let machine = said(Document::Schema, true).await;
    assert_eq!(plain, machine);
    assert!(plain.contains("schema_version"), "{plain}");
}

/// The two listings are listings for a person and documents for a parser.
#[tokio::test]
async fn a_listing_reads_as_prose_and_a_document_reads_as_json() {
    for read in [Document::Points, Document::Vocabulary] {
        let listed = said(read, false).await;
        let document = said(read, true).await;
        assert_ne!(listed, document, "{read:?}");
        assert!(document.starts_with('{'), "{document}");
        assert!(listed.contains("generation 1"), "{listed}");
    }
}

/// A document that will not read as the listing it is asked as is shown as it was
/// published rather than not at all.
#[test]
fn a_document_that_reads_as_no_listing_is_shown_as_it_is() {
    for read in [Document::Points, Document::Vocabulary] {
        assert_eq!(shown(read, "[]", false).text(), "[]", "{read:?}");
    }
}
