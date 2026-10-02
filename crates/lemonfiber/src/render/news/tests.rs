use lemonfiber_core::news::{News, NewsKind, NewsProblem, NewsRequest, NewsUpdate};

use lemonfiber_core::app::Outcome;

use super::news;

fn report() -> News {
    News {
        updates: vec![NewsUpdate {
            version: "0.17.1".to_owned(),
            delivers: Some("Fixes".to_owned()),
        }],
        requests: vec![NewsRequest {
            number: 12,
            title: Some("Dune".to_owned()),
            by: "Anna".to_owned(),
        }],
        problems: vec![NewsProblem {
            check: "service.sonarr".to_owned(),
            onset: "1759400000".to_owned(),
            summary: "Sonarr is stopped".to_owned(),
        }],
        unread: Vec::new(),
    }
}

#[test]
fn each_item_leads_with_what_names_it() {
    let said = news(&report()).text();

    assert!(said.contains("  0.17.1   Fixes"), "{said}");
    assert!(said.contains("  #12   Dune, asked for by Anna"), "{said}");
    assert!(
        said.contains("  service.sonarr   since 2025-10-02T10:13:20 UTC   Sonarr is stopped"),
        "{said}"
    );
}

#[test]
fn a_kind_that_could_not_be_read_says_so_rather_than_none() {
    let said = news(&News {
        requests: Vec::new(),
        problems: Vec::new(),
        unread: vec![NewsKind::Requests],
        ..report()
    })
    .text();

    assert!(
        said.contains("Requests, newest first:\n  could not be read just now"),
        "{said}"
    );
    assert!(said.contains("Problems, newest first:\n  none"), "{said}");
}

#[test]
fn an_item_without_its_words_is_named_by_what_names_it_alone() {
    let said = news(&News {
        updates: vec![NewsUpdate {
            version: "0.16.0".to_owned(),
            delivers: None,
        }],
        requests: vec![NewsRequest {
            number: 3,
            title: None,
            by: "Bram".to_owned(),
        }],
        problems: vec![NewsProblem {
            check: "storage.space".to_owned(),
            onset: "written by hand".to_owned(),
            summary: "The volume is full".to_owned(),
        }],
        unread: Vec::new(),
    })
    .text();

    assert!(
        said.contains("Releases, newest first:\n  0.16.0\n"),
        "{said}"
    );
    assert!(said.contains("  #3   asked for by Bram"), "{said}");
    assert!(
        said.contains("  storage.space   since written by hand   The volume is full"),
        "{said}"
    );
}

#[test]
fn the_answer_is_drawn_by_this_renderer() {
    assert_eq!(
        super::super::shaped(&Outcome::News(report())).text(),
        news(&report()).text()
    );
}
